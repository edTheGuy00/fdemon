//! Inspector details view — tab strip and per-tab dispatch.
//!
//! Renders the tabbed details panel shown when the user opens an inspector
//! node's details (Enter key). The panel contains three tabs:
//!
//! - **Widget properties** — layout preview + property list (populated in Phase 2).
//! - **Render object** — render-object property nodes (stub in Phase 1).
//! - **Flex explorer** — flex layout explorer (stub in Phase 1).
//!
//! ## Phase 1 vs Phase 2
//!
//! Phase 1 populates the Widget-properties tab by delegating to the existing
//! `layout_panel` rendering logic. The Render-object and Flex-explorer tabs
//! show a "Coming soon — Phase 2" stub until Phase 2 fills their bodies.
//!
//! ## Mouse clicks on tab labels
//!
//! TODO (Phase 2 polish): register mouse-click regions for each tab label so
//! clicking a label fires `Message::DevToolsInspectorSelectTab(DetailsTab)`.
//! For Phase 1, keyboard cycling (Tab / Shift+Tab) is sufficient.

use fdemon_app::state::{DetailsTab, InspectorState};
use fdemon_core::widget_tree::InspectorRow;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Widget},
};

use super::WidgetInspector;
use crate::theme::palette;

mod flex_explorer_tab;
mod properties_tab;
mod render_object_tab;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Height of the tab strip above the tab content.
/// 1 row for tab labels + 1 row for the underline / separator.
const TAB_STRIP_HEIGHT: u16 = 2;

/// The three tab labels in display order.
const TAB_LABELS: &[(&str, DetailsTab)] = &[
    ("Widget properties", DetailsTab::Properties),
    ("Render object", DetailsTab::RenderObject),
    ("Flex explorer", DetailsTab::FlexExplorer),
];

/// Horizontal spacing between tab labels (spaces).
const TAB_GAP: usize = 3;

// ── impl WidgetInspector ──────────────────────────────────────────────────────

impl WidgetInspector<'_> {
    /// Render the tabbed details view in `area`.
    ///
    /// Called from `inspector/mod.rs` when `inspector_state.details_open == true`.
    /// `rows` is the pre-built row slice from `inspector_rows()` (called once per
    /// frame at the top of `render_impl`) — using it here avoids a redundant
    /// `visible_nodes()` call (review item m11).
    pub(super) fn render_details_panel(
        &self,
        area: Rect,
        buf: &mut Buffer,
        rows: &[InspectorRow<'_>],
    ) {
        // ── Outer block + title ───────────────────────────────────────────────
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                " Inspector Details ",
                Style::default().fg(palette::ACCENT_DIM),
            ))
            .title_alignment(Alignment::Left);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // ── Split inner area: tab strip (top) + tab content (rest) ───────────
        if inner.height <= TAB_STRIP_HEIGHT {
            // Not enough room for any content — just draw the tab strip.
            render_tab_strip(inner, buf, self.inspector_state);
            return;
        }

        let chunks = Layout::vertical([Constraint::Length(TAB_STRIP_HEIGHT), Constraint::Min(0)])
            .split(inner);

        let strip_area = chunks[0];
        let content_area = chunks[1];

        // ── Tab strip ─────────────────────────────────────────────────────────
        render_tab_strip(strip_area, buf, self.inspector_state);

        // ── Tab content ───────────────────────────────────────────────────────
        // Derive the (node, depth) pairs the layout panel needs directly from the
        // pre-built row slice — no extra visible_nodes() call here (m11 fix).
        let visible: Vec<(&fdemon_core::widget_tree::DiagnosticsNode, usize)> =
            rows.iter().map(|r| (r.node, r.depth)).collect();
        let selected = self.inspector_state.selected_index;

        match self.inspector_state.details_tab {
            DetailsTab::Properties => {
                self.render_properties_tab(content_area, buf, &visible, selected);
            }
            DetailsTab::RenderObject => {
                render_object_tab::render(content_area, buf);
            }
            DetailsTab::FlexExplorer => {
                flex_explorer_tab::render(content_area, buf);
            }
        }
    }
}

// ── Tab strip rendering ───────────────────────────────────────────────────────

/// Render the two-row tab strip (labels row + underline row).
///
/// The active tab is highlighted (bold + accent colour) and has an underline
/// of `━` characters in the second row. Inactive tabs use `TEXT_MUTED`.
fn render_tab_strip(area: Rect, buf: &mut Buffer, state: &InspectorState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let active = state.details_tab;

    // ── Row 0: labels ─────────────────────────────────────────────────────────
    let label_y = area.y;

    // Build the full label line to measure positions.
    // Each label is followed by TAB_GAP spaces (except the last).
    let mut tab_starts: Vec<u16> = Vec::with_capacity(TAB_LABELS.len());
    let mut tab_widths: Vec<u16> = Vec::with_capacity(TAB_LABELS.len());

    let mut cursor_x = area.x;
    for (i, (label, tab)) in TAB_LABELS.iter().enumerate() {
        if cursor_x >= area.x + area.width {
            break;
        }
        let label_len = label.chars().count() as u16;
        let available = (area.x + area.width).saturating_sub(cursor_x);
        let render_len = label_len.min(available);

        tab_starts.push(cursor_x);
        tab_widths.push(render_len);

        // Draw label text.
        let is_active = *tab == active;
        let style = if is_active {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_MUTED)
        };

        // Truncate label if it doesn't fit.
        let label_text: String = label.chars().take(render_len as usize).collect();
        buf.set_string(cursor_x, label_y, &label_text, style);
        cursor_x += render_len;

        // Advance past the gap (unless this is the last tab).
        if i + 1 < TAB_LABELS.len() {
            cursor_x += TAB_GAP as u16;
        }
    }

    // ── Row 1: underline ──────────────────────────────────────────────────────
    if area.height < 2 {
        return;
    }
    let underline_y = area.y + 1;

    for (i, ((_label, tab), start)) in TAB_LABELS
        .iter()
        .zip(tab_starts.iter().copied())
        .enumerate()
    {
        if i >= tab_widths.len() {
            break;
        }
        let width = tab_widths[i];
        if *tab == active {
            // Draw `━` characters spanning the tab label width.
            for dx in 0..width {
                let x = start + dx;
                if x < area.x + area.width {
                    buf.set_string(
                        x,
                        underline_y,
                        "\u{2501}", // ━
                        Style::default().fg(palette::ACCENT),
                    );
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use fdemon_app::state::{DetailsTab, InspectorState, VmConnectionStatus};
    use fdemon_core::widget_tree::DiagnosticsNode;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::super::WidgetInspector;

    // ── Helpers ───────────────────────────────────────────────────────────────

    // Canonical copy lives in inspector::test_helpers (m13 fix).
    use super::super::test_helpers::collect_buf_text;

    fn collect_row(buf: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .filter_map(|x| buf.cell((x, y)))
            .filter_map(|c| c.symbol().chars().next())
            .collect()
    }

    /// Build a minimal state with details open and the given active tab.
    fn make_state_with_details_open(tab: DetailsTab) -> InspectorState {
        let root = DiagnosticsNode {
            description: "Root".to_string(),
            value_id: Some("root".to_string()),
            ..Default::default()
        };
        let mut state = InspectorState::new();
        state.root = Some(root);
        state.details_open = true;
        state.details_tab = tab;
        state
    }

    // ── Tab-strip tests ───────────────────────────────────────────────────────

    #[test]
    fn tab_strip_renders_three_labels_in_order() {
        let state = make_state_with_details_open(DetailsTab::Properties);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let text = collect_buf_text(&buf, 80, 20);
        // All three tab labels must appear.
        assert!(
            text.contains("Widget properties"),
            "Expected 'Widget properties' label, got: {text:?}"
        );
        assert!(
            text.contains("Render object"),
            "Expected 'Render object' label, got: {text:?}"
        );
        assert!(
            text.contains("Flex explorer"),
            "Expected 'Flex explorer' label, got: {text:?}"
        );
    }

    #[test]
    fn tab_strip_underlines_active_tab() {
        // Active tab: RenderObject — the underline row for "Render object" should
        // contain at least one `━` character.
        let state = make_state_with_details_open(DetailsTab::RenderObject);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        // Inner area: border (1) + label row (1) + underline row (1) + content.
        // Use area (0, 0, 80, 20) so all rows are visible.
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        // The underline row is at: block border top (row 0 = '┌'), inner starts
        // at y=1. Tab strip occupies inner rows y=1 (labels) and y=2 (underline).
        let underline_row = collect_row(&buf, 2, 80);
        assert!(
            underline_row.contains('\u{2501}'), // ━
            "Underline row should contain '━' for the active tab, got: {underline_row:?}"
        );
    }

    #[test]
    fn tab_strip_only_underlines_active_tab_not_others() {
        // When Properties is active, only the "Widget properties" underline
        // should be filled — "Render object" and "Flex explorer" should not have ━
        // in the column range covered by their labels.
        //
        // This is a lighter check: we verify the full text contains ━ (active tab)
        // and does not contain it in both the "Render object" and "Flex explorer"
        // zones (we can't easily address columns, so we just check total ━ count
        // is <= label-width of "Widget properties").
        let state = make_state_with_details_open(DetailsTab::Properties);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let underline_row = collect_row(&buf, 2, 80);
        // Count ━ characters: should equal "Widget properties" label length (17).
        let underline_count = underline_row.chars().filter(|&c| c == '\u{2501}').count();
        let expected_label_len = "Widget properties".chars().count();
        assert_eq!(
            underline_count, expected_label_len,
            "Only the active tab should be underlined; expected {expected_label_len} ━ chars, got {underline_count}. Row: {underline_row:?}"
        );
    }

    // ── Content dispatch tests ────────────────────────────────────────────────

    #[test]
    fn details_panel_shows_coming_soon_for_render_object_tab() {
        let state = make_state_with_details_open(DetailsTab::RenderObject);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let text = collect_buf_text(&buf, 80, 20);
        assert!(
            text.contains("Coming") && text.contains("soon"),
            "Expected 'Coming soon' stub in Render-object tab, got: {text:?}"
        );
    }

    #[test]
    fn details_panel_shows_coming_soon_for_flex_explorer_tab() {
        let state = make_state_with_details_open(DetailsTab::FlexExplorer);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let text = collect_buf_text(&buf, 80, 20);
        assert!(
            text.contains("Coming") && text.contains("soon"),
            "Expected 'Coming soon' stub in Flex-explorer tab, got: {text:?}"
        );
    }

    #[test]
    fn details_panel_shows_properties_content_for_properties_tab() {
        // With no layout data but properties tab active, the panel should
        // render some content (at minimum the empty-state message from layout panel).
        let state = make_state_with_details_open(DetailsTab::Properties);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let text = collect_buf_text(&buf, 80, 20);
        // Should NOT show "Coming soon".
        assert!(
            !text.contains("Coming soon"),
            "Properties tab must not show 'Coming soon' stub, got: {text:?}"
        );
    }

    // ── Edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn details_panel_no_panic_zero_area() {
        let state = make_state_with_details_open(DetailsTab::Properties);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        widget.render_details_panel(buf.area, &mut buf, &[]);
        // Should not panic
    }

    #[test]
    fn details_panel_no_panic_single_row() {
        let state = make_state_with_details_open(DetailsTab::Properties);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        widget.render_details_panel(buf.area, &mut buf, &[]);
        // Should not panic
    }

    #[test]
    fn details_panel_no_panic_narrow_terminal() {
        // 20 cols — labels will be truncated.
        let state = make_state_with_details_open(DetailsTab::Properties);
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        widget.render_details_panel(buf.area, &mut buf, &[]);
        // Should not panic
    }

    #[test]
    fn details_panel_all_tabs_no_panic() {
        // Smoke test all three tabs.
        for tab in [
            DetailsTab::Properties,
            DetailsTab::RenderObject,
            DetailsTab::FlexExplorer,
        ] {
            let state = make_state_with_details_open(tab);
            let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
            let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
            widget.render_details_panel(buf.area, &mut buf, &[]);
        }
    }
}
