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
