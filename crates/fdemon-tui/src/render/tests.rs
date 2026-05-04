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
    // Phase 5: modal overlay regions (tag-filter, Settings panel internals) may push
    // additional entries into the registry. Update this exact-count assertion to
    // `>= 6` (or split into per-source counts) when those regions land.
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
    // Phase 5: modal overlay regions may register additional entries alongside tab
    // regions. Update this exact-count assertion to `>= 3` (or split into per-source
    // counts) when Phase 5 overlay regions land.
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
    // Phase 5: when the Settings panel wires its internal regions, update this
    // assertion to also verify that panel regions exist alongside header regions
    // (e.g. split into per-source counts or check specific panel-region entries).
    assert!(
        !regions.is_empty(),
        "header is rendered in Settings mode — registry must be non-empty at 120 cols"
    );
}

// ===========================================================================
// Phase 4 Registry Snapshot Tests
//
// These tests verify that after a full `view()` call the mouse-region registry
// contains the expected Phase-4 entries (log rows, DevTools tab-bar regions,
// inspector row/glyph regions, performance frame bars, and network request rows
// + detail-tab labels).  They count entries rather than pixel-diff the screen
// so the assertions remain stable across layout tweaks.
// ===========================================================================

/// Build a state with one session and `entry_count` plain log entries.
fn build_state_with_logs(entry_count: usize) -> AppState {
    use crate::test_utils::test_device;
    use fdemon_core::{LogEntry, LogSource};

    let mut state = AppState::new();
    state.project_name = Some("test_app".to_string());
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();

    let handle = state.session_manager.selected_mut().unwrap();
    for i in 0..entry_count {
        let entry = LogEntry::info(LogSource::Flutter, format!("log message {}", i));
        handle.session.add_log(entry);
    }
    state
}

/// Build a state in DevTools mode with Inspector as the active panel.
fn build_state_devtools_inspector() -> AppState {
    use crate::test_utils::test_device;

    let mut state = AppState::new();
    state.project_name = Some("test_app".to_string());
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    state.enter_devtools_mode();
    state.switch_devtools_panel(fdemon_app::state::DevToolsPanel::Inspector);
    state
}

/// Build a state in DevTools Inspector mode with `node_count` visible nodes
/// (root + node_count-1 children).
///
/// Marks the session as VM-connected so the inspector `render_with_regions`
/// path does not bail out early into the disconnected-view.
fn build_state_devtools_inspector_with_nodes(node_count: usize) -> AppState {
    let mut state = build_state_devtools_inspector();

    // Inspector render_with_regions gates on vm_connected.
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.vm_connected = true;
    }

    let mut children = Vec::new();
    for i in 1..node_count {
        children.push(fdemon_core::DiagnosticsNode {
            description: format!("Child{}", i),
            value_id: Some(format!("child-{}", i)),
            has_children: false,
            children: vec![],
            ..Default::default()
        });
    }

    let root = fdemon_core::DiagnosticsNode {
        description: "Root".to_string(),
        value_id: Some("root-id".to_string()),
        has_children: !children.is_empty(),
        children,
        ..Default::default()
    };

    state
        .devtools_view_state
        .inspector
        .expanded
        .insert("root-id".to_string());
    state.devtools_view_state.inspector.root = Some(root);
    state.devtools_view_state.inspector.selected_index = 0;

    state
}

/// Build a state in DevTools Performance mode with `frame_count` frames.
///
/// Marks the session as VM-connected and `monitoring_active = true` so the
/// performance `render_with_regions` path reaches the frame-bar rendering
/// (and registers click regions) rather than bailing into the disconnected-view.
fn build_state_devtools_performance_with_frames(frame_count: u64) -> AppState {
    use crate::test_utils::test_device;
    use fdemon_core::performance::FrameTiming;

    let mut state = AppState::new();
    state.project_name = Some("test_app".to_string());
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    state.enter_devtools_mode();
    state.switch_devtools_panel(fdemon_app::state::DevToolsPanel::Performance);

    let handle = state.session_manager.selected_mut().unwrap();
    // Gates in render_with_regions: vm_connected AND monitoring_active.
    handle.session.vm_connected = true;
    handle.session.performance.monitoring_active = true;

    for n in 0..frame_count {
        let frame = FrameTiming {
            number: n,
            build_micros: 8_000,
            raster_micros: 8_000,
            elapsed_micros: 16_000,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        };
        handle.session.performance.frame_history.push(frame);
    }

    state
}

/// Build a state in DevTools Network mode with `request_count` entries and
/// the first entry selected (so the detail panel — and its 5 tab regions —
/// is visible).
///
/// Marks the session as VM-connected so the network `render_with_regions` path
/// reaches the table / detail rendering instead of the disconnected-view.
fn build_state_devtools_network_with_selection(request_count: usize) -> AppState {
    use crate::test_utils::test_device;
    use fdemon_core::network::HttpProfileEntry;

    let mut state = AppState::new();
    state.project_name = Some("test_app".to_string());
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    state.enter_devtools_mode();
    state.switch_devtools_panel(fdemon_app::state::DevToolsPanel::Network);

    {
        let handle = state.session_manager.selected_mut().unwrap();
        // Gate in network render_with_regions: vm_connected must be true.
        handle.session.vm_connected = true;

        for i in 0..request_count {
            let entry = HttpProfileEntry {
                id: format!("req-{}", i),
                method: "GET".to_string(),
                uri: format!("https://example.com/api/{}", i),
                status_code: Some(200),
                content_type: None,
                start_time_us: (i as i64) * 1_000_000,
                end_time_us: Some((i as i64) * 1_000_000 + 50_000),
                request_content_length: None,
                response_content_length: Some(256),
                error: None,
            };
            handle.session.network.entries.push_back(entry);
        }
        // Select the first entry so the detail panel appears.
        handle.session.network.selected_index = Some(0);
    }

    state
}

#[test]
fn view_renders_expected_log_view_regions_at_80x24() {
    use fdemon_app::message::Message;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = build_state_with_logs(12);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let click_log_rows = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(Message::ClickLogRow { .. })
            )
        })
        .count();

    // At 80×24 with 12 entries and the standard header height the log area
    // fits all 12 rows, so at least 12 ClickLogRow regions must be registered.
    assert!(
        click_log_rows >= 12,
        "expected ≥ 12 ClickLogRow regions for 12 visible entries, got {}",
        click_log_rows
    );
}

#[test]
fn view_renders_expected_devtools_tab_regions_at_80x24() {
    use fdemon_app::message::Message;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = build_state_devtools_inspector();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_regions: Vec<_> = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(Message::SwitchDevToolsPanel(_))
            )
        })
        .collect();

    // Inspector / Performance / Network — exactly 3 sub-tab click regions.
    assert_eq!(
        tab_regions.len(),
        3,
        "expected 3 SwitchDevToolsPanel regions for the DevTools sub-tab bar, got {}",
        tab_regions.len()
    );
}

#[test]
fn view_renders_expected_inspector_tree_regions_at_80x24() {
    use fdemon_app::message::Message;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = build_state_devtools_inspector_with_nodes(5);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let select_count = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(Message::DevToolsInspectorSelectRow { .. })
            )
        })
        .count();
    let toggle_count = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(Message::DevToolsInspectorToggleNode { .. })
            )
        })
        .count();

    // 5 visible nodes → 5 row regions + 5 glyph regions.
    assert_eq!(
        select_count, 5,
        "expected 5 DevToolsInspectorSelectRow regions for 5 visible nodes, got {}",
        select_count
    );
    assert_eq!(
        toggle_count, 5,
        "expected 5 DevToolsInspectorToggleNode regions for 5 nodes, got {}",
        toggle_count
    );
}

#[test]
fn view_renders_expected_performance_frame_regions_at_80x40() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // Use a 40-row terminal so the performance panel's frame chart has enough
    // height to reach the bar-chart rendering path (MIN_CHART_HEIGHT +
    // DETAIL_PANEL_HEIGHT = 7 inner rows, which requires more vertical space
    // than the default 24-row terminal can provide after headers and borders).
    let mut state = build_state_devtools_performance_with_frames(8);
    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let frame_regions = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(fdemon_app::message::Message::SelectPerformanceFrame { index: Some(_) })
            )
        })
        .count();

    assert_eq!(
        frame_regions, 8,
        "expected 8 SelectPerformanceFrame regions for 8 frames, got {}",
        frame_regions
    );
}

#[test]
fn view_renders_expected_network_regions_with_selection_at_160x30() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // Wide terminal (160 cols, well above WIDE_THRESHOLD=100) so the horizontal
    // split is used and the 5-tab detail panel has room for all tab labels.
    // At 120 cols the detail panel inner width is only 53 chars — not wide enough
    // for all five tab labels (total 65 chars). 160 cols gives detail_inner = 71.
    let mut state = build_state_devtools_network_with_selection(10);
    let backend = TestBackend::new(160, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();

    let row_regions = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(fdemon_app::message::Message::NetworkSelectRequest { .. })
            )
        })
        .count();

    let detail_tab_regions = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(fdemon_app::message::Message::NetworkSwitchDetailTab(_))
            )
        })
        .count();

    assert!(
        row_regions >= 10,
        "expected ≥ 10 NetworkSelectRequest regions for 10 entries, got {}",
        row_regions
    );
    assert_eq!(
        detail_tab_regions, 5,
        "expected 5 NetworkSwitchDetailTab regions (one per sub-tab), got {}",
        detail_tab_regions
    );
}

/// All Phase-4 regions must use z_index = 0.  z_index = 1 is reserved for
/// Phase-5 modal dialogs and overlays; any accidental use here would break
/// modal-precedence logic in Phase 5.
#[test]
fn phase_4_records_no_z1_regions() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = build_state_devtools_inspector_with_nodes(5);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    for entry in regions.iter() {
        assert_eq!(
            entry.z_index, 0,
            "Phase 4 must not register z_index = 1 regions (reserved for Phase 5 overlays)"
        );
    }
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
