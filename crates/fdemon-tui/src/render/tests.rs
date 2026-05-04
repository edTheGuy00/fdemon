//! Full-screen snapshot tests for TUI rendering
//!
//! These tests capture the entire screen render for each UI mode
//! and compare against golden snapshots using insta.

use super::view;
use crate::test_utils::TestTerminal;
use fdemon_app::state::{AppState, UiMode};
use fdemon_core::AppPhase;
use insta::assert_snapshot;

fn create_base_state() -> AppState {
    let mut state = AppState::new();
    state.project_name = Some("flutter_app".to_string());
    state
}

// Helper to render full screen and return content
fn render_screen(state: &mut AppState) -> String {
    let mut term = TestTerminal::new();
    term.draw_with(|frame| view(frame, state));
    term.content()
}

// ===========================================================================
// Mouse Region Registry Tests
// ===========================================================================

// Task 06: header shortcut regions are populated after a full-screen render
// at 80 cols. At 80 cols the shortcuts do NOT fit (left_width + shortcuts_width
// + device_width + HEADER_SECTION_PADDING > 80), so no shortcut regions are
// registered. Verify the registry is non-empty only at wide-enough terminals.
#[test]
fn test_view_shortcut_regions_registered_at_120_cols() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    assert!(
        !regions.is_empty(),
        "header shortcut regions should be registered at 120 cols (Task 06)"
    );
}

// ===========================================================================
// Normal Mode Snapshots
// ===========================================================================

#[test]
fn snapshot_normal_mode_initializing() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Initializing;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_initializing", content);
}

#[test]
fn snapshot_normal_mode_running() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    // Add a session with device name
    // Note: In the current architecture, we would need to add a proper session
    // For now, we'll test the basic render

    let content = render_screen(&mut state);
    assert_snapshot!("normal_running", content);
}

#[test]
fn snapshot_normal_mode_reloading() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Reloading;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_reloading", content);
}

#[test]
fn snapshot_normal_mode_stopped() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Stopped;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_stopped", content);
}
