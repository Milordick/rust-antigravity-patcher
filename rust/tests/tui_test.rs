use crossterm::event::KeyCode;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use open_ag_patcher::ui::tui::{
    draw_ui, handle_key_input, AppAction, AppState, PaneFocus, Tab,
};

#[test]
fn test_app_state_initialization() {
    let state = AppState::new();
    assert_eq!(state.current_tab, Tab::Targets);
    assert_eq!(state.focus, PaneFocus::Main);
    assert_eq!(state.targets.len(), 5);
    assert!(!state.logs.is_empty());
}

#[test]
fn test_tab_switching_keys() {
    let mut state = AppState::new();

    // Key '2' -> Tab::Paths
    handle_key_input(KeyCode::Char('2'), &mut state);
    assert_eq!(state.current_tab, Tab::Paths);

    // Key '3' -> Tab::Logs
    handle_key_input(KeyCode::Char('3'), &mut state);
    assert_eq!(state.current_tab, Tab::Logs);

    // Key '4' -> Tab::Updates
    handle_key_input(KeyCode::Char('4'), &mut state);
    assert_eq!(state.current_tab, Tab::Updates);

    // Key '5' -> Tab::About
    handle_key_input(KeyCode::Char('5'), &mut state);
    assert_eq!(state.current_tab, Tab::About);

    // Key '1' -> Tab::Targets
    handle_key_input(KeyCode::Char('1'), &mut state);
    assert_eq!(state.current_tab, Tab::Targets);
}

#[test]
fn test_focus_cycling() {
    let mut state = AppState::new();
    assert_eq!(state.focus, PaneFocus::Main);

    // Pressing Tab moves focus to buttons
    handle_key_input(KeyCode::Tab, &mut state);
    assert_eq!(state.focus, PaneFocus::Buttons(0));

    // Navigating buttons right
    handle_key_input(KeyCode::Right, &mut state);
    assert_eq!(state.focus, PaneFocus::Buttons(1));

    // Pressing Tab again returns to main
    handle_key_input(KeyCode::Tab, &mut state);
    assert_eq!(state.focus, PaneFocus::Main);
}

#[test]
fn test_render_all_tabs_headless() {
    let backend = TestBackend::new(110, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    for tab in Tab::ALL {
        state.current_tab = tab;
        state.clickables.clear();

        terminal
            .draw(|f| {
                draw_ui(f, &mut state);
            })
            .expect("Render should succeed");

        assert!(
            !state.clickables.is_empty(),
            "Tab {:?} must register clickable hitboxes",
            tab
        );
    }
}

#[test]
fn test_mouse_tab_clicks() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    state.clickables.clear();
    terminal
        .draw(|f| {
            draw_ui(f, &mut state);
        })
        .unwrap();

    // Find clickable hitbox for Tab 2 (Paths)
    let paths_tab_box = state
        .clickables
        .iter()
        .find(|c| matches!(c.action, AppAction::SelectTab(1)))
        .cloned();

    assert!(paths_tab_box.is_some(), "Must have Tab 1 hitbox");
    let box_rect = paths_tab_box.unwrap().rect;

    // Simulate clicking in the center of that tab
    let click_x = box_rect.x + box_rect.width / 2;
    let click_y = box_rect.y + box_rect.height / 2;

    state.handle_click(click_x, click_y);
    assert_eq!(state.current_tab, Tab::Paths);
}

#[test]
fn test_fullscreen_mouse_tab_clicks() {
    // Test on a 200x50 fullscreen monitor
    let backend = TestBackend::new(200, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    state.clickables.clear();
    terminal
        .draw(|f| {
            draw_ui(f, &mut state);
        })
        .unwrap();

    let tab_y = 4; // Inside tabs row (y = 4)

    state.handle_click(60, tab_y);
    assert_eq!(state.current_tab, Tab::Paths, "Click at col 60 in fullscreen must select Paths");

    state.handle_click(100, tab_y);
    assert_eq!(state.current_tab, Tab::Logs, "Click at col 100 in fullscreen must select Logs");

    state.handle_click(140, tab_y);
    assert_eq!(state.current_tab, Tab::Updates, "Click at col 140 in fullscreen must select Updates");

    state.handle_click(180, tab_y);
    assert_eq!(state.current_tab, Tab::About, "Click at col 180 in fullscreen must select About");

    state.handle_click(20, tab_y);
    assert_eq!(state.current_tab, Tab::Targets, "Click at col 20 in fullscreen must select Targets");
}

#[test]
fn test_small_terminal_resize_guard() {
    // When terminal is shrunk below 28x7, it must not panic and render the guard message
    let backend = TestBackend::new(20, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    let res = terminal.draw(|f| {
        draw_ui(f, &mut state);
    });
    assert!(res.is_ok(), "Small terminal should render guard cleanly without panic");
}

#[test]
fn test_mobile_termux_layouts() {
    // Smartphone / Termux dimensions in portrait and landscape
    let mobile_sizes = [
        (40, 16), // Typical Termux portrait with keyboard
        (45, 20), // Standard smartphone portrait
        (55, 24), // Large smartphone / small tablet
        (35, 12), // Narrow split view on mobile
        (70, 18), // Termux landscape
    ];

    let mut state = AppState::new();

    for (w, h) in mobile_sizes {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        for tab in Tab::ALL {
            state.current_tab = tab;
            state.clickables.clear();

            let res = terminal.draw(|f| {
                draw_ui(f, &mut state);
            });
            assert!(res.is_ok(), "Must render mobile tab {:?} cleanly at {}x{}", tab, w, h);
            assert!(
                !state.clickables.is_empty(),
                "Tab {:?} must have touchable hitboxes on mobile at {}x{}",
                tab, w, h
            );
        }
    }
}

#[test]
fn test_terminal_dynamic_resize() {
    // Test transitioning between multiple window dimensions
    let mut state = AppState::new();

    let sizes = [(120, 40), (80, 24), (60, 15), (45, 10), (100, 30)];
    for (w, h) in sizes {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        state.clickables.clear();

        let res = terminal.draw(|f| {
            draw_ui(f, &mut state);
        });
        assert!(res.is_ok(), "Must render cleanly at size {}x{}", w, h);
    }
}
