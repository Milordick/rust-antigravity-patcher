//! Background update checking and release browser launcher.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;

pub const GITHUB_REPO: &str = "Milordick/rust-antigravity-patcher";
pub const RELEASES_URL: &str = "https://github.com/Milordick/rust-antigravity-patcher/releases";
pub const PROJECT_REPO_URL: &str = "https://github.com/Milordick/rust-antigravity-patcher";
pub const UPSTREAM_GITLAB: &str = "https://gitlab.com/avencores/open-antigravity-patcher";
pub const UPSTREAM_SOURCEFORGE: &str = "https://sourceforge.net/projects/open-antigravity-patcher/";
pub const CURRENT_VERSION: &str = "1.4.1";

#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Checking,
    UpToDate,
    Available(String, String), // tag, url
    Error(String),
}

static UPDATE_STATUS: Mutex<Option<UpdateStatus>> = Mutex::new(None);
static HAS_STARTED: AtomicBool = AtomicBool::new(false);

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
