//! Safe backup and restore engine.
//! Prevents silently overwriting pristine original backups upon software updates.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn make_backup(path: &Path, ext: &str) -> io::Result<PathBuf> {
    let bak_path = PathBuf::from(format!("{}{}", path.display(), ext));

    if bak_path.exists() {
        if files_identical(path, &bak_path)? {
            return Ok(bak_path);
        }
        // If the live file and backup differ, keep the original pristine backup
        // and create a timestamped secondary backup.
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let ts_bak = PathBuf::from(format!("{}{}.{}", path.display(), ext, ts));
        fs::copy(path, &ts_bak)?;
        return Ok(bak_path);
    }

    fs::copy(path, &bak_path)?;

    #[cfg(unix)]
    {
        if let Ok(meta) = fs::metadata(path) {
            let _ = fs::set_permissions(&bak_path, meta.permissions());
        }
    }

    Ok(bak_path)
}

pub fn restore_backup(path: &Path, ext: &str) -> io::Result<bool> {
    let bak_path = PathBuf::from(format!("{}{}", path.display(), ext));
    if !bak_path.exists() {
        return Ok(false);
    }

    let tmp_path = format!("{}.rstmp.{}", path.display(), std::process::id());
    let tmp_path = Path::new(&tmp_path);

    fs::copy(&bak_path, tmp_path)?;

    #[cfg(unix)]
    {
        if let Ok(meta) = fs::metadata(&bak_path) {
            let _ = fs::set_permissions(tmp_path, meta.permissions());
        }
    }

    fs::rename(tmp_path, path)?;
    Ok(true)
}

pub fn files_identical(a: &Path, b: &Path) -> io::Result<bool> {
    let meta_a = fs::metadata(a)?;
    let meta_b = fs::metadata(b)?;

    if meta_a.len() != meta_b.len() {
        return Ok(false);
    }
    if meta_a.len() == 0 {
        return Ok(true);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta_a.ino() == meta_b.ino() && meta_a.dev() == meta_b.dev() {
            return Ok(true);
        }
    }

    let mut fa = fs::File::open(a)?;
    let mut fb = fs::File::open(b)?;

    let mut buf_a = [0u8; 65536];
    let mut buf_b = [0u8; 65536];

    loop {
        let n_a = fa.read(&mut buf_a)?;
        let n_b = fb.read(&mut buf_b)?;
        if n_a != n_b || buf_a[..n_a] != buf_b[..n_b] {
            return Ok(false);
        }
        if n_a == 0 {
            break;
        }
    }

    Ok(true)
}
