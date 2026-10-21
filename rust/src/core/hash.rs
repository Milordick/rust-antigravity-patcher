//! Fast SHA-256 checksum calculation for targets and backups.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use sha2::{Digest, Sha256};

/// Computes SHA-256 hex digest for a file.
/// Uses a 64KB streaming buffer for minimal memory usage and maximal throughput.
pub fn file_sha256(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }

    let result = hasher.finalize();
    let mut hex_str = String::with_capacity(64);
    for byte in result {
        let _ = write!(hex_str, "{:02x}", byte);
    }
    Some(hex_str)
}

/// Truncates a hex hash to standard short representation: `12345678...abcdef01` (8 + 8 chars).
pub fn format_short_hash(hash: &str) -> String {
    if hash.len() > 16 {
        format!("{}...{}", &hash[..8], &hash[hash.len() - 8..])
    } else {
        hash.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_sha256_calculation() {
        let tmp_path = std::env::temp_dir().join(format!("test_sha256_{}.tmp", std::process::id()));
        {
            let mut f = fs::File::create(&tmp_path).unwrap();
            f.write_all(b"Open AG Patcher Rust Edition").unwrap();
        }

        let hash = file_sha256(&tmp_path).expect("hash calculation failed");
        let _ = fs::remove_file(&tmp_path);

        assert_eq!(hash.len(), 64);
        let short = format_short_hash(&hash);
        assert!(short.contains("..."));
    }
}
