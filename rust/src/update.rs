//! Background update checking, release browser launcher, and atomic self-updater.

use std::fs;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;

use crate::core::arch::{detect_arch_of_file, Arch};

pub const GITHUB_REPO: &str = "Milordick/rust-antigravity-patcher";
pub const RELEASES_URL: &str = "https://github.com/Milordick/rust-antigravity-patcher/releases";
pub const PROJECT_REPO_URL: &str = "https://github.com/Milordick/rust-antigravity-patcher";
pub const UPSTREAM_GITLAB: &str = "https://gitlab.com/avencores/open-antigravity-patcher";
pub const UPSTREAM_SOURCEFORGE: &str = "https://sourceforge.net/projects/open-antigravity-patcher/";
pub const CURRENT_VERSION: &str = "1.4.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Checking,
    UpToDate,
    Available(String, String), // tag, url
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfUpdateState {
    Idle,
    InProgress(String),
    Success(String),
    Failed(String),
}

static UPDATE_STATUS: Mutex<Option<UpdateStatus>> = Mutex::new(None);
static HAS_STARTED: AtomicBool = AtomicBool::new(false);
static SELF_UPDATE_STATE: Mutex<SelfUpdateState> = Mutex::new(SelfUpdateState::Idle);

pub fn start_background_update_check() {
    if HAS_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    *UPDATE_STATUS.lock().unwrap() = Some(UpdateStatus::Checking);

    thread::spawn(|| {
        let status = check_github_release();
        *UPDATE_STATUS.lock().unwrap() = Some(status);
    });
}

pub fn get_update_status() -> UpdateStatus {
    UPDATE_STATUS
        .lock()
        .unwrap()
        .clone()
        .unwrap_or(UpdateStatus::Checking)
}

pub fn get_self_update_state() -> SelfUpdateState {
    SELF_UPDATE_STATE.lock().unwrap().clone()
}

pub fn start_self_update(target_tag: Option<String>) {
    let mut lock = SELF_UPDATE_STATE.lock().unwrap();
    if let SelfUpdateState::InProgress(_) = *lock {
        return;
    }
    *lock = SelfUpdateState::InProgress("Загрузка обновления...".into());
    drop(lock);

    thread::spawn(move || {
        match perform_self_update(target_tag.as_deref()) {
            Ok(msg) => {
                *SELF_UPDATE_STATE.lock().unwrap() = SelfUpdateState::Success(msg);
            }
            Err(e) => {
                *SELF_UPDATE_STATE.lock().unwrap() = SelfUpdateState::Failed(e);
            }
        }
    });
}

pub fn get_asset_name() -> Result<&'static str, String> {
    #[cfg(target_os = "android")]
    {
        return Ok("agyptch-android-arm64");
    }

    #[cfg(target_os = "windows")]
    {
        #[cfg(target_arch = "aarch64")]
        return Ok("agyptch-windows-arm64.exe");
        #[cfg(target_arch = "x86_64")]
        return Ok("agyptch-windows-x86_64.exe");
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        return Err("Неподдерживаемая архитектура Windows".into());
    }

    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        return Ok("agyptch-macos-arm64");
        #[cfg(target_arch = "x86_64")]
        return Ok("agyptch-macos-x86_64");
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        return Err("Неподдерживаемая архитектура macOS".into());
    }

    #[cfg(target_os = "linux")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            let is_termux = std::env::var("PREFIX").map(|p| p.contains("com.termux")).unwrap_or(false)
                || Path::new("/data/data/com.termux").exists();
            if is_termux {
                return Ok("agyptch-android-arm64");
            } else {
                return Ok("agyptch-linux-arm64");
            }
        }

        #[cfg(target_arch = "x86_64")]
        {
            return Ok("agyptch-linux-x86_64");
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        return Err("Неподдерживаемая архитектура Linux".into());
    }

    #[cfg(not(any(target_os = "android", target_os = "windows", target_os = "macos", target_os = "linux")))]
    return Err("Неподдерживаемая операционная система".into());
}

pub fn perform_self_update(target_tag: Option<&str>) -> Result<String, String> {
    let asset_name = get_asset_name()?;
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Не удалось определить путь к текущему файлу: {}", e))?;
    let real_target = fs::canonicalize(&current_exe).unwrap_or(current_exe.clone());

    let download_url = if let Some(tag) = target_tag {
        format!(
            "https://github.com/{}/releases/download/{}/{}",
            GITHUB_REPO, tag, asset_name
        )
    } else {
        format!(
            "https://github.com/{}/releases/latest/download/{}",
            GITHUB_REPO, asset_name
        )
    };

    let tmp_path = format!("{}.upd.{}", real_target.display(), std::process::id());
    let tmp_path = PathBuf::from(tmp_path);

    // Clean up stale file
    let _ = fs::remove_file(&tmp_path);

    let output = std::process::Command::new("curl")
        .args([
            "-4", "-f", "-L",
            "--connect-timeout", "15",
            "--max-time", "90",
            "-H", &format!("User-Agent: OpenAGPatcher/{}", CURRENT_VERSION),
            "-o", tmp_path.to_str().ok_or_else(|| "Некорректный путь к временному файлу".to_string())?,
            &download_url,
        ])
        .output()
        .map_err(|e| format!("Ошибка запуска curl: {}", e))?;

    if !output.status.success() {
        let _ = fs::remove_file(&tmp_path);
        return Err("Ошибка загрузки: проверьте интернет или включите VPN".into());
    }

    // Verify downloaded file size
    let meta = fs::metadata(&tmp_path)
        .map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("Ошибка проверки скачанного файла: {}", e)
        })?;

    if meta.len() < 100_000 {
        let _ = fs::remove_file(&tmp_path);
        return Err("Скачанный файл повреждён или пуст".into());
    }

    // Verify executable binary architecture
    let arch = detect_arch_of_file(&tmp_path);
    if arch == Arch::Other {
        let _ = fs::remove_file(&tmp_path);
        return Err("Скачанный файл не является валидным бинарником".into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755));
    }

    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(&tmp_path, &real_target)
            .map_err(|e| {
                let _ = fs::remove_file(&tmp_path);
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    "Отказано в доступе. Запустите утилиту с sudo (sudo agyptch --update)".into()
                } else {
                    format!("Не удалось заменить файл: {}", e)
                }
            })?;
    }

    #[cfg(target_os = "windows")]
    {
        let old_backup = format!("{}.old.{}", real_target.display(), std::process::id());
        let old_backup = PathBuf::from(old_backup);
        let _ = fs::remove_file(&old_backup);

        fs::rename(&real_target, &old_backup)
            .map_err(|e| {
                let _ = fs::remove_file(&tmp_path);
                format!("Не удалось переименовать файл: {}", e)
            })?;

        if let Err(e) = fs::rename(&tmp_path, &real_target) {
            let _ = fs::rename(&old_backup, &real_target);
            return Err(format!("Не удалось установить новый файл: {}", e));
        }
    }

    Ok(format!("Утилита успешно обновлена! Перезапустите agyptch."))
}

fn check_github_release() -> UpdateStatus {
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time", "4",
            "-H", "User-Agent: OpenAGPatcher/1.4.1",
            "-H", "Accept: application/vnd.github.v3+json",
            "https://api.github.com/repos/Milordick/rust-antigravity-patcher/releases/latest",
        ])
        .output();

    let output = match output {
        Ok(out) if out.status.success() => out.stdout,
        _ => return UpdateStatus::Error("Сеть недоступна или запрос к GitHub отклонен".into()),
    };

    let text = String::from_utf8_lossy(&output);
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&text) {
        let tag = map.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
        let html_url = map.get("html_url").and_then(|v| v.as_str()).unwrap_or(RELEASES_URL);

        let clean_tag = tag.trim_start_matches('v').trim_start_matches('V');
        let current = CURRENT_VERSION.trim_start_matches('v');

        if !clean_tag.is_empty() && clean_tag != current && is_newer(clean_tag, current) {
            return UpdateStatus::Available(tag.to_string(), html_url.to_string());
        }
        return UpdateStatus::UpToDate;
    }

    UpdateStatus::Error("Не удалось разобрать ответ GitHub API".into())
}

fn is_newer(remote: &str, current: &str) -> bool {
    let r_parts: Vec<u32> = remote.split('.').filter_map(|s| s.parse().ok()).collect();
    let c_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();
    r_parts > c_parts
}

pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/c", "start", url]).spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
