//! Inspector details view — tab strip and per-tab dispatch.
//!
//! Renders the tabbed details panel shown when the user opens an inspector
//! node's details (Enter key). The panel contains up to three tabs, shown
//! conditionally based on widget type:
//!
//! - **Widget properties** — always visible; layout preview + property list.
//! - **Render object** — visible when `render_properties` is non-empty.
//! - **Flex explorer** — visible when `details_context.is_flex_layout` is true.
//!
//! ## Tab visibility
//!
//! The set of visible tabs is determined by [`InspectorState::visible_tabs`],
//! which is called each frame during rendering. Hidden tabs leave no gap or
//! placeholder in the strip.
//!
//! ## Mouse clicks on tab labels
//!
//! TODO (Phase 2 polish): register mouse-click regions for each tab label so
//! clicking a label fires `Message::DevToolsInspectorSelectTab(DetailsTab)`.
//! For Phase 1, keyboard cycling (Tab / Shift+Tab) is sufficient.

use fdemon_app::state::{DetailsTab, InspectorState};
use fdemon_core::widget_tree::{DiagnosticsNode, InspectorRow};
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

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Filter hidden-level properties and separate default-level (`"fine"`) ones to
/// the end of the list.
///
/// Returns a `Vec` of `(&DiagnosticsNode, bool)` pairs where the `bool` is
/// `true` for default/fine-level nodes (rendered muted) and `false` for
/// non-default nodes (rendered normally). Hidden-level nodes are dropped.
///
/// Both `properties_tab` and `render_object_tab` use this helper to keep the
/// sort/filter logic in one place.
pub(super) fn filter_and_sort_by_level<'a>(
    props: &'a [DiagnosticsNode],
) -> Vec<(&'a DiagnosticsNode, bool)> {
    let mut non_default: Vec<(&'a DiagnosticsNode, bool)> = Vec::new();
    let mut default: Vec<(&'a DiagnosticsNode, bool)> = Vec::new();
    for p in props {
        match p.level.as_deref() {
            Some("hidden") => continue,
            Some("fine") => default.push((p, true)),
            _ => non_default.push((p, false)),
        }
    }
    non_default.extend(default);
    non_default
}

// ── Layout constants ──────────────────────────────────────────────────────────

/// Height of the tab strip above the tab content.
/// 1 row for tab labels + 1 row for the underline / separator.
const TAB_STRIP_HEIGHT: u16 = 2;

/// Horizontal spacing between tab labels (spaces).
const TAB_GAP: usize = 3;

/// Label string for a given details tab, used by [`render_tab_strip`].
///
/// Returned as a static string slice; lifetime is `'static`.
fn label_for(tab: DetailsTab) -> &'static str {
    match tab {
        DetailsTab::Properties => "Widget properties",
        DetailsTab::RenderObject => "Render object",
        DetailsTab::FlexExplorer => "Flex explorer",
    }
}

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

        // Defensive dispatch: if details_tab somehow points at a hidden tab
        // (handler's clamp_details_tab should have already run, but the renderer
        // must be robust), fall back to the first visible tab (always Properties).
        // The renderer is pure and cannot mutate state to fix this.
        let visible_tabs = self.inspector_state.visible_tabs();
        let dispatch_tab = if visible_tabs.contains(&self.inspector_state.details_tab) {
            self.inspector_state.details_tab
        } else {
            visible_tabs
                .first()
                .copied()
                .unwrap_or(DetailsTab::Properties)
        };

        match dispatch_tab {
            DetailsTab::Properties => {
                self.render_properties_tab(content_area, buf, &visible, selected);
            }
            DetailsTab::RenderObject => {
                self.render_render_object_tab(content_area, buf);
            }
            DetailsTab::FlexExplorer => {
                flex_explorer_tab::render(content_area, buf, self.inspector_state);
            }
        }
    }
}

// ── Tab strip rendering ───────────────────────────────────────────────────────

/// Render the two-row tab strip (labels row + underline row).
///
/// Iterates [`InspectorState::visible_tabs`] to determine which labels to draw
/// and in what order. Hidden tabs leave no gap or placeholder. The active tab
/// is highlighted (bold + accent colour) and has an underline of `━` characters
/// in the second row. Inactive visible tabs use `TEXT_MUTED`.
fn render_tab_strip(area: Rect, buf: &mut Buffer, state: &InspectorState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let visible = state.visible_tabs();
    if visible.is_empty() {
        return; // defensive — visible_tabs always returns at least [Properties]
    }

    let active = state.details_tab;

    // ── Row 0: labels ─────────────────────────────────────────────────────────
    let label_y = area.y;

    // Build the full label line to measure positions.
    // Each label is followed by TAB_GAP spaces (except the last).
    let mut tab_starts: Vec<u16> = Vec::with_capacity(visible.len());
    let mut tab_widths: Vec<u16> = Vec::with_capacity(visible.len());

    let mut cursor_x = area.x;
    for (i, tab) in visible.iter().enumerate() {
        if cursor_x >= area.x + area.width {
            break;
        }
        let label = label_for(*tab);
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
        if i + 1 < visible.len() {
            cursor_x += TAB_GAP as u16;
        }
    }

    // ── Row 1: underline ──────────────────────────────────────────────────────
    if area.height < 2 {
        return;
    }
    let underline_y = area.y + 1;

    for (i, (tab, start)) in visible.iter().zip(tab_starts.iter().copied()).enumerate() {
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
    use fdemon_core::widget_tree::{DetailsContext, DiagnosticsNode};
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
    ///
    /// Note: only Properties tab is visible with this minimal fixture (no
    /// `render_properties` and `details_context.is_flex_layout == false`).
    /// Tests that require other tabs visible must build a custom fixture.
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

    /// Render a `render_details_panel` call into a new buffer and return the
    /// full text content. Mirrors the rendering pattern used across these tests.
    fn render_for_state(state: &InspectorState, area_w: u16, area_h: u16) -> String {
        let widget = WidgetInspector::new(state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, area_w, area_h));
        widget.render_details_panel(buf.area, &mut buf, &[]);
        collect_buf_text(&buf, area_w, area_h)
    }

    // ── Tab-strip tests ───────────────────────────────────────────────────────

    /// All three tabs visible when `render_properties` is non-empty and
    /// `details_context.is_flex_layout` is true.
    #[test]
    fn tab_strip_renders_three_labels_when_all_visible() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            root: Some(DiagnosticsNode {
                description: "Root".to_string(),
                value_id: Some("root".to_string()),
                ..Default::default()
            }),
            render_properties: vec![DiagnosticsNode {
                description: "RenderFlex".to_string(),
                ..Default::default()
            }],
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            },
            ..Default::default()
        };
        let text = render_for_state(&state, 80, 20);
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
        // contain at least one `━` character. Populate render_properties so the
        // Render object tab is visible.
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            root: Some(DiagnosticsNode {
                description: "Root".to_string(),
                value_id: Some("root".to_string()),
                ..Default::default()
            }),
            render_properties: vec![DiagnosticsNode {
                description: "RenderBox".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
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
        // When Properties is the only visible tab (default fixture), only the
        // "Widget properties" underline should be filled.
        //
        // Count ━ characters: should equal "Widget properties" label length (17).
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
    fn details_panel_shows_render_object_content_for_render_object_tab() {
        // Phase 2: the Render-object tab now shows a populated property table
        // (or an appropriate state message) instead of the Phase 1 "Coming soon" stub.
        // Populate render_properties so the RenderObject tab is visible and dispatch
        // reaches the render_render_object_tab path.
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            root: Some(DiagnosticsNode {
                description: "Root".to_string(),
                value_id: Some("root".to_string()),
                ..Default::default()
            }),
            render_properties: vec![DiagnosticsNode {
                description: "RenderBox".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let text = collect_buf_text(&buf, 80, 20);
        // Should NOT show "Coming soon" any more (Phase 1 stub removed).
        assert!(
            !text.contains("Coming soon"),
            "Phase 2 Render-object tab must not show 'Coming soon' stub, got: {text:?}"
        );
        // Should show the Render Object block title or a state message.
        assert!(
            text.contains("Render Object")
                || text.contains("No render object")
                || text.contains("Loading")
                || text.contains("widget"),
            "Expected Render-object tab content in buffer, got: {text:?}"
        );
    }

    #[test]
    fn details_panel_flex_explorer_tab_shows_no_layout_data() {
        // With no layout data and layout_loading == false, the flex explorer tab
        // should show the "No layout data — press Enter to fetch." message.
        // Set details_context.is_flex_layout = true so the FlexExplorer tab is visible.
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            root: Some(DiagnosticsNode {
                description: "Root".to_string(),
                value_id: Some("root".to_string()),
                ..Default::default()
            }),
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            },
            ..Default::default()
        };
        let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        widget.render_details_panel(buf.area, &mut buf, &[]);

        let text = collect_buf_text(&buf, 80, 20);
        // The stub "Coming soon" is replaced — the real renderer shows the no-data state.
        assert!(
            !text.contains("Coming soon"),
            "Flex-explorer tab must no longer show 'Coming soon' stub, got: {text:?}"
        );
        // It should show the no-data message or be empty (layout not loaded yet).
        assert!(
            text.contains("No layout data")
                || text.contains("press Enter")
                || text.is_empty()
                || text.chars().all(|c| c == ' '),
            "Expected no-data message in Flex-explorer tab, got: {text:?}"
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
        // Smoke test: each tab variant is set as active. Tabs that require
        // visibility fields (RenderObject needs render_properties non-empty,
        // FlexExplorer needs is_flex_layout) are provided; this exercises both
        // the visible dispatch path and the fallback dispatch (Properties) without
        // panicking in either case.
        for tab in [
            DetailsTab::Properties,
            DetailsTab::RenderObject,
            DetailsTab::FlexExplorer,
        ] {
            let state = InspectorState {
                details_open: true,
                details_tab: tab,
                root: Some(DiagnosticsNode {
                    description: "Root".to_string(),
                    value_id: Some("root".to_string()),
                    ..Default::default()
                }),
                render_properties: vec![DiagnosticsNode {
                    description: "RenderBox".to_string(),
                    ..Default::default()
                }],
                details_context: DetailsContext {
                    is_flex_layout: true,
                    parent_type: None,
                },
                ..Default::default()
            };
            let widget = WidgetInspector::new(&state, true, &VmConnectionStatus::Connected);
            let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
            widget.render_details_panel(buf.area, &mut buf, &[]);
        }
    }

    // ── Widget-type snapshot tests ────────────────────────────────────────────

    /// Container with no render properties and no flex context → only
    /// "Widget properties" tab is visible.
    #[test]
    fn details_strip_container_shows_only_properties_tab() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("c-id".into()),
            root: Some(DiagnosticsNode {
                description: "Container".into(),
                value_id: Some("c-id".into()),
                ..Default::default()
            }),
            // render_properties empty — RenderObject tab hidden
            // details_context default — FlexExplorer tab hidden
            ..Default::default()
        };
        let text = render_for_state(&state, 80, 10);
        assert!(
            text.contains("Widget properties"),
            "Container: expected 'Widget properties' label, got: {text:?}"
        );
        assert!(
            !text.contains("Render object"),
            "Container: 'Render object' tab must not be visible, got: {text:?}"
        );
        assert!(
            !text.contains("Flex explorer"),
            "Container: 'Flex explorer' tab must not be visible, got: {text:?}"
        );
    }

    /// Padding has a render object but is not a flex widget and its parent is
    /// not flex → "Widget properties" + "Render object" tabs visible; no Flex.
    #[test]
    fn details_strip_padding_shows_properties_and_render_object_tabs() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("p-id".into()),
            root: Some(DiagnosticsNode {
                description: "Padding".into(),
                value_id: Some("p-id".into()),
                ..Default::default()
            }),
            render_properties: vec![DiagnosticsNode {
                description: "RenderPadding".into(),
                ..Default::default()
            }],
            // details_context default (is_flex_layout = false) — FlexExplorer hidden
            ..Default::default()
        };
        let text = render_for_state(&state, 80, 10);
        assert!(
            text.contains("Widget properties"),
            "Padding: expected 'Widget properties' label, got: {text:?}"
        );
        assert!(
            text.contains("Render object"),
            "Padding: expected 'Render object' label, got: {text:?}"
        );
        assert!(
            !text.contains("Flex explorer"),
            "Padding: 'Flex explorer' tab must not be visible, got: {text:?}"
        );
    }

    /// Column is a flex widget with a render object → all three tabs visible.
    #[test]
    fn details_strip_column_shows_all_three_tabs() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("col-id".into()),
            root: Some(DiagnosticsNode {
                description: "Column".into(),
                value_id: Some("col-id".into()),
                ..Default::default()
            }),
            render_properties: vec![DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }],
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            },
            ..Default::default()
        };
        let text = render_for_state(&state, 80, 10);
        assert!(
            text.contains("Widget properties"),
            "Column: expected 'Widget properties' label, got: {text:?}"
        );
        assert!(
            text.contains("Render object"),
            "Column: expected 'Render object' label, got: {text:?}"
        );
        assert!(
            text.contains("Flex explorer"),
            "Column: expected 'Flex explorer' label, got: {text:?}"
        );
    }

    /// Container child of Column: `parent_type = Some("Column")` and
    /// `is_flex_layout = true` (child of flex) → all three tabs visible.
    #[test]
    fn details_strip_container_child_of_column_shows_all_three_tabs() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::Properties,
            details_node_id: Some("c-id".into()),
            root: Some(DiagnosticsNode {
                description: "Column".into(),
                value_id: Some("col-id".into()),
                children: vec![DiagnosticsNode {
                    description: "Container".into(),
                    value_id: Some("c-id".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            render_properties: vec![DiagnosticsNode {
                description: "RenderConstrainedBox".into(),
                ..Default::default()
            }],
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: Some("Column".into()),
            },
            ..Default::default()
        };
        let text = render_for_state(&state, 80, 10);
        assert!(
            text.contains("Widget properties"),
            "Container-child-of-Column: expected 'Widget properties' label, got: {text:?}"
        );
        assert!(
            text.contains("Render object"),
            "Container-child-of-Column: expected 'Render object' label, got: {text:?}"
        );
        assert!(
            text.contains("Flex explorer"),
            "Container-child-of-Column: expected 'Flex explorer' label, got: {text:?}"
        );
    }

    // ── Defensive dispatch test ───────────────────────────────────────────────

    /// If `details_tab` is stale (e.g. `RenderObject` but `render_properties`
    /// is empty), the renderer must fall back to Properties without panicking
    /// and without mutating state.
    #[test]
    fn details_panel_falls_back_to_properties_when_active_tab_hidden() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject, // stale — RenderObject tab is hidden
            details_node_id: Some("c-id".into()),
            root: Some(DiagnosticsNode {
                description: "Container".into(),
                value_id: Some("c-id".into()),
                ..Default::default()
            }),
            // render_properties empty → RenderObject tab hidden
            // details_context default → FlexExplorer tab hidden
            ..Default::default()
        };
        // Render should not panic.
        let text = render_for_state(&state, 80, 10);
        // Only the Widget properties label is in the strip.
        assert!(
            text.contains("Widget properties"),
            "Fallback: expected 'Widget properties' label in strip, got: {text:?}"
        );
        // The Render object label must NOT appear (tab is hidden).
        assert!(
            !text.contains("Render object"),
            "Fallback: 'Render object' tab must not be visible, got: {text:?}"
        );
        // State is not mutated — details_tab is still RenderObject.
        assert_eq!(
            state.details_tab,
            DetailsTab::RenderObject,
            "Renderer must not mutate state.details_tab"
        );
    }
}
