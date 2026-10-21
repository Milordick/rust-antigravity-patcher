//! Two-tier persistent scan cache.
//! Fully compatible with Python's `~/.cache/open-antigravity-patcher/scan_cache.json`.
//! Provides sub-millisecond status responses on subsequent launches.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use serde_json::Value;

static CACHE: Mutex<Option<HashMap<String, Value>>> = Mutex::new(None);

pub fn get_cache_file_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(PathBuf::from))
        .ok()?;

    #[cfg(target_os = "macos")]
    let base = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Library").join("Caches"))
        .ok()?;

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .ok()?;

    let dir = base.join("open-antigravity-patcher");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join("scan_cache.json"))
}

pub fn make_cache_key(path: &Path) -> Option<String> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let meta = fs::metadata(&abs_path).ok()?;
    let sz = meta.len();

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mtime_ns = (meta.mtime() as i128) * 1_000_000_000 + (meta.mtime_nsec() as i128);
        return Some(format!("{}:{}:{}", abs_path.display(), sz, mtime_ns));
    }

    #[cfg(not(unix))]
    {
        let mtime = meta.modified().ok()?
            .duration_since(std::time::UNIX_EPOCH).ok()?
            .as_nanos();
        Some(format!("{}:{}:{}", abs_path.display(), sz, mtime))
    }
}

pub fn load_cache() {
    let mut lock = CACHE.lock().unwrap();
    if lock.is_some() {
        return;
    }

    let mut map = HashMap::new();
    if let Some(cpath) = get_cache_file_path() {
        if let Ok(content) = fs::read_to_string(&cpath) {
            if let Ok(Value::Object(obj)) = serde_json::from_str(&content) {
                for (k, v) in obj {
                    map.insert(k, v);
                }
            }
        }
    }
    *lock = Some(map);
}

pub fn get_cached(path: &Path) -> Option<Value> {
    load_cache();
    let key = make_cache_key(path)?;
    let lock = CACHE.lock().unwrap();
    lock.as_ref()?.get(&key).cloned()
}

pub fn put_cached(path: &Path, val: Value) {
    load_cache();
    let Some(key) = make_cache_key(path) else { return };
    {
        let mut lock = CACHE.lock().unwrap();
        if let Some(ref mut map) = *lock {
            map.insert(key, val);
        }
    }
    save_cache();
}

pub fn drop_cached(path: &Path) {
    load_cache();
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(d) => d.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    let prefix = format!("{}:", abs_path.display());
    let mut changed = false;
    {
        let mut lock = CACHE.lock().unwrap();
        if let Some(ref mut map) = *lock {
            map.retain(|k, _| {
                let remove = k.starts_with(&prefix);
                if remove {
                    changed = true;
                }
                !remove
            });
        }
    }
    if changed {
        save_cache();
    }
}

pub fn save_cache() {
    let Some(cpath) = get_cache_file_path() else { return };
    let lock = CACHE.lock().unwrap();
    let Some(ref map) = *lock else { return };

    let tmp = format!("{}.tmp.{}", cpath.display(), std::process::id());
    let tmp_path = Path::new(&tmp);

    if let Ok(mut f) = File::create(tmp_path) {
        if serde_json::to_writer(&mut f, map).is_ok() && f.flush().is_ok() {
            drop(f);
            let _ = fs::rename(tmp_path, &cpath);
        } else {
            let _ = fs::remove_file(tmp_path);
        }
    }
}
