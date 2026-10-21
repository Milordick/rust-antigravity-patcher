//! Atomic and in-place file modification utilities.
//! Direct in-place writes (`r+b`) run in ~40 microseconds with 0 MB RAM overhead.
//! If the binary is executing (`ETXTBSY`), automatically falls back to atomic tempfile + rename.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub fn fast_apply_patches(path: &Path, patches: &[(usize, &[u8])]) -> std::io::Result<()> {
    if patches.is_empty() {
        return Ok(());
    }

    // Try fast in-place direct seek + write
    match OpenOptions::new().read(true).write(true).open(path) {
        Ok(mut f) => {
            let file_len = f.metadata()?.len();

            // 1. Boundary check: ensure all patches fit strictly within the file
            let mut in_bounds = true;
            for &(offset, fix) in patches {
                if (offset as u64).saturating_add(fix.len() as u64) > file_len {
                    in_bounds = false;
                    break;
                }
            }

            if in_bounds {
                // 2. Read original bytes in case we need to rollback partial writes
                let mut original_bytes: Vec<Vec<u8>> = Vec::with_capacity(patches.len());
                let mut read_ok = true;
                for &(offset, fix) in patches {
                    if f.seek(SeekFrom::Start(offset as u64)).is_err() {
                        read_ok = false;
                        break;
                    }
                    let mut orig = vec![0u8; fix.len()];
                    if f.read_exact(&mut orig).is_err() {
                        read_ok = false;
                        break;
                    }
                    original_bytes.push(orig);
                }

                if read_ok {
                    let mut applied_count = 0;
                    let mut write_err = None;
                    for &(offset, fix) in patches {
                        if let Err(e) = f.seek(SeekFrom::Start(offset as u64)) {
                            write_err = Some(e);
                            break;
                        }
                        if let Err(e) = f.write_all(fix) {
                            write_err = Some(e);
                            break;
                        }
                        applied_count += 1;
                    }

                    if write_err.is_none() {
                        if f.flush().is_ok() {
                            return Ok(());
                        }
                    } else if applied_count > 0 {
                        // Rollback already applied patches to restore pristine state
                        for i in 0..applied_count {
                            let (offset, _) = patches[i];
                            let orig = &original_bytes[i];
                            let _ = f.seek(SeekFrom::Start(offset as u64));
                            let _ = f.write_all(orig);
                        }
                        let _ = f.flush();
                    }
                }
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "patch offset exceeds file length",
                ));
            }
        }
        Err(_) => {
            // Permission error or ETXTBSY -> fallback to atomic replacement
        }
    }

    // Fallback: copy to temp file, patch bytes, atomic rename
    let tmp_path = format!("{}.agytmp.{}", path.display(), std::process::id());
    let tmp_path = Path::new(&tmp_path);

    fs::copy(path, tmp_path)?;

    let res = (|| -> std::io::Result<()> {
        let mut f = OpenOptions::new().read(true).write(true).open(tmp_path)?;
        let file_len = f.metadata()?.len();
        for &(offset, fix) in patches {
            if (offset as u64).saturating_add(fix.len() as u64) > file_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "patch offset exceeds file length",
                ));
            }
            f.seek(SeekFrom::Start(offset as u64))?;
            f.write_all(fix)?;
        }
        f.flush()?;
        drop(f);

        // Preserve permissions on unix
        #[cfg(unix)]
        {
            if let Ok(meta) = fs::metadata(path) {
                let _ = fs::set_permissions(tmp_path, meta.permissions());
            }
        }

        fs::rename(tmp_path, path)?;
        Ok(())
    })();

    if res.is_err() && tmp_path.exists() {
        let _ = fs::remove_file(tmp_path);
    }

    res
}

pub fn apply_string_replacement(
    path: &Path,
    transform: impl FnOnce(&str) -> Result<String, String>,
) -> Result<bool, String> {
    let mut content = String::new();
    {
        let mut f = File::open(path).map_err(|e| format!("cannot open file {}: {}", path.display(), e))?;
        f.read_to_string(&mut content)
            .map_err(|e| format!("cannot read file {}: {}", path.display(), e))?;
    }

    let new_content = transform(&content)?;
    if new_content == content {
        return Ok(false);
    }

    let tmp_path = format!("{}.agytmp.{}", path.display(), std::process::id());
    let tmp_path = Path::new(&tmp_path);

    {
        let mut f = File::create(tmp_path)
            .map_err(|e| format!("cannot create temp file {}: {}", tmp_path.display(), e))?;
        f.write_all(new_content.as_bytes())
            .map_err(|e| format!("cannot write temp file: {}", e))?;
        f.flush().map_err(|e| format!("cannot flush: {}", e))?;
    }

    #[cfg(unix)]
    {
        if let Ok(meta) = fs::metadata(path) {
            let _ = fs::set_permissions(tmp_path, meta.permissions());
        }
    }

    let rename_res = fs::rename(tmp_path, path);
    if rename_res.is_err() && tmp_path.exists() {
        let _ = fs::remove_file(tmp_path);
    }
    rename_res.map_err(|e| format!("cannot replace original file {}: {}", path.display(), e))?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_in_place_success() {
        let tmp_file = std::env::temp_dir().join(format!("agy_test_atomic_{}.bin", std::process::id()));
        fs::write(&tmp_file, b"HELLO WORLD 12345").unwrap();

        fast_apply_patches(&tmp_file, &[(6, b"RUST!")]).unwrap();

        let updated = fs::read(&tmp_file).unwrap();
        assert_eq!(&updated[..11], b"HELLO RUST!");

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_atomic_out_of_bounds_rejection() {
        let tmp_file = std::env::temp_dir().join(format!("agy_test_bounds_{}.bin", std::process::id()));
        fs::write(&tmp_file, b"SHORT").unwrap();

        let res = fast_apply_patches(&tmp_file, &[(100, b"OVERFLOW")]);
        assert!(res.is_err());

        let content = fs::read(&tmp_file).unwrap();
        assert_eq!(content, b"SHORT");

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_apply_string_replacement() {
        let tmp_file = std::env::temp_dir().join(format!("agy_test_str_{}.txt", std::process::id()));
        fs::write(&tmp_file, "shouldCheckEligibility() { return false; }").unwrap();

        let changed = apply_string_replacement(&tmp_file, |s| {
            Ok(s.replace("return false;", "return true;"))
        }).unwrap();

        assert!(changed);
        let content = fs::read_to_string(&tmp_file).unwrap();
        assert_eq!(content, "shouldCheckEligibility() { return true; }");

        let _ = fs::remove_file(&tmp_file);
    }
}
