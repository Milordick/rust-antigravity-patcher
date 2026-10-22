pub mod ide;
pub mod manager;
pub mod agy;
pub mod vscode;

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::core::arch::{detect_arch_of_file, Arch};

/// Pick the best binary from a list of candidate paths.
///
/// Strategy: collect every candidate that exists, dedup by canonical path
/// (symlinks collapse to one entry), order newest-mtime-first (larger file
/// wins ties), then accept the first candidate that is actually a native
/// executable (ELF/PE/Mach-O, x64 or arm64). A shell stub, installer shim
/// or leftover wrapper that merely shares the target's file name no longer
/// wins just by being listed first — this is the "collect all, newest
/// wins" approach, with a robust header architecture check on top.
///
/// Returns `None` when no candidate looks like a real binary, so callers
/// report "not found" instead of patching (or misdiagnosing) the wrong file.
pub fn select_best_binary(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut ranked: Vec<(SystemTime, u64, PathBuf)> = Vec::new();

    for c in candidates {
        if !c.is_file() {
            continue;
        }
        let key = fs::canonicalize(&c).unwrap_or_else(|_| c.clone());
        if !seen.insert(key) {
            continue;
        }
        let (mtime, size) = c
            .metadata()
            .ok()
            .map(|m| {
                let t = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                (t, m.len())
            })
            .unwrap_or((SystemTime::UNIX_EPOCH, 0));
        ranked.push((mtime, size, c));
    }

    // Newest first; on equal mtime prefer the larger file.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    for (_, _, path) in ranked {
        if detect_arch_of_file(&path) != Arch::Other {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn elf_bytes(machine: u16) -> [u8; 64] {
        let mut b = [0u8; 64];
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 2; // 64-bit
        b[5] = 1; // little endian
        b[18..20].copy_from_slice(&machine.to_le_bytes());
        b
    }

    fn tmp_file(name: &str, data: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join("agy_finder_test");
        let _ = fs::create_dir_all(&dir);
        let p = dir.join(name);
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(data).unwrap();
        f.sync_all().unwrap();
        p
    }

    fn cleanup(paths: &[PathBuf]) {
        for p in paths {
            let _ = fs::remove_file(p);
        }
    }

    #[test]
    fn test_rejects_newer_larger_stub_in_favor_of_real_binary() {
        // Regression test for the Termux/Linux bug: a 20 KB shim named `agy`
        // (newer and even larger than the header-only fake, like the real
        // 20 KB vs 180 MB case) must not win over the real binary.
        let real = tmp_file("agy_real", &elf_bytes(0xB7)); // arm64 ELF
        let stub = tmp_file("agy_stub", &vec![b'#'; 20 * 1024]);
        let picked = select_best_binary(vec![stub.clone(), real.clone()]);
        assert_eq!(picked, Some(real.clone()));
        cleanup(&[stub, real]);
    }

    #[test]
    fn test_newest_valid_binary_wins() {
        let old = tmp_file("agy_old", &elf_bytes(0x3E)); // x64 ELF
        std::thread::sleep(std::time::Duration::from_millis(30));
        let new = tmp_file("agy_new", &elf_bytes(0xB7)); // arm64 ELF
        let picked = select_best_binary(vec![old.clone(), new.clone()]);
        assert_eq!(picked, Some(new.clone()));
        cleanup(&[old, new]);
    }

    #[test]
    fn test_returns_none_when_no_candidate_is_a_binary() {
        let stub = tmp_file("agy_only_stub", b"#!/bin/sh\necho shim\n");
        assert_eq!(select_best_binary(vec![stub.clone()]), None);
        cleanup(&[stub]);
    }

    #[test]
    fn test_skips_missing_files() {
        let missing = std::env::temp_dir().join("agy_finder_test/agy_missing_file_xyz");
        let _ = fs::remove_file(&missing);
        assert_eq!(select_best_binary(vec![missing]), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_dedups_symlinks() {
        use std::os::unix::fs::symlink;
        let real = tmp_file("agy_sym_target", &elf_bytes(0xB7));
        let link = std::env::temp_dir().join("agy_finder_test/agy_sym_link");
        let _ = fs::remove_file(&link);
        symlink(&real, &link).unwrap();
        // Both entries resolve to the same file; either path is acceptable.
        let picked = select_best_binary(vec![link.clone(), real.clone()]);
        assert!(picked == Some(link.clone()) || picked == Some(real.clone()));
        cleanup(&[link, real]);
    }
}
