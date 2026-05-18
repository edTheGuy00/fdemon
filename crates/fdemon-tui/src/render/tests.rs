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
    // 120×24 Normal mode renders: header brackets `[r] [d] [D] [s] [c] [q]`
    // (six z=0 regions) + log-row regions if any logs exist. Modal regions
    // (NewSessionDialog z=1, ConfirmDialog z=1, TagFilter overlay z=1, Settings
    // z=1) are NOT in this registry — they are only registered when the
    // corresponding `UiMode` is active. Phase 5/5.5 do not change this baseline.
    assert_eq!(
        shortcut_msgs.len(),
        6,
        "exactly six shortcut regions in 120×24 Normal mode"
    );
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
/// Phase 5.5 Task 01 introduced a modal-gate in `render::view()`: when
/// `UiMode::Settings` is active the header receives `None` ctx, so header
/// shortcut regions are **not** registered.  However the Settings panel itself
/// is rendered with `Some(&mut mouse_ctx)` and registers its own tab + row
/// regions — so the registry remains non-empty.
///
/// This test locks in the Phase-5.5 invariant: the registry is non-empty in
/// Settings mode because the Settings panel registers its own regions.
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

    // Settings panel registers tab + row regions (Phase 5) — registry is non-empty
    // even though header shortcut regions are suppressed by the Phase-5.5 modal gate.
    assert!(
        !regions.is_empty(),
        "Settings mode must produce a non-empty registry (settings panel regions) at 120 cols"
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

    // At 80×24 with 12 entries the log content area is exactly 15 rows tall
    // (21-row logs area → Borders::ALL removes 2 → inner 19 → top metadata bar 1
    // + top_gap 1 + bottom metadata bar 1 + bottom_gap 1 = 4 overhead → 15 content
    // rows). 12 entries each take one row → exactly 12 ClickLogRow regions.
    assert_eq!(
        click_log_rows, 12,
        "expected exactly 12 ClickLogRow regions for 12 visible entries, got {}",
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

    // Inspector / Performance / Memory / Network — exactly 4 sub-tab click regions.
    assert_eq!(
        tab_regions.len(),
        4,
        "expected 4 SwitchDevToolsPanel regions for the DevTools sub-tab bar, got {}",
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

/// Baseline: performance compact-mode path at 80×24 registers no frame regions.
///
/// At 80×24 with one session the DevTools panel area has 18 rows after the
/// 3-row sub-tab bar is removed. `PerformancePanel::render_with_regions` enters
/// the two-section split path (height 18 ≥ DUAL_SECTION_MIN_HEIGHT 16).  After
/// subtracting 1 footer row the usable height is 17; the 45% frame-timing chunk
/// rounds to 7 rows.  With `Borders::ALL` removed that leaves a 5-row inner
/// area for `FrameChart`, which is below `MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT`
/// (4 + 3 = 7) — so `FrameChart::render_with_regions` takes the compact-mode
/// branch and records **no** `SelectPerformanceFrame` regions.
///
/// This test locks in the "no regions in compact mode" contract so a future
/// refactor of `FrameChart` cannot silently produce spurious click regions at
/// 80×24.
#[test]
fn performance_compact_mode_at_80x24_records_no_regions() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // 8 frames — enough to populate the chart at wider terminals.
    let mut state = build_state_devtools_performance_with_frames(8);
    let backend = TestBackend::new(80, 24);
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

    // The frame chart inner area (5 rows) is below the 7-row threshold for
    // bar-chart rendering, so no SelectPerformanceFrame regions are pushed.
    assert_eq!(
        frame_regions, 0,
        "compact mode at 80×24 must register 0 SelectPerformanceFrame regions, got {}",
        frame_regions
    );
}

/// Baseline: network table-only path at 80×24 registers no detail-tab regions.
///
/// When no request is selected the `NetworkMonitor` at 80×24 takes the
/// `render_table_only_with_regions` path — the detail panel is not rendered
/// at all and therefore no `NetworkSwitchDetailTab` regions can be pushed.
///
/// This test locks in the "no detail-tab regions when table-only" contract at
/// the spec-mandated 80×24 size.  If the detail panel is accidentally rendered
/// in the no-selection state a future refactor would cause this assertion to
/// fail and prompt investigation.
#[test]
fn network_compact_mode_at_80x24_records_no_detail_tab_regions() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // Build network state with entries but NO selection — exercises the
    // table_only path where the detail panel is not rendered.
    let mut state = {
        use crate::test_utils::test_device;
        use fdemon_core::network::HttpProfileEntry;

        let mut s = AppState::new();
        s.project_name = Some("test_app".to_string());
        s.session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        s.enter_devtools_mode();
        s.switch_devtools_panel(fdemon_app::state::DevToolsPanel::Network);

        {
            let handle = s.session_manager.selected_mut().unwrap();
            handle.session.vm_connected = true;
            // Populate 5 entries but leave selected_index = None (no selection).
            for i in 0..5 {
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
            // selected_index stays None — table_only path.
        }
        s
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let detail_tab_regions = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(fdemon_app::message::Message::NetworkSwitchDetailTab(_))
            )
        })
        .count();

    // No selection → table_only path → detail panel not rendered → 0 tab regions.
    assert_eq!(
        detail_tab_regions, 0,
        "table-only path at 80×24 must register 0 NetworkSwitchDetailTab regions, got {}",
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
// Phase 5 Sister-Function Smoke Tests (Task 02)
// ===========================================================================

/// Smoke test: Phase-5 sister functions delegate to existing `Widget::render`
/// and record **zero** new regions.
///
/// After Task 02 lands but before Tasks 06-10, the new sister functions for
/// `ConfirmDialog`, `SettingsPanel`, `NewSessionDialog`, and `render_tag_filter`
/// are pure delegates — they do not push any regions into the registry.
///
/// This test locks that invariant until Tasks 06–10 land.
///
/// NOTE: The full `matches_phase5_message_shape` helper (which checks for
/// `NewSessionDialogSelectDeviceAt`, `SettingsClickRow`, `TagFilterClickRow`,
/// etc.) is deferred to Task 11 because Phase 5 Task 01 message variants are
/// not yet defined.  Instead, this test verifies the registry is identical in
/// size and shape to a baseline render without any modal active (header+log
/// regions only), then re-runs in `ConfirmDialog` mode and asserts the count
/// does not grow beyond the header baseline.
#[test]
fn phase5_sister_functions_record_no_regions_in_stub_state() {
    use fdemon_app::confirm_dialog::ConfirmDialogState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // Render with ConfirmDialog active.  The dialog replaces the log-view
    // content area visually.  Phase 5.5 Task 01: the header receives None ctx
    // so header shortcut regions are NOT registered while the dialog is up.
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            view(frame, &mut state);
        })
        .unwrap();

    // After the render the registry must exist (guard has put it back).
    let regions = state.mouse_regions.take();

    // Task 06 is now implemented: confirm-dialog render_with_regions records one
    // region per button at z_index = 1.  ConfirmDialogState::quit_confirmation(1)
    // produces 2 options ("Quit" and "Cancel"), so we expect exactly 2 z=1 regions.
    let z1_count = regions.iter().filter(|e| e.z_index == 1).count();
    assert_eq!(
        z1_count, 2,
        "ConfirmDialog must register exactly 2 button regions at z=1 (one per option)"
    );

    // Confirm the render did not panic and the buffer is non-empty.
    let buffer = terminal.backend().buffer();
    assert!(
        !buffer.content.is_empty(),
        "ConfirmDialog render must produce non-empty buffer"
    );

    state.mouse_regions.set(regions);
}

/// Smoke test: Settings mode sister function records no regions in stub state.
#[test]
fn phase5_settings_sister_records_no_new_regions() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = AppState::new();
    state.show_settings();
    assert_eq!(state.ui_mode, UiMode::Settings);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            view(frame, &mut state);
        })
        .unwrap();

    let regions = state.mouse_regions.take();

    // settings_panel::render_with_regions is a stub — no z_index = 1 regions.
    for entry in regions.iter() {
        assert_eq!(
            entry.z_index, 0,
            "Phase-5 Settings stub must not register z_index = 1 regions"
        );
    }

    let buffer = terminal.backend().buffer();
    assert!(
        !buffer.content.is_empty(),
        "Settings render must produce non-empty buffer"
    );

    state.mouse_regions.set(regions);
}

// ===========================================================================
// Phase 5 Integration Snapshot Tests (Task 11)
//
// These tests render each Phase-5 UI mode via the full `view()` call and
// assert expected region counts and z-distribution.  They lock in the
// Phase-5 contracts so that a future refactor cannot silently break the
// click-precedence invariants or drop button/row regions.
// ===========================================================================

/// Extract the `Message` emitted by a region's left-click `MouseAction::Emit`,
/// returning `None` for `EmitWithCoord` actions (which require runtime coords).
///
/// Shared by all Phase-5 snapshot tests.  If a similar helper exists in
/// widget-level test modules it could be factored into a shared `test_utils`
/// module; for now it lives here alongside the consumers.
fn extract_action(entry: &fdemon_app::MouseRegionEntry) -> Option<fdemon_app::message::Message> {
    use fdemon_app::MouseAction;
    match entry.on_left.as_ref()? {
        MouseAction::Emit(msg) => Some((**msg).clone()),
        MouseAction::EmitWithCoord(_) => None,
    }
}

/// Build a minimal one-device helper that avoids the two-argument
/// `test_device(id, name)` call pattern used by the other helpers in this file.
fn test_device() -> fdemon_daemon::Device {
    crate::test_utils::test_device("d1", "iPhone")
}

/// Render the ConfirmDialog UI mode and assert:
/// - Exactly 2 button regions are registered (Quit + Cancel).
/// - All button regions are at `z_index = 1`.
#[test]
fn phase5_view_renders_expected_confirm_dialog_regions() {
    use fdemon_app::{confirm_dialog::ConfirmDialogState, message::Message, state::UiMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(2));
    state.ui_mode = UiMode::ConfirmDialog;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let confirm_buttons = regions
        .iter()
        .filter(|e| {
            matches!(
                extract_action(e),
                Some(Message::ConfirmQuit) | Some(Message::CancelQuit)
            )
        })
        .count();
    assert_eq!(
        confirm_buttons, 2,
        "ConfirmDialog must register exactly 2 button regions (ConfirmQuit + CancelQuit), got {}",
        confirm_buttons
    );
    for entry in regions.iter().filter(|e| {
        matches!(
            extract_action(e),
            Some(Message::ConfirmQuit) | Some(Message::CancelQuit)
        )
    }) {
        assert_eq!(
            entry.z_index, 1,
            "confirm dialog button regions must be at z=1 (modal layer)"
        );
    }
    state.mouse_regions.set(regions);
}

/// Render the Settings UI mode (100×40) and assert:
/// - Exactly 4 tab regions (`SettingsGotoTab`).
/// - At least one row region (`SettingsClickRow`).
/// - All settings regions are at `z_index = 0` (full-screen, not a modal).
#[test]
fn phase5_view_renders_expected_settings_regions() {
    use fdemon_app::{message::Message, state::UiMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    state.show_settings();
    state.ui_mode = UiMode::Settings;

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SettingsGotoTab(_))))
        .count();
    let row_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SettingsClickRow { .. })))
        .count();
    assert_eq!(
        tab_count, 4,
        "Settings panel must register exactly 4 tab regions, got {}",
        tab_count
    );
    assert!(
        row_count > 0,
        "Settings panel must register at least one SettingsClickRow region"
    );
    // Settings is a full-screen mode, not a modal: all regions at z=0.
    for entry in regions.iter() {
        assert_eq!(
            entry.z_index, 0,
            "Settings regions must all be at z=0 (full-screen, not modal)"
        );
    }
    state.mouse_regions.set(regions);
}

/// Render Normal mode with `tag_filter_visible = true` and two tags; assert:
/// - Exactly 2 tag-row regions (`TagFilterClickRow`).
/// - Exactly 2 action label regions (`ShowAllNativeTags` + `HideAllNativeTags`).
#[test]
fn phase5_view_renders_expected_tag_filter_regions() {
    use fdemon_app::{message::Message, state::UiMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    let id = state
        .session_manager
        .create_session(&test_device())
        .unwrap();
    let handle = state.session_manager.get_mut(id).unwrap();
    handle.native_tag_state.observe_tag("alpha");
    handle.native_tag_state.observe_tag("beta");
    state.tag_filter_visible = true;
    state.ui_mode = UiMode::Normal;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tag_rows = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::TagFilterClickRow { .. })))
        .count();
    let action_labels = regions
        .iter()
        .filter(|e| {
            matches!(
                extract_action(e),
                Some(Message::ShowAllNativeTags) | Some(Message::HideAllNativeTags)
            )
        })
        .count();
    assert_eq!(
        tag_rows, 2,
        "tag filter with 2 tags must register exactly 2 TagFilterClickRow regions, got {}",
        tag_rows
    );
    assert_eq!(
        action_labels, 2,
        "tag filter must register exactly 2 action-label regions (ShowAll + HideAll), got {}",
        action_labels
    );
    state.mouse_regions.set(regions);
}

/// Render NewSessionDialog mode (120×40, wide terminal) with one connected
/// device and assert:
/// - Exactly 2 tab regions (`NewSessionDialogSwitchTab`).
/// - Exactly 1 device row region (`NewSessionDialogSelectDeviceAt`).
/// - At least 4 field regions (`NewSessionDialogFocusField`).
/// - Exactly 1 launch button region (`NewSessionDialogLaunch`).
/// - All main-dialog regions are at `z_index = 1`.
///
/// Note: compact (narrow-terminal) layout does not register device-row regions.
/// This test uses a wide terminal (120 cols, ≥ 120 threshold) so the
/// horizontal layout is used and device rows are clickable.
#[test]
fn phase5_view_renders_expected_new_session_dialog_regions() {
    use fdemon_app::{message::Message, state::UiMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;
    state
        .new_session_dialog_state
        .target_selector
        .set_connected_devices(vec![test_device()]);

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_count = regions
        .iter()
        .filter(|e| {
            matches!(
                extract_action(e),
                Some(Message::NewSessionDialogSwitchTab(_))
            )
        })
        .count();
    let device_count = regions
        .iter()
        .filter(|e| {
            matches!(
                extract_action(e),
                Some(Message::NewSessionDialogSelectDeviceAt { .. })
            )
        })
        .count();
    let field_count = regions
        .iter()
        .filter(|e| {
            matches!(
                extract_action(e),
                Some(Message::NewSessionDialogFocusField { .. })
            )
        })
        .count();
    let launch_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::NewSessionDialogLaunch)))
        .count();

    assert_eq!(
        tab_count, 2,
        "NewSessionDialog must register exactly 2 tab regions, got {}",
        tab_count
    );
    assert_eq!(
        device_count, 1,
        "NewSessionDialog with 1 device must register exactly 1 device region, got {}",
        device_count
    );
    assert!(
        field_count >= 4,
        "NewSessionDialog must register at least 4 field regions (Config, Mode, Flavor, Entry Point), got {}",
        field_count
    );
    assert_eq!(
        launch_count, 1,
        "NewSessionDialog must register exactly 1 Launch button region, got {}",
        launch_count
    );

    // All main-dialog regions must be at z=1 (modal layer).
    for entry in regions.iter().filter(|e| {
        matches!(
            extract_action(e),
            Some(Message::NewSessionDialogSwitchTab(_))
                | Some(Message::NewSessionDialogSelectDeviceAt { .. })
                | Some(Message::NewSessionDialogFocusField { .. })
                | Some(Message::NewSessionDialogLaunch)
        )
    }) {
        assert_eq!(
            entry.z_index, 1,
            "NewSessionDialog regions must be at z=1 (modal layer)"
        );
    }
    state.mouse_regions.set(regions);
}

/// Render LinkHighlight mode with 2 links whose display text appears in
/// the session log, and assert at least 2 `SelectLink` regions are registered.
///
/// The log view registers one badge region per link whose `entry_index` matches
/// a rendered log entry and whose `display_text` appears inside that entry's
/// message.  We create a session with 2 log entries, each containing a file
/// reference string, and populate the `link_highlight_state` to match.
#[test]
fn phase5_view_renders_expected_link_highlight_badge_regions() {
    use fdemon_app::{
        hyperlinks::{DetectedLink, FileReference, LinkHighlightState},
        message::Message,
        state::UiMode,
    };
    use fdemon_core::{LogEntry, LogSource};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    let id = state
        .session_manager
        .create_session(&test_device())
        .unwrap();

    // Add two log entries whose messages contain file reference strings.
    let display_a = "lib/main.dart:10:1";
    let display_b = "lib/widget.dart:20:1";
    let entry_a = LogEntry::info(LogSource::Flutter, format!("Error at {}", display_a));
    let entry_b = LogEntry::info(LogSource::Flutter, format!("See also {}", display_b));

    {
        let handle = state.session_manager.get_mut(id).unwrap();
        handle.session.add_log(entry_a);
        handle.session.add_log(entry_b);
    }

    // Build a LinkHighlightState whose links reference the two entries.
    let mut link_state = LinkHighlightState::new();
    {
        let file_ref_a = FileReference::new("lib/main.dart", 10, 1);
        let mut link_a = DetectedLink::new(file_ref_a, 0, None, '1', 0);
        link_a.display_text = display_a.to_string();
        link_state.add_link(link_a);
    }
    {
        let file_ref_b = FileReference::new("lib/widget.dart", 20, 1);
        let mut link_b = DetectedLink::new(file_ref_b, 1, None, '2', 1);
        link_b.display_text = display_b.to_string();
        link_state.add_link(link_b);
    }
    link_state.activate();

    // Store the link state on the selected session.
    state
        .session_manager
        .get_mut(id)
        .unwrap()
        .session
        .link_highlight_state = link_state;

    state.ui_mode = UiMode::LinkHighlight;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let link_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SelectLink(_))))
        .count();
    assert!(
        link_count >= 2,
        "expected at least 2 SelectLink badge regions, got {}",
        link_count
    );
    state.mouse_regions.set(regions);
}

// ===========================================================================
// Phase 5.5 Renderer-Invariant Tests (Task 01)
//
// These tests verify that `render::view()` does NOT thread `Some(&mut mouse_ctx)`
// into `MainHeader` or `LogView` when a modal `UiMode` is active (or when
// `tag_filter_visible` is true).  They do this by rendering via `view()` and
// then inspecting the resulting registry for the absence of header-shortcut
// regions (e.g. `HotReload` from `[r]`), which are z=0 regions registered by
// `MainHeader` only when it receives a non-`None` ctx.
// ===========================================================================

/// Helper: count how many `HotReload` regions are in the registry after a
/// render.  Used by the modal-gate invariant tests below.
fn count_hot_reload_regions(state: &AppState) -> usize {
    let regions = state.mouse_regions.take();
    let n = regions
        .iter()
        .filter(|e| {
            matches!(
                e.on_left.as_ref().and_then(|a| a.as_emit()),
                Some(fdemon_app::message::Message::HotReload)
            )
        })
        .count();
    // Put the registry back so state is not left inconsistent.
    state.mouse_regions.set(regions);
    n
}

/// In `ConfirmDialog` mode the renderer must NOT register header shortcut
/// regions (e.g. `HotReload` from `[r]`).
///
/// Before Phase 5.5 Task 01, `render::view()` called
/// `render_main_header(..., Some(&mut mouse_ctx))` unconditionally, so a click
/// that fell outside the dialog's z=1 rects would hit the underlying z=0 header
/// region and fire `HotReload`.  After the fix, `None` is passed for the header
/// ctx when `in_modal` is true, so no header regions are registered.
#[test]
fn phase5_5_renderer_invariant_modal_modes_register_no_main_header_regions() {
    use fdemon_app::confirm_dialog::ConfirmDialogState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    state.ui_mode = fdemon_app::state::UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));

    // Use a wide terminal so that, if the gate were missing, the header WOULD
    // register shortcuts at this width (as verified by the Phase-3 test above).
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| view(f, &mut state)).unwrap();

    let hot_reload_count = count_hot_reload_regions(&state);
    assert_eq!(
        hot_reload_count, 0,
        "ConfirmDialog mode must not register HotReload (header z=0) regions; \
         found {} — the modal gate in render::view() may be missing",
        hot_reload_count
    );
}

/// In `Normal` mode with `tag_filter_visible = true` the renderer must NOT
/// register header shortcut regions.
///
/// The tag-filter overlay is "modal" for click purposes even though `ui_mode`
/// stays `Normal`.  The `in_modal` flag in `render::view()` ORs
/// `is_modal_ui_mode` with `state.tag_filter_visible`, so both sources of
/// modal state suppress base-UI region recording.
#[test]
fn phase5_5_renderer_invariant_normal_mode_with_tag_filter_registers_no_main_header_regions() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    state.ui_mode = fdemon_app::state::UiMode::Normal;
    state.tag_filter_visible = true;
    // Create a session so the tag filter overlay has something to render.
    state
        .session_manager
        .create_session(&crate::test_utils::test_device("d1", "iPhone"))
        .unwrap();

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| view(f, &mut state)).unwrap();

    let hot_reload_count = count_hot_reload_regions(&state);
    assert_eq!(
        hot_reload_count, 0,
        "Normal mode with tag_filter_visible=true must not register HotReload \
         (header z=0) regions; found {} — check the tag_filter_visible OR-clause \
         in render::view()",
        hot_reload_count
    );
}

/// `LinkHighlight` mode must NOT suppress base-UI regions (negative gate test).
///
/// Links are overlaid on top of the log view and the user expects the log view
/// and header to remain interactive (scrolling, clicking links).  The
/// `is_modal_ui_mode` function intentionally excludes `LinkHighlight`, so the
/// header shortcut regions ARE registered in that mode.
#[test]
fn phase5_5_renderer_invariant_link_highlight_keeps_main_header_regions() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut state = fdemon_app::state::AppState::new();
    state.ui_mode = fdemon_app::state::UiMode::LinkHighlight;

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| view(f, &mut state)).unwrap();

    let hot_reload_count = count_hot_reload_regions(&state);
    assert!(
        hot_reload_count > 0,
        "LinkHighlight mode must keep header (base-UI) regions registered \
         (it is NOT modal for the renderer gate); found 0 HotReload regions at 120 cols"
    );
}

// ===========================================================================
// Normal Mode Snapshots
// ===========================================================================

// Redact the rendered `vX.Y.Z` so version bumps in `Cargo.toml` don't
// invalidate stored snapshots. Filtered content matches the literal
// `vX.Y.Z` placeholder baked into the `.snap` files.
const VERSION_FILTER: &[(&str, &str)] =
    &[(r"Flutter Demon v\d+\.\d+\.\d+", "Flutter Demon vX.Y.Z")];

#[test]
fn snapshot_normal_mode_initializing() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Initializing;

    let content = render_screen(&mut state);
    insta::with_settings!({ filters => VERSION_FILTER.to_vec() }, {
        assert_snapshot!("normal_initializing", content);
    });
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
    insta::with_settings!({ filters => VERSION_FILTER.to_vec() }, {
        assert_snapshot!("normal_running", content);
    });
}

#[test]
fn snapshot_normal_mode_reloading() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Reloading;

    let content = render_screen(&mut state);
    insta::with_settings!({ filters => VERSION_FILTER.to_vec() }, {
        assert_snapshot!("normal_reloading", content);
    });
}

#[test]
fn snapshot_normal_mode_stopped() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Stopped;

    let content = render_screen(&mut state);
    insta::with_settings!({ filters => VERSION_FILTER.to_vec() }, {
        assert_snapshot!("normal_stopped", content);
    });
}

// ===========================================================================
// Toast overlay tests (Minor #15)
// ===========================================================================

/// Empty toast list must not panic and must not write anywhere near the
/// bottom (background remains untouched). This is the no-op guard for the
/// `render_toasts` loop.
#[test]
fn render_with_no_toasts_does_not_panic() {
    let mut state = create_base_state();
    assert!(state.toasts.is_empty(), "precondition: no toasts");

    // Render — `render_toasts` is called via `view()`.
    let _ = render_screen(&mut state);
}

/// A single Warn toast lands somewhere in the rendered output. We do not
/// pin the exact row (layout-dependent) — just that the message text and
/// the warn glyph both appear.
#[test]
fn render_warn_toast_appears_in_output() {
    use fdemon_app::state::ToastLevel;

    let mut state = create_base_state();
    state.push_toast(ToastLevel::Warn, "test warn toast");

    let content = render_screen(&mut state);

    assert!(
        content.contains("test warn toast"),
        "Warn toast text should be present in rendered output"
    );
    assert!(
        content.contains('\u{26A0}'),
        "Warn toast icon (⚠) should be present in rendered output"
    );
}

/// Info toast renders with the info glyph instead of the warn glyph.
#[test]
fn render_info_toast_uses_info_glyph() {
    use fdemon_app::state::ToastLevel;

    let mut state = create_base_state();
    state.push_toast(ToastLevel::Info, "test info toast");

    let content = render_screen(&mut state);

    assert!(
        content.contains("test info toast"),
        "Info toast text should be present"
    );
    assert!(
        content.contains('\u{2139}'),
        "Info toast icon (ℹ) should be present in rendered output"
    );
}

/// Multiple toasts stack — both messages must be visible simultaneously.
#[test]
fn render_multiple_toasts_stack_without_overlap() {
    use fdemon_app::state::ToastLevel;

    let mut state = create_base_state();
    state.push_toast(ToastLevel::Warn, "first toast");
    state.push_toast(ToastLevel::Info, "second toast");

    let content = render_screen(&mut state);

    assert!(content.contains("first toast"), "first toast missing");
    assert!(content.contains("second toast"), "second toast missing");
}

/// Toast text longer than the available width is truncated with an ellipsis.
/// The truncation budget uses the ICON_DISPLAY_WIDTH constant — this test
/// guards against a regression where the magic number diverges from the
/// actual icon width.
#[test]
fn render_long_toast_text_is_truncated_with_ellipsis() {
    use fdemon_app::state::ToastLevel;

    let mut state = create_base_state();
    // 200 chars — far longer than a 120-col terminal can fit.
    let long_text = "a".repeat(200);
    state.push_toast(ToastLevel::Warn, long_text.clone());

    let content = render_screen(&mut state);

    // The full text must NOT appear verbatim (would require >120 cols).
    assert!(
        !content.contains(&long_text),
        "long toast text should be truncated, not rendered in full"
    );
    // The ellipsis marker indicates truncation happened.
    assert!(
        content.contains('\u{2026}'),
        "ellipsis (…) should appear when toast is truncated"
    );
}
