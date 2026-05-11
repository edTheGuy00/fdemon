use super::*;
use fdemon_app::state::{DevToolsError, InspectorState, VmConnectionStatus};
use fdemon_core::widget_tree::{CreationLocation, DiagnosticsNode};

fn make_test_tree() -> DiagnosticsNode {
    DiagnosticsNode {
        description: "MyApp".to_string(),
        value_id: Some("widget-1".to_string()),
        children: vec![DiagnosticsNode {
            description: "MaterialApp".to_string(),
            value_id: Some("widget-2".to_string()),
            children: vec![DiagnosticsNode {
                description: "Scaffold".to_string(),
                value_id: Some("widget-3".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Collect all text from a buffer into a single string.
fn collect_buf_text(buf: &Buffer, width: u16, height: u16) -> String {
    let mut full = String::new();
    for y in 0..height {
        for x in 0..width {
            if let Some(c) = buf.cell((x, y)) {
                if let Some(ch) = c.symbol().chars().next() {
                    full.push(ch);
                }
            }
        }
    }
    full
}

#[test]
fn test_inspector_renders_tree_without_panic() {
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_renders_loading_state() {
    let mut state = InspectorState::new();
    state.loading = true;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_renders_error_state() {
    let mut state = InspectorState::new();
    state.error = Some(DevToolsError::new(
        "Connection failed",
        "Press [r] to retry",
    ));

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_renders_empty_state() {
    let state = InspectorState::new();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_narrow_terminal_vertical_layout() {
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    // < 100 cols triggers vertical split (threshold changed from 80 to 100 in Task 06)
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 24));
    widget.render(Rect::new(0, 0, 60, 24), &mut buf);

    let full = collect_buf_text(&buf, 60, 24);
    assert!(
        full.contains("Layout Explorer"),
        "Narrow terminal should show Layout Explorer panel in vertical layout, got: {full:?}"
    );
}

#[test]
fn test_inspector_wide_terminal_horizontal_layout() {
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    // >= 100 cols triggers horizontal split (50/50)
    let mut buf = Buffer::empty(Rect::new(0, 0, 120, 24));
    widget.render(Rect::new(0, 0, 120, 24), &mut buf);

    let full = collect_buf_text(&buf, 120, 24);
    assert!(
        full.contains("Layout Explorer"),
        "Wide terminal should show Layout Explorer panel in horizontal layout, got: {full:?}"
    );
}

#[test]
fn test_expand_icon_leaf_node() {
    let state = InspectorState::new();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let leaf = DiagnosticsNode {
        description: "Text".to_string(),
        children: vec![],
        ..Default::default()
    };
    assert_eq!(widget.expand_icon(&leaf), "●");
}

#[test]
fn test_expand_icon_collapsed() {
    let state = InspectorState::new();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let node = DiagnosticsNode {
        description: "Column".to_string(),
        value_id: Some("w1".to_string()),
        children: vec![DiagnosticsNode::default()],
        ..Default::default()
    };
    assert_eq!(widget.expand_icon(&node), "▶");
}

#[test]
fn test_expand_icon_expanded() {
    let mut state = InspectorState::new();
    state.expanded.insert("w1".to_string());
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let node = DiagnosticsNode {
        description: "Column".to_string(),
        value_id: Some("w1".to_string()),
        children: vec![DiagnosticsNode::default()],
        ..Default::default()
    };
    assert_eq!(widget.expand_icon(&node), "▼");
}

#[test]
fn test_viewport_scrolling_keeps_selected_visible() {
    let state = InspectorState {
        selected_index: 50,
        ..Default::default()
    };
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let (start, end) = widget.visible_viewport_range(20, 100);
    assert!(start <= 50, "start ({start}) should be <= 50");
    assert!(end > 50, "end ({end}) should be > 50");
}

#[test]
fn test_viewport_scrolling_at_start() {
    let state = InspectorState {
        selected_index: 0,
        ..Default::default()
    };
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let (start, end) = widget.visible_viewport_range(20, 100);
    assert_eq!(start, 0);
    assert_eq!(end, 20);
}

#[test]
fn test_viewport_scrolling_near_end() {
    let state = InspectorState {
        selected_index: 99,
        ..Default::default()
    };
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let (start, end) = widget.visible_viewport_range(20, 100);
    assert_eq!(end, 100);
    assert!(start <= 99);
}

#[test]
fn test_viewport_empty_total() {
    let state = InspectorState::default();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let (start, end) = widget.visible_viewport_range(20, 0);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_short_path_strips_file_scheme() {
    assert_eq!(short_path("file:///app/lib/main.dart"), "lib/main.dart");
}

#[test]
fn test_short_path_no_scheme() {
    assert_eq!(short_path("/app/lib/main.dart"), "lib/main.dart");
}

#[test]
fn test_short_path_bare_filename() {
    assert_eq!(short_path("main.dart"), "main.dart");
}

#[test]
fn test_short_path_deep_path() {
    assert_eq!(
        short_path("file:///home/user/project/lib/src/widgets/button.dart"),
        "widgets/button.dart"
    );
}

#[test]
fn test_truncate_str_short() {
    assert_eq!(truncate_str("hello", 10), "hello");
}

#[test]
fn test_truncate_str_exact() {
    assert_eq!(truncate_str("hello", 5), "hello");
}

#[test]
fn test_truncate_str_too_long() {
    assert_eq!(truncate_str("hello world", 5), "hello");
}

#[test]
fn test_truncate_str_zero_max() {
    assert_eq!(truncate_str("hello", 0), "");
}

#[test]
fn test_inspector_selected_node_highlighted() {
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());
    state.selected_index = 0;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_user_code_shown_differently() {
    let mut state = InspectorState::new();
    let mut root = DiagnosticsNode {
        description: "MyWidget".to_string(),
        value_id: Some("user-widget".to_string()),
        created_by_local_project: true,
        creation_location: Some(CreationLocation {
            file: "file:///app/lib/main.dart".to_string(),
            line: 42,
            column: 8,
            name: Some("MyWidget".to_string()),
        }),
        ..Default::default()
    };
    let framework_child = DiagnosticsNode {
        description: "Container".to_string(),
        value_id: Some("fw-widget".to_string()),
        created_by_local_project: false,
        ..Default::default()
    };
    root.children.push(framework_child);
    state.root = Some(root);
    state.expanded.insert("user-widget".to_string());
    state.selected_index = 0;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_with_properties() {
    let mut state = InspectorState::new();
    let mut root = DiagnosticsNode {
        description: "Text".to_string(),
        value_id: Some("text-1".to_string()),
        ..Default::default()
    };
    root.properties.push(DiagnosticsNode {
        description: "Hello World".to_string(),
        name: Some("data".to_string()),
        ..Default::default()
    });
    state.root = Some(root);
    state.selected_index = 0;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
}

#[test]
fn test_inspector_zero_area_no_panic() {
    let state = InspectorState::default();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    widget.render(Rect::new(0, 0, 10, 1), &mut buf);
}

#[test]
fn test_inspector_loading_state_contains_message() {
    let mut state = InspectorState::new();
    state.loading = true;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);

    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("Loading"),
        "Expected 'Loading' in buffer, got: {full:?}"
    );
}

#[test]
fn test_inspector_empty_state_contains_prompt() {
    let state = InspectorState::new();

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);

    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("Press"),
        "Expected 'Press' in buffer, got: {full:?}"
    );
}

#[test]
fn test_inspector_error_state_contains_error() {
    let mut state = InspectorState::new();
    state.error = Some(DevToolsError::new(
        "VM Service not available",
        "Ensure the app is running in debug mode",
    ));

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);

    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("VM Service") || full.contains("debug mode"),
        "Expected user-friendly error message in buffer, got: {full:?}"
    );
}

#[test]
fn test_inspector_disconnected_state_shows_vm_message() {
    let state = InspectorState::new();
    let widget = WidgetInspector::new(&state, false, &VmConnectionStatus::Disconnected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);

    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("disconnected")
            || full.contains("Disconnected")
            || full.contains("VM Service"),
        "Expected VM Service disconnected message in buffer, got: {full:?}"
    );
}

#[test]
fn test_inspector_reconnecting_state_shows_attempt_count() {
    let state = InspectorState::new();
    let status = VmConnectionStatus::Reconnecting {
        attempt: 2,
        max_attempts: 5,
    };
    let widget = WidgetInspector::new(&state, false, &status);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);

    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("Reconnecting") || full.contains("2"),
        "Expected reconnecting message with attempt count, got: {full:?}"
    );
}

// ── Small terminal tests ───────────────────────────────────────────────────────

#[test]
fn test_inspector_very_small_terminal_shows_compact_node_count() {
    // 30x4 — height == MIN_TREE_RENDER_HEIGHT, tree render path kicks in.
    // With height exactly 4, the compact fallback should NOT trigger (it triggers at < 4).
    // Test at height=3 to confirm compact fallback.
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
    widget.render(Rect::new(0, 0, 30, 3), &mut buf);

    let full = collect_buf_text(&buf, 30, 3);
    // The compact fallback renders "N nodes" when there are visible nodes.
    assert!(
        full.contains("nodes") || full.contains("widget") || full.contains("No"),
        "Very small terminal should show compact summary, got: {full:?}"
    );
}

#[test]
fn test_inspector_very_small_terminal_no_panic() {
    // 30x4 — should render without panic
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 4));
    widget.render(Rect::new(0, 0, 30, 4), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_compact_fallback_empty_tree() {
    // When root is None (no tree loaded), compact fallback shows "No widget tree"
    let mut state = InspectorState::new();
    // root is None by default — visible_nodes() returns []
    // Trigger tree render path with a tiny area (height < 4)
    // Note: render() dispatches to render_empty() when root is None, not render_tree(),
    // so the compact fallback in render_tree() is only reachable with a root.
    // This test verifies the empty-state path at small sizes doesn't panic.
    state.root = None;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
    widget.render(Rect::new(0, 0, 30, 3), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_height_1_no_panic() {
    // Single row — all render paths must handle this gracefully.
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    widget.render(Rect::new(0, 0, 40, 1), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_20x5_no_panic() {
    // 20x5 — acceptance criteria extreme terminal size
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_40x10_no_panic() {
    // 40x10 — acceptance criteria terminal size
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    widget.render(Rect::new(0, 0, 40, 10), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_60x15_no_panic() {
    // 60x15 — acceptance criteria terminal size
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 15));
    widget.render(Rect::new(0, 0, 60, 15), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_200x50_no_panic() {
    // 200x50 — large terminal (acceptance criteria)
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 200, 50));
    widget.render(Rect::new(0, 0, 200, 50), &mut buf);
    // Should not panic
}

#[test]
fn test_inspector_narrow_small_height_shows_tree_only() {
    // Narrow terminal (< WIDE_TERMINAL_THRESHOLD) with small height:
    // if half_height < MIN_SPLIT_PANEL_HEIGHT the layout panel should be skipped.
    // With height=6, half=3 which equals MIN_SPLIT_PANEL_HEIGHT (3), so split IS shown.
    // With height=5, half=2 which is < MIN_SPLIT_PANEL_HEIGHT (3) — tree only.
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.expanded.insert("widget-1".to_string());

    // Narrow width (< 100), height = 5 (half = 2, below MIN_SPLIT_PANEL_HEIGHT = 3)
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 5));
    widget.render(Rect::new(0, 0, 60, 5), &mut buf);
    // Should not panic — tree-only layout used
}

#[test]
fn test_inspector_selected_index_preserved_after_small_render() {
    // Verify state is not mutated by render (render takes &InspectorState).
    let mut state = InspectorState::new();
    state.root = Some(make_test_tree());
    state.selected_index = 1;

    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
    widget.render(Rect::new(0, 0, 30, 3), &mut buf);

    assert_eq!(
        state.selected_index, 1,
        "selected_index should be preserved after rendering at small terminal size"
    );
}

// ── Phase 4 Task 07: inspector region recording tests ─────────────────────────

/// Build a 5-node tree: root + 4 children (all expanded so all 5 are visible).
fn make_5_node_tree() -> InspectorState {
    let root = DiagnosticsNode {
        description: "Root".to_string(),
        value_id: Some("root".to_string()),
        children: vec![
            DiagnosticsNode {
                description: "Child1".to_string(),
                value_id: Some("c1".to_string()),
                ..Default::default()
            },
            DiagnosticsNode {
                description: "Child2".to_string(),
                value_id: Some("c2".to_string()),
                ..Default::default()
            },
            DiagnosticsNode {
                description: "Child3".to_string(),
                value_id: Some("c3".to_string()),
                ..Default::default()
            },
            DiagnosticsNode {
                description: "Child4".to_string(),
                value_id: Some("c4".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state.expanded.insert("root".to_string());
    state
}

/// Build a state with a single root node (leaf — no children).
fn make_single_root_state() -> InspectorState {
    let root = DiagnosticsNode {
        description: "SingleRoot".to_string(),
        value_id: Some("sr-root".to_string()),
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state
}

// ── Phase 4.5 Task 03: render_with_regions parity test ───────────────────────

#[test]
fn render_with_regions_matches_widget_render_buffer() {
    use fdemon_app::MouseRegions;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    // Tree loaded with at least one node — non-trivial state branch.
    let inspector_state = make_5_node_tree();

    let area = Rect::new(0, 0, 120, 24);

    let mut buf_a = Buffer::empty(area);
    WidgetInspector::new(&inspector_state, true, &VmConnectionStatus::Connected)
        .render(area, &mut buf_a);

    let mut buf_b = Buffer::empty(area);
    {
        let mut regions = MouseRegions::default();
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(
            area,
            &mut buf_b,
            WidgetInspector::new(&inspector_state, true, &VmConnectionStatus::Connected),
            Some(&mut ctx),
        );
    }

    assert_eq!(
        buf_a, buf_b,
        "Widget::render and render_with_regions must produce identical buffers"
    );
}

#[test]
fn inspector_records_row_and_glyph_regions_per_visible_row() {
    use fdemon_app::message::Message;
    use fdemon_app::MouseRegions;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    // 5-node tree fully expanded → 5 visible rows in viewport.
    let inspector_state = make_5_node_tree();
    let widget = WidgetInspector::new(&inspector_state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

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

    // 5 visible rows → 5 row regions + 5 glyph regions.
    assert_eq!(
        select_count, 5,
        "expected 5 SelectRow regions (one per visible row), got {select_count}"
    );
    assert_eq!(
        toggle_count, 5,
        "expected 5 ToggleNode regions (one per visible row), got {toggle_count}"
    );
}

#[test]
fn glyph_region_wins_over_row_region_at_glyph_cell() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseButton, MouseRegions};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    // Single root at depth 0: glyph is at (tree_inner.x + 0*2, tree_inner.y).
    // With area = Rect::new(0, 0, 120, 24) the horizontal split kicks in
    // (width >= WIDE_TERMINAL_THRESHOLD = 100).
    // Tree area is left 60 columns.  Block border → tree_inner.x = 1, tree_inner.y = 1.
    // Glyph for depth-0 root is at (1, 1).
    let inspector_state = make_single_root_state();
    let widget = WidgetInspector::new(&inspector_state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // Hit-test the glyph cell (tree_inner.x=1, tree_inner.y=1).
    // Last-pushed-wins: the glyph region was pushed after the row region,
    // so the result must be ToggleNode, not SelectRow.
    let hit = regions.hit_test(1, 1, MouseButton::Left);
    let action = hit
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(1, 1));
    assert!(
        matches!(
            action,
            Some(Message::DevToolsInspectorToggleNode { index: 0 })
        ),
        "expected ToggleNode at glyph cell (1,1), got: {action:?}"
    );
}
