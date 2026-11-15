//! VS Code Extension patcher and discovery.
//! 1. Patches `extension.js` to skip binary re-downloads and keep channel locked.
//! 2. Patches the standalone `~/.gemini/bin/antigravity` binary.

use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;

use crate::core::atomic::apply_string_replacement;
use crate::core::backup::{make_backup, restore_backup};
use crate::core::cache::{drop_cached, get_cached, put_cached};

pub const BAK_EXT: &str = ".vscodebak";

pub const PART1_ANCHOR: &str = "outputChannel.appendLine('[INSTALL] Checking Antigravity releases...');";

pub const PART1_INJECT: &str = concat!(
    "{const __primary=options.targetPathOverride||getInstalledTargetPath();",
    "const __candidates=[__primary,",
    "(0,path_1.join)((0,path_1.dirname)(__primary),'antigravity'+((0,path_1.extname)(__primary)||''))];",
    "for(const __p of __candidates){if(__p&&await pathExists(__p)){",
    "outputChannel.appendLine('[INSTALL] Existing binary found at '+__p+'. ",
    "Skipping version check and re-download.');return __p;}}}"
);

pub const PART2_REPLACEMENT: &str = "const isChannelChanged = false;";

static PART1_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(outputChannel\s*\.\s*appendLine\s*\(\s*['"]\[INSTALL\]\s*Checking Antigravity releases[^'"]*['"]\s*\)\s*;?)"#).unwrap()
});

static PART2_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"const\s+isChannelChanged\s*=\s*manifestFetched\s*&&\s*lastInstalledUrl\s*!==\s*releaseBaseUrl\s*;"#).unwrap()
});

static PART2_DONE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"const\s+isChannelChanged\s*=\s*false\s*;"#).unwrap()
});

#[derive(Debug, Clone)]
pub struct VscodeStatus {
    pub exists: bool,
    pub is_patched: bool,
    pub file_size: u64,
}

pub fn check_vscode_status(path: &Path) -> VscodeStatus {
    if !path.is_file() {
        return VscodeStatus {
            exists: false,
            is_patched: false,
            file_size: 0,
        };
    }

    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return VscodeStatus {
                exists: true,
                is_patched: false,
                file_size: 0,
            }
        }
    };
    let sz = meta.len();

    if let Some(val) = get_cached(path) {
        if let Some(arr) = val.as_array() {
            if !arr.is_empty() {
                let patched = arr[0].as_bool().unwrap_or(false);
                return VscodeStatus {
                    exists: true,
                    is_patched: patched,
                    file_size: sz,
                };
            }
        }
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            return VscodeStatus {
                exists: true,
                is_patched: false,
                file_size: sz,
            }
        }
    };

    let is_patched = is_already_patched(&content);
    put_cached(path, serde_json::json!([is_patched, sz]));

    VscodeStatus {
        exists: true,
        is_patched,
        file_size: sz,
    }
}

pub fn is_already_patched(content: &str) -> bool {
    content.contains(PART1_INJECT) && content.contains(PART2_REPLACEMENT)
}

pub fn apply_vscode_patch(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Err(format!("extension.js not found: {}", path.display()));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read extension.js: {}", e))?;

    if is_already_patched(&content) {
        return Ok(false);
    }

    make_backup(path, BAK_EXT).map_err(|e| format!("backup failed: {}", e))?;

    let changed = apply_string_replacement(path, |raw| {
        let mut text = raw.to_string();

        // Part 1: inject after anchor
        if !text.contains(PART1_INJECT) {
            if let Some(m) = PART1_RE.find(&text) {
                let insert_pos = m.end();
                text.insert_str(insert_pos, PART1_INJECT);
            } else if let Some(pos) = text.find(PART1_ANCHOR) {
                let insert_pos = pos + PART1_ANCHOR.len();
                text.insert_str(insert_pos, PART1_INJECT);
            }
        }

        // Part 2: replace isChannelChanged
        if !PART2_DONE_RE.is_match(&text) {
            text = PART2_RE.replace(&text, PART2_REPLACEMENT).to_string();
        }

        Ok(text)
    })?;

    if changed {
        drop_cached(path);
    }

    Ok(changed)
}

pub fn restore_vscode(path: &Path) -> Result<bool, String> {
    let restored = restore_backup(path, BAK_EXT)
        .map_err(|e| format!("restore failed: {}", e))?;
    if restored {
        drop_cached(path);
    }
    Ok(restored)
}

pub fn find_extension_js() -> Option<PathBuf> {
    let mut homes = Vec::new();
    if let Ok(h) = std::env::var("HOME") {
        if !h.is_empty() {
            homes.push(PathBuf::from(h));
        }
    }
    if let Ok(u) = std::env::var("USERPROFILE") {
        if !u.is_empty() {
            let p = PathBuf::from(u);
            if !homes.contains(&p) {
                homes.push(p);
            }
        }
    }

    let mut roots = Vec::new();

    if let Ok(custom) = std::env::var("VSCODE_EXTENSIONS") {
        if !custom.is_empty() {
            roots.push(PathBuf::from(custom));
        }
    }

    for h in &homes {
        roots.push(h.join(".vscode/extensions"));
        roots.push(h.join(".vscode-insiders/extensions"));
        roots.push(h.join(".vscode-oss/extensions"));
        roots.push(h.join(".vscode-server/extensions"));
        roots.push(h.join(".vscode-server-insiders/extensions"));
        roots.push(h.join(".vscodium/extensions"));
        roots.push(h.join(".cursor/extensions"));
        roots.push(h.join(".windsurf/extensions"));
    }

    if let Ok(appdata) = std::env::var("APPDATA") {
        let ad = Path::new(&appdata);
        roots.push(ad.join(".vscode/extensions"));
        roots.push(ad.join("Code/extensions"));
        roots.push(ad.join("Code - Insiders/extensions"));
        roots.push(ad.join("VSCodium/extensions"));
    }

    if let Ok(lad) = std::env::var("LOCALAPPDATA") {
        let p = Path::new(&lad);
        roots.push(p.join("Programs/Microsoft VS Code/resources/app/extensions"));
        roots.push(p.join("Programs/Microsoft VS Code Insiders/resources/app/extensions"));
        roots.push(p.join(".vscode/extensions"));
    }

    if let Ok(pf) = std::env::var("ProgramFiles") {
        let p = Path::new(&pf);
        roots.push(p.join("Microsoft VS Code/resources/app/extensions"));
        roots.push(p.join("Microsoft VS Code Insiders/resources/app/extensions"));
    }

    if let Ok(pfx86) = std::env::var("ProgramFiles(x86)") {
        let p = Path::new(&pfx86);
        roots.push(p.join("Microsoft VS Code/resources/app/extensions"));
    }

    let prefix1 = "google.google-antigravity-";
    let prefix2 = "google.antigravity-";
    let mut best_path = None;
    let mut best_mtime = std::time::SystemTime::UNIX_EPOCH;

    for root in &roots {
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if (name_str.starts_with(prefix1) || name_str.starts_with(prefix2)) && entry.path().is_dir() {
                    let js_cand = entry.path().join("extension.js");
                    if js_cand.is_file() {
                        let mtime = entry.metadata().ok()
                            .and_then(|m| m.modified().ok())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        if mtime >= best_mtime {
                            best_mtime = mtime;
                            best_path = Some(js_cand);
                        }
                    }
                }
            }
        }
    }

    best_path
}

pub fn find_gemini_antigravity_binary() -> Option<PathBuf> {
    let mut homes = Vec::new();
    if let Ok(h) = std::env::var("HOME") {
        if !h.is_empty() {
            homes.push(PathBuf::from(h));
        }
    }
    if let Ok(u) = std::env::var("USERPROFILE") {
        if !u.is_empty() {
            let p = PathBuf::from(u);
            if !homes.contains(&p) {
                homes.push(p);
            }
        }
    }

    let mut candidates = Vec::new();

    for h in &homes {
        candidates.push(h.join(".gemini/bin/antigravity.exe"));
        candidates.push(h.join(".gemini/bin/antigravity"));
        candidates.push(h.join(".gemini/bin/agy.exe"));
        candidates.push(h.join(".gemini/bin/agy"));
        candidates.push(h.join(".gemini/antigravity-cli/bin/antigravity.exe"));
        candidates.push(h.join(".gemini/antigravity-cli/bin/antigravity"));
        candidates.push(h.join(".gemini/antigravity-cli/bin/agy.exe"));
        candidates.push(h.join(".gemini/antigravity-cli/bin/agy"));
    }

    if let Ok(appdata) = std::env::var("APPDATA") {
        let ad = Path::new(&appdata);
        candidates.push(ad.join(".gemini/bin/antigravity.exe"));
        candidates.push(ad.join(".gemini/bin/antigravity"));
        candidates.push(ad.join(".gemini/bin/agy.exe"));
        candidates.push(ad.join(".gemini/bin/agy"));
        candidates.push(ad.join("antigravity/bin/antigravity.exe"));
        candidates.push(ad.join("antigravity/bin/antigravity"));
        candidates.push(ad.join("agy/bin/agy.exe"));
        candidates.push(ad.join("agy/bin/agy"));
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let lad = Path::new(&local_app_data);
        candidates.push(lad.join(".gemini/bin/antigravity.exe"));
        candidates.push(lad.join(".gemini/bin/antigravity"));
        candidates.push(lad.join("Programs/antigravity/bin/antigravity.exe"));
        candidates.push(lad.join("Programs/antigravity/antigravity.exe"));
        candidates.push(lad.join("Programs/agy/bin/antigravity.exe"));
        candidates.push(lad.join("Programs/agy/bin/agy.exe"));
    }

    let mut pf_dirs = Vec::new();
    if let Ok(prog_files) = std::env::var("ProgramFiles") {
        pf_dirs.push(PathBuf::from(prog_files));
    }
    if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
        pf_dirs.push(PathBuf::from(prog_files_x86));
    }
    if let Ok(prog_w6432) = std::env::var("ProgramW6432") {
        pf_dirs.push(PathBuf::from(prog_w6432));
    }

    for pf in pf_dirs {
        candidates.push(pf.join("antigravity/bin/antigravity.exe"));
        candidates.push(pf.join("antigravity/antigravity.exe"));
        candidates.push(pf.join("agy/bin/antigravity.exe"));
        candidates.push(pf.join("agy/bin/agy.exe"));
    }

    // Search PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join("antigravity.exe"));
            candidates.push(dir.join("antigravity"));
            candidates.push(dir.join("agy.exe"));
            candidates.push(dir.join("agy"));
        }
    }

    super::select_best_binary(candidates)
}
