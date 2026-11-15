//! Antigravity IDE (Electron) patcher and discovery.
//! Bypasses regional lock (isGoogleInternal) in main.js.

use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use serde_json::Value;

use crate::core::atomic::apply_string_replacement;
use crate::core::backup::{make_backup, restore_backup};
use crate::core::cache::{drop_cached, get_cached, put_cached};

pub const IDE_DONE: &str = "resetIsTierGCPTos(),true";

static IDE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(resetIsTierGCPTos\(\),)this\.[A-Za-z_$0-9]+\.isGoogleInternal").unwrap()
});

#[derive(Debug, Clone)]
pub struct IdeStatus {
    pub exists: bool,
    pub is_patched: bool,
    pub version: Option<String>,
    pub file_size: u64,
}

pub fn check_ide_status(path: &Path) -> IdeStatus {
    if !path.is_file() {
        return IdeStatus {
            exists: false,
            is_patched: false,
            version: None,
            file_size: 0,
        };
    }

    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return IdeStatus {
                exists: true,
                is_patched: false,
                version: None,
                file_size: 0,
            }
        }
    };
    let sz = meta.len();

    // Check cache
    if let Some(val) = get_cached(path) {
        if let Some(arr) = val.as_array() {
            if arr.len() >= 3 {
                let patched = arr[0].as_bool().unwrap_or(false);
                let ver = arr[1].as_str().map(|s| s.to_string());
                return IdeStatus {
                    exists: true,
                    is_patched: patched,
                    version: ver,
                    file_size: sz,
                };
            }
        }
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            return IdeStatus {
                exists: true,
                is_patched: false,
                version: None,
                file_size: sz,
            }
        }
    };

    let is_patched = is_already_patched(&content);
    let ver = get_ag_version(path);

    put_cached(
        path,
        serde_json::json!([is_patched, ver.as_deref(), sz]),
    );

    IdeStatus {
        exists: true,
        is_patched,
        version: ver,
        file_size: sz,
    }
}

pub fn is_already_patched(content: &str) -> bool {
    content.contains(IDE_DONE) && !IDE_RE.is_match(content)
}

pub const BAK_EXT: &str = ".agybak";

pub fn apply_ide_patch(main_js_path: &Path) -> Result<bool, String> {
    if !main_js_path.is_file() {
        return Err("main.js not found".to_string());
    }

    let content = fs::read_to_string(main_js_path)
        .map_err(|e| format!("cannot read main.js: {}", e))?;

    if is_already_patched(&content) {
        return Ok(false); // already patched
    }

    // Backup
    make_backup(main_js_path, BAK_EXT).map_err(|e| format!("backup failed: {}", e))?;

    let changed = apply_string_replacement(main_js_path, |raw| {
        let res = IDE_RE.replace_all(raw, "${1}true");
        Ok(res.to_string())
    })?;

    if changed {
        drop_cached(main_js_path);
        clear_ide_caches();
    }

    Ok(changed)
}

pub fn restore_ide(main_js_path: &Path) -> Result<bool, String> {
    let mut restored = restore_backup(main_js_path, BAK_EXT)
        .map_err(|e| format!("restore failed: {}", e))?;
    if !restored {
        // Fallback for legacy .bak backups
        restored = restore_backup(main_js_path, ".bak")
            .map_err(|e| format!("restore failed: {}", e))?;
    }
    if restored {
        drop_cached(main_js_path);
        clear_ide_caches();
    }
    Ok(restored)
}

pub fn get_ag_version(main_js_path: &Path) -> Option<String> {
    // Check package.json in parent directories
    let mut dir = main_js_path.parent();
    for _ in 0..4 {
        if let Some(d) = dir {
            let pkg = d.join("package.json");
            if pkg.is_file() {
                if let Ok(c) = fs::read_to_string(&pkg) {
                    if let Ok(Value::Object(map)) = serde_json::from_str(&c) {
                        if let Some(Value::String(ver)) = map.get("version") {
                            return Some(ver.clone());
                        }
                    }
                }
            }
            dir = d.parent();
        } else {
            break;
        }
    }
    None
}

pub fn clear_ide_caches() {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    // Windows caches: Roaming APPDATA and Local LOCALAPPDATA
    let mut win_bases = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        win_bases.push(PathBuf::from(appdata));
    } else if cfg!(windows) && !home.is_empty() {
        win_bases.push(PathBuf::from(&home).join("AppData").join("Roaming"));
    }

    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        win_bases.push(PathBuf::from(localappdata));
    } else if cfg!(windows) && !home.is_empty() {
        win_bases.push(PathBuf::from(&home).join("AppData").join("Local"));
    }

    for base in win_bases {
        for app_name in &["Antigravity IDE", "antigravity-ide", "Antigravity"] {
            let app_dir = base.join(app_name);
            dirs.push(app_dir.join("CachedData"));
            dirs.push(app_dir.join("Code Cache").join("js"));
            dirs.push(app_dir.join("Code Cache"));
            dirs.push(app_dir.join("Cache"));
            dirs.push(app_dir.join("GPUCache"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        for app_name in &["Antigravity IDE", "antigravity-ide", "Antigravity"] {
            let base = PathBuf::from(&home).join("Library").join("Application Support").join(app_name);
            dirs.push(base.join("CachedData"));
            dirs.push(base.join("Code Cache").join("js"));
            dirs.push(base.join("Code Cache"));
            dirs.push(base.join("Cache"));
            dirs.push(base.join("GPUCache"));
        }
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let cfg = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&home).join(".config"));
        for app_name in &["Antigravity IDE", "antigravity-ide", "Antigravity"] {
            let base = cfg.join(app_name);
            dirs.push(base.join("CachedData"));
            dirs.push(base.join("Code Cache").join("js"));
            dirs.push(base.join("Code Cache"));
            dirs.push(base.join("Cache"));
            dirs.push(base.join("GPUCache"));
        }
    }

    for d in dirs {
        if d.is_dir() {
            let _ = fs::remove_dir_all(&d);
        }
    }
}

pub fn find_ide_main_js() -> Option<PathBuf> {
    // Current directory
    let local = PathBuf::from("main.js");
    if local.is_file() {
        return Some(local);
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let home_path = Path::new(&home);

    let mut candidates = Vec::new();

    if !home.is_empty() {
        candidates.push(home_path.join(".local/opt/antigravity-ide/resources/app/out/main.js"));
        candidates.push(home_path.join(".local/share/antigravity-ide/resources/app/out/main.js"));
        candidates.push(home_path.join("Downloads/Antigravity IDE/resources/app/out/main.js"));
        candidates.push(home_path.join("Загрузки/Antigravity IDE/resources/app/out/main.js"));
        candidates.push(home_path.join("Applications/Antigravity IDE.app/Contents/Resources/app/out/main.js"));
        candidates.push(home_path.join("Applications/Antigravity.app/Contents/Resources/app/out/main.js"));
    }

    // System paths (Unix/macOS)
    candidates.push(PathBuf::from("/opt/antigravity-ide/resources/app/out/main.js"));
    candidates.push(PathBuf::from("/opt/Antigravity IDE/resources/app/out/main.js"));
    candidates.push(PathBuf::from("/usr/lib/antigravity-ide/resources/app/out/main.js"));
    candidates.push(PathBuf::from("/usr/share/antigravity-ide/resources/app/out/main.js"));
    candidates.push(PathBuf::from("/Applications/Antigravity IDE.app/Contents/Resources/app/out/main.js"));
    candidates.push(PathBuf::from("/Applications/Antigravity.app/Contents/Resources/app/out/main.js"));

    // Windows paths: standard and alternate locations
    let app_folders = ["Antigravity IDE", "antigravity-ide", "Antigravity"];
    let subpaths = ["resources/app/out/main.js", "resources/app/main.js"];

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let lad = Path::new(&local_app_data);
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(lad.join("Programs").join(folder).join(sub));
                candidates.push(lad.join(folder).join(sub));
            }
        }
    }

    if let Ok(prog_files) = std::env::var("ProgramFiles") {
        let pf = Path::new(&prog_files);
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(pf.join(folder).join(sub));
            }
        }
    }

    if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
        let pf = Path::new(&prog_files_x86);
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(pf.join(folder).join(sub));
            }
        }
    }

    if let Ok(prog_w6432) = std::env::var("ProgramW6432") {
        let pf = Path::new(&prog_w6432);
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(pf.join(folder).join(sub));
            }
        }
    }

    if let Ok(app_data) = std::env::var("APPDATA") {
        let ad = Path::new(&app_data);
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(ad.join(folder).join(sub));
                candidates.push(ad.join("Programs").join(folder).join(sub));
            }
        }
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let up = Path::new(&user_profile);
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(up.join("AppData/Local/Programs").join(folder).join(sub));
                candidates.push(up.join("AppData/Local").join(folder).join(sub));
                candidates.push(up.join("AppData/Roaming").join(folder).join(sub));
                candidates.push(up.join("Downloads").join(folder).join(sub));
                candidates.push(up.join("Загрузки").join(folder).join(sub));
                candidates.push(up.join("Desktop").join(folder).join(sub));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        for folder in &app_folders {
            for sub in &subpaths {
                candidates.push(PathBuf::from(r"C:\Program Files").join(folder).join(sub));
                candidates.push(PathBuf::from(r"C:\Program Files (x86)").join(folder).join(sub));
            }
        }
    }

    // Check user-dirs.dirs for custom download dir
    if !home.is_empty() {
        let user_dirs = home_path.join(".config/user-dirs.dirs");
        if user_dirs.is_file() {
            if let Ok(content) = fs::read_to_string(user_dirs) {
                for line in content.lines() {
                    if line.starts_with("XDG_DOWNLOAD_DIR=") {
                        let raw = line.trim_start_matches("XDG_DOWNLOAD_DIR=").trim_matches('"');
                        let expanded = raw.replace("$HOME", &home);
                        let cand = PathBuf::from(expanded).join("Antigravity IDE/resources/app/out/main.js");
                        candidates.push(cand);
                    }
                }
            }
        }
    }

    // Dedup and sort newest mtime first
    let mut seen = std::collections::HashSet::new();
    let mut ranked = Vec::new();
    for c in candidates {
        if !c.is_file() {
            continue;
        }
        let key = fs::canonicalize(&c).unwrap_or_else(|_| c.clone());
        if !seen.insert(key) {
            continue;
        }
        let mtime = c.metadata().ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        ranked.push((mtime, c));
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    ranked.into_iter().next().map(|(_, p)| p)
}
