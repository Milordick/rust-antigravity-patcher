//! Antigravity CLI (agy) binary patcher and discovery.
//! Bypasses:
//! 1. Eligibility screen (CLI_GATE)
//! 2. Internal auth gate (AUTH_GATE)
//! 3. Print-mode eligibility (PRINT_ELIGIBILITY_GATE)

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
pub const UPDATER_URL: &[u8] = b"https://antigravity-cli-auto-updater-974169037036.us-central1.run.app";
pub const DISABLED_URL: &[u8] = b"http://127.0.0.1:9999/disabled-auto-updater-server-url-disabled-block";

pub struct AgyGates {
    pub cli_gate: MultiGate,
    pub auth_gate: MultiGate,
    pub print_gate: Gate,
}

impl AgyGates {
    pub fn new() -> Result<Self, regex::Error> {
        // Gate 1: CLI_GATE
        let cli_x64 = Gate::new(
            r"\x48\x85\xc0\x0f\x84....\x80\x78\x08\x00\x0f\x85....",
            r"\x48\x85\xc0\x0f\x84....\x48\x85\xc0\x90\x0f\x85....",
            b"\x48\x85\xc0\x90",
            9,
            "eligibility screen off (x64)",
            Some(Arch::X64),
            Some((b"\x80\x78\x08\x00", 9)),
            Some((b"\x48\x85\xc0\x90", 9)),
            false,
        )?;

        let cli_arm64_sig = r"...\xb5...\xb4\x01\x20\x40\x39[\x01\x21\x41\x61\x81\xa1\xc1\xe1].[\x00-\x07]\x37...[\x94-\x97](?:\xe0\x4b\x00\xf9\xe1\x33\x00\xf9\xe2\x43\x00\xf9|\xe0\x4f\x00\xf9\xe1\x33\x00\xf9\xe2\x47\x00\xf9\xe3\x2f\x00\xf9)";
        let cli_arm64_patched = r"...\xb5...\xb4\x21\x00\x80\x52[\x01\x21\x41\x61\x81\xa1\xc1\xe1].[\x00-\x07]\x37...[\x94-\x97](?:\xe0\x4b\x00\xf9\xe1\x33\x00\xf9\xe2\x43\x00\xf9|\xe0\x4f\x00\xf9\xe1\x33\x00\xf9\xe2\x47\x00\xf9\xe3\x2f\x00\xf9)";
        let cli_arm64 = Gate::new(
            cli_arm64_sig,
            cli_arm64_patched,
            b"\x21\x00\x80\x52",
            8,
            "eligibility screen off (arm64)",
            Some(Arch::Arm64),
            Some((b"\x01\x20\x40\x39", 8)),
            Some((b"\x21\x00\x80\x52", 8)),
            false,
        )?;

        let cli_arm64_w4 = Gate::new(
            r"[\x00\x20\x40\x60\x80\xa0\xc0\xe0]..\xb4\x04\x20\x40\x39[\x04\x24\x44\x64\x84\xa4\xc4\xe4].[\x00-\x07]\x37\x04\x94\x44\xa9",
            r"[\x00\x20\x40\x60\x80\xa0\xc0\xe0]..\xb4[\x24]\x00\x80\x52[\x04\x24\x44\x64\x84\xa4\xc4\xe4].[\x00-\x07]\x37\x04\x94\x44\xa9",
            b"\x24\x00\x80\x52",
            4,
            "eligibility screen off (arm64, w4)",
            Some(Arch::Arm64),
            Some((b"\x04\x20\x40\x39", 4)),
            Some((b"\x24\x00\x80\x52", 4)),
            false,
        )?;

        let cli_arm64_w2 = Gate::new(
            r"[\x01\x21\x41\x61\x81\xa1\xc1\xe1]..\xb5[\x00\x20\x40\x60\x80\xa0\xc0\xe0]..\xb4\x02\x20\x40\x39[\x02\x22\x42\x62\x82\xa2\xc2\xe2].[\x00-\x07]\x37...[\x94-\x97]",
            r"[\x01\x21\x41\x61\x81\xa1\xc1\xe1]..\xb5[\x00\x20\x40\x60\x80\xa0\xc0\xe0]..\xb4\x22\x00\x80\x52[\x02\x22\x42\x62\x82\xa2\xc2\xe2].[\x00-\x07]\x37...[\x94-\x97]",
            b"\x22\x00\x80\x52",
            8,
            "eligibility screen off (arm64, w2)",
            Some(Arch::Arm64),
            Some((b"\x02\x20\x40\x39", 8)),
            Some((b"\x22\x00\x80\x52", 8)),
            false,
        )?;

        let cli_gate = MultiGate::new(vec![cli_x64, cli_arm64, cli_arm64_w2, cli_arm64_w4], "eligibility screen off");

        // Gate 2: AUTH_GATE
        let auth_x64 = Gate::new(
            r"\x80\x78\x08\x00\x74.\x48\x8b.[\x24].\x48\x89.\x60",
            r"\xc6\x40\x08\x01\x90\x90\x48\x8b.[\x24].\x48\x89.\x60",
            b"\xc6\x40\x08\x01\x90\x90",
            0,
            "hasValidAuth=true inside agy (x64)",
            Some(Arch::X64),
            Some((b"\x80\x78\x08\x00", 0)),
            Some((b"\xc6\x40\x08\x01", 0)),
            false,
        )?;

        let auth_arm64 = Gate::new(
            r"\x03\x20\x40\x39[\x03\x23\x43\x63\x83\xa3\xc3\xe3]..\x36(?:....){1,2}\x03\x10\x06\xa9",
            r"\x23\x00\x80\x52\x03\x20\x00\x39(?:....){1,2}\x03\x10\x06\xa9",
            b"\x23\x00\x80\x52\x03\x20\x00\x39",
            0,
            "hasValidAuth=true inside agy (arm64)",
            Some(Arch::Arm64),
            Some((b"\x03\x20\x40\x39", 0)),
            Some((b"\x23\x00\x80\x52", 0)),
            false,
        )?;

        let auth_gate = MultiGate::new(vec![auth_x64, auth_arm64], "hasValidAuth=true inside agy");

        // Gate 3: PRINT_ELIGIBILITY_GATE (x64)
        let print_gate = Gate::new(
            r"\x48\x85\xc0\x0f\x85....\x48\x85\xc9\x0f\x84....\x80\x79\x08\x00\x0f\x85....",
            r"\x48\x85\xc0\x0f\x85....\x48\x85\xc9\x0f\x84....\x48\x85\xc9\x90\x0f\x85....",
            b"\x48\x85\xc9\x90",
            18,
            "print-mode eligibility (rcx) off (x64)",
            Some(Arch::X64),
            Some((b"\x80\x79\x08\x00", 18)),
            Some((b"\x48\x85\xc9\x90", 18)),
            false,
        )?;

        Ok(Self {
            cli_gate,
            auth_gate,
            print_gate,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AgyStatus {
    pub exists: bool,
    pub is_patched: bool,
    pub is_android: bool,
    pub file_size: u64,
}

pub fn is_android_aarch64(data: &[u8]) -> bool {
    if data.len() < 64 || &data[..4] != b"\x7fELF" {
        return false;
    }
    if data[4] != 2 || data[5] != 1 {
        return false;
    }
    let machine = u16::from_le_bytes([data[18], data[19]]);
    if machine != 0xB7 {
        return false;
    }
    let head_limit = data.len().min(65536);
    let head = &data[..head_limit];
    memchr::memmem::find(head, b"Android").is_some()
        || memchr::memmem::find(head, b".note.android.ident").is_some()
        || memchr::memmem::find(head, b"/system/bin/linker64").is_some()
}

pub struct TermuxGate {
    pub unpatched_re: regex::bytes::Regex,
    pub patched_re: regex::bytes::Regex,
}

impl TermuxGate {
    pub fn new() -> Result<Self, regex::Error> {
        let unpatched_re = regex::bytes::RegexBuilder::new(
            r"(?-u)\xe0[\x00-\xff]\x00\xf9...\xb5...\xb4....([\x41\x42]..\x37)...[\x94-\x97]",
        )
        .dot_matches_new_line(true)
        .build()?;

        let patched_re = regex::bytes::RegexBuilder::new(
            r"(?-u)\xe0[\x00-\xff]\x00\xf9...\xb5...\xb4.......\x14...[\x94-\x97]",
        )
        .dot_matches_new_line(true)
        .build()?;

        Ok(Self {
            unpatched_re,
            patched_re,
        })
    }

    /// Resolves status and returns (GateStatus, Vec<(offset, [u8; 4])>)
    pub fn resolve(
        &self,
        data: &[u8],
        bounds: Option<crate::core::bounds::CodeBounds>,
    ) -> Result<(GateStatus, Vec<(usize, [u8; 4])>), String> {
        let (start, end) = match bounds {
            Some(b) => (b.start.min(data.len()), b.end.min(data.len())),
            None => (0, data.len()),
        };
        let slice = &data[start..end];

        // 1. Check if already patched
        let mut patched_offsets = Vec::new();
        for m in self.patched_re.find_iter(slice) {
            patched_offsets.push(start + m.start() + 16);
        }
        if patched_offsets.len() > 1 {
            return Err("Termux gate matched more than once (corrupted/unsupported build?)".to_string());
        }
        if !patched_offsets.is_empty() {
            return Ok((GateStatus::Patched, Vec::new()));
        }

        // 2. Check unpatched
        let mut unpatched = Vec::new();
        for m in self.unpatched_re.find_iter(slice) {
            let tbnz_offset = start + m.start() + 16;
            if tbnz_offset + 4 <= data.len() {
                let tbnz_bytes: [u8; 4] = data[tbnz_offset..tbnz_offset + 4].try_into().unwrap();
                let insn = u32::from_le_bytes(tbnz_bytes);
                let imm14 = (insn >> 5) & 0x3fff;
                let b_insn = 0x1400_0000 | (imm14 & 0x03ff_ffff);
                let b_bytes = b_insn.to_le_bytes();
                unpatched.push((tbnz_offset, b_bytes));
            }
        }

        if unpatched.len() > 1 {
            return Err("Termux gate matched more than once (corrupted/unsupported build?)".to_string());
        }
        if unpatched.is_empty() {
            return Err("Termux/Android arm64 gate signature not found (unsupported agy build?)".to_string());
        }

        Ok((GateStatus::Unpatched, unpatched))
    }
}

pub fn check_agy_status(path: &Path) -> AgyStatus {
    if !path.is_file() {
        return AgyStatus {
            exists: false,
            is_patched: false,
            is_android: false,
            file_size: 0,
        };
    }

    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return AgyStatus {
                exists: true,
                is_patched: false,
                is_android: false,
                file_size: 0,
            }
        }
    };
    let sz = meta.len();

    // Check cache
    if let Some(val) = get_cached(path) {
        if let Some(arr) = val.as_array() {
            if !arr.is_empty() {
                let state = arr[0].as_str().unwrap_or("");
                let is_android = arr.get(2).and_then(|v| v.as_bool()).unwrap_or(false);
                return AgyStatus {
                    exists: true,
                    is_patched: state == "patched",
                    is_android,
                    file_size: sz,
                };
            }
        }
    }

    let f = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return AgyStatus {
                exists: true,
                is_patched: false,
                is_android: false,
                file_size: sz,
            }
        }
    };

    let mmap = match unsafe { Mmap::map(&f) } {
        Ok(m) => m,
        Err(_) => {
            return AgyStatus {
                exists: true,
                is_patched: false,
                is_android: false,
                file_size: sz,
            }
        }
    };

    let is_android = is_android_aarch64(&mmap);
    let bounds = get_code_bounds(&mmap);

    let is_patched = if is_android {
        let termux_gate = match TermuxGate::new() {
            Ok(g) => g,
            Err(_) => {
                return AgyStatus {
                    exists: true,
                    is_patched: false,
                    is_android,
                    file_size: sz,
                }
            }
        };
        let res = termux_gate.resolve(&mmap, bounds)
            .or_else(|_| termux_gate.resolve(&mmap, None));
        res.map(|(s, _)| s == GateStatus::Patched).unwrap_or(false)
    } else {
        let arch = detect_arch(&mmap);
        let gates = match AgyGates::new() {
            Ok(g) => g,
            Err(_) => {
                return AgyStatus {
                    exists: true,
                    is_patched: false,
                    is_android,
                    file_size: sz,
                }
            }
        };

        let g1 = gates.cli_gate.resolve(&mmap, Some(arch), bounds).map(|(s, _, _, _)| s == GateStatus::Patched).unwrap_or(false);
        let g2 = gates.auth_gate.resolve(&mmap, Some(arch), bounds).map(|(s, _, _, _)| s == GateStatus::Patched).unwrap_or(true);
        let g3 = if arch == Arch::X64 {
            gates.print_gate.find(&mmap, Some(arch), Some(1), bounds).map(|(s, _)| s == GateStatus::Patched).unwrap_or(false)
        } else {
            true
        };
        g1 && g2 && g3
    };

    let state_str = if is_patched { "patched" } else { "unpatched" };
    put_cached(path, serde_json::json!([state_str, null, is_android]));

    AgyStatus {
        exists: true,
        is_patched,
        is_android,
        file_size: sz,
    }
}

pub fn apply_agy_patch(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Err(format!("agy binary not found: {}", path.display()));
    }

    let f = File::open(path).map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
    let mmap = unsafe { Mmap::map(&f) }.map_err(|e| format!("cannot mmap {}: {}", path.display(), e))?;

    let bounds = get_code_bounds(&mmap);
    let is_android = is_android_aarch64(&mmap);

    let patches_to_apply: Vec<(usize, Vec<u8>)> = if is_android {
        let termux_gate = TermuxGate::new().map_err(|e| e.to_string())?;
        let (status, dynamic_patches) = termux_gate.resolve(&mmap, bounds)
            .or_else(|_| termux_gate.resolve(&mmap, None))?;
        if status == GateStatus::Patched {
            return Ok(false);
        }
        dynamic_patches.into_iter().map(|(off, bytes)| (off, bytes.to_vec())).collect()
    } else {
        let arch = detect_arch(&mmap);
        let gates = AgyGates::new().map_err(|e| e.to_string())?;

        let mut patches = Vec::new();
        // 1. Gate 1 (CLI eligibility gate)
        let (s1, off1, _, fix1) = gates.cli_gate.resolve(&mmap, Some(arch), bounds)?;
        if s1 != GateStatus::Patched {
            patches.push((off1, fix1.to_vec()));
        }

        // 2. Gate 2 (Auth gate)
        if let Ok((s2, off2, _, fix2)) = gates.auth_gate.resolve(&mmap, Some(arch), bounds) {
            if s2 != GateStatus::Patched {
                patches.push((off2, fix2.to_vec()));
            }
        }

        // 3. Gate 3 (x64)
        if arch == Arch::X64 {
            if let Ok((s3, offsets)) = gates.print_gate.find(&mmap, Some(arch), None, bounds) {
                if s3 != GateStatus::Patched {
                    for off in offsets {
                        patches.push((off, b"\x48\x85\xc9\x90".to_vec()));
                    }
                }
            }
        }

        // 4. Mobile Update Lock integration
        let lock_active = std::fs::read_to_string("/workspace/.update_lock")
            .map(|s| s.trim() != "false")
            .unwrap_or(false);

        if lock_active {
            if let Some(off) = memchr::memmem::find(&mmap, UPDATER_URL) {
                patches.push((off, DISABLED_URL.to_vec()));
            }
        } else if Path::new("/workspace/.update_lock").exists() {
            if let Some(off) = memchr::memmem::find(&mmap, DISABLED_URL) {
                patches.push((off, UPDATER_URL.to_vec()));
            }
        }

        patches
    };

    if patches_to_apply.is_empty() {
        return Ok(false); // already fully patched
    }

    drop(mmap);
    drop(f);

    // Backup
    make_backup(path, BAK_EXT).map_err(|e| format!("backup failed: {}", e))?;

    let patch_refs: Vec<(usize, &[u8])> = patches_to_apply.iter().map(|(off, b)| (*off, b.as_slice())).collect();
    fast_apply_patches(path, &patch_refs).map_err(|e| format!("write patch failed: {}", e))?;

    drop_cached(path);
    Ok(true)
}

pub fn restore_agy(path: &Path) -> Result<bool, String> {
    let restored = restore_backup(path, BAK_EXT)
        .map_err(|e| format!("restore failed: {}", e))?;
    if restored {
        drop_cached(path);
    }
    Ok(restored)
}

pub fn set_update_lock(path: &Path, lock: bool) -> Result<bool, String> {
    if !path.is_file() {
        return Err(format!("agy binary not found: {}", path.display()));
    }
    let f = File::open(path).map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
    let mmap = unsafe { Mmap::map(&f) }.map_err(|e| format!("cannot mmap {}: {}", path.display(), e))?;

    let (from, to) = if lock {
        (UPDATER_URL, DISABLED_URL)
    } else {
        (DISABLED_URL, UPDATER_URL)
    };

    let found_off = memchr::memmem::find(&mmap, from);
    drop(mmap);
    drop(f);

    if let Some(off) = found_off {
        fast_apply_patches(path, &[(off, to)]).map_err(|e| format!("write update lock failed: {}", e))?;
        drop_cached(path);
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn find_agy_binary() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let home_path = Path::new(&home);

    let mut candidates = Vec::new();

    if let Ok(prefix) = std::env::var("PREFIX") {
        if !prefix.is_empty() {
            candidates.push(Path::new(&prefix).join("bin/agy"));
        }
    }

    candidates.push(PathBuf::from("/data/data/com.termux/files/usr/bin/agy"));
    candidates.push(PathBuf::from("/root/.local/bin/agy"));
    candidates.push(PathBuf::from("/workspace/bin/agy"));
    candidates.push(PathBuf::from("/workspace/agy"));

    if !home.is_empty() {
        candidates.push(home_path.join(".local/bin/agy"));
        candidates.push(home_path.join("bin/agy"));
        #[cfg(target_os = "windows")]
        candidates.push(home_path.join(".local/bin/agy.exe"));
    }

    candidates.push(PathBuf::from("/usr/local/bin/agy"));
    candidates.push(PathBuf::from("/usr/bin/agy"));
    candidates.push(PathBuf::from("/opt/antigravity/bin/agy"));
    candidates.push(PathBuf::from("/opt/antigravity/agy"));
    candidates.push(PathBuf::from("./agy"));
    #[cfg(target_os = "windows")]
    candidates.push(PathBuf::from("./agy.exe"));

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let lad = Path::new(&local_app_data);
            candidates.push(lad.join("Programs/agy/bin/agy.exe"));
            candidates.push(lad.join("Programs/agy/agy.exe"));
            candidates.push(lad.join("agy/bin/agy.exe"));
        }
        if let Ok(prog_files) = std::env::var("ProgramFiles") {
            let pf = Path::new(&prog_files);
            candidates.push(pf.join("agy/bin/agy.exe"));
            candidates.push(pf.join("agy/agy.exe"));
        }
    }

    // Search PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join(if cfg!(windows) { "agy.exe" } else { "agy" }));
        }
    }

    super::select_best_binary(candidates)
}
