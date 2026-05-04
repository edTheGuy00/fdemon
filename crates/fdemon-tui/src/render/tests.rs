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

// TODO(phase-3): Tasks 06 (header regions) and 07 (tab regions) will
// change this assertion. Replace with snapshot tests on the populated
// registry contents.
#[test]
fn test_view_leaves_mouse_regions_empty_when_no_widget_records() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    assert!(
        regions.is_empty(),
        "no widget records regions yet (Phase 3 Tasks 06/07 will change this)"
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
