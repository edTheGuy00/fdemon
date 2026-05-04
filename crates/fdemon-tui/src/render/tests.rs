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
// Mouse Region Registry Tests (Phase 3, Task 08)
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

/// Snapshot: at 120×24 the render produces exactly six shortcut regions in
/// left-to-right `r R x d D q` order, each with `width = 2` on the title row.
///
/// The title row is at `y = 1` (inside the glass-block border at y=0).
/// Shortcut regions are registered only when the full shortcut line fits within
/// the available width — 120 cols is wide enough for all six.
///
/// TODO(phase-5): tag-filter overlay precedence — Phase 5 modal regions may
/// push additional entries into the registry; this test's `len() == 6` check
/// will need to be relaxed to `>= 6` if overlays register regions globally.
#[test]
fn view_populates_header_shortcut_regions_at_120x24() {
    use fdemon_app::MouseAction;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();

    // Collect just the bracketed-shortcut regions (width 2, on title row).
    // The title row is at y=1: the glass-block border occupies y=0, inner
    // content starts at y=1.
    let title_y = 1_u16;
    let shortcut_msgs: Vec<String> = regions
        .iter()
        .filter(|e| e.rect.width == 2 && e.rect.y == title_y)
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(m)) => Some(format!("{:?}", m)),
            _ => None,
        })
        .collect();

    // Expected order: HotReload, HotRestart, CloseCurrentSession,
    // EnterDevToolsMode, ToggleDap, RequestQuit — matching SHORTCUTS_DEF
    // in widgets/header.rs.
    assert_eq!(shortcut_msgs.len(), 6, "exactly six shortcut regions");
    assert!(shortcut_msgs[0].contains("HotReload"));
    assert!(shortcut_msgs[1].contains("HotRestart"));
    assert!(shortcut_msgs[2].contains("CloseCurrentSession"));
    assert!(shortcut_msgs[3].contains("EnterDevToolsMode"));
    assert!(shortcut_msgs[4].contains("ToggleDap"));
    assert!(shortcut_msgs[5].contains("RequestQuit"));
}

/// Snapshot: with three sessions at 120×24 the render produces exactly three
/// tab regions, each with both left-click (SelectSessionByIndex) and
/// middle-click (CloseSessionAt) bindings.
///
/// TODO(phase-5): tag-filter overlay precedence — Phase 5 modal regions may
/// register additional entries; update the `len() == 3` check if needed.
#[test]
fn view_populates_tab_regions_with_three_sessions() {
    use fdemon_app::{Message, MouseAction};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    for (id_str, name) in [("d1", "iPhone"), ("d2", "Pixel"), ("d3", "Web")] {
        state
            .session_manager
            .create_session(&crate::test_utils::test_device(id_str, name))
            .unwrap();
    }

    terminal.draw(|f| view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_regions: Vec<_> = regions
        .iter()
        .filter(|e| {
            matches!(
                &e.on_left,
                Some(MouseAction::Emit(m)) if matches!(**m, Message::SelectSessionByIndex(_))
            )
        })
        .collect();
    assert_eq!(tab_regions.len(), 3, "three tabs → three regions");
    for entry in &tab_regions {
        assert!(
            matches!(
                &entry.on_middle,
                Some(MouseAction::Emit(m)) if matches!(**m, Message::CloseSessionAt(_))
            ),
            "middle-click bound to CloseSessionAt"
        );
    }
}

/// Probe: document registry contents when the Settings panel is active.
///
/// The header IS rendered in Settings mode (it is painted before the modal
/// overlay match in `render::view`), so shortcut regions ARE registered.
/// The Settings panel overlays the content area but does not interact with
/// the mouse registry in Phase 3 — panel-internal clicks are wired in Phase 5.
///
/// This test locks in the observed behavior: the registry is non-empty in
/// Settings mode because the header is always rendered.
///
/// TODO(phase-5): tag-filter overlay precedence — when Phase 5 wires the
/// Settings panel's internal regions, this test should be updated to also
/// verify that panel regions exist alongside header regions.
#[test]
fn view_header_regions_present_in_settings_mode_because_header_always_renders() {
    use fdemon_app::state::UiMode;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state.show_settings();
    assert_eq!(state.ui_mode, UiMode::Settings);

    terminal.draw(|f| view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();

    // The header IS rendered in Settings mode (before the modal overlay).
    // Shortcut regions are therefore present at 120 cols.
    // Phase 5 will add Settings-panel-internal regions on top of these.
    assert!(
        !regions.is_empty(),
        "header is rendered in Settings mode — registry must be non-empty at 120 cols"
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
