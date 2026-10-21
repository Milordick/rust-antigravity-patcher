//! Antigravity 2.0 (language_server) binary patcher and discovery.
//! Bypasses auth gate: hasValidAuth=true (Go binary).

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use memmap2::Mmap;

use crate::core::arch::{detect_arch, Arch};
use crate::core::bounds::get_code_bounds;
use crate::core::gate::{Gate, GateStatus, MultiGate};
use crate::core::atomic::fast_apply_patches;
use crate::core::backup::{make_backup, restore_backup};
use crate::core::cache::{drop_cached, get_cached, put_cached};

pub const BAK_EXT: &str = ".agybak";

pub struct ManagerGates {
    pub multi_gate: MultiGate,
}

impl ManagerGates {
    pub fn new() -> Result<Self, regex::Error> {
        let x64 = Gate::new(
            r"\x80\x78\x08\x00\x74.\x48\x8b.[\x24].\x48\x89.\x60",
            r"\xc6\x40\x08\x01\x90\x90\x48\x8b.[\x24].\x48\x89.\x60",
            b"\xc6\x40\x08\x01\x90\x90",
            0,
            "hasValidAuth=true (x64)",
            Some(Arch::X64),
            Some((b"\x80\x78\x08\x00", 0)),
            Some((b"\xc6\x40\x08\x01", 0)),
            true,
        )?;

        let arm64_sig = r"\x03\x20\x40\x39[\x03\x23\x43\x63\x83\xa3\xc3\xe3]..\x36(?:....){1,2}\x03\x10\x06\xa9";
        let arm64_patched = r"\x23\x00\x80\x52\x03\x20\x00\x39(?:....){1,2}\x03\x10\x06\xa9";
        let arm64 = Gate::new(
            arm64_sig,
            arm64_patched,
            b"\x23\x00\x80\x52\x03\x20\x00\x39",
            0,
            "hasValidAuth=true (arm64)",
            Some(Arch::Arm64),
            Some((b"\x03\x20\x40\x39", 0)),
            Some((b"\x23\x00\x80\x52", 0)),
            true,
        )?;

        let multi = MultiGate::new(vec![x64, arm64], "hasValidAuth=true");
        Ok(Self { multi_gate: multi })
    }
}

#[derive(Debug, Clone)]
pub struct ManagerStatus {
    pub exists: bool,
    pub is_patched: bool,
    pub desc: Option<String>,
    pub file_size: u64,
}

pub fn check_manager_status(path: &Path) -> ManagerStatus {
    if !path.is_file() {
        return ManagerStatus {
            exists: false,
            is_patched: false,
            desc: None,
            file_size: 0,
        };
    }

    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return ManagerStatus {
                exists: true,
                is_patched: false,
                desc: None,
                file_size: 0,
            }
        }
    };
    let sz = meta.len();

    // Check cache
    if let Some(val) = get_cached(path) {
        if let Some(arr) = val.as_array() {
            if arr.len() >= 2 {
                let state = arr[0].as_str().unwrap_or("");
                let desc = arr[1].as_str().map(|s| s.to_string());
                return ManagerStatus {
                    exists: true,
                    is_patched: state == "patched",
                    desc,
                    file_size: sz,
                };
            }
        }
    }

    let f = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return ManagerStatus {
                exists: true,
                is_patched: false,
                desc: None,
                file_size: sz,
            }
        }
    };

    let mmap = match unsafe { Mmap::map(&f) } {
        Ok(m) => m,
        Err(_) => {
            return ManagerStatus {
                exists: true,
                is_patched: false,
                desc: None,
                file_size: sz,
            }
        }
    };

    let arch = detect_arch(&mmap);
    let bounds = get_code_bounds(&mmap);

    let gates = match ManagerGates::new() {
        Ok(g) => g,
        Err(_) => {
            return ManagerStatus {
                exists: true,
                is_patched: false,
                desc: None,
                file_size: sz,
            }
        }
    };

    let (is_patched, desc) = match gates.multi_gate.resolve(&mmap, Some(arch), bounds) {
        Ok((status, _, desc)) => (status == GateStatus::Patched, Some(desc.to_string())),
        Err(_) => (false, None),
    };

    let state_str = if is_patched { "patched" } else { "unpatched" };
    put_cached(
        path,
        serde_json::json!([state_str, desc.as_deref()]),
    );

    ManagerStatus {
        exists: true,
        is_patched,
        desc,
        file_size: sz,
    }
}

pub fn apply_manager_patch(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Err(format!("manager binary not found: {}", path.display()));
    }

    let f = File::open(path).map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
    let mmap = unsafe { Mmap::map(&f) }.map_err(|e| format!("cannot mmap {}: {}", path.display(), e))?;

    let arch = detect_arch(&mmap);
    let bounds = get_code_bounds(&mmap);
    let gates = ManagerGates::new().map_err(|e| e.to_string())?;

    let (status, offset, _) = gates.multi_gate.resolve(&mmap, Some(arch), bounds)?;
    if status == GateStatus::Patched {
        return Ok(false); // already patched
    }

    drop(mmap);
    drop(f);

    // Backup
    make_backup(path, BAK_EXT).map_err(|e| format!("backup failed: {}", e))?;

    // Determine fix bytes
    let fix: &[u8] = match arch {
        Arch::Arm64 => b"\x23\x00\x80\x52\x03\x20\x00\x39",
        _ => b"\xc6\x40\x08\x01\x90\x90",
    };

    fast_apply_patches(path, &[(offset, fix)])
        .map_err(|e| format!("write patch failed: {}", e))?;

    drop_cached(path);
    Ok(true)
}

pub fn restore_manager(path: &Path) -> Result<bool, String> {
    let restored = restore_backup(path, BAK_EXT)
        .map_err(|e| format!("restore failed: {}", e))?;
    if restored {
        drop_cached(path);
    }
    Ok(restored)
}

pub fn find_manager_binary() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let home_path = Path::new(&home);

    let candidates = [
        home_path.join(".local/opt/antigravity-ide/resources/app/extensions/antigravity/bin/language_server_linux_x64"),
        home_path.join(".local/opt/antigravity-ide/resources/app/extensions/antigravity/bin/language_server"),
        home_path.join("Downloads/Antigravity IDE/resources/app/extensions/antigravity/bin/language_server_linux_x64"),
        home_path.join("Загрузки/Antigravity IDE/resources/app/extensions/antigravity/bin/language_server_linux_x64"),
        PathBuf::from("/opt/antigravity-ide/resources/app/extensions/antigravity/bin/language_server_linux_x64"),
        PathBuf::from("/opt/antigravity-ide/resources/app/extensions/antigravity/bin/language_server"),
        PathBuf::from("/usr/lib/antigravity-ide/resources/app/extensions/antigravity/bin/language_server_linux_x64"),
        PathBuf::from("/usr/share/antigravity-ide/resources/app/extensions/antigravity/bin/language_server_linux_x64"),
    ];

    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }

    // Check relative to IDE main.js if found
    if let Some(main_js) = crate::targets::ide::find_ide_main_js() {
        if let Some(app_dir) = main_js.parent().and_then(|p| p.parent()) {
            let bin_dir = app_dir.join("extensions/antigravity/bin");
            if bin_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(bin_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if name.starts_with("language_server") && !name.ends_with(".agybak") && p.is_file() {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }

    None
}
