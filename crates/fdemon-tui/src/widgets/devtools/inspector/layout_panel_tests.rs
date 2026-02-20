use super::*;
use fdemon_app::state::{DevToolsError, InspectorState, VmConnectionStatus};
use fdemon_core::widget_tree::{
    BoxConstraints, CreationLocation, DiagnosticsNode, EdgeInsets, LayoutInfo, WidgetSize,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// ── Test helpers ──────────────────────────────────────────────────────────────

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

fn make_layout_with_all() -> LayoutInfo {
    LayoutInfo {
        constraints: Some(BoxConstraints {
            min_width: 0.0,
            max_width: 414.0,
            min_height: 0.0,
            max_height: 896.0,
        }),
        size: Some(WidgetSize {
            width: 200.0,
            height: 48.0,
        }),
        flex_factor: Some(1.0),
        flex_fit: Some("tight".to_string()),
        description: None,
        padding: Some(EdgeInsets {
            top: 8.0,
            right: 16.0,
            bottom: 8.0,
            left: 16.0,
        }),
        margin: None,
    }
}

fn make_node_with_location() -> DiagnosticsNode {
    DiagnosticsNode {
        description: "Column".to_string(),
        value_id: Some("col-1".to_string()),
        created_by_local_project: true,
        creation_location: Some(CreationLocation {
            file: "file:///app/lib/screens/home.dart".to_string(),
            line: 42,
            column: 8,
            name: Some("Column".to_string()),
        }),
        ..Default::default()
    }
}

/// Render the layout panel and return the buffer text.
fn render_panel_with_node(
    state: &InspectorState,
    node: Option<DiagnosticsNode>,
    width: u16,
    height: u16,
) -> String {
    let nodes: Vec<DiagnosticsNode> = node.into_iter().collect();
    let refs: Vec<(&DiagnosticsNode, usize)> = nodes.iter().map(|n| (n, 0)).collect();
    let widget = WidgetInspector::new(state, true, &VmConnectionStatus::Connected);
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    widget.render_layout_panel(Rect::new(0, 0, width, height), &mut buf, &refs, 0);
    collect_buf_text(&buf, width, height)
}

fn render_panel(state: &InspectorState, width: u16, height: u16) -> String {
    render_panel_with_node(state, None, width, height)
}

// ── State rendering tests ─────────────────────────────────────────────────────

#[test]
fn test_layout_panel_loading_state() {
    let mut state = InspectorState::new();
    state.layout_loading = true;
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("Loading"),
        "Expected 'Loading' in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_error_state() {
    let mut state = InspectorState::new();
    state.layout_error = Some(DevToolsError::new(
        "VM Service not available",
        "Ensure app is in debug mode",
    ));
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("VM Service") || text.contains("debug mode"),
        "Expected error message in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_empty_state() {
    let state = InspectorState::new();
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("Select"),
        "Expected 'Select' in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_empty_state_full_phrase() {
    let state = InspectorState::new();
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("Select") && text.contains("widget"),
        "Expected full empty-state phrase in buffer, got: {text:?}"
    );
}

// ── Widget name + source location ─────────────────────────────────────────────

#[test]
fn test_layout_panel_shows_widget_name() {
    let mut state = InspectorState::new();
    state.layout = Some(make_layout_with_all());
    let text = render_panel_with_node(&state, Some(make_node_with_location()), 80, 24);
    assert!(
        text.contains("Column"),
        "Expected 'Column' in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_source_location() {
    let mut state = InspectorState::new();
    state.layout = Some(make_layout_with_all());
    let text = render_panel_with_node(&state, Some(make_node_with_location()), 80, 24);
    assert!(
        text.contains("home.dart") && text.contains("42"),
        "Expected 'home.dart:42' in buffer, got: {text:?}"
    );
}

// ── Box model ─────────────────────────────────────────────────────────────────

#[test]
fn test_layout_panel_shows_box_model_with_padding() {
    let mut state = InspectorState::new();
    state.layout = Some(make_layout_with_all());
    let text = render_panel(&state, 80, 30);
    assert!(
        text.contains("padding"),
        "Expected 'padding' block in buffer, got: {text:?}"
    );
    assert!(
        text.contains("widget"),
        "Expected 'widget' block in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_shows_size_box_without_padding() {
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        size: Some(WidgetSize {
            width: 200.0,
            height: 48.0,
        }),
        padding: None,
        ..Default::default()
    });
    let text = render_panel(&state, 80, 30);
    assert!(
        text.contains("Size"),
        "Expected 'Size' block in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_dimensions_row() {
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        size: Some(WidgetSize {
            width: 200.0,
            height: 48.0,
        }),
        ..Default::default()
    });
    let text = render_panel(&state, 80, 20);
    assert!(
        text.contains("200.0") && text.contains("48.0"),
        "Expected dimensions in buffer, got: {text:?}"
    );
}

// ── Constraints ───────────────────────────────────────────────────────────────

#[test]
fn test_layout_panel_shows_constraints() {
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        constraints: Some(BoxConstraints {
            min_width: 0.0,
            max_width: f64::INFINITY,
            min_height: 0.0,
            max_height: f64::INFINITY,
        }),
        ..Default::default()
    });
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("Inf"),
        "Expected 'Inf' in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_shows_tight_indicator() {
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        constraints: Some(BoxConstraints {
            min_width: 100.0,
            max_width: 100.0,
            min_height: 50.0,
            max_height: 50.0,
        }),
        size: Some(WidgetSize {
            width: 100.0,
            height: 50.0,
        }),
        ..Default::default()
    });
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("tight"),
        "Expected '(tight)' in buffer, got: {text:?}"
    );
}

// ── Flex properties ───────────────────────────────────────────────────────────

#[test]
fn test_layout_panel_shows_flex_properties() {
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        flex_factor: Some(1.0),
        flex_fit: Some("tight".to_string()),
        ..Default::default()
    });
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("flex") || text.contains("fit"),
        "Expected flex properties in buffer, got: {text:?}"
    );
}

#[test]
fn test_layout_panel_flex_fit_loose() {
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        flex_factor: Some(2.0),
        flex_fit: Some("loose".to_string()),
        ..Default::default()
    });
    let text = render_panel(&state, 80, 24);
    assert!(
        text.contains("loose") || text.contains("flex"),
        "Expected flex/loose in buffer, got: {text:?}"
    );
}

// ── Compact mode ──────────────────────────────────────────────────────────────

#[test]
fn test_layout_panel_compact_mode_no_panic() {
    // height=3: outer border takes 2 rows → inner height=1, which is < COMPACT_MODE_HEIGHT
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        size: Some(WidgetSize {
            width: 200.0,
            height: 48.0,
        }),
        ..Default::default()
    });
    // Should not panic
    let _ = render_panel_with_node(
        &state,
        Some(DiagnosticsNode {
            description: "Column".to_string(),
            ..Default::default()
        }),
        80,
        3,
    );
}

#[test]
fn test_layout_panel_compact_mode_shows_content() {
    // height=6: inner=4 < COMPACT_MODE_HEIGHT=5 → compact mode
    let mut state = InspectorState::new();
    state.layout = Some(LayoutInfo {
        size: Some(WidgetSize {
            width: 200.0,
            height: 48.0,
        }),
        ..Default::default()
    });
    let text = render_panel_with_node(
        &state,
        Some(DiagnosticsNode {
            description: "Column".to_string(),
            ..Default::default()
        }),
        80,
        6,
    );
    // Compact mode should render something (200.0 and 48.0 in size_str)
    assert!(
        text.contains("200.0") || text.contains("Column"),
        "Expected compact content, got: {text:?}"
    );
}

// ── No panic / edge cases ─────────────────────────────────────────────────────

#[test]
fn test_layout_panel_zero_area_no_panic() {
    let state = InspectorState::new();
    let _ = render_panel(&state, 1, 1);
}

#[test]
fn test_layout_panel_renders_without_panic_with_all_data() {
    let mut state = InspectorState::new();
    state.layout = Some(make_layout_with_all());
    let _ = render_panel_with_node(&state, Some(make_node_with_location()), 80, 30);
}

// ── format_constraint_value ───────────────────────────────────────────────────

#[test]
fn test_format_constraint_value_infinity() {
    assert_eq!(format_constraint_value(f64::INFINITY), "Inf");
}

#[test]
fn test_format_constraint_value_large_value() {
    assert_eq!(format_constraint_value(1e10), "Inf");
}

#[test]
fn test_format_constraint_value_normal() {
    assert_eq!(format_constraint_value(414.0), "414.0");
}

#[test]
fn test_format_constraint_value_zero() {
    assert_eq!(format_constraint_value(0.0), "0.0");
}

#[test]
fn test_format_constraint_value_fractional() {
    assert_eq!(format_constraint_value(123.5), "123.5");
}
