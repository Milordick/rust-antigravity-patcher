//! Google Material Design 3 (M3) TUI for Open AG Patcher (Russian language).
//! Follows Material 3 Dark specifications:
//! - Semantic tonal containers and clean surface elevations
//! - Accessible typography and high-contrast labels
//! - Subtle, non-intrusive breathing animations and progress meters
//! - Interactive, focusable buttons with full keyboard and mouse support
//! - Author: Coshick; Upstream/Original: AvenCores (GitLab & SourceForge)

use std::fs::File;
use std::io::{self, stdout, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Terminal,
};

use crate::core::arch::{detect_arch, Arch};
use crate::core::hash::{file_sha256, format_short_hash};
use crate::targets::agy::{apply_agy_patch, check_agy_status, find_agy_binary, restore_agy};
use crate::targets::ide::{apply_ide_patch, check_ide_status, find_ide_main_js, restore_ide};
use crate::targets::manager::{
    apply_manager_patch, check_manager_status, find_manager_binary, restore_manager,
};
use crate::targets::vscode::{
    apply_vscode_patch, check_vscode_status, find_extension_js,
    find_gemini_antigravity_binary, restore_vscode,
};
use crate::ui::theme::*;
use crate::update::{
    get_update_status, open_in_browser, start_background_update_check, UpdateStatus,
    CURRENT_VERSION, GITHUB_REPO, PROJECT_REPO_URL, RELEASES_URL, UPSTREAM_GITLAB,
    UPSTREAM_SOURCEFORGE,
};

pub const AUTHOR_NAME: &str = "Coshick";
pub const UPSTREAM_AUTHOR: &str = "AvenCores";

// ─────────────────────────────────────────────────────────────────────────────
// TABS & FOCUS
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Targets = 0,
    Paths = 1,
    Logs = 2,
    Updates = 3,
    About = 4,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Targets,
        Tab::Paths,
        Tab::Logs,
        Tab::Updates,
        Tab::About,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Tab::Targets => "1. Компоненты",
            Tab::Paths => "2. Настройка путей",
            Tab::Logs => "3. Журнал событий",
            Tab::Updates => "4. Обновления",
            Tab::About => "5. О программе",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Tab::Targets => "1.Цели",
            Tab::Paths => "2.Пути",
            Tab::Logs => "3.Логи",
            Tab::Updates => "4.Обнов",
            Tab::About => "5.Инфо",
        }
    }

    pub fn tiny_name(&self) -> &'static str {
        match self {
            Tab::Targets => "1",
            Tab::Paths => "2",
            Tab::Logs => "3",
            Tab::Updates => "4",
            Tab::About => "5",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus {
    Main,
    Buttons(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modal {
    None,
    PathInput(usize),
}

// ─────────────────────────────────────────────────────────────────────────────
// DATA TYPES
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TargetItem {
    pub name: &'static str,
    pub tag: &'static str,
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub is_patched: bool,
    pub file_size: u64,
    pub version: Option<String>,
    pub arch: Option<Arch>,
    pub backup_found: bool,
    pub backup_size: u64,
    pub sha256: Option<String>,
    pub desc: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Clone, Debug)]
pub struct LogItem {
    pub time: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAction {
    SelectTab(usize),
    SelectTarget(usize),
    SelectPathSlot(usize),
    PatchSelected,
    RestoreSelected,
    PatchAll,
    SwitchToPaths,
    Refresh,
    EditPath(usize),
    ResetPath(usize),
    AutoDetectPaths,
    ClearLogs,
    OpenRelease,
    CheckUpdates,
    OpenProjectRepo,
    OpenGitLabRepo,
    OpenSourceForgeRepo,
}

#[derive(Clone, Debug)]
pub struct ClickableArea {
    pub rect: Rect,
    pub action: AppAction,
}

// ─────────────────────────────────────────────────────────────────────────────
// APPLICATION STATE
// ─────────────────────────────────────────────────────────────────────────────

pub struct AppState {
    pub current_tab: Tab,
    pub focus: PaneFocus,
    pub selected_idx: usize,
    pub selected_path_slot: usize,
    pub targets: Vec<TargetItem>,
    pub custom_paths: [Option<PathBuf>; 5],
    pub logs: Vec<LogItem>,
    pub log_scroll: usize,
    pub modal: Modal,
    pub input_buf: String,
    pub last_message: (String, bool), // (text, is_success)
    pub tick: usize,
    pub clickables: Vec<ClickableArea>,
}

impl AppState {
    pub fn new() -> Self {
        let mut app = Self {
            current_tab: Tab::Targets,
            focus: PaneFocus::Main,
            selected_idx: 0,
            selected_path_slot: 0,
            targets: Vec::new(),
            custom_paths: [None, None, None, None, None],
            logs: Vec::new(),
            log_scroll: 0,
            modal: Modal::None,
            input_buf: String::new(),
            last_message: (
                "Готов к работе. Выберите компонент для патчинга.".into(),
                true,
            ),
            tick: 0,
            clickables: Vec::new(),
        };

        app.log(LogLevel::Info, "Инициализация Open AG Patcher (Rust Edition)");
        app.refresh();
        app
    }

    pub fn log(&mut self, level: LogLevel, msg: impl Into<String>) {
        let text = msg.into();
        let time = current_time_str();
        let is_ok = level == LogLevel::Info || level == LogLevel::Success;

        self.last_message = (format!("[{}] {}", time, text), is_ok);
        self.logs.push(LogItem {
            time,
            level,
            message: text,
        });

        if self.logs.len() > 12 {
            self.log_scroll = self.logs.len().saturating_sub(12);
        }
    }

    pub fn refresh(&mut self) {
        let ide_p = self.custom_paths[0].clone().or_else(find_ide_main_js);
        let mgr_p = self.custom_paths[1].clone().or_else(find_manager_binary);
        let agy_p = self.custom_paths[2].clone().or_else(find_agy_binary);
        let vs_p = self.custom_paths[3].clone().or_else(find_extension_js);
        let vagy_p = self.custom_paths[4]
            .clone()
            .or_else(find_gemini_antigravity_binary);

        let mut items = Vec::new();

        // 1. IDE
        if let Some(ref p) = ide_p {
            let st = check_ide_status(p);
            let (bak_found, bak_sz) = check_backup_file(p, ".agybak");
            let sha256 = file_sha256(p);
            items.push(TargetItem {
                name: "Antigravity IDE",
                tag: "ide",
                path: Some(p.clone()),
                exists: st.exists,
                is_patched: st.is_patched,
                file_size: st.file_size,
                version: st.version,
                arch: None,
                backup_found: bak_found,
                backup_size: bak_sz,
                sha256,
                desc: "Редактор кода (снятие блокировки Google в main.js и очистка v8-cache)",
            });
        } else {
            items.push(TargetItem {
                name: "Antigravity IDE",
                tag: "ide",
                path: None,
                exists: false,
                is_patched: false,
                file_size: 0,
                version: None,
                arch: None,
                backup_found: false,
                backup_size: 0,
                sha256: None,
                desc: "Редактор кода (файл main.js не найден)",
            });
        }

        // 2. Manager
        if let Some(ref p) = mgr_p {
            let st = check_manager_status(p);
            let (bak_found, bak_sz) = check_backup_file(p, ".agybak");
            let arch = read_binary_arch(p);
            let sha256 = file_sha256(p);
            items.push(TargetItem {
                name: "Antigravity 2.0 (Сервер)",
                tag: "mgr",
                path: Some(p.clone()),
                exists: st.exists,
                is_patched: st.is_patched,
                file_size: st.file_size,
                version: None,
                arch,
                backup_found: bak_found,
                backup_size: bak_sz,
                sha256,
                desc: "Сервер языковых моделей language_server (патч гейта hasValidAuth)",
            });
        } else {
            items.push(TargetItem {
                name: "Antigravity 2.0 (Сервер)",
                tag: "mgr",
                path: None,
                exists: false,
                is_patched: false,
                file_size: 0,
                version: None,
                arch: None,
                backup_found: false,
                backup_size: 0,
                sha256: None,
                desc: "Сервер language_server не найден в системе",
            });
        }

        // 3. CLI
        if let Some(ref p) = agy_p {
            let st = check_agy_status(p);
            let (bak_found, bak_sz) = check_backup_file(p, ".agybak");
            let arch = read_binary_arch(p);
            let sha256 = file_sha256(p);
            items.push(TargetItem {
                name: "Консольная утилита agy",
                tag: "agy",
                path: Some(p.clone()),
                exists: st.exists,
                is_patched: st.is_patched,
                file_size: st.file_size,
                version: None,
                arch,
                backup_found: bak_found,
                backup_size: bak_sz,
                sha256,
                desc: "CLI-инструмент agy (обход проверки региона, Eligibility и токена)",
            });
        } else {
            items.push(TargetItem {
                name: "Консольная утилита agy",
                tag: "agy",
                path: None,
                exists: false,
                is_patched: false,
                file_size: 0,
                version: None,
                arch: None,
                backup_found: false,
                backup_size: 0,
                sha256: None,
                desc: "Бинарник agy не найден в PATH или ~/.local/bin",
            });
        }

        // 4. VS Code Extension
        if let Some(ref p) = vs_p {
            let st = check_vscode_status(p);
            let (bak_found, bak_sz) = check_backup_file(p, ".vscodebak");
            let sha256 = file_sha256(p);
            items.push(TargetItem {
                name: "Расширение для VS Code",
                tag: "vsc",
                path: Some(p.clone()),
                exists: st.exists,
                is_patched: st.is_patched,
                file_size: st.file_size,
                version: None,
                arch: None,
                backup_found: bak_found,
                backup_size: bak_sz,
                sha256,
                desc: "extension.js (блокировка авто-перезагрузки и смены каналов)",
            });
        } else {
            items.push(TargetItem {
                name: "Расширение для VS Code",
                tag: "vsc",
                path: None,
                exists: false,
                is_patched: false,
                file_size: 0,
                version: None,
                arch: None,
                backup_found: false,
                backup_size: 0,
                sha256: None,
                desc: "Расширение google.google-antigravity не найдено",
            });
        }

        // 5. VS Code Binary
        if let Some(ref p) = vagy_p {
            let st = check_agy_status(p);
            let (bak_found, bak_sz) = check_backup_file(p, ".agybak");
            let arch = read_binary_arch(p);
            let sha256 = file_sha256(p);
            items.push(TargetItem {
                name: "Бинарник VS Code (agy)",
                tag: "vagy",
                path: Some(p.clone()),
                exists: st.exists,
                is_patched: st.is_patched,
                file_size: st.file_size,
                version: None,
                arch,
                backup_found: bak_found,
                backup_size: bak_sz,
                sha256,
                desc: "Файл ~/.gemini/bin/antigravity, запускаемый плагином VS Code",
            });
        } else {
            items.push(TargetItem {
                name: "Бинарник VS Code (agy)",
                tag: "vagy",
                path: None,
                exists: false,
                is_patched: false,
                file_size: 0,
                version: None,
                arch: None,
                backup_found: false,
                backup_size: 0,
                sha256: None,
                desc: "Файл ~/.gemini/bin/antigravity не найден",
            });
        }

        self.targets = items;
    }

    pub fn patch_selected(&mut self) {
        if self.selected_idx >= self.targets.len() {
            return;
        }
        let item = &self.targets[self.selected_idx];
        let name = item.name;
        if let Some(ref p) = item.path {
            let hash_before = file_sha256(p);
            let res = match item.tag {
                "ide" => apply_ide_patch(p),
                "mgr" => apply_manager_patch(p),
                "agy" | "vagy" => apply_agy_patch(p),
                "vsc" => apply_vscode_patch(p),
                _ => Ok(false),
            };
            let hash_after = file_sha256(p);
            match res {
                Ok(true) => {
                    let short_b = hash_before.as_deref().map(format_short_hash).unwrap_or_else(|| "—".into());
                    let short_a = hash_after.as_deref().map(format_short_hash).unwrap_or_else(|| "—".into());
                    self.log(LogLevel::Success, format!("Патч успешно применён: {}", name));
                    if let (Some(b), Some(a)) = (hash_before, hash_after) {
                        self.log(LogLevel::Info, format!("  SHA-256 (до):    {}", b));
                        self.log(LogLevel::Info, format!("  SHA-256 (после): {}", a));
                        self.log(LogLevel::Info, format!("  Контрольная сумма: {} ➔ {}", short_b, short_a));
                    }
                }
                Ok(false) => {
                    let short_cur = hash_after.as_deref().map(format_short_hash).unwrap_or_else(|| "—".into());
                    self.log(LogLevel::Info, format!("{} уже пропатчен (SHA-256: {}).", name, short_cur));
                }
                Err(e) => {
                    self.log(LogLevel::Error, format!("Ошибка патчинга {}: {}", name, e));
                }
            }
        } else {
            self.log(LogLevel::Warn, format!("Файл для '{}' не найден.", name));
        }
        self.refresh();
    }

    pub fn restore_selected(&mut self) {
        if self.selected_idx >= self.targets.len() {
            return;
        }
        let item = &self.targets[self.selected_idx];
        let name = item.name;
        if let Some(ref p) = item.path {
            let hash_before = file_sha256(p);
            let res = match item.tag {
                "ide" => restore_ide(p),
                "mgr" => restore_manager(p),
                "agy" | "vagy" => restore_agy(p),
                "vsc" => restore_vscode(p),
                _ => Ok(false),
            };
            let hash_after = file_sha256(p);
            match res {
                Ok(true) => {
                    let short_a = hash_after.as_deref().map(format_short_hash).unwrap_or_else(|| "—".into());
                    self.log(LogLevel::Success, format!("Восстановлено из бэкапа: {}", name));
                    if let Some(ref a) = hash_after {
                        self.log(LogLevel::Info, format!("  SHA-256 (оригинал): {}", a));
                    }
                    if let (Some(ref b), Some(_)) = (hash_before, hash_after) {
                        self.log(LogLevel::Info, format!("  Контрольная сумма: {} ➔ {}", format_short_hash(b), short_a));
                    }
                }
                Ok(false) => {
                    self.log(LogLevel::Warn, format!("Резервная копия для {} не найдена.", name));
                }
                Err(e) => {
                    self.log(LogLevel::Error, format!("Ошибка отката {}: {}", name, e));
                }
            }
        } else {
            self.log(LogLevel::Warn, format!("Файл для '{}' не найден.", name));
        }
        self.refresh();
    }

    pub fn patch_all(&mut self) {
        let mut count = 0;
        for i in 0..self.targets.len() {
            let t = &self.targets[i];
            if let Some(ref p) = t.path {
                let hash_before = file_sha256(p);
                let ok = match t.tag {
                    "ide" => apply_ide_patch(p).unwrap_or(false),
                    "mgr" => apply_manager_patch(p).unwrap_or(false),
                    "agy" | "vagy" => apply_agy_patch(p).unwrap_or(false),
                    "vsc" => apply_vscode_patch(p).unwrap_or(false),
                    _ => false,
                };
                if ok {
                    count += 1;
                    let hash_after = file_sha256(p);
                    let short_b = hash_before.as_deref().map(format_short_hash).unwrap_or_else(|| "—".into());
                    let short_a = hash_after.as_deref().map(format_short_hash).unwrap_or_else(|| "—".into());
                    self.log(
                        LogLevel::Success,
                        format!("  [+] {} пропатчен (SHA-256: {} ➔ {})", t.name, short_b, short_a),
                    );
                }
            }
        }
        self.log(
            LogLevel::Success,
            format!("Пакетный патчинг завершён: обновлено целей: {}.", count),
        );
        self.refresh();
    }

    pub fn execute_action(&mut self, action: AppAction) {
        match action {
            AppAction::SelectTab(idx) => {
                if idx < Tab::ALL.len() {
                    self.current_tab = Tab::ALL[idx];
                    self.focus = PaneFocus::Main;
                }
            }
            AppAction::SelectTarget(idx) => {
                if idx < self.targets.len() {
                    self.selected_idx = idx;
                }
            }
            AppAction::SelectPathSlot(idx) => {
                if idx < 5 {
                    self.selected_path_slot = idx;
                }
            }
            AppAction::PatchSelected => self.patch_selected(),
            AppAction::RestoreSelected => self.restore_selected(),
            AppAction::PatchAll => self.patch_all(),
            AppAction::SwitchToPaths => {
                self.current_tab = Tab::Paths;
                self.focus = PaneFocus::Main;
            }
            AppAction::Refresh => {
                self.refresh();
                self.log(LogLevel::Info, "Состояние компонентов обновлено.");
            }
            AppAction::EditPath(idx) => {
                let init = self.custom_paths[idx]
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.input_buf = init;
                self.modal = Modal::PathInput(idx);
            }
            AppAction::ResetPath(idx) => {
                self.custom_paths[idx] = None;
                self.refresh();
                self.log(
                    LogLevel::Info,
                    format!("Сброшен путь для слота #{}.", idx + 1),
                );
            }
            AppAction::AutoDetectPaths => {
                self.custom_paths = [None, None, None, None, None];
                self.refresh();
                self.log(
                    LogLevel::Info,
                    "Все пути сброшены на автоматическое определение.",
                );
            }
            AppAction::ClearLogs => {
                self.logs.clear();
                self.log(LogLevel::Info, "Журнал событий очищен.");
            }
            AppAction::OpenRelease => open_in_browser(RELEASES_URL),
            AppAction::CheckUpdates => {
                start_background_update_check();
                self.log(LogLevel::Info, "Запрошена проверка обновлений на GitHub.");
            }
            AppAction::OpenProjectRepo => open_in_browser(PROJECT_REPO_URL),
            AppAction::OpenGitLabRepo => open_in_browser(UPSTREAM_GITLAB),
            AppAction::OpenSourceForgeRepo => open_in_browser(UPSTREAM_SOURCEFORGE),
        }
    }

    pub fn handle_click(&mut self, col: u16, row: u16) {
        if let Modal::PathInput(_) = self.modal {
            return;
        }

        for c in self.clickables.iter().rev() {
            if col >= c.rect.x
                && col < c.rect.x + c.rect.width
                && row >= c.rect.y
                && row < c.rect.y + c.rect.height
            {
                let act = c.action;
                self.execute_action(act);
                break;
            }
        }
    }

    pub fn scroll_up(&mut self) {
        match self.current_tab {
            Tab::Targets => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                }
            }
            Tab::Paths => {
                if self.selected_path_slot > 0 {
                    self.selected_path_slot -= 1;
                }
            }
            Tab::Logs => {
                if self.log_scroll > 0 {
                    self.log_scroll -= 1;
                }
            }
            _ => {}
        }
    }

    pub fn scroll_down(&mut self) {
        match self.current_tab {
            Tab::Targets => {
                if self.selected_idx + 1 < self.targets.len() {
                    self.selected_idx += 1;
                }
            }
            Tab::Paths => {
                if self.selected_path_slot + 1 < 5 {
                    self.selected_path_slot += 1;
                }
            }
            Tab::Logs => {
                if self.log_scroll + 1 < self.logs.len() {
                    self.log_scroll += 1;
                }
            }
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MAIN ENTRY POINT & EVENT LOOP
// ─────────────────────────────────────────────────────────────────────────────

pub fn run_tui() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    start_background_update_check();

    let mut state = AppState::new();
    let res = main_loop(&mut terminal, &mut state);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn main_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut AppState,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(80);
    let mut last_tick = Instant::now();

    loop {
        state.tick += 1;
        state.clickables.clear();

        terminal.draw(|f| draw_ui(f, state))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            let mut should_exit = false;
            let mut needs_clear = false;

            // Drain all immediately available events in the queue to avoid backlog
            while event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                            if handle_key_input(key.code, state) {
                                should_exit = true;
                                break;
                            }
                        }
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            state.handle_click(mouse.column, mouse.row);
                        }
                        MouseEventKind::ScrollDown => {
                            state.scroll_down();
                        }
                        MouseEventKind::ScrollUp => {
                            state.scroll_up();
                        }
                        _ => {}
                    },
                    Event::Resize(_, _) => {
                        needs_clear = true;
                    }
                    _ => {}
                }
            }

            if needs_clear {
                terminal.clear()?;
            }

            if should_exit {
                break;
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

pub fn handle_key_input(code: KeyCode, state: &mut AppState) -> bool {
    // 1. Modal Input Handling
    if let Modal::PathInput(idx) = state.modal {
        match code {
            KeyCode::Esc => {
                state.modal = Modal::None;
            }
            KeyCode::Enter => {
                let trimmed = state.input_buf.trim().to_string();
                if !trimmed.is_empty() {
                    let p = PathBuf::from(&trimmed);
                    state.custom_paths[idx] = Some(p.clone());
                    state.log(
                        LogLevel::Success,
                        format!("Сохранён путь слота #{}: {}", idx + 1, p.display()),
                    );
                    state.refresh();
                }
                state.modal = Modal::None;
            }
            KeyCode::Backspace => {
                state.input_buf.pop();
            }
            KeyCode::Char(c) => {
                state.input_buf.push(c);
            }
            _ => {}
        }
        return false;
    }

    // 2. Global Quit
    if matches!(code, KeyCode::Char('q') | KeyCode::Char('Q')) {
        return true;
    }

    // 3. Tab Number Direct Switching: 1..5
    match code {
        KeyCode::Char('1') => {
            state.execute_action(AppAction::SelectTab(0));
            return false;
        }
        KeyCode::Char('2') => {
            state.execute_action(AppAction::SelectTab(1));
            return false;
        }
        KeyCode::Char('3') => {
            state.execute_action(AppAction::SelectTab(2));
            return false;
        }
        KeyCode::Char('4') => {
            state.execute_action(AppAction::SelectTab(3));
            return false;
        }
        KeyCode::Char('5') => {
            state.execute_action(AppAction::SelectTab(4));
            return false;
        }
        _ => {}
    }

    // 4. Global Action Shortcuts
    match code {
        KeyCode::Char('p') | KeyCode::Char('P') => {
            state.patch_selected();
            return false;
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            match state.current_tab {
                Tab::Targets => state.restore_selected(),
                Tab::Paths => state.execute_action(AppAction::AutoDetectPaths),
                Tab::Logs => state.refresh(),
                Tab::Updates => state.execute_action(AppAction::CheckUpdates),
                Tab::About => {}
            }
            return false;
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            state.patch_all();
            return false;
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            if state.current_tab == Tab::About {
                state.execute_action(AppAction::OpenSourceForgeRepo);
            } else {
                state.execute_action(AppAction::SelectTab(1));
            }
            return false;
        }
        KeyCode::Char('g') | KeyCode::Char('G') => {
            if state.current_tab == Tab::About {
                state.execute_action(AppAction::OpenGitLabRepo);
                return false;
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            state.execute_action(AppAction::SelectTab(2));
            return false;
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            state.execute_action(AppAction::SelectTab(3));
            return false;
        }
        KeyCode::Char('m') | KeyCode::Char('M') | KeyCode::Char('?') | KeyCode::Char('h') => {
            state.execute_action(AppAction::SelectTab(4));
            return false;
        }
        KeyCode::F(5) => {
            state.execute_action(AppAction::Refresh);
            return false;
        }
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
            if state.current_tab == Tab::Paths {
                state.execute_action(AppAction::ResetPath(state.selected_path_slot));
                return false;
            }
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if state.current_tab == Tab::Logs {
                state.execute_action(AppAction::ClearLogs);
                return false;
            }
        }
        KeyCode::Char('o') | KeyCode::Char('O') => {
            if state.current_tab == Tab::Updates {
                state.execute_action(AppAction::OpenRelease);
                return false;
            } else if state.current_tab == Tab::About {
                state.execute_action(AppAction::OpenProjectRepo);
                return false;
            }
        }
        _ => {}
    }

    // 5. Navigation & Focus Switching
    match code {
        KeyCode::Tab => {
            match state.focus {
                PaneFocus::Main => state.focus = PaneFocus::Buttons(0),
                PaneFocus::Buttons(_) => state.focus = PaneFocus::Main,
            }
        }
        KeyCode::BackTab => {
            match state.focus {
                PaneFocus::Main => {
                    let max_btn = get_button_count_for_tab(state.current_tab);
                    state.focus = PaneFocus::Buttons(max_btn.saturating_sub(1));
                }
                PaneFocus::Buttons(_) => state.focus = PaneFocus::Main,
            }
        }
        KeyCode::Esc => {
            if matches!(state.focus, PaneFocus::Buttons(_)) {
                state.focus = PaneFocus::Main;
            } else {
                return true;
            }
        }
        _ => {}
    }

    // 6. Focus-specific controls
    match state.focus {
        PaneFocus::Main => {
            match state.current_tab {
                Tab::Targets => match code {
                    KeyCode::Up | KeyCode::Char('k') => state.scroll_up(),
                    KeyCode::Down | KeyCode::Char('j') => state.scroll_down(),
                    KeyCode::Enter => state.patch_selected(),
                    _ => {}
                },
                Tab::Paths => match code {
                    KeyCode::Up | KeyCode::Char('k') => state.scroll_up(),
                    KeyCode::Down | KeyCode::Char('j') => state.scroll_down(),
                    KeyCode::Enter | KeyCode::Char('e') => {
                        state.execute_action(AppAction::EditPath(state.selected_path_slot));
                    }
                    _ => {}
                },
                Tab::Logs => match code {
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::PageUp => state.scroll_up(),
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::PageDown => state.scroll_down(),
                    _ => {}
                },
                Tab::Updates => {
                    if code == KeyCode::Enter {
                        state.execute_action(AppAction::OpenRelease);
                    }
                }
                Tab::About => {
                    if code == KeyCode::Enter {
                        state.execute_action(AppAction::OpenProjectRepo);
                    }
                }
            }
        }
        PaneFocus::Buttons(btn_idx) => {
            let max_btn = get_button_count_for_tab(state.current_tab);
            match code {
                KeyCode::Left | KeyCode::Char('h') => {
                    if btn_idx > 0 {
                        state.focus = PaneFocus::Buttons(btn_idx - 1);
                    } else {
                        state.focus = PaneFocus::Buttons(max_btn.saturating_sub(1));
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if btn_idx + 1 < max_btn {
                        state.focus = PaneFocus::Buttons(btn_idx + 1);
                    } else {
                        state.focus = PaneFocus::Buttons(0);
                    }
                }
                KeyCode::Up => {
                    state.focus = PaneFocus::Main;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    execute_focused_button(state, btn_idx);
                }
                _ => {}
            }
        }
    }

    false
}

fn get_button_count_for_tab(tab: Tab) -> usize {
    match tab {
        Tab::Targets => 3,
        Tab::Paths => 3,
        Tab::Logs => 2,
        Tab::Updates => 2,
        Tab::About => 3,
    }
}

fn execute_focused_button(state: &mut AppState, btn_idx: usize) {
    match state.current_tab {
        Tab::Targets => match btn_idx {
            0 => state.execute_action(AppAction::PatchSelected),
            1 => state.execute_action(AppAction::RestoreSelected),
            2 => state.execute_action(AppAction::PatchAll),
            _ => {}
        },
        Tab::Paths => match btn_idx {
            0 => state.execute_action(AppAction::EditPath(state.selected_path_slot)),
            1 => state.execute_action(AppAction::ResetPath(state.selected_path_slot)),
            2 => state.execute_action(AppAction::AutoDetectPaths),
            _ => {}
        },
        Tab::Logs => match btn_idx {
            0 => state.execute_action(AppAction::ClearLogs),
            1 => state.execute_action(AppAction::Refresh),
            _ => {}
        },
        Tab::Updates => match btn_idx {
            0 => state.execute_action(AppAction::OpenRelease),
            1 => state.execute_action(AppAction::CheckUpdates),
            _ => {}
        },
        Tab::About => match btn_idx {
            0 => state.execute_action(AppAction::OpenProjectRepo),
            1 => state.execute_action(AppAction::OpenGitLabRepo),
            2 => state.execute_action(AppAction::OpenSourceForgeRepo),
            _ => {}
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RENDER PIPELINE (MATERIAL 3)
// ─────────────────────────────────────────────────────────────────────────────

pub fn draw_ui(f: &mut ratatui::Frame, state: &mut AppState) {
    let area = f.area();

    // Responsive guard for ultra-small terminals (< 28x7)
    if area.width < 28 || area.height < 7 {
        let msg = vec![
            Line::from(Span::styled(
                "Терминал слишком мал",
                Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD),
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Размер: ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(format!("{}x{}", area.width, area.height), Style::default().fg(MD_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled("Мин: ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled("28x7", Style::default().fg(MD_TEXT_HIGH)),
            ]),
        ];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BORDER_ROUNDED)
            .border_style(style_border_normal());
        f.render_widget(Paragraph::new(msg).block(block).alignment(Alignment::Center), area);
        return;
    }

    // Material Design 3 Responsive Layout:
    // When height < 22 (common in Termux / mobile screens with keyboard):
    // Use 1-row compact header, 1-row compact tabs bar, 1-row footer.
    // When height >= 22 (desktop / large terminal):
    // Use 3 rows with rounded cards for header, tabs, and footer.
    let (header_h, tabs_h, footer_h) = if area.height < 22 {
        (1, 1, 1)
    } else {
        (3, 3, 3)
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Length(tabs_h),
            Constraint::Min(4),
            Constraint::Length(footer_h),
        ])
        .split(area);

    draw_header(f, rows[0], state);
    draw_tabs_bar(f, rows[1], state);

    match state.current_tab {
        Tab::Targets => draw_tab_targets(f, rows[2], state),
        Tab::Paths => draw_tab_paths(f, rows[2], state),
        Tab::Logs => draw_tab_logs(f, rows[2], state),
        Tab::Updates => draw_tab_updates(f, rows[2], state),
        Tab::About => draw_tab_about(f, rows[2], state),
    }

    draw_footer(f, rows[3]);

    if let Modal::PathInput(idx) = state.modal {
        draw_modal_path_input(f, area, idx, state);
    }
}

fn draw_header(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_compact = area.height < 3;
    let inner = if is_compact {
        area
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BORDER_ROUNDED)
            .border_style(style_border_normal());
        let in_rect = block.inner(area);
        f.render_widget(block, area);
        in_rect
    };

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if inner.width < 45 {
        // Ultra-narrow (Termux portrait / small screen)
        let left_spans = vec![
            Span::styled("Open AG Patcher", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
        ];
        let right_spans = vec![
            Span::styled("● ", Style::default().fg(get_breathing_color(state.tick))),
            Span::styled(format!("v{}", CURRENT_VERSION), Style::default().fg(MD_SECONDARY)),
        ];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(16),
                Constraint::Length(9),
            ])
            .split(inner);
        f.render_widget(Paragraph::new(Line::from(left_spans)), cols[0]);
        f.render_widget(Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right), cols[1]);
    } else if inner.width < 75 {
        // Medium narrow
        let left_spans = vec![
            Span::styled("Open AG Patcher ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(format!("v{} ", CURRENT_VERSION), Style::default().fg(MD_SECONDARY)),
            Span::styled("• Rust", Style::default().fg(MD_TEXT_MED)),
        ];
        let right_spans = vec![
            Span::styled("● ", Style::default().fg(get_breathing_color(state.tick))),
            Span::styled("Активен", Style::default().fg(MD_TEXT_HIGH)),
        ];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(24),
                Constraint::Length(12),
            ])
            .split(inner);
        f.render_widget(Paragraph::new(Line::from(left_spans)), cols[0]);
        f.render_widget(Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right), cols[1]);
    } else {
        // Full desktop width
        let time_str = current_time_str();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(30),
                Constraint::Length(22),
            ])
            .split(inner);

        let left_spans = vec![
            Span::styled(" Open AG Patcher ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(format!("v{} ", CURRENT_VERSION), Style::default().fg(MD_SECONDARY)),
            Span::styled("• Rust Native ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("• Разблокировка региона Antigravity", Style::default().fg(MD_TEXT_LOW)),
        ];
        f.render_widget(Paragraph::new(Line::from(left_spans)), cols[0]);

        let right_spans = vec![
            Span::styled("● ", Style::default().fg(get_breathing_color(state.tick))),
            Span::styled("Активен  ", Style::default().fg(MD_TEXT_HIGH)),
            Span::styled(format!("[ {} ] ", time_str), Style::default().fg(MD_TEXT_LOW)),
        ];
        f.render_widget(Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right), cols[1]);
    }
}

fn draw_tabs_bar(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_compact = area.height < 3;
    let inner = if is_compact {
        area
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BORDER_ROUNDED)
            .border_style(style_border_normal());
        let in_rect = block.inner(area);
        f.render_widget(block, area);
        in_rect
    };

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let tab_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(inner);

    for (i, &t) in Tab::ALL.iter().enumerate() {
        let is_active = t == state.current_tab;
        let cell_rect = tab_cols[i];

        state.clickables.push(ClickableArea {
            rect: cell_rect,
            action: AppAction::SelectTab(i),
        });

        let label = if inner.width < 45 {
            if is_active {
                format!("[{}]", t.tiny_name())
            } else {
                format!(" {} ", t.tiny_name())
            }
        } else if inner.width < 75 {
            if is_active {
                format!("●{}", t.short_name())
            } else {
                format!(" {}", t.short_name())
            }
        } else {
            if is_active {
                format!("● {}", t.name())
            } else {
                format!("  {}", t.name())
            }
        };

        let style = if is_active {
            style_tab_active()
        } else {
            style_tab_inactive()
        };

        let p = Paragraph::new(Line::from(Span::styled(label, style)))
            .alignment(Alignment::Center);
        f.render_widget(p, cell_rect);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 1: COMPONENTS & ACTIONS
// ─────────────────────────────────────────────────────────────────────────────

fn draw_tab_targets(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    if area.width < 80 {
        // Vertical layout for narrow terminals / mobile / Termux
        let list_h = if area.height < 18 { 6 } else { 7 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(list_h),
                Constraint::Min(6),
            ])
            .split(area);

        draw_targets_list(f, chunks[0], state);
        draw_target_telemetry_card(f, chunks[1], state);
    } else {
        // Horizontal layout for desktop / wide terminals
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Percentage(55),
            ])
            .split(area);

        draw_targets_list(f, cols[0], state);
        draw_target_telemetry_card(f, cols[1], state);
    }
}

fn draw_targets_list(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    let is_focused = state.focus == PaneFocus::Main;
    let block = Block::default()
        .title(Span::styled(" Компоненты ", style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(if is_focused {
            style_border_active()
        } else {
            style_border_normal()
        });

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let show_summary = inner.height > state.targets.len() as u16;
    let chunks = if show_summary {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(state.targets.len() as u16),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(state.targets.len() as u16),
            ])
            .split(inner)
    };

    let items: Vec<ListItem> = state
        .targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_sel = i == state.selected_idx;
            let (badge, badge_style) = if !t.exists {
                if inner.width < 45 {
                    ("—", style_missing())
                } else {
                    ("— Не найден", style_missing())
                }
            } else if t.is_patched {
                if inner.width < 45 {
                    ("● Да", style_patched())
                } else {
                    ("● Пропатчен", style_patched())
                }
            } else {
                if inner.width < 45 {
                    ("○ Нет", style_unpatched())
                } else {
                    ("○ Не пропатчен", style_unpatched())
                }
            };

            let prefix = if is_sel { "› " } else { "  " };
            let name_style = if is_sel {
                Style::default().fg(MD_TEXT_HIGH).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MD_TEXT_MED)
            };

            let max_name_len = if inner.width < 45 {
                (inner.width.saturating_sub(8) as usize).max(6)
            } else {
                (inner.width.saturating_sub(18) as usize).min(26).max(8)
            };
            let short_name = truncate_middle(&t.name, max_name_len);

            let spans = vec![
                Span::styled(prefix, Style::default().fg(MD_PRIMARY)),
                Span::styled(format!("{:<max_name_len$} ", short_name), name_style),
                Span::styled(badge, badge_style),
            ];

            if is_sel {
                ListItem::new(Line::from(spans)).style(Style::default().bg(MD_CONTAINER_HIGH))
            } else {
                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    f.render_widget(List::new(items), chunks[0]);

    let list_height = chunks[0].height as usize;
    for (i, _) in state.targets.iter().enumerate() {
        if i < list_height {
            state.clickables.push(ClickableArea {
                rect: Rect {
                    x: chunks[0].x,
                    y: chunks[0].y + i as u16,
                    width: chunks[0].width,
                    height: 1,
                },
                action: AppAction::SelectTarget(i),
            });
        }
    }

    if show_summary && chunks.len() > 1 {
        let installed = state.targets.iter().filter(|t| t.exists).count();
        let patched = state
            .targets
            .iter()
            .filter(|t| t.exists && t.is_patched)
            .count();

        let mut sum_spans = Vec::new();
        if inner.width < 50 {
            sum_spans.push(Span::styled(
                format!("Уст:{}/{} Патч:{}/{}", installed, state.targets.len(), patched, state.targets.len()),
                Style::default().fg(MD_TEXT_MED),
            ));
        } else {
            let ratio = if !state.targets.is_empty() {
                patched as f64 / state.targets.len() as f64
            } else {
                0.0
            };
            sum_spans.push(Span::styled("Установлено: ", Style::default().fg(MD_TEXT_LOW)));
            sum_spans.push(Span::styled(
                format!("{}/{}  ", installed, state.targets.len()),
                Style::default().fg(MD_PRIMARY),
            ));
            sum_spans.push(Span::styled("Пропатчено: ", Style::default().fg(MD_TEXT_LOW)));
            sum_spans.push(Span::styled(format!("{}/{}  ", patched, state.targets.len()), style_patched()));
            if inner.width >= 65 {
                sum_spans.push(Span::raw("["));
                sum_spans.extend(render_mini_bar(ratio, 8));
                sum_spans.push(Span::raw("]"));
            }
        }

        let sum_block = Block::default()
            .borders(Borders::TOP)
            .border_style(style_border_normal());
        f.render_widget(Paragraph::new(Line::from(sum_spans)).block(sum_block), chunks[1]);
    }
}

fn draw_target_telemetry_card(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(Span::styled(" Сведения о компоненте ", style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(style_border_normal());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.selected_idx >= state.targets.len() || inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),    // Details lines
            Constraint::Length(3), // Action buttons
            Constraint::Length(1), // Notification line
        ])
        .split(inner);

    let item = &state.targets[state.selected_idx];
    let mut lines = Vec::new();

    // Name & status
    let status_badge = if !item.exists {
        Span::styled(if inner.width < 50 { "— Не найден" } else { "— Не установлен" }, style_missing())
    } else if item.is_patched {
        Span::styled(if inner.width < 50 { "● Пропатчен" } else { "● Пропатчен (региональный лок снят)" }, style_patched())
    } else {
        Span::styled(if inner.width < 50 { "○ Не пропатчен" } else { "○ Не пропатчен (активны ограничения)" }, style_unpatched())
    };

    lines.push(Line::from(vec![
        Span::styled("Компонент:    ", Style::default().fg(MD_TEXT_LOW)),
        Span::styled(item.name, Style::default().fg(MD_TEXT_HIGH).add_modifier(Modifier::BOLD)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Состояние:    ", Style::default().fg(MD_TEXT_LOW)),
        status_badge,
    ]));

    // Path
    let raw_path = item
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Файл не найден на диске".into());
    let short_path = truncate_middle(&raw_path, (inner.width.saturating_sub(15) as usize).max(10));

    lines.push(Line::from(vec![
        Span::styled("Путь:         ", Style::default().fg(MD_TEXT_LOW)),
        Span::styled(short_path, Style::default().fg(MD_SECONDARY)),
    ]));

    // Size, Arch & Version
    let sz_str = if item.exists {
        format_bytes_ru(item.file_size)
    } else {
        "—".into()
    };
    let arch_str = item.arch.map(|a| a.as_str()).unwrap_or("—");
    let ver_str = item.version.as_deref().unwrap_or("—");

    if inner.width < 55 {
        lines.push(Line::from(vec![
            Span::styled("Размер: ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(sz_str, Style::default().fg(MD_TEXT_HIGH)),
            Span::styled(" • Архит: ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(arch_str, Style::default().fg(MD_PRIMARY)),
            Span::styled(" • Верс: ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(ver_str, Style::default().fg(MD_SUCCESS)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Размер:       ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(sz_str, Style::default().fg(MD_TEXT_HIGH)),
            Span::raw("    "),
            Span::styled("Архитектура: ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(arch_str, Style::default().fg(MD_PRIMARY)),
            Span::raw("    "),
            Span::styled("Версия: ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(ver_str, Style::default().fg(MD_SUCCESS)),
        ]));
    }

    // SHA-256 Checksum
    let sha_display = if let Some(ref h) = item.sha256 {
        Span::styled(format_short_hash(h), Style::default().fg(MD_SECONDARY))
    } else {
        Span::styled("—", Style::default().fg(MD_TEXT_LOW))
    };
    lines.push(Line::from(vec![
        Span::styled("SHA-256:      ", Style::default().fg(MD_TEXT_LOW)),
        sha_display,
    ]));

    // Backup info
    let bak_span = if item.backup_found {
        if inner.width < 50 {
            Span::styled(format!("Есть ({})", format_bytes_ru(item.backup_size)), style_patched())
        } else {
            Span::styled(format!("Сохранена (.agybak, {})", format_bytes_ru(item.backup_size)), style_patched())
        }
    } else {
        if inner.width < 50 {
            Span::styled("Нет", style_unpatched())
        } else {
            Span::styled("Отсутствует (создастся автоматически)", style_unpatched())
        }
    };
    lines.push(Line::from(vec![
        Span::styled("Бэкап:        ", Style::default().fg(MD_TEXT_LOW)),
        bak_span,
    ]));

    // Description only if height allows
    if inner.height >= 12 {
        lines.push(Line::from(vec![
            Span::styled("Операция:     ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(item.desc, Style::default().fg(MD_TEXT_MED)),
        ]));
    }

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), chunks[0]);

    // Action Buttons
    let focused_btn = match state.focus {
        PaneFocus::Buttons(idx) => Some(idx),
        _ => None,
    };

    let btn_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(chunks[1]);

    let (b_patch, b_restore, b_all) = if inner.width < 50 {
        ("Патч (↵)", "Откат (R)", "Всё (A)")
    } else if inner.width < 70 {
        ("Патч (Enter)", "Откат (R)", "Все (A)")
    } else {
        ("Патчить (Enter)", "Откатить (R)", "Патчить всё (A)")
    };

    render_button(f, btn_row[0], b_patch, focused_btn == Some(0), AppAction::PatchSelected, state);
    render_button(f, btn_row[1], b_restore, focused_btn == Some(1), AppAction::RestoreSelected, state);
    render_button(f, btn_row[2], b_all, focused_btn == Some(2), AppAction::PatchAll, state);

    // Notification line
    let (ref msg_text, is_success) = state.last_message;
    let msg_color = if is_success { MD_SUCCESS } else { MD_ERROR };
    let notif_p = Paragraph::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(msg_color)),
        Span::styled(msg_text, Style::default().fg(msg_color)),
    ]));
    f.render_widget(notif_p, chunks[2]);
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 2: PATH CONFIGURATION
// ─────────────────────────────────────────────────────────────────────────────

fn draw_tab_paths(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(Span::styled(" Настройка путей (Слоты 1-5) ", style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(if state.focus == PaneFocus::Main {
            style_border_active()
        } else {
            style_border_normal()
        });

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(inner);

    let slot_names_full = [
        "1. Antigravity IDE (файл main.js)",
        "2. Antigravity 2.0 (файл language_server)",
        "3. Antigravity CLI (утилита agy)",
        "4. Расширение для VS Code (extension.js)",
        "5. Бинарник VS Code (~/.gemini/bin/antigravity)",
    ];

    let slot_names_short = [
        "1. IDE (main.js)",
        "2. Lang Server",
        "3. CLI (agy)",
        "4. VS Code Ext",
        "5. VS Code Bin",
    ];

    let mut items = Vec::new();
    for i in 0..5 {
        let is_sel = i == state.selected_path_slot;
        let is_custom = state.custom_paths[i].is_some();
        let target_item = state.targets.get(i);
        let path_display = target_item
            .and_then(|t| t.path.as_ref())
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(файл не найден)".into());

        let exists = target_item.map(|t| t.exists).unwrap_or(false);
        let prefix = if is_sel { "› " } else { "  " };

        let badge = if is_custom {
            if inner.width < 50 {
                Span::styled("[Свой]", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(" [Свой путь] ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD))
            }
        } else {
            if inner.width < 50 {
                Span::styled("[Авто]", Style::default().fg(MD_TEXT_LOW))
            } else {
                Span::styled(" [Авто] ", Style::default().fg(MD_TEXT_LOW))
            }
        };

        let status_span = if exists {
            Span::styled(if inner.width < 50 { " ●" } else { " ● Найден " }, style_patched())
        } else {
            Span::styled(if inner.width < 50 { " ○" } else { " ○ Отсутствует " }, style_unpatched())
        };

        let slot_str = if inner.width < 60 {
            slot_names_short[i]
        } else {
            slot_names_full[i]
        };

        let max_slot_w = if inner.width < 50 {
            (inner.width.saturating_sub(14) as usize).max(8)
        } else {
            44
        };

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(MD_PRIMARY)),
            Span::styled(
                format!("{:<max_slot_w$} ", truncate_middle(slot_str, max_slot_w)),
                if is_sel {
                    Style::default().fg(MD_TEXT_HIGH).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MD_TEXT_MED)
                },
            ),
            badge,
            status_span,
        ]);

        let path_w = (inner.width.saturating_sub(8) as usize).max(10);
        let path_line = Line::from(vec![
            Span::raw("     ↳ "),
            Span::styled(truncate_middle(&path_display, path_w), Style::default().fg(MD_SECONDARY)),
        ]);

        if is_sel {
            items.push(ListItem::new(vec![line, path_line]).style(Style::default().bg(MD_CONTAINER_HIGH)));
        } else {
            items.push(ListItem::new(vec![line, path_line]));
        }

        let slot_y = chunks[0].y + (i as u16 * 2);
        if slot_y + 1 < chunks[0].y + chunks[0].height {
            state.clickables.push(ClickableArea {
                rect: Rect {
                    x: chunks[0].x,
                    y: slot_y,
                    width: chunks[0].width,
                    height: 2,
                },
                action: AppAction::SelectPathSlot(i),
            });
        }
    }

    f.render_widget(List::new(items), chunks[0]);

    // Buttons
    let focused_btn = match state.focus {
        PaneFocus::Buttons(idx) => Some(idx),
        _ => None,
    };

    let btn_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(chunks[1]);

    let (b_edit, b_reset, b_auto) = if inner.width < 50 {
        ("Путь (↵)", "Сброс (D)", "Авто (R)")
    } else if inner.width < 70 {
        ("Изменить (↵)", "Сбросить (D)", "Авто (R)")
    } else {
        ("Изменить путь (Enter)", "Сбросить (D)", "Автопоиск всех (R)")
    };

    render_button(f, btn_row[0], b_edit, focused_btn == Some(0), AppAction::EditPath(state.selected_path_slot), state);
    render_button(f, btn_row[1], b_reset, focused_btn == Some(1), AppAction::ResetPath(state.selected_path_slot), state);
    render_button(f, btn_row[2], b_auto, focused_btn == Some(2), AppAction::AutoDetectPaths, state);
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 3: ACTIVITY LOGS
// ─────────────────────────────────────────────────────────────────────────────

fn draw_tab_logs(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(Span::styled(" Журнал событий ", style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(if state.focus == PaneFocus::Main {
            style_border_active()
        } else {
            style_border_normal()
        });

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .split(inner);

    let log_slice = if state.logs.is_empty() {
        vec![ListItem::new("Журнал пуст.")]
    } else {
        let skip_count = state.log_scroll.min(state.logs.len().saturating_sub(1));
        state.logs[skip_count..]
            .iter()
            .map(|l| {
                let (badge, style) = match l.level {
                    LogLevel::Info => if inner.width < 50 { ("[i]", Style::default().fg(MD_PRIMARY)) } else { ("[ ИНФО  ]", Style::default().fg(MD_PRIMARY)) },
                    LogLevel::Success => if inner.width < 50 { ("[✓]", style_patched()) } else { ("[ УСПЕХ ]", style_patched()) },
                    LogLevel::Warn => if inner.width < 50 { ("[?]", style_unpatched()) } else { ("[ ВНИМ  ]", style_unpatched()) },
                    LogLevel::Error => if inner.width < 50 { ("[x]", Style::default().fg(MD_ERROR)) } else { ("[ ОШИБК ]", Style::default().fg(MD_ERROR)) },
                };
                let line = if inner.width < 45 {
                    Line::from(vec![
                        Span::styled(format!("{} ", badge), style),
                        Span::styled(&l.message, Style::default().fg(MD_TEXT_HIGH)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(format!("[{}] ", l.time), Style::default().fg(MD_TEXT_LOW)),
                        Span::styled(format!("{} ", badge), style),
                        Span::styled(&l.message, Style::default().fg(MD_TEXT_HIGH)),
                    ])
                };
                ListItem::new(line)
            })
            .collect()
    };

    f.render_widget(List::new(log_slice), chunks[0]);

    // Buttons
    let focused_btn = match state.focus {
        PaneFocus::Buttons(idx) => Some(idx),
        _ => None,
    };

    let btn_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[1]);

    let (b_clear, b_refresh) = if inner.width < 50 {
        ("Очистить (C)", "Обновить (R)")
    } else {
        ("Очистить журнал (C)", "Обновить статус (R)")
    };

    render_button(f, btn_row[0], b_clear, focused_btn == Some(0), AppAction::ClearLogs, state);
    render_button(f, btn_row[1], b_refresh, focused_btn == Some(1), AppAction::Refresh, state);
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 4: UPDATES
// ─────────────────────────────────────────────────────────────────────────────

fn draw_tab_updates(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(Span::styled(" Проверка обновлений ", style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(style_border_normal());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(inner);

    let status = get_update_status();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Установленная версия: ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(
                format!("v{} (Rust Edition)", CURRENT_VERSION),
                Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Репозиторий проекта:  ", Style::default().fg(MD_TEXT_LOW)),
            Span::styled(truncate_middle(GITHUB_REPO, (inner.width.saturating_sub(24) as usize).max(10)), Style::default().fg(MD_SECONDARY)),
        ]),
        Line::raw(""),
    ];

    match status {
        UpdateStatus::Checking => {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} Запрос релизов на GitHub API...", get_spinner_char(state.tick)),
                    Style::default().fg(MD_WARNING),
                ),
            ]));
        }
        UpdateStatus::UpToDate => {
            lines.push(Line::from(vec![
                Span::styled("● У вас самая актуальная версия.", style_patched()),
            ]));
            if inner.height >= 10 {
                lines.push(Line::from(vec![
                    Span::styled("Бинарные патчи соответствуют последнему официальному релизу.", Style::default().fg(MD_TEXT_MED)),
                ]));
            }
        }
        UpdateStatus::Available(tag, url) => {
            lines.push(Line::from(vec![
                Span::styled("● Новая версия: ", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
                Span::styled(tag, Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Ссылка: ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(truncate_middle(&url, (inner.width.saturating_sub(10) as usize).max(10)), Style::default().fg(MD_SECONDARY)),
            ]));
        }
        UpdateStatus::Error(e) => {
            lines.push(Line::from(vec![
                Span::styled(format!("Ошибка: {}", e), Style::default().fg(MD_ERROR)),
            ]));
            lines.push(Line::from("Подсказка: Включите VPN, если доступ к GitHub API ограничен."));
        }
    }

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), chunks[0]);

    // Buttons
    let focused_btn = match state.focus {
        PaneFocus::Buttons(idx) => Some(idx),
        _ => None,
    };

    let btn_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[1]);

    let (b_releases, b_check) = if inner.width < 55 {
        ("Релизы (↵)", "Проверить (R)")
    } else {
        ("Открыть страницу релизов (Enter)", "Проверить снова (R)")
    };

    render_button(f, btn_row[0], b_releases, focused_btn == Some(0), AppAction::OpenRelease, state);
    render_button(f, btn_row[1], b_check, focused_btn == Some(1), AppAction::CheckUpdates, state);
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 5: ABOUT & CREDITS
// ─────────────────────────────────────────────────────────────────────────────

fn draw_tab_about(f: &mut ratatui::Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(Span::styled(" О программе и справка ", style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(style_border_normal());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if inner.width < 80 {
        // Narrow / mobile layout: single column
        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(inner);

        let about_lines = vec![
            Line::from(vec![
                Span::styled("Open AG Patcher (Rust Edition) ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled(format!("v{}", CURRENT_VERSION), Style::default().fg(MD_SECONDARY)),
            ]),
            Line::from("Сверхбыстрая нативная утилита обхода региональных ограничений."),
            Line::raw(""),
            Line::from(vec![
                Span::styled("• Автор:        ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(AUTHOR_NAME, Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::raw(" (Milordick)"),
            ]),
            Line::from(vec![
                Span::styled("• Upstream:     ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(UPSTREAM_AUTHOR, Style::default().fg(MD_SUCCESS)),
                Span::raw(" (GitLab / SF)"),
            ]),
            Line::from(vec![
                Span::styled("• Управление:   ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled("Табы: 1-5  •  Патч: ↵  •  Откат: R  •  Всё: A", Style::default().fg(MD_TEXT_MED)),
            ]),
        ];

        f.render_widget(Paragraph::new(about_lines).wrap(Wrap { trim: true }), v_chunks[0]);

        let focused_btn = match state.focus {
            PaneFocus::Buttons(idx) => Some(idx),
            _ => None,
        };

        let btn_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(v_chunks[1]);

        let (b0, b1, b2) = if inner.width < 50 {
            ("GitHub (O)", "GitLab (G)", "SF (S)")
        } else {
            ("GitHub (O)", "GitLab (G)", "SourceForge (S)")
        };

        render_button(f, btn_row[0], b0, focused_btn == Some(0), AppAction::OpenProjectRepo, state);
        render_button(f, btn_row[1], b1, focused_btn == Some(1), AppAction::OpenGitLabRepo, state);
        render_button(f, btn_row[2], b2, focused_btn == Some(2), AppAction::OpenSourceForgeRepo, state);
    } else {
        // Desktop / wide layout: side-by-side
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .split(inner);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(cols[0]);

        let about_lines = vec![
            Line::from(vec![
                Span::styled("Open AG Patcher (Rust Edition) ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled(format!("v{}", CURRENT_VERSION), Style::default().fg(MD_SECONDARY)),
            ]),
            Line::from("Сверхбыстрая нативная утилита обхода региональных ограничений."),
            Line::raw(""),
            Line::from(vec![
                Span::styled("• Автор патчера:       ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(AUTHOR_NAME, Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("• Репозиторий:         ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(PROJECT_REPO_URL, Style::default().fg(MD_SECONDARY)),
            ]),
            Line::from(vec![
                Span::styled("• Автор оригинала:     ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(format!("{} (первоисточник идеи и Python-патчера)", UPSTREAM_AUTHOR), Style::default().fg(MD_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled("• GitLab оригинала:    ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(UPSTREAM_GITLAB, Style::default().fg(MD_SECONDARY)),
            ]),
            Line::from(vec![
                Span::styled("• SourceForge:         ", Style::default().fg(MD_TEXT_LOW)),
                Span::styled(UPSTREAM_SOURCEFORGE, Style::default().fg(MD_SECONDARY)),
            ]),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Безопасность: ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled("Все патчи обратимы, оригинальные бэкапы .agybak сохраняются.", Style::default().fg(MD_TEXT_MED)),
            ]),
        ];

        f.render_widget(Paragraph::new(about_lines).wrap(Wrap { trim: true }), left_chunks[0]);

        let focused_btn = match state.focus {
            PaneFocus::Buttons(idx) => Some(idx),
            _ => None,
        };

        let btn_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(left_chunks[1]);

        render_button(f, btn_row[0], "GitHub проекта (O)", focused_btn == Some(0), AppAction::OpenProjectRepo, state);
        render_button(f, btn_row[1], "GitLab оригинала (G)", focused_btn == Some(1), AppAction::OpenGitLabRepo, state);
        render_button(f, btn_row[2], "SourceForge (S)", focused_btn == Some(2), AppAction::OpenSourceForgeRepo, state);

        let sheet_block = Block::default()
            .title(Span::styled(" Горячие клавиши ", Style::default().fg(MD_PRIMARY)))
            .borders(Borders::LEFT)
            .border_style(style_border_normal());

        let sheet_lines = vec![
            Line::from(vec![
                Span::styled("  1 - 5       ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled("Переключение вкладок", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  Tab / ⇄     ", Style::default().fg(MD_SECONDARY).add_modifier(Modifier::BOLD)),
                Span::styled("Фокус: список ⇄ кнопки", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  Enter / P   ", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
                Span::styled("Пропатчить выбранный", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  R           ", Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD)),
                Span::styled("Восстановить / Обновить", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  A           ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled("Пропатчить всё за 1 клик", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  S           ", Style::default().fg(MD_SECONDARY).add_modifier(Modifier::BOLD)),
                Span::styled("Пути файлов / SourceForge", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  L           ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled("Журнал событий", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  U           ", Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD)),
                Span::styled("Проверить обновления", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  Мышь        ", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
                Span::styled("Клик по вкладкам и кнопкам", Style::default().fg(MD_TEXT_MED)),
            ]),
            Line::from(vec![
                Span::styled("  Q / Esc     ", Style::default().fg(MD_ERROR).add_modifier(Modifier::BOLD)),
                Span::styled("Выход из программы", Style::default().fg(MD_TEXT_MED)),
            ]),
        ];

        f.render_widget(Paragraph::new(sheet_lines).block(sheet_block), cols[1]);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FOOTER
// ─────────────────────────────────────────────────────────────────────────────

fn draw_footer(f: &mut ratatui::Frame, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_compact = area.height < 3;
    let inner = if is_compact {
        area
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BORDER_ROUNDED)
            .border_style(style_border_normal());
        let in_rect = block.inner(area);
        f.render_widget(block, area);
        in_rect
    };

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let line = if inner.width < 45 {
        Line::from(vec![
            Span::styled("↵", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled(":Патч ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("R", Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD)),
            Span::styled(":Откат ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("A", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(":Всё ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("Q", Style::default().fg(MD_ERROR).add_modifier(Modifier::BOLD)),
            Span::styled(":Выход", Style::default().fg(MD_TEXT_MED)),
        ])
    } else if inner.width < 75 {
        Line::from(vec![
            Span::styled("1-5", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(":Табы • ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("↵", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled(":Патч • ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("R", Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD)),
            Span::styled(":Откат • ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("A", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(":Всё • ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("Q", Style::default().fg(MD_ERROR).add_modifier(Modifier::BOLD)),
            Span::styled(":Выход", Style::default().fg(MD_TEXT_MED)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Tab ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled("Фокус  •  ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("Enter ", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled("Выбор  •  ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("1-5 ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled("Вкладки  •  ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("P ", Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled("Патч  •  ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("R ", Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD)),
            Span::styled("Откат  •  ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("A ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled("Всё  •  ", Style::default().fg(MD_TEXT_MED)),
            Span::styled("Q ", Style::default().fg(MD_ERROR).add_modifier(Modifier::BOLD)),
            Span::styled("Выход", Style::default().fg(MD_TEXT_MED)),
        ])
    };

    f.render_widget(Paragraph::new(line).alignment(Alignment::Center), inner);
}

// ─────────────────────────────────────────────────────────────────────────────
// BUTTON COMPONENT & HITBOX
// ─────────────────────────────────────────────────────────────────────────────

fn render_button(
    f: &mut ratatui::Frame,
    area: Rect,
    label: &str,
    is_focused: bool,
    action: AppAction,
    state: &mut AppState,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    state.clickables.push(ClickableArea {
        rect: area,
        action,
    });

    let border_style = if is_focused {
        style_border_active()
    } else {
        style_border_normal()
    };

    let text_style = if is_focused {
        style_button_focused()
    } else {
        style_button_normal()
    };

    let display_label = if area.width < 12 {
        label.trim().to_string()
    } else if is_focused {
        format!("›{}‹", label.trim())
    } else {
        format!(" {} ", label.trim())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let p = Paragraph::new(Line::from(Span::styled(display_label, text_style)))
        .alignment(Alignment::Center);
    f.render_widget(p, inner);
}

// ─────────────────────────────────────────────────────────────────────────────
// MODAL: PATH INPUT
// ─────────────────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_modal_path_input(f: &mut ratatui::Frame, area: Rect, idx: usize, state: &AppState) {
    let (percent_x, percent_y) = if area.width < 60 {
        (96, 60)
    } else if area.width < 90 {
        (85, 45)
    } else {
        (66, 28)
    };
    let popup_area = centered_rect(percent_x, percent_y, area);
    f.render_widget(Clear, popup_area);

    let titles = [
        "Путь к Antigravity IDE (main.js)",
        "Путь к Antigravity 2.0 (language_server)",
        "Путь к Antigravity CLI (agy)",
        "Путь к расширению VS Code (extension.js)",
        "Путь к бинарнику VS Code (antigravity)",
    ];

    let block = Block::default()
        .title(Span::styled(format!(" {} ", titles[idx]), style_title()))
        .borders(Borders::ALL)
        .border_type(BORDER_ROUNDED)
        .border_style(style_border_active());

    let lines = vec![
        Line::from("Введите или вставьте полный путь к файлу:"),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Путь: ", Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(&state.input_buf, Style::default().fg(MD_TEXT_HIGH).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(MD_PRIMARY)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("[ Enter ] Сохранить  •  [ Esc ] Отмена", Style::default().fg(MD_TEXT_LOW)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: true }), popup_area);
}

// ─────────────────────────────────────────────────────────────────────────────
// HELPERS & VISUAL ANIMATION
// ─────────────────────────────────────────────────────────────────────────────

fn check_backup_file(target: &Path, ext: &str) -> (bool, u64) {
    let bak_path = PathBuf::from(format!("{}{}", target.display(), ext));
    if bak_path.exists() {
        let sz = bak_path.metadata().map(|m| m.len()).unwrap_or(0);
        (true, sz)
    } else {
        (false, 0)
    }
}

fn read_binary_arch(path: &Path) -> Option<Arch> {
    let mut f = File::open(path).ok()?;
    let mut buf = [0u8; 64];
    let n = f.read(&mut buf).ok()?;
    if n >= 4 {
        Some(detect_arch(&buf[..n]))
    } else {
        None
    }
}

fn format_bytes_ru(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} Б", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} КБ", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} МБ", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} ГБ", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn current_time_str() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, s)
}

fn truncate_middle(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s.chars().take(max_len).collect()
    } else {
        let chars: Vec<char> = s.chars().collect();
        let left = (max_len - 3) / 2;
        let right = max_len - 3 - left;
        let left_part: String = chars[..left].iter().collect();
        let right_part: String = chars[chars.len() - right..].iter().collect();
        format!("{}...{}", left_part, right_part)
    }
}

fn get_breathing_color(tick: usize) -> Color {
    // Smooth 16-step wave between Google Green (129, 201, 149) and Soft Mint (175, 235, 195)
    let step = (tick / 2) % 16;
    let factor = if step < 8 { step } else { 16 - step } as f64 / 8.0;
    let r = (129.0 * (1.0 - factor) + 175.0 * factor) as u8;
    let g = (201.0 * (1.0 - factor) + 235.0 * factor) as u8;
    let b = (149.0 * (1.0 - factor) + 195.0 * factor) as u8;
    Color::Rgb(r, g, b)
}

const SPINNER_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn get_spinner_char(tick: usize) -> char {
    SPINNER_FRAMES[(tick / 2) % SPINNER_FRAMES.len()]
}

fn render_mini_bar(filled_ratio: f64, width: usize) -> Vec<Span<'static>> {
    let filled_chars = ((filled_ratio.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    let empty_chars = width.saturating_sub(filled_chars);

    let mut spans = Vec::new();
    if filled_chars > 0 {
        spans.push(Span::styled(
            "█".repeat(filled_chars),
            Style::default().fg(MD_SUCCESS),
        ));
    }
    if empty_chars > 0 {
        spans.push(Span::styled(
            "░".repeat(empty_chars),
            Style::default().fg(MD_OUTLINE),
        ));
    }
    spans
}
