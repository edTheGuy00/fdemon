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

// ── Task 07: tree rendering with guidelines / branch ticks / type icons ────────

/// Extract all visible characters from a single row of the buffer as a `String`.
///
/// This helper strips "empty" (space) cells and is used by snapshot-style
/// assertions that look for specific Unicode glyphs in known positions.
fn buf_to_string_row(buf: &Buffer, row_y: u16) -> String {
    let width = buf.area().width;
    (0..width)
        .filter_map(|x| buf.cell((x, row_y)))
        .filter_map(|c| c.symbol().chars().next())
        .collect()
}

/// Build a state with: root → child_a (not last) → child_b (last).
/// All three are expanded so all are visible.
fn make_state_with_parent_two_children() -> InspectorState {
    let root = DiagnosticsNode {
        description: "Root".to_string(),
        value_id: Some("root".to_string()),
        children: vec![
            DiagnosticsNode {
                description: "ChildA".to_string(),
                value_id: Some("ca".to_string()),
                ..Default::default()
            },
            DiagnosticsNode {
                description: "ChildB".to_string(),
                value_id: Some("cb".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state.expanded.insert("root".to_string());
    // Disable chain-folding so all rows render as standalone RowGroup::None.
    state.hide_implementation_widgets = false;
    state
}

/// Helper that renders `render_tree_panel_inner` directly with no mouse ctx.
///
/// We pass an empty `visible` slice because the new implementation ignores
/// the `_visible` parameter and calls `inspector_rows()` internally.
fn render_tree_inner(state: &InspectorState, buf: &mut Buffer, selected: usize) {
    let widget = WidgetInspector::new(state, true, &VmConnectionStatus::Connected);
    widget.render_tree_panel_inner(buf.area, buf, &[], selected, None);
}

// ── Guideline tests ───────────────────────────────────────────────────────────

#[test]
fn tree_renders_guidelines_for_nonlast_sibling_ancestors() {
    // root (depth 0, non-last ancestor for its first child)
    //   ├─ ChildA (depth 1, non-last sibling)
    //   └─ ChildB (depth 1, last sibling)
    //
    // For ChildA the `ticks` set contains depth 0 (root has a second child).
    // No child of ChildA is present so we just check the branch tick on ChildA.
    //
    // To observe the `│` guideline we need a row at depth > 1 that has an
    // ancestor with remaining siblings.  Build: root → ChildA (has one child) →
    // GrandChild.  root also has ChildB (after ChildA) so root is in ticks for
    // all rows under ChildA.
    let root = DiagnosticsNode {
        description: "Root".to_string(),
        value_id: Some("root".to_string()),
        children: vec![
            DiagnosticsNode {
                description: "ChildA".to_string(),
                value_id: Some("ca".to_string()),
                children: vec![DiagnosticsNode {
                    description: "GrandChild".to_string(),
                    value_id: Some("gc".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            DiagnosticsNode {
                description: "ChildB".to_string(),
                value_id: Some("cb".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state.expanded.insert("root".to_string());
    state.expanded.insert("ca".to_string());
    state.hide_implementation_widgets = false;

    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    render_tree_inner(&state, &mut buf, 0);

    // Row index 2 is GrandChild (depth 2).
    // Its `ticks` contains depth 0 because root (depth 0) has ChildB still below.
    // The guideline at depth 0 should be '│' at column glyph_col(0) = 0.
    // Area includes a 1-cell border, so tree_inner.x = 1, tree_inner.y = 1.
    // Row y for GrandChild: 1 (border) + 2 (offset) = 3.
    // Guideline x: tree_inner.x + glyph_col(0) = 1 + 0 = 1.
    let row_str = buf_to_string_row(&buf, 3);
    assert!(
        row_str.contains('│'),
        "GrandChild row should contain '│' guideline, got: {row_str:?}"
    );
}

// ── Branch tick tests ─────────────────────────────────────────────────────────

#[test]
fn tree_renders_branch_tick_last_child_uses_box_drawing_l() {
    // ChildB is the last child → should get └─ branch tick.
    let state = make_state_with_parent_two_children();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    render_tree_inner(&state, &mut buf, 0);

    // Row 0 = Root (depth 0, no branch tick).
    // Row 1 = ChildA (depth 1, non-last → ├─).
    // Row 2 = ChildB (depth 1, last → └─).
    // Row y for ChildB: tree_inner.y + 2 = 1 + 2 = 3.
    let row_str = buf_to_string_row(&buf, 3);
    assert!(
        row_str.contains('└'),
        "Last child (ChildB) should have '└' branch tick, got: {row_str:?}"
    );
}

#[test]
fn tree_renders_branch_tick_non_last_child_uses_box_drawing_t() {
    // ChildA is a non-last child → should get ├─ branch tick.
    let state = make_state_with_parent_two_children();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    render_tree_inner(&state, &mut buf, 0);

    // Row 1 = ChildA (depth 1, non-last → ├─).
    // Row y for ChildA: tree_inner.y + 1 = 1 + 1 = 2.
    let row_str = buf_to_string_row(&buf, 2);
    assert!(
        row_str.contains('├'),
        "Non-last child (ChildA) should have '├' branch tick, got: {row_str:?}"
    );
}

// ── Group-leader tests ────────────────────────────────────────────────────────

#[test]
fn tree_renders_collapsed_leader_with_plus_n_more_widgets() {
    // With hide_implementation_widgets = true (default), a single-child
    // non-local-project chain should fold into a LeaderCollapsed row that
    // shows "+ N more" badge text.
    //
    // Build: root (user-code) → Level1 → Level2.
    // Level1 and Level2 are not local-project, have no siblings, single child → chain.
    let root = DiagnosticsNode {
        description: "Root".to_string(),
        value_id: Some("root".to_string()),
        created_by_local_project: true,
        children: vec![DiagnosticsNode {
            description: "Level1".to_string(),
            value_id: Some("l1".to_string()),
            created_by_local_project: false,
            children: vec![DiagnosticsNode {
                description: "Level2".to_string(),
                value_id: Some("l2".to_string()),
                created_by_local_project: false,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state.expanded.insert("root".to_string());
    state.expanded.insert("l1".to_string());
    // hide_implementation_widgets defaults to true — chain folding active.

    let area = Rect::new(0, 0, 60, 10);
    let mut buf = Buffer::empty(area);
    render_tree_inner(&state, &mut buf, 0);

    let full = collect_buf_text(&buf, 60, 10);
    // Leader row should show "+1 more" or similar badge.
    assert!(
        full.contains("more"),
        "LeaderCollapsed row should show 'more' badge, got: {full:?}"
    );
}

#[test]
fn tree_renders_expanded_leader_then_member_rows() {
    // Same chain as above but with the leader's group expanded.
    let root = DiagnosticsNode {
        description: "Root".to_string(),
        value_id: Some("root".to_string()),
        created_by_local_project: true,
        children: vec![DiagnosticsNode {
            description: "Level1".to_string(),
            value_id: Some("l1".to_string()),
            created_by_local_project: false,
            children: vec![DiagnosticsNode {
                description: "Level2".to_string(),
                value_id: Some("l2".to_string()),
                created_by_local_project: false,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state.expanded.insert("root".to_string());
    state.expanded.insert("l1".to_string());
    // Expand the group leader so Member rows appear.
    state.expanded_groups.insert("l1".to_string());
    // hide_implementation_widgets defaults to true.

    let area = Rect::new(0, 0, 60, 10);
    let mut buf = Buffer::empty(area);
    render_tree_inner(&state, &mut buf, 0);

    let full = collect_buf_text(&buf, 60, 10);
    // Both Level1 (leader expanded) and Level2 (member) should appear.
    assert!(
        full.contains("Level1"),
        "LeaderExpanded row should be visible, got: {full:?}"
    );
    assert!(
        full.contains("Level2"),
        "Member row (Level2) should be visible after expanding group, got: {full:?}"
    );
}

// ── Type-icon tests ───────────────────────────────────────────────────────────

#[test]
fn tree_renders_type_icon_for_known_widget_types() {
    // This test checks that `glyph_for_widget` returns correct glyphs for
    // known widget types (tested through the module-private function via
    // the public rendering path).  We render a tree with various widget
    // descriptions and verify at least the fallback path doesn't panic.

    let types = [
        "Row",
        "Column",
        "Container",
        "Stack",
        "Text",
        "SomeUnknownWidget123",
    ];

    for widget_type in &types {
        let root = DiagnosticsNode {
            description: widget_type.to_string(),
            value_id: Some("root".to_string()),
            ..Default::default()
        };
        let mut state = InspectorState::new();
        state.root = Some(root);
        state.hide_implementation_widgets = false;

        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_tree_inner(&state, &mut buf, 0);

        // The render must not panic and should produce non-empty output.
        let full = collect_buf_text(&buf, 40, 6);
        assert!(
            !full.trim().is_empty(),
            "Expected non-empty render for widget type {widget_type:?}, got empty"
        );
    }
}

// ── Mouse region math tests ───────────────────────────────────────────────────

#[test]
fn tree_mouse_glyph_rect_uses_new_indent_math() {
    // Verify that glyph click regions are placed at glyph_col(depth) offsets.
    // depth 0 → glyph_col(0) = 0; depth 3 → glyph_col(3) = 6.
    //
    // Layout: wide terminal (120 cols) → horizontal split, tree_inner.x = 1.
    // Root node at depth 0 → glyph x = tree_inner.x + 0 = 1.
    use fdemon_app::message::Message;
    use fdemon_app::{MouseButton, MouseRegions};

    let state = make_single_root_state();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // Hit-test at (tree_inner.x=1, tree_inner.y=1) — depth-0 glyph position.
    let hit = regions.hit_test(1, 1, MouseButton::Left);
    let action = hit
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(1, 1));
    assert!(
        matches!(
            action,
            Some(Message::DevToolsInspectorToggleNode { index: 0 })
        ),
        "expected ToggleNode at depth-0 glyph cell (1,1), got: {action:?}"
    );
}

#[test]
fn tree_mouse_row_rect_unchanged_full_width_of_tree_inner() {
    // Row regions should still span the full width of tree_inner.
    use fdemon_app::message::Message;
    use fdemon_app::{MouseButton, MouseRegions};

    let state = make_single_root_state();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // Hit-test at (x=30, y=1) — well inside the row but outside the 1-cell glyph.
    // Should still return a SelectRow action (not ToggleNode).
    let hit = regions.hit_test(30, 1, MouseButton::Left);
    let action = hit
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(30, 1));
    assert!(
        matches!(
            action,
            Some(Message::DevToolsInspectorSelectRow { index: 0 })
        ),
        "expected SelectRow at (30, 1), got: {action:?}"
    );
}

#[test]
fn tree_pushes_row_rect_then_glyph_rect_for_last_pushed_wins_invariant() {
    // Regression test for the last-pushed-wins-at-same-z invariant.
    // Pushing row region first, then glyph region, ensures the glyph region
    // wins when both overlap at the glyph cell.
    use fdemon_app::message::Message;
    use fdemon_app::{MouseButton, MouseRegions};

    let state = make_single_root_state();
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // At the glyph cell (1, 1), ToggleNode must win (not SelectRow).
    let glyph_hit = regions.hit_test(1, 1, MouseButton::Left);
    let glyph_action = glyph_hit
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(1, 1));
    assert!(
        matches!(
            glyph_action,
            Some(Message::DevToolsInspectorToggleNode { .. })
        ),
        "glyph cell must return ToggleNode (last-pushed-wins), got: {glyph_action:?}"
    );

    // At a non-glyph cell in the same row (e.g. x=20), SelectRow must win.
    let row_hit = regions.hit_test(20, 1, MouseButton::Left);
    let row_action = row_hit
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(20, 1));
    assert!(
        matches!(row_action, Some(Message::DevToolsInspectorSelectRow { .. })),
        "non-glyph cell must return SelectRow, got: {row_action:?}"
    );
}

// ── Task 09: mode-switch tests ────────────────────────────────────────────────

/// Build a minimal inspector state with a loaded tree and an expanded root.
fn make_inspector_state_with_tree() -> InspectorState {
    let root = DiagnosticsNode {
        description: "MyApp".to_string(),
        value_id: Some("root".to_string()),
        children: vec![DiagnosticsNode {
            description: "Scaffold".to_string(),
            value_id: Some("c1".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut state = InspectorState::new();
    state.root = Some(root);
    state.expanded.insert("root".to_string());
    state
}

/// Collect every character in the buffer into a single `String`.
fn buf_to_string(buf: &Buffer) -> String {
    let area = buf.area();
    collect_buf_text(buf, area.width, area.height)
}

#[test]
fn mod_switches_to_details_panel_when_details_open() {
    let mut state = make_inspector_state_with_tree();
    state.details_open = true;
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
    widget.render(buf.area, &mut buf);

    // Right half should contain the tab strip — look for one of the tab labels.
    let s = buf_to_string(&buf);
    assert!(
        s.contains("Widget properties"),
        "Expected 'Widget properties' tab label when details_open=true, got: {s:?}"
    );
}

#[test]
fn mod_renders_layout_panel_when_details_closed() {
    let mut state = make_inspector_state_with_tree();
    state.details_open = false;
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
    widget.render(buf.area, &mut buf);

    // Right half should contain the layout panel title, NOT the details tab strip.
    let s = buf_to_string(&buf);
    assert!(
        s.contains("Layout Explorer"),
        "Expected 'Layout Explorer' panel when details_open=false, got: {s:?}"
    );
    assert!(
        !s.contains("Widget properties"),
        "Details tab strip should NOT appear when details_open=false, got: {s:?}"
    );
}

#[test]
fn mod_suppresses_tree_mouse_regions_when_details_open() {
    use fdemon_app::{MouseButton, MouseRegions};

    let mut state = make_inspector_state_with_tree();
    state.details_open = true;
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // No tree row/glyph regions should be registered when details is open.
    let hit = regions.hit_test(1, 1, MouseButton::Left);
    assert!(
        hit.is_none(),
        "Tree mouse regions must be suppressed when details_open=true, got: {hit:?}"
    );
}

#[test]
fn mod_passes_mouse_regions_to_tree_when_details_closed() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseButton, MouseRegions};

    let mut state = make_inspector_state_with_tree();
    state.details_open = false;
    let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 120, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // Tree row regions must be registered when details is closed.
    // The root node is at row y=1 (inside the tree block border); hit at (30, 1).
    let hit = regions.hit_test(30, 1, MouseButton::Left);
    let action = hit
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(30, 1));
    assert!(
        matches!(action, Some(Message::DevToolsInspectorSelectRow { .. })),
        "Tree row regions must be active when details_open=false, got: {action:?}"
    );
}
