//! Architecture detection for ELF, PE, and Mach-O binaries.

use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X64,
    Arm64,
    Other,
}

impl Arch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Arch::X64 => "x64",
            Arch::Arm64 => "arm64",
            Arch::Other => "other",
        }
    }
}

pub fn detect_arch(data: &[u8]) -> Arch {
    if data.len() < 4 {
        return Arch::Other;
    }

    // ELF
    if &data[..4] == b"\x7fELF" {
        if data.len() < 0x14 {
            return Arch::Other;
        }
        let machine = u16::from_le_bytes([data[0x12], data[0x13]]);
        return match machine {
            0x3E => Arch::X64,
            0xB7 => Arch::Arm64,
            _ => Arch::Other,
        };
    }

    // PE (Windows)
    if &data[..2] == b"MZ" {
        if data.len() < 0x40 {
            return Arch::Other;
        }
        let e_lfanew = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
        if e_lfanew + 6 <= data.len() {
            let machine = u16::from_le_bytes([data[e_lfanew + 4], data[e_lfanew + 5]]);
            return match machine {
                0x8664 => Arch::X64,
                0xAA64 => Arch::Arm64,
                _ => Arch::Other,
            };
        }
        return Arch::Other;
    }

    // Mach-O (Single Architecture - Big Endian)
    let magic_be = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic_be == 0xFEEDFACE || magic_be == 0xFEEDFACF {
        if data.len() < 8 {
            return Arch::Other;
        }
        let cputype = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        return match cputype {
            0x01000007 => Arch::X64,
            0x0100000C => Arch::Arm64,
            _ => Arch::Other,
        };
    }

    // Mach-O (Single Architecture - Little Endian)
    let magic_le = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic_le == 0xFEEDFACE || magic_le == 0xFEEDFACF {
        if data.len() < 8 {
            return Arch::Other;
        }
        let cputype = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        return match cputype {
            0x01000007 => Arch::X64,
            0x0100000C => Arch::Arm64,
            _ => Arch::Other,
        };
    }

    // Mach-O Universal / Fat Binary (0xCAFEBABE or 0xBEBAFECA)
    let is_fat_be = magic_be == 0xCAFEBABE || magic_be == 0xCAFEBABF;
    let is_fat_le = magic_le == 0xCAFEBABE || magic_le == 0xCAFEBABF;
    if is_fat_be || is_fat_le {
        if data.len() < 8 {
            return Arch::Other;
        }
        let nfat_arch = if is_fat_be {
            u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize
        } else {
            u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize
        };

        let mut has_x64 = false;
        let mut has_arm64 = false;
        let count = nfat_arch.min(32);

        for i in 0..count {
            let offset = 8 + i * 20;
            if offset + 4 <= data.len() {
                let cputype = if is_fat_be {
                    u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
                } else {
                    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
                };
                if cputype == 0x01000007 {
                    has_x64 = true;
                } else if cputype == 0x0100000C {
                    has_arm64 = true;
                }
            }
        }

        if has_x64 && has_arm64 {
            // If binary supports both architectures, match current host or default to arm64
            #[cfg(target_arch = "aarch64")]
            return Arch::Arm64;
            #[cfg(not(target_arch = "aarch64"))]
            return Arch::X64;
        } else if has_arm64 {
            return Arch::Arm64;
        } else if has_x64 {
            return Arch::X64;
        }
    }

    Arch::Other
}

/// Detect the architecture of a file on disk by reading its header only
/// (first 1024 bytes — enough for ELF, PE with DOS stub, and Mach-O Fat headers).
/// Returns `Arch::Other` when the file cannot be read or is not a recognized executable format
/// (ELF/PE/Mach-O). Used by the binary finders to reject installer shims
/// and wrapper scripts that merely share the target's file name.
pub fn detect_arch_of_file(path: &Path) -> Arch {
    let mut buf = [0u8; 1024];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .unwrap_or(0);
    detect_arch(&buf[..n])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_x64() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(b"\x7fELF");
        data[0x12..0x14].copy_from_slice(&0x3Eu16.to_le_bytes());
        assert_eq!(detect_arch(&data), Arch::X64);
    }

    #[test]
    fn test_elf_arm64() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(b"\x7fELF");
        data[0x12..0x14].copy_from_slice(&0xB7u16.to_le_bytes());
        assert_eq!(detect_arch(&data), Arch::Arm64);
    }

    #[test]
    fn test_macho_single_x64_le() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(&0xFEEDFACFu32.to_le_bytes());
        data[4..8].copy_from_slice(&0x01000007u32.to_le_bytes());
        assert_eq!(detect_arch(&data), Arch::X64);
    }

    #[test]
    fn test_macho_single_arm64_le() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(&0xFEEDFACFu32.to_le_bytes());
        data[4..8].copy_from_slice(&0x0100000Cu32.to_le_bytes());
        assert_eq!(detect_arch(&data), Arch::Arm64);
    }

    #[test]
    fn test_macho_single_x64_be() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(&0xFEEDFACFu32.to_be_bytes());
        data[4..8].copy_from_slice(&0x01000007u32.to_be_bytes());
        assert_eq!(detect_arch(&data), Arch::X64);
    }

    #[test]
    fn test_macho_single_arm64_be() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(&0xFEEDFACFu32.to_be_bytes());
        data[4..8].copy_from_slice(&0x0100000Cu32.to_be_bytes());
        assert_eq!(detect_arch(&data), Arch::Arm64);
    }

    #[test]
    fn test_macho_fat_universal() {
        let mut data = vec![0u8; 128];
        // FAT_MAGIC (BE)
        data[..4].copy_from_slice(&0xCAFEBABEu32.to_be_bytes());
        // 2 architectures
        data[4..8].copy_from_slice(&2u32.to_be_bytes());
        // Arch 0: x86_64
        data[8..12].copy_from_slice(&0x01000007u32.to_be_bytes());
        // Arch 1: arm64
        data[28..32].copy_from_slice(&0x0100000Cu32.to_be_bytes());

        let detected = detect_arch(&data);
        assert!(detected == Arch::X64 || detected == Arch::Arm64);
    }

    #[test]
    fn test_detect_arch_of_file_pe_windows() {
        // Construct a valid PE header with e_lfanew at 0x80 (128 bytes)
        let mut data = vec![0u8; 256];
        data[0..2].copy_from_slice(b"MZ");
        let e_lfanew = 0x80u32;
        data[0x3C..0x40].copy_from_slice(&e_lfanew.to_le_bytes());
        // PE signature
        data[0x80..0x84].copy_from_slice(b"PE\0\0");
        // Machine x86_64 = 0x8664
        data[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());

        let dir = std::env::temp_dir().join("arch_pe_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("test_windows.exe");
        std::fs::write(&p, &data).unwrap();

        assert_eq!(detect_arch_of_file(&p), Arch::X64);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn test_detect_arch_of_file_rejects_shell_script() {
        let dir = std::env::temp_dir().join("arch_stub_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("test_stub.sh");
        std::fs::write(&p, b"#!/bin/bash\necho 'shim'\n").unwrap();

        assert_eq!(detect_arch_of_file(&p), Arch::Other);
        let _ = std::fs::remove_file(&p);
    }
}
