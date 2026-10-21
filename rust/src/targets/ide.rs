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
    let home = std::env::var("HOME").unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let base = PathBuf::from(appdata).join("Antigravity IDE");
            dirs.push(base.join("CachedData"));
            dirs.push(base.join("Code Cache").join("js"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let base = PathBuf::from(&home).join("Library").join("Application Support").join("Antigravity IDE");
        dirs.push(base.join("CachedData"));
        dirs.push(base.join("Code Cache").join("js"));
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let cfg = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&home).join(".config"));
        let base = cfg.join("Antigravity IDE");
        dirs.push(base.join("CachedData"));
        dirs.push(base.join("Code Cache").join("js"));
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

    let home = std::env::var("HOME").unwrap_or_default();
    let home_path = Path::new(&home);

    let candidates = [
        // User paths (instant hit)
        home_path.join(".local/opt/antigravity-ide/resources/app/out/main.js"),
        home_path.join("Downloads/Antigravity IDE/resources/app/out/main.js"),
        home_path.join("Загрузки/Antigravity IDE/resources/app/out/main.js"),
        // System paths
        PathBuf::from("/opt/antigravity-ide/resources/app/out/main.js"),
        PathBuf::from("/usr/lib/antigravity-ide/resources/app/out/main.js"),
        PathBuf::from("/usr/share/antigravity-ide/resources/app/out/main.js"),
        #[cfg(target_os = "macos")]
        PathBuf::from("/Applications/Antigravity.app/Contents/Resources/app/out/main.js"),
    ];

    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }

    // Check user-dirs.dirs for custom download dir
    let user_dirs = home_path.join(".config/user-dirs.dirs");
    if user_dirs.is_file() {
        if let Ok(content) = fs::read_to_string(user_dirs) {
            for line in content.lines() {
                if line.starts_with("XDG_DOWNLOAD_DIR=") {
                    let raw = line.trim_start_matches("XDG_DOWNLOAD_DIR=").trim_matches('"');
                    let expanded = raw.replace("$HOME", &home);
                    let cand = PathBuf::from(expanded).join("Antigravity IDE/resources/app/out/main.js");
                    if cand.is_file() {
                        return Some(cand);
                    }
                }
            }
        }
    }

    None
}
