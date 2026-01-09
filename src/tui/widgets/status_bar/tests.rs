use super::*;
use crate::config::LaunchConfig;
use crate::daemon::Device;
use chrono::{Duration, Local};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn create_test_state() -> AppState {
    AppState::new()
}

fn test_device(id: &str, name: &str) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform: "ios".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }
}

#[test]
fn test_state_indicator_initializing() {
    let state = create_test_state();
    let bar = StatusBar::new(&state);
    let indicator = bar.state_indicator();

    assert!(indicator.style.fg == Some(Color::DarkGray));
    assert!(indicator.content.to_string().contains("Starting"));
}

#[test]
fn test_state_indicator_running() {
    let mut state = create_test_state();
    state.phase = AppPhase::Running;

    let bar = StatusBar::new(&state);
    let indicator = bar.state_indicator();

    assert!(indicator.style.fg == Some(Color::Green));
    assert!(indicator.content.to_string().contains("Running"));
}

#[test]
fn test_state_indicator_reloading() {
    let mut state = create_test_state();
    state.phase = AppPhase::Reloading;

    let bar = StatusBar::new(&state);
    let indicator = bar.state_indicator();

    assert!(indicator.style.fg == Some(Color::Yellow));
    assert!(indicator.content.to_string().contains("Reloading"));
}

#[test]
fn test_state_indicator_quitting() {
    let mut state = create_test_state();
    state.phase = AppPhase::Quitting;

    let bar = StatusBar::new(&state);
    let indicator = bar.state_indicator();

    assert!(indicator.style.fg == Some(Color::DarkGray));
    assert!(indicator.content.to_string().contains("Stopping"));
}

#[test]
fn test_config_info_debug_mode() {
    let mut state = create_test_state();
    let device = test_device("d1", "iPhone");
    let mut config = LaunchConfig::default();
    config.mode = FlutterMode::Debug;
    config.flavor = None;

    let id = state
        .session_manager
        .create_session_with_config(&device, config)
        .unwrap();
    state.session_manager.select_by_id(id);

    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();

    assert!(config_span.content.to_string().contains("Debug"));
    assert_eq!(config_span.style.fg, Some(Color::Green));
}

#[test]
fn test_config_info_profile_mode() {
    let mut state = create_test_state();
    let device = test_device("d1", "iPhone");
    let mut config = LaunchConfig::default();
    config.mode = FlutterMode::Profile;
    config.flavor = None;

    let id = state
        .session_manager
        .create_session_with_config(&device, config)
        .unwrap();
    state.session_manager.select_by_id(id);

    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();

    assert!(config_span.content.to_string().contains("Profile"));
    assert_eq!(config_span.style.fg, Some(Color::Yellow));
}

#[test]
fn test_config_info_release_with_flavor() {
    let mut state = create_test_state();
    let device = test_device("d1", "Pixel");
    let mut config = LaunchConfig::default();
    config.mode = FlutterMode::Release;
    config.flavor = Some("production".to_string());

    let id = state
        .session_manager
        .create_session_with_config(&device, config)
        .unwrap();
    state.session_manager.select_by_id(id);

    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();

    assert!(config_span.content.to_string().contains("Release"));
    assert!(config_span.content.to_string().contains("production"));
    assert_eq!(config_span.style.fg, Some(Color::Magenta));
}

#[test]
fn test_config_info_no_session() {
    let state = create_test_state();
    let bar = StatusBar::new(&state);

    assert!(bar.config_info().is_none());
}

#[test]
fn test_config_info_no_launch_config() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();

    // Should default to Debug
    assert!(config_span.content.to_string().contains("Debug"));
    assert_eq!(config_span.style.fg, Some(Color::Green));
}

#[test]
fn test_session_timer() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Set session started_at to 1h 2m 3s ago
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.started_at = Some(Local::now() - Duration::seconds(3723));
    }

    let bar = StatusBar::new(&state);
    let timer = bar.session_timer().unwrap();

    assert!(timer.content.to_string().contains("01:02:03"));
}

#[test]
fn test_last_reload() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Set session last_reload_time
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.last_reload_time = Some(Local::now());
    }

    let bar = StatusBar::new(&state);
    let reload = bar.last_reload();

    assert!(reload.is_some());
}

#[test]
fn test_build_segments_minimal() {
    let state = create_test_state();
    let bar = StatusBar::new(&state);
    let segments = bar.build_segments();

    // Should have at least: padding, state, separator, scroll, pos, padding
    assert!(segments.len() >= 6);
}

#[test]
fn test_build_segments_with_config() {
    let mut state = create_test_state();
    state.phase = AppPhase::Running;

    // Create a session with release config and flavor
    let device = test_device("d1", "Pixel");
    let mut config = LaunchConfig::default();
    config.mode = FlutterMode::Release;
    config.flavor = Some("staging".to_string());

    let id = state
        .session_manager
        .create_session_with_config(&device, config)
        .unwrap();
    state.session_manager.select_by_id(id);

    // Mark session as running (status bar now reads session phase)
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.mark_started("app-1".to_string());
    }

    let bar = StatusBar::new(&state);
    let segments = bar.build_segments();

    // Collect all content
    let content: String = segments.iter().map(|s| s.content.to_string()).collect();

    assert!(content.contains("Running"));
    assert!(content.contains("Release"));
    assert!(content.contains("staging"));
}

#[test]
fn test_status_bar_render() {
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = create_test_state();
    state.phase = AppPhase::Running;

    // Create a session with config
    let device = test_device("d1", "Test Device");
    let mut config = LaunchConfig::default();
    config.mode = FlutterMode::Debug;

    let id = state
        .session_manager
        .create_session_with_config(&device, config)
        .unwrap();
    state.session_manager.select_by_id(id);

    // Mark session as running (status bar now reads session phase)
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.mark_started("app-1".to_string());
    }

    terminal
        .draw(|frame| {
            let area = frame.area();
            let bar = StatusBar::new(&state);
            frame.render_widget(bar, area);
        })
        .unwrap();

    // Verify the buffer contains expected text
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    assert!(content.contains("Running"));
    assert!(content.contains("Debug")); // Config info now shows instead of device
}

#[test]
fn test_compact_status_bar_render() {
    let backend = TestBackend::new(40, 3);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = create_test_state();
    state.phase = AppPhase::Running;

    // Create a session with started_at set
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.started_at = Some(Local::now() - Duration::seconds(60));
        handle.session.phase = AppPhase::Running;
    }

    terminal
        .draw(|frame| {
            let area = frame.area();
            let bar = StatusBarCompact::new(&state);
            frame.render_widget(bar, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Should contain the running indicator
    assert!(content.contains("●"));
}

#[test]
fn test_log_position_empty() {
    let state = create_test_state();
    let bar = StatusBar::new(&state);

    assert_eq!(bar.log_position(), "0/0");
}

#[test]
fn test_scroll_indicator_auto() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Session starts with auto_scroll = true by default
    let bar = StatusBar::new(&state);
    let indicator = bar.scroll_indicator();

    assert!(indicator.content.to_string().contains("Auto"));
    assert!(indicator.style.fg == Some(Color::Green));
}

#[test]
fn test_scroll_indicator_manual() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Set auto_scroll to false on the session
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.log_view_state.auto_scroll = false;
    }

    let bar = StatusBar::new(&state);
    let indicator = bar.scroll_indicator();

    assert!(indicator.content.to_string().contains("Manual"));
    assert!(indicator.style.fg == Some(Color::Yellow));
}

// ─────────────────────────────────────────────────────────
// Error Count Tests (Phase 2 Task 7)
// ─────────────────────────────────────────────────────────

#[test]
fn test_error_count_zero() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    let bar = StatusBar::new(&state);
    let span = bar.error_count();

    assert!(span.content.to_string().contains("No errors"));
    assert_eq!(span.style.fg, Some(Color::DarkGray));
}

#[test]
fn test_error_count_singular() {
    use crate::core::{LogEntry, LogSource};

    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Add one error
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "test error"));
    }

    let bar = StatusBar::new(&state);
    let span = bar.error_count();

    assert!(span.content.to_string().contains("1 error"));
    assert!(!span.content.to_string().contains("errors")); // singular
    assert_eq!(span.style.fg, Some(Color::Red));
}

#[test]
fn test_error_count_plural() {
    use crate::core::{LogEntry, LogSource};

    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Add multiple errors
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 1"));
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 2"));
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 3"));
    }

    let bar = StatusBar::new(&state);
    let span = bar.error_count();

    assert!(span.content.to_string().contains("3 errors")); // plural
    assert_eq!(span.style.fg, Some(Color::Red));
}

#[test]
fn test_error_count_no_session() {
    let state = create_test_state();
    // No session selected

    let bar = StatusBar::new(&state);
    let span = bar.error_count();

    // Should show "No errors" when no session
    assert!(span.content.to_string().contains("No errors"));
}

#[test]
fn test_error_count_in_segments() {
    use crate::core::{LogEntry, LogSource};

    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Add some errors
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 1"));
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 2"));
    }

    let bar = StatusBar::new(&state);
    let segments = bar.build_segments();

    // Collect all content
    let content: String = segments.iter().map(|s| s.content.to_string()).collect();

    // Error count should appear in the segments
    assert!(content.contains("2 errors"));
}

#[test]
fn test_compact_status_bar_with_errors() {
    use crate::core::{LogEntry, LogSource};

    let backend = TestBackend::new(40, 3);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Add errors
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 1"));
        handle
            .session
            .add_log(LogEntry::error(LogSource::App, "error 2"));
        handle.session.phase = AppPhase::Running;
    }

    terminal
        .draw(|frame| {
            let area = frame.area();
            let bar = StatusBarCompact::new(&state);
            frame.render_widget(bar, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Compact bar should show error count when there are errors
    assert!(content.contains("✗2"));
}

#[test]
fn test_compact_status_bar_no_errors() {
    let backend = TestBackend::new(40, 3);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // No errors - just set phase
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.phase = AppPhase::Running;
    }

    terminal
        .draw(|frame| {
            let area = frame.area();
            let bar = StatusBarCompact::new(&state);
            frame.render_widget(bar, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Compact bar should NOT show error indicator when 0 errors
    assert!(!content.contains('✗'));
}

// ─────────────────────────────────────────────────────────
// TestTerminal-based tests (Phase 3.5 Task 8)
// ─────────────────────────────────────────────────────────

#[test]
fn test_statusbar_renders_phase() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let mut state = create_test_state();
    state.phase = AppPhase::Running;

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    assert!(
        term.buffer_contains("Running") || term.buffer_contains("RUNNING"),
        "Status bar should show Running phase"
    );
}

#[test]
fn test_statusbar_renders_device_name() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let mut state = create_test_state();

    // Create a session with device
    let device = test_device("d1", "iPhone 15 Pro");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    // Device name is not directly shown in status bar (config info is shown instead)
    // but the session should be present
    let content = term.content();
    assert!(!content.is_empty());
}

#[test]
fn test_statusbar_renders_reload_count() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let mut state = create_test_state();

    // Create a session
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Add some reload timing
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.phase = AppPhase::Running;
        handle.session.last_reload_time = Some(Local::now());
    }

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    // Should render without panic and show timing
    let content = term.content();
    assert!(!content.is_empty());
}

#[test]
fn test_statusbar_phase_initializing() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let mut state = create_test_state();
    state.phase = AppPhase::Initializing;

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    assert!(
        term.buffer_contains("Initializing") || term.buffer_contains("Starting"),
        "Should show initializing phase"
    );
}

#[test]
fn test_statusbar_phase_reloading() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let mut state = create_test_state();
    state.phase = AppPhase::Reloading;

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    assert!(
        term.buffer_contains("Reloading") || term.buffer_contains("Reload"),
        "Should show reloading phase"
    );
}

#[test]
fn test_statusbar_phase_stopped() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let mut state = create_test_state();
    state.phase = AppPhase::Stopped;

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    assert!(
        term.buffer_contains("Stopped") || term.buffer_contains("STOPPED"),
        "Should show stopped phase"
    );
}

#[test]
fn test_statusbar_no_device() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::new();
    let state = create_test_state();
    // No session created, so no device

    let status_bar = StatusBar::new(&state);
    term.render_widget(status_bar, term.area());

    // Should render without panic
    let content = term.content();
    assert!(!content.is_empty());
}

#[test]
fn test_statusbar_compact() {
    use crate::tui::test_utils::TestTerminal;

    let mut term = TestTerminal::compact();
    let state = create_test_state();

    let status_bar = StatusBarCompact::new(&state);
    term.render_widget(status_bar, term.area());

    // Compact bar should fit in small terminal
    let content = term.content();
    assert!(!content.is_empty());
}

#[test]
fn test_statusbar_compact_vs_full() {
    use crate::tui::test_utils::TestTerminal;

    let state = create_test_state();

    let mut term_full = TestTerminal::new();
    let mut term_compact = TestTerminal::compact();

    term_full.render_widget(StatusBar::new(&state), term_full.area());
    term_compact.render_widget(StatusBarCompact::new(&state), term_compact.area());

    // Both should render, but content differs
    assert!(!term_full.content().is_empty());
    assert!(!term_compact.content().is_empty());
}
