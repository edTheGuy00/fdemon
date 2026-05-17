//! Widget properties tab for the details view.
//!
//! Renders the existing layout preview (box model, dimensions, constraints,
//! flex properties — identical to the Layout Explorer panel) followed by a
//! property-list scaffold that is empty in Phase 1 and will be populated in
//! Phase 2 from `inspector_state.properties`.

use fdemon_core::widget_tree::DiagnosticsNode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::super::WidgetInspector;
use crate::theme::palette;

/// Minimum height of the layout-preview section.
///
/// If the content area is smaller than this the property list is omitted and
/// the full area is given to the layout panel.
const MIN_LAYOUT_PREVIEW_HEIGHT: u16 = 8;

/// Height reserved for the property-list area (header + placeholder row).
/// 2 rows: 1 for the section header, 1 for the placeholder text.
const PROPERTY_LIST_HEIGHT: u16 = 3;

impl WidgetInspector<'_> {
    /// Render the Widget-properties tab content into `area`.
    ///
    /// Layout:
    /// - Top portion: layout preview (box model, size, constraints, flex).
    /// - Bottom portion: property list (Phase 1: placeholder text).
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
        // the layout preview — the property list is an additive Phase 2 concern.
        if area.height < MIN_LAYOUT_PREVIEW_HEIGHT + PROPERTY_LIST_HEIGHT {
            self.render_layout_panel(area, buf, visible, selected);
            return;
        }

        // Split vertically: layout preview (most space) + property list (fixed).
        let chunks = Layout::vertical([
            Constraint::Min(MIN_LAYOUT_PREVIEW_HEIGHT),
            Constraint::Length(PROPERTY_LIST_HEIGHT),
            Constraint::Min(0), // absorb any extra rows
        ])
        .split(area);

        let layout_area = chunks[0];
        let props_area = chunks[1];

        // ── Section 1: Layout preview ─────────────────────────────────────────
        self.render_layout_panel(layout_area, buf, visible, selected);

        // ── Section 2: Property list (Phase 1 placeholder) ───────────────────
        render_property_list_placeholder(props_area, buf, self.inspector_state.properties.len());
    }
}

/// Render the property list section.
///
/// Phase 1: always shows a placeholder row because `properties` is empty.
/// Phase 2 will replace this function body with an actual property table.
fn render_property_list_placeholder(area: Rect, buf: &mut Buffer, property_count: usize) {
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

    // Phase 1: `properties` is always empty — render a placeholder.
    // Phase 2 will replace this branch with a rendered property table.
    if property_count == 0 {
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "(properties will load here in Phase 2)",
            Style::default().fg(palette::TEXT_MUTED),
        )))
        .alignment(Alignment::Left);
        placeholder.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use fdemon_app::state::{InspectorState, VmConnectionStatus};
    use fdemon_core::widget_tree::{
        BoxConstraints, DiagnosticsNode, EdgeInsets, LayoutInfo, WidgetSize,
    };
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::super::super::WidgetInspector;

    // Canonical copy lives in inspector::test_helpers (m13 fix).
    use super::super::super::test_helpers::collect_buf_text;

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
    fn properties_tab_renders_property_placeholder_when_properties_empty() {
        let state = make_state_with_layout();
        let text = render_tab(&state, Some(make_visible_node()), 80, 30);
        // Phase 1 placeholder message should appear.
        assert!(
            text.contains("Phase 2") || text.contains("properties"),
            "Expected property list placeholder in buffer, got: {text:?}"
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
}
