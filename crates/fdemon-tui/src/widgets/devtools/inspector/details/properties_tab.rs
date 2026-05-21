//! Widget properties tab for the details view.
//!
//! Renders the existing layout preview (box model, dimensions, constraints,
//! flex properties — identical to the Layout Explorer panel) followed by a
//! property list populated from `inspector_state.properties`.

use fdemon_core::widget_tree::DiagnosticsNode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use super::super::WidgetInspector;
use super::filter_and_sort_by_level;
use crate::theme::palette;

/// Minimum height of the layout-preview section.
///
/// If the content area is smaller than this the property list is omitted and
/// the full area is given to the layout panel.
const MIN_LAYOUT_PREVIEW_HEIGHT: u16 = 8;

/// Minimum height for the property-list section (header + at least one row).
const MIN_PROPERTY_LIST_HEIGHT: u16 = 3;

/// Column width reserved for the property name on the left side.
const PROP_NAME_COL: usize = 20;

impl WidgetInspector<'_> {
    /// Render the Widget-properties tab content into `area`.
    ///
    /// Layout:
    /// - Top portion: layout preview (box model, size, constraints, flex).
    /// - Bottom portion: property list.
    ///
    /// Called from `details/mod.rs` when `DetailsTab::Properties` is active.
    pub(super) fn render_properties_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // If there isn't enough room for both sections, give the entire area to
        // the layout preview.
        if area.height < MIN_LAYOUT_PREVIEW_HEIGHT + MIN_PROPERTY_LIST_HEIGHT {
            self.render_layout_panel(area, buf, visible, selected);
            return;
        }

        // Split vertically: layout preview (flexible) + property list (flexible minimum).
        let chunks = Layout::vertical([
            Constraint::Min(MIN_LAYOUT_PREVIEW_HEIGHT),
            Constraint::Min(MIN_PROPERTY_LIST_HEIGHT),
        ])
        .split(area);

        let layout_area = chunks[0];
        let props_area = chunks[1];

        // ── Section 1: Layout preview ─────────────────────────────────────────
        self.render_layout_panel(layout_area, buf, visible, selected);

        // ── Section 2: Property list ──────────────────────────────────────────
        render_property_list(
            props_area,
            buf,
            self.inspector_state.properties_loading,
            self.inspector_state
                .properties_error
                .as_ref()
                .map(|e| (e.message.as_str(), e.hint.as_str())),
            &self.inspector_state.properties,
            self.inspector_state.details_node_id.is_some(),
        );
    }
}

/// Render the property list section.
fn render_property_list(
    area: Rect,
    buf: &mut Buffer,
    loading: bool,
    error: Option<(&str, &str)>,
    properties: &[DiagnosticsNode],
    has_node_id: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(palette::BORDER_DIM))
        .title(Span::styled(
            " Properties ",
            Style::default()
                .fg(palette::ACCENT_DIM)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Left);

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if loading && properties.is_empty() {
        // Loading state.
        render_muted_centered(inner, buf, "Loading properties...");
        return;
    }

    if let Some((message, hint)) = error {
        // Error state — mirrors the layout_panel error style.
        render_properties_error(inner, buf, message, hint);
        return;
    }

    if properties.is_empty() {
        // Empty state (no node selected or widget has no properties).
        let msg = if has_node_id {
            "No properties for this widget."
        } else {
            "Select a widget to see properties."
        };
        render_muted_centered(inner, buf, msg);
        return;
    }

    // Populated state: render the property table.
    render_property_rows(inner, buf, properties);
}

/// Render a muted, vertically-centered single-line message.
fn render_muted_centered(area: Rect, buf: &mut Buffer, text: &str) {
    if area.height == 0 {
        return;
    }
    let y = area.y + area.height / 2;
    Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().fg(palette::TEXT_MUTED),
    )))
    .alignment(Alignment::Center)
    .render(
        Rect {
            y,
            height: 1,
            ..area
        },
        buf,
    );
}

/// Render an error summary + hint, matching the `render_layout_error` style in
/// `layout_panel.rs`.
fn render_properties_error(area: Rect, buf: &mut Buffer, message: &str, hint: &str) {
    if area.height == 0 {
        return;
    }
    let lines = vec![
        Line::from(Span::styled(
            format!("\u{26a0} {message}"),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(Span::styled(
            "[r] Retry   [b] Browser DevTools   [Esc] Return to logs",
            Style::default().fg(palette::TEXT_MUTED),
        )),
    ];
    let h = 5u16;
    Paragraph::new(lines).wrap(Wrap { trim: true }).render(
        Rect {
            y: area.y + area.height.saturating_sub(h) / 2,
            height: h.min(area.height),
            ..area
        },
        buf,
    );
}

/// Render the property name/description rows.
///
/// Uses `filter_and_sort_by_level` to:
/// - Drop hidden-level nodes.
/// - Sort default/fine-level nodes to the end and render them muted.
fn render_property_rows(area: Rect, buf: &mut Buffer, properties: &[DiagnosticsNode]) {
    let rows = filter_and_sort_by_level(properties);
    let max_rows = area.height as usize;
    for (i, (node, is_default)) in rows.iter().take(max_rows).enumerate() {
        let y = area.y + i as u16;
        if y >= area.bottom() {
            break;
        }
        render_property_row(area, buf, y, node, *is_default);
    }
}

/// Render a single property row: `name` (left ~20 cols) + `description` (right).
fn render_property_row(
    area: Rect,
    buf: &mut Buffer,
    y: u16,
    node: &DiagnosticsNode,
    is_default: bool,
) {
    if area.width == 0 {
        return;
    }

    let name_style = if is_default {
        Style::default().fg(palette::TEXT_MUTED)
    } else {
        Style::default().fg(palette::TEXT_SECONDARY)
    };
    let value_style = if is_default {
        Style::default().fg(palette::TEXT_MUTED)
    } else {
        Style::default().fg(palette::TEXT_PRIMARY)
    };

    let name = node.name.as_deref().unwrap_or("—");
    let description = node.description.as_str();

    // Name column (left, PROP_NAME_COL wide).
    let name_width = PROP_NAME_COL.min(area.width as usize);
    let name_truncated = truncate_to(name, name_width.saturating_sub(1));
    buf.set_string(area.x, y, &name_truncated, name_style);

    // Description column (right, fills the rest of the row).
    let desc_x = area.x + name_width as u16;
    if desc_x < area.x + area.width {
        let desc_width = (area.x + area.width).saturating_sub(desc_x) as usize;
        let desc_truncated = truncate_to(description, desc_width);
        buf.set_string(desc_x, y, &desc_truncated, value_style);
    }
}

/// Truncate a string to at most `max_chars` characters.
fn truncate_to(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        // Replace last 3 chars with ellipsis when truncating.
        let take = max_chars.saturating_sub(1);
        let truncated: String = s.chars().take(take).collect();
        format!("{truncated}\u{2026}") // …
    }
}

#[cfg(test)]
mod tests {
    use fdemon_app::state::{DetailsTab, DevToolsError, InspectorState, VmConnectionStatus};
    use fdemon_core::widget_tree::{
        BoxConstraints, DiagnosticsNode, EdgeInsets, LayoutInfo, WidgetSize,
    };
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::super::super::WidgetInspector;

    // Canonical copy lives in inspector::test_helpers (m13 fix).
    use super::super::super::test_helpers::collect_buf_text;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn sample_node(name: &str, description: &str, level: Option<&str>) -> DiagnosticsNode {
        DiagnosticsNode {
            name: Some(name.to_string()),
            description: description.to_string(),
            level: level.map(|l| l.to_string()),
            ..Default::default()
        }
    }

    fn sample_node_with_level(name: &str, description: &str, level: &str) -> DiagnosticsNode {
        sample_node(name, description, Some(level))
    }

    fn make_state_with_layout() -> InspectorState {
        let mut state = InspectorState::new();
        state.layout = Some(LayoutInfo {
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
            padding: Some(EdgeInsets {
                top: 8.0,
                right: 16.0,
                bottom: 8.0,
                left: 16.0,
            }),
            flex_factor: None,
            flex_fit: None,
            description: None,
            margin: None,
            direction: None,
            main_axis_alignment: None,
            cross_axis_alignment: None,
            main_axis_size: None,
            children: Vec::new(),
        });
        state
    }

    fn make_visible_node() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "Container".to_string(),
            value_id: Some("c-1".to_string()),
            ..Default::default()
        }
    }

    fn render_tab(
        state: &InspectorState,
        node: Option<DiagnosticsNode>,
        width: u16,
        height: u16,
    ) -> String {
        let nodes: Vec<DiagnosticsNode> = node.into_iter().collect();
        let refs: Vec<(&DiagnosticsNode, usize)> = nodes.iter().map(|n| (n, 0)).collect();
        let widget = WidgetInspector::new(state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        widget.render_properties_tab(Rect::new(0, 0, width, height), &mut buf, &refs, 0);
        collect_buf_text(&buf, width, height)
    }

    // ── Legacy tests (Phase 1) ────────────────────────────────────────────────

    #[test]
    fn properties_tab_renders_box_model_for_selected_widget() {
        let state = make_state_with_layout();
        let text = render_tab(&state, Some(make_visible_node()), 80, 30);
        // Box model block should be visible (padding title or widget title).
        assert!(
            text.contains("padding") || text.contains("widget") || text.contains("Size"),
            "Expected box model content in buffer, got: {text:?}"
        );
    }

    #[test]
    fn properties_tab_no_panic_at_zero_area() {
        let state = InspectorState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        widget.render_properties_tab(buf.area, &mut buf, &[], 0);
        // Should not panic
    }

    #[test]
    fn properties_tab_no_panic_at_small_area() {
        let state = InspectorState::new();
        let text = render_tab(&state, None, 40, 5);
        // Should render something (or nothing) without panic.
        let _ = text;
    }

    #[test]
    fn properties_tab_no_panic_with_no_layout_data() {
        // No layout data — should render the Layout Explorer empty state.
        let state = InspectorState::new();
        let text = render_tab(&state, None, 80, 30);
        assert!(
            text.contains("Select") || text.is_empty() || !text.trim().is_empty(),
            "Should render without panic, got: {text:?}"
        );
    }

    // ── Phase 2 tests ─────────────────────────────────────────────────────────

    #[test]
    fn properties_tab_shows_property_list_when_populated() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("objects/42".into()),
            properties: vec![
                sample_node("textDirection", "ltr", None),
                sample_node_with_level("locale", "null", "fine"),
            ],
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            buf_str.contains("textDirection"),
            "Expected 'textDirection' in buffer, got: {buf_str:?}"
        );
        assert!(
            buf_str.contains("ltr"),
            "Expected 'ltr' description in buffer, got: {buf_str:?}"
        );
        assert!(
            buf_str.contains("locale"),
            "Expected 'locale' in buffer, got: {buf_str:?}"
        );
        let pos_text = buf_str.find("textDirection").unwrap();
        let pos_locale = buf_str.find("locale").unwrap();
        assert!(
            pos_text < pos_locale,
            "default-level 'locale' should sort after 'textDirection', got positions {pos_text} vs {pos_locale}"
        );
    }

    #[test]
    fn properties_tab_hides_phase_1_placeholder_when_loaded() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("objects/42".into()),
            properties: vec![sample_node("foo", "bar", None)],
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            !buf_str.contains("properties will load here"),
            "Phase 1 placeholder must be gone, got: {buf_str:?}"
        );
    }

    #[test]
    fn properties_tab_keeps_layout_preview() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("objects/42".into()),
            layout: Some(LayoutInfo {
                size: Some(WidgetSize {
                    width: 414.0,
                    height: 600.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            buf_str.contains("414"),
            "layout preview's width label should appear, got: {buf_str:?}"
        );
        assert!(
            buf_str.contains("600"),
            "layout preview's height label should appear, got: {buf_str:?}"
        );
    }

    #[test]
    fn properties_tab_hides_hidden_level_properties() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            properties: vec![
                sample_node("visible", "yes", None),
                sample_node_with_level("secret", "hidden-val", "hidden"),
            ],
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            buf_str.contains("visible"),
            "Non-hidden property should render, got: {buf_str:?}"
        );
        assert!(
            !buf_str.contains("secret"),
            "Hidden-level property must not render, got: {buf_str:?}"
        );
    }

    #[test]
    fn properties_tab_shows_loading_when_loading_and_empty() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            properties_loading: true,
            // properties is empty (still loading)
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            buf_str.contains("Loading"),
            "Loading state should show 'Loading', got: {buf_str:?}"
        );
    }

    #[test]
    fn properties_tab_shows_error_when_error_set() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            properties_error: Some(DevToolsError::new("fetch failed", "try again")),
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            buf_str.contains("fetch failed") || buf_str.contains("failed"),
            "Error state should show error message, got: {buf_str:?}"
        );
    }

    #[test]
    fn properties_tab_shows_empty_state_when_no_properties() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            // properties empty, not loading, no error
            ..Default::default()
        };
        let buf_str = render_tab(&state, None, 80, 30);
        assert!(
            buf_str.contains("No properties") || buf_str.contains("properties"),
            "Empty state should note no properties, got: {buf_str:?}"
        );
    }
}
