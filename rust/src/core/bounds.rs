//! Executable section bounding for ELF, PE, and Mach-O.
//! Narrows signature search to executable code (typically 20-30% of binary size),
//! providing a ~4x speedup on cold scans.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeBounds {
    pub start: usize,
    pub end: usize,
}

pub fn get_code_bounds(data: &[u8]) -> Option<CodeBounds> {
    let size = data.len();
    if size < 64 {
        return None;
    }

    // 1. ELF
    if &data[..4] == b"\x7fELF" {
        let is_64 = data[4] == 2;
        // Section headers: SHT_PROGBITS (1) & SHF_EXECINSTR (4)
        let (e_shoff, e_shentsize, e_shnum) = if is_64 {
            if size < 64 {
                return None;
            }
            let shoff = u64::from_le_bytes(data[40..48].try_into().ok()?) as usize;
            let shentsize = u16::from_le_bytes(data[58..60].try_into().ok()?) as usize;
            let shnum = u16::from_le_bytes(data[60..62].try_into().ok()?) as usize;
            (shoff, shentsize, shnum)
        } else {
            if size < 52 {
                return None;
            }
            let shoff = u32::from_le_bytes(data[32..36].try_into().ok()?) as usize;
            let shentsize = u16::from_le_bytes(data[46..48].try_into().ok()?) as usize;
            let shnum = u16::from_le_bytes(data[48..50].try_into().ok()?) as usize;
            (shoff, shentsize, shnum)
        };

        if e_shoff > 0 && e_shoff < size && e_shnum > 0 && e_shoff + e_shnum * e_shentsize <= size {
            let mut min_sec = size;
            let mut max_sec = 0;
            for i in 0..e_shnum {
                let off = e_shoff + i * e_shentsize;
                if off + (if is_64 { 64 } else { 40 }) > size {
                    break;
                }
                let s_type = u32::from_le_bytes(data[off + 4..off + 8].try_into().ok()?);
                let (s_flags, s_off, s_sz) = if is_64 {
                    let flags = u64::from_le_bytes(data[off + 8..off + 16].try_into().ok()?);
                    let offset = u64::from_le_bytes(data[off + 24..off + 32].try_into().ok()?) as usize;
                    let sz = u64::from_le_bytes(data[off + 32..off + 40].try_into().ok()?) as usize;
                    (flags, offset, sz)
                } else {
                    let flags = u32::from_le_bytes(data[off + 8..off + 12].try_into().ok()?) as u64;
                    let offset = u32::from_le_bytes(data[off + 16..off + 20].try_into().ok()?) as usize;
                    let sz = u32::from_le_bytes(data[off + 20..off + 24].try_into().ok()?) as usize;
                    (flags, offset, sz)
                };

                if s_type == 1 && (s_flags & 4) != 0 && s_sz > 0 && s_off < size {
                    min_sec = min_sec.min(s_off);
                    max_sec = max_sec.max(size.min(s_off + s_sz));
                }
            }
            if max_sec > min_sec {
                return Some(CodeBounds { start: min_sec, end: max_sec });
            }
        }

        // Fallback: Program headers PT_LOAD (1) & PF_X (1)
        let (e_phoff, e_phentsize, e_phnum) = if is_64 {
            let phoff = u64::from_le_bytes(data[32..40].try_into().ok()?) as usize;
            let phentsize = u16::from_le_bytes(data[54..56].try_into().ok()?) as usize;
            let phnum = u16::from_le_bytes(data[56..58].try_into().ok()?) as usize;
            (phoff, phentsize, phnum)
        } else {
            let phoff = u32::from_le_bytes(data[28..32].try_into().ok()?) as usize;
            let phentsize = u16::from_le_bytes(data[42..44].try_into().ok()?) as usize;
            let phnum = u16::from_le_bytes(data[44..46].try_into().ok()?) as usize;
            (phoff, phentsize, phnum)
        };

        if e_phoff > 0 && e_phoff < size && e_phnum > 0 && e_phoff + e_phnum * e_phentsize <= size {
            let mut min_off = size;
            let mut max_off = 0;
            for i in 0..e_phnum {
                let off = e_phoff + i * e_phentsize;
                let p_type = u32::from_le_bytes(data[off..off + 4].try_into().ok()?);
                let p_flags = u32::from_le_bytes(data[off + 4..off + 8].try_into().ok()?);
                if p_type == 1 && (p_flags & 1) != 0 {
                    let (p_off, p_sz) = if is_64 {
                        let offset = u64::from_le_bytes(data[off + 8..off + 16].try_into().ok()?) as usize;
                        let sz = u64::from_le_bytes(data[off + 32..off + 40].try_into().ok()?) as usize;
                        (offset, sz)
                    } else {
                        let offset = u32::from_le_bytes(data[off + 4..off + 8].try_into().ok()?) as usize;
                        let sz = u32::from_le_bytes(data[off + 16..off + 20].try_into().ok()?) as usize;
                        (offset, sz)
                    };
                    if p_sz > 0 && p_off < size {
                        min_off = min_off.min(p_off);
                        max_off = max_off.max(size.min(p_off + p_sz));
                    }
                }
            }
            if max_off > min_off {
                return Some(CodeBounds { start: min_off, end: max_off });
            }
        }
    }

    // 2. PE (Windows)
    if &data[..2] == b"MZ" && size > 0x40 {
        let e_lfanew = u32::from_le_bytes(data[0x3C..0x40].try_into().ok()?) as usize;
        if e_lfanew > 0 && e_lfanew < size.saturating_sub(256) && &data[e_lfanew..e_lfanew + 4] == b"PE\0\0" {
            let num_sec = u16::from_le_bytes(data[e_lfanew + 6..e_lfanew + 8].try_into().ok()?) as usize;
            let opt_sz = u16::from_le_bytes(data[e_lfanew + 20..e_lfanew + 22].try_into().ok()?) as usize;
            let sec_hdr_start = e_lfanew + 24 + opt_sz;
            let mut min_off = size;
            let mut max_off = 0;
            for i in 0..num_sec {
                let hdr = sec_hdr_start + i * 40;
                if hdr + 40 > size {
                    break;
                }
                let chars = u32::from_le_bytes(data[hdr + 36..hdr + 40].try_into().ok()?);
                if (chars & 0x20000020) != 0 { // IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_CNT_CODE
                    let raw_sz = u32::from_le_bytes(data[hdr + 16..hdr + 20].try_into().ok()?) as usize;
                    let raw_ptr = u32::from_le_bytes(data[hdr + 20..hdr + 24].try_into().ok()?) as usize;
                    if raw_sz > 0 && raw_ptr < size {
                        min_off = min_off.min(raw_ptr);
                        max_off = max_off.max(size.min(raw_ptr + raw_sz));
                    }
                }
            }
            if max_off > min_off {
                return Some(CodeBounds { start: min_off, end: max_off });
            }
        }
    }

    // 3. Mach-O (macOS)
    if data.len() >= 32 {
        let magic = u32::from_be_bytes(data[..4].try_into().ok()?);
        let is_64 = magic == 0xFEEDFACF || magic == 0xCFFAEDFE;
        let is_le = magic == 0xFEEDFACE || magic == 0xFEEDFACF;
        let is_valid = is_64 || magic == 0xFEEDFACE || magic == 0xCEFAEDFE;
        if is_valid {
            let ncmds = if is_le {
                u32::from_le_bytes(data[16..20].try_into().ok()?)
            } else {
                u32::from_be_bytes(data[16..20].try_into().ok()?)
            } as usize;
            let mut cmd_off = if is_64 { 32 } else { 28 };
            for _ in 0..ncmds {
                if cmd_off + 8 > size {
                    break;
                }
                let (cmd, cmdsize) = if is_le {
                    (
                        u32::from_le_bytes(data[cmd_off..cmd_off + 4].try_into().ok()?),
                        u32::from_le_bytes(data[cmd_off + 4..cmd_off + 8].try_into().ok()?) as usize,
                    )
                } else {
                    (
                        u32::from_be_bytes(data[cmd_off..cmd_off + 4].try_into().ok()?),
                        u32::from_be_bytes(data[cmd_off + 4..cmd_off + 8].try_into().ok()?) as usize,
                    )
                };
                if (is_64 && cmd == 0x19) || (!is_64 && cmd == 0x1) { // LC_SEGMENT_64 / LC_SEGMENT
                    if cmd_off + 24 <= size {
                        let segname_raw = &data[cmd_off + 8..cmd_off + 24];
                        let segname = segname_raw.split(|&b| b == 0).next().unwrap_or(b"");
                        if segname == b"__TEXT" {
                            let (fileoff, filesz) = if is_64 {
                                if is_le {
                                    (
                                        u64::from_le_bytes(data[cmd_off + 24..cmd_off + 32].try_into().ok()?) as usize,
                                        u64::from_le_bytes(data[cmd_off + 32..cmd_off + 40].try_into().ok()?) as usize,
                                    )
                                } else {
                                    (
                                        u64::from_be_bytes(data[cmd_off + 24..cmd_off + 32].try_into().ok()?) as usize,
                                        u64::from_be_bytes(data[cmd_off + 32..cmd_off + 40].try_into().ok()?) as usize,
                                    )
                                }
                            } else {
                                if is_le {
                                    (
                                        u32::from_le_bytes(data[cmd_off + 16..cmd_off + 20].try_into().ok()?) as usize,
                                        u32::from_le_bytes(data[cmd_off + 20..cmd_off + 24].try_into().ok()?) as usize,
                                    )
                                } else {
                                    (
                                        u32::from_be_bytes(data[cmd_off + 16..cmd_off + 20].try_into().ok()?) as usize,
                                        u32::from_be_bytes(data[cmd_off + 20..cmd_off + 24].try_into().ok()?) as usize,
                                    )
                                }
                            };
                            if filesz > 0 && fileoff < size {
                                return Some(CodeBounds { start: fileoff, end: size.min(fileoff + filesz) });
                            }
                        }
                    }
                }
                if cmdsize == 0 {
                    break;
                }
                cmd_off += cmdsize;
            }
        }
    }

    None
}
