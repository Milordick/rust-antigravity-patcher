//! Architecture detection for ELF, PE, and Mach-O binaries.

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

    // Mach-O
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic == 0xFEEDFACE || magic == 0xFEEDFACF {
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

    // Mach-O little-endian magic
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

    Arch::Other
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
}
