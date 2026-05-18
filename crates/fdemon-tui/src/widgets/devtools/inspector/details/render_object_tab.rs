//! Render object tab — populated property table.
//!
//! Displays `InspectorState.render_properties` as a two-column key/value table.
//! Properties with `level == "fine"` are sorted to the end with muted styling.
//! Properties with `level == "hidden"` are filtered out entirely.
//! Loading, error, and empty states are surfaced with appropriate messages.

use fdemon_core::widget_tree::DiagnosticsNode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use super::super::WidgetInspector;
use crate::theme::palette;

// ── Column layout ────────────────────────────────────────────────────────────

/// Minimum column width for the name column.
const MIN_NAME_COL: u16 = 10;

/// Maximum column width for the name column.
const MAX_NAME_COL: u16 = 24;

// ── impl WidgetInspector ─────────────────────────────────────────────────────

impl WidgetInspector<'_> {
    /// Render the Render-object tab content into `area`.
    ///
    /// Displays `inspector_state.render_properties` as a sorted, filtered
    /// key/value table. Properties with `level == "fine"` sort to the end
    /// with muted styling; `level == "hidden"` entries are excluded.
    ///
    /// Called from `details/mod.rs` when `DetailsTab::RenderObject` is active.
    pub(super) fn render_render_object_tab(&self, area: Rect, buf: &mut Buffer) {
        let state = self.inspector_state;

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                " Render Object ",
                Style::default().fg(palette::ACCENT_DIM),
            ))
            .title_alignment(Alignment::Left);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // ── State dispatch ────────────────────────────────────────────────────

        // 1. No details_node_id — shouldn't normally happen but guard anyway.
        if state.details_node_id.is_none() {
            render_muted_text(inner, buf, "No widget selected.");
            return;
        }

        // 2. Loading (and no cached data yet).
        if state.properties_loading && state.render_properties.is_empty() {
            render_muted_text(inner, buf, "Loading render-object properties...");
            return;
        }

        // 3. Error state.
        if let Some(ref err) = state.properties_error {
            render_error(inner, buf, &err.message, &err.hint);
            return;
        }

        // 4. Empty — widget has no render-object properties (e.g. pure Container).
        if state.render_properties.is_empty() {
            render_muted_text(inner, buf, "No render object for this widget.");
            return;
        }

        // 5. Property table.
        render_property_table(inner, buf, &state.render_properties);
    }
}

// ── Property table rendering ─────────────────────────────────────────────────

/// Sort and filter `props` according to the DevTools `_filterAndSortPropertiesByLevel`
/// algorithm:
/// 1. Filter out `level == "hidden"`.
/// 2. Stable partition: non-"fine" first, "fine" last.
///
/// Returns an iterator of `(&DiagnosticsNode, is_default: bool)`.
fn filtered_and_sorted(
    props: &[DiagnosticsNode],
) -> (Vec<&DiagnosticsNode>, Vec<&DiagnosticsNode>) {
    let (non_default, default): (Vec<_>, Vec<_>) = props
        .iter()
        .filter(|p| p.level.as_deref() != Some("hidden"))
        .partition(|p| p.level.as_deref() != Some("fine"));
    (non_default, default)
}

/// Truncate a string to at most `max_chars` characters, appending `…` if
/// the string was longer.
fn truncate_to(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        // There were more characters — add ellipsis (replace last char).
        let mut truncated: String = collected
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect();
        truncated.push('\u{2026}'); // …
        truncated
    } else {
        collected
    }
}

/// Render the property key/value table into `area`.
fn render_property_table(area: Rect, buf: &mut Buffer, props: &[DiagnosticsNode]) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let (non_default, default) = filtered_and_sorted(props);

    // Name column width: min(MAX_NAME_COL, area.width / 3).
    let name_col_w = (area.width / 3).clamp(MIN_NAME_COL, MAX_NAME_COL) as usize;
    // Description column gets the remainder minus a 1-space gap.
    let desc_col_w = (area.width as usize).saturating_sub(name_col_w + 1).max(1);

    let has_default_section = !default.is_empty();

    // Build all rows: non-default rows, optional divider, default rows.
    let total_data_rows =
        non_default.len() + if has_default_section { 1 } else { 0 } + default.len();

    let visible_rows = (area.height as usize).min(total_data_rows);
    let has_overflow = total_data_rows > area.height as usize;

    // If overflow, reserve last row for overflow indicator.
    let show_rows = if has_overflow {
        visible_rows.saturating_sub(1)
    } else {
        visible_rows
    };

    let mut y = area.y;
    let mut rows_drawn = 0usize;

    // ── Non-default rows ──────────────────────────────────────────────────────
    for node in &non_default {
        if rows_drawn >= show_rows {
            break;
        }
        render_property_row(buf, area, y, node, false, name_col_w, desc_col_w);
        y += 1;
        rows_drawn += 1;
    }

    // ── Divider row (if there are default entries) ────────────────────────────
    if has_default_section && rows_drawn < show_rows {
        // Draw a muted horizontal divider spanning the content area.
        let divider: String = "\u{2500}".repeat(area.width as usize); // ─
        buf.set_string(
            area.x,
            y,
            &divider,
            Style::default().fg(palette::BORDER_DIM),
        );
        y += 1;
        rows_drawn += 1;
    }

    // ── Default (fine) rows ───────────────────────────────────────────────────
    for node in &default {
        if rows_drawn >= show_rows {
            break;
        }
        render_property_row(buf, area, y, node, true, name_col_w, desc_col_w);
        y += 1;
        rows_drawn += 1;
    }

    // ── Overflow indicator ────────────────────────────────────────────────────
    if has_overflow {
        let remaining = total_data_rows - rows_drawn;
        let msg = format!("... +{remaining} more (resize window or expand details to see)");
        let msg_truncated = truncate_to(&msg, area.width as usize);
        buf.set_string(
            area.x,
            y,
            &msg_truncated,
            Style::default().fg(palette::TEXT_MUTED),
        );
    }
}

/// Render a single property row at row `y`.
///
/// `is_default` → muted style.
fn render_property_row(
    buf: &mut Buffer,
    area: Rect,
    y: u16,
    node: &DiagnosticsNode,
    is_default: bool,
    name_col_w: usize,
    desc_col_w: usize,
) {
    let style = if is_default {
        Style::default().fg(palette::TEXT_MUTED)
    } else {
        Style::default().fg(palette::TEXT_PRIMARY)
    };

    // Name column: use `node.name` when present, fall back to deriving from description.
    let raw_name = node.name.as_deref().unwrap_or(node.description.as_str());
    let name = truncate_to(raw_name, name_col_w);

    // Description column.
    let desc = truncate_to(&node.description, desc_col_w);

    // Pad name to name_col_w.
    let name_padded = format!("{:<width$}", name, width = name_col_w);

    let row = format!("{name_padded} {desc}");
    buf.set_string(area.x, y, &row, style);
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Render a single muted centred text line (for loading / empty states).
fn render_muted_text(area: Rect, buf: &mut Buffer, text: &str) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let y = area.y + area.height / 2;
    let text_len = text.chars().count() as u16;
    let x = area.x + area.width.saturating_sub(text_len) / 2;
    buf.set_string(x, y, text, Style::default().fg(palette::TEXT_MUTED));
}

/// Render an error message following the `render_layout_error` convention.
fn render_error(area: Rect, buf: &mut Buffer, message: &str, hint: &str) {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use fdemon_app::state::{DetailsTab, DevToolsError, InspectorState, VmConnectionStatus};
    use fdemon_core::widget_tree::DiagnosticsNode;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::super::super::WidgetInspector;

    // Canonical copy lives in inspector::test_helpers (m13 fix).
    // Path: tests → super (render_object_tab) → super (details) → super (inspector) → test_helpers
    use super::super::super::test_helpers::collect_buf_text;

    // ── Test helpers ──────────────────────────────────────────────────────────

    /// Build a `DiagnosticsNode` with the given name, description, and no level.
    fn sample_node(name: &str, desc: &str, _unused: Option<()>) -> DiagnosticsNode {
        DiagnosticsNode {
            name: Some(name.to_string()),
            description: desc.to_string(),
            ..Default::default()
        }
    }

    /// Build a `DiagnosticsNode` with the given name, description, and level.
    fn sample_node_with_level(name: &str, desc: &str, level: &str) -> DiagnosticsNode {
        DiagnosticsNode {
            name: Some(name.to_string()),
            description: desc.to_string(),
            level: Some(level.to_string()),
            ..Default::default()
        }
    }

    /// Collect the full buffer text as a `String`, same as `buffer_to_string` in
    /// the task spec.
    fn buffer_to_string(buf: &Buffer) -> String {
        let area = buf.area;
        collect_buf_text(buf, area.width, area.height)
    }

    /// Render the render-object tab for the given `InspectorState` into a
    /// `(width, height)` buffer and return the buffer.
    fn render_render_object_tab(state: &InspectorState, (width, height): (u16, u16)) -> Buffer {
        let widget = WidgetInspector::new(state, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        widget.render_render_object_tab(buf.area, &mut buf);
        buf
    }

    // ── Task-specified tests ──────────────────────────────────────────────────

    #[test]
    fn render_object_tab_shows_loading_state() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("objects/42".into()),
            properties_loading: true,
            // render_properties empty
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (60, 10));
        assert!(buffer_to_string(&buf).contains("Loading"));
    }

    #[test]
    fn render_object_tab_shows_error_state() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("objects/42".into()),
            properties_error: Some(DevToolsError::new("Fetch failed", "Press [r] to retry")),
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (60, 10));
        let s = buffer_to_string(&buf);
        assert!(s.contains("Fetch failed"));
        assert!(s.contains("retry"));
    }

    #[test]
    fn render_object_tab_shows_no_render_object_message() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("objects/42".into()),
            // properties_loading == false, error == None, render_properties empty.
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (60, 10));
        assert!(buffer_to_string(&buf).contains("No render object"));
    }

    #[test]
    fn render_object_tab_renders_property_rows() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("objects/42".into()),
            render_properties: vec![
                sample_node("needsCompositing", "false", None),
                sample_node("creator", "Padding \u{2190} Container", None),
                sample_node("size", "Size(414.0, 600.0)", None),
            ],
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (60, 10));
        let s = buffer_to_string(&buf);
        assert!(s.contains("needsCompositing"));
        assert!(s.contains("false"));
        assert!(s.contains("creator"));
        assert!(s.contains("size"));
    }

    #[test]
    fn render_object_tab_sorts_default_level_to_end() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("objects/42".into()),
            render_properties: vec![
                sample_node_with_level("layer", "null", "fine"),
                sample_node("needsCompositing", "false", None),
                sample_node_with_level("semantics", "null", "fine"),
            ],
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (80, 10));
        let s = buffer_to_string(&buf);
        let pos_compositing = s.find("needsCompositing").unwrap();
        let pos_layer = s.find("layer").unwrap();
        let pos_semantics = s.find("semantics").unwrap();
        assert!(
            pos_compositing < pos_layer,
            "non-default should appear before default"
        );
        assert!(pos_compositing < pos_semantics);
    }

    #[test]
    fn render_object_tab_filters_hidden_level() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("objects/42".into()),
            render_properties: vec![
                sample_node("visible", "yes", None),
                sample_node_with_level("hiddenProp", "secret", "hidden"),
            ],
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (60, 10));
        let s = buffer_to_string(&buf);
        assert!(s.contains("visible"));
        assert!(!s.contains("hiddenProp"));
    }

    // ── Additional edge-case tests ────────────────────────────────────────────

    #[test]
    fn render_object_tab_no_panic_zero_area() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            render_properties: vec![sample_node("prop", "val", None)],
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (0, 0));
        let _ = buf; // should not panic
    }

    #[test]
    fn render_object_tab_no_panic_single_row() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            render_properties: vec![sample_node("prop", "val", None)],
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (40, 1));
        let _ = buf;
    }

    #[test]
    fn render_object_tab_no_node_id_shows_no_widget_selected() {
        // details_node_id == None — defensive guard.
        let state = InspectorState::default();
        let buf = render_render_object_tab(&state, (60, 10));
        let s = buffer_to_string(&buf);
        assert!(
            s.contains("No widget selected") || s.contains("widget"),
            "Expected placeholder text for no node, got: {s:?}"
        );
    }

    #[test]
    fn render_object_tab_overflow_indicator_appears_when_many_rows() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            // Create more properties than can fit in a 10-row buffer (border = 2,
            // so inner height = 8 rows).
            render_properties: (0..20)
                .map(|i| sample_node(&format!("prop{i}"), &format!("val{i}"), None))
                .collect(),
            ..Default::default()
        };
        let buf = render_render_object_tab(&state, (60, 10));
        let s = buffer_to_string(&buf);
        // Should show an overflow indicator.
        assert!(
            s.contains("more") || s.contains("..."),
            "Expected overflow indicator for many rows, got: {s:?}"
        );
    }

    #[test]
    fn render_object_tab_divider_between_non_default_and_default() {
        let state = InspectorState {
            details_node_id: Some("objects/42".into()),
            render_properties: vec![
                sample_node("normalProp", "value1", None),
                sample_node_with_level("fineProp", "value2", "fine"),
            ],
            ..Default::default()
        };
        // Use a tall area to avoid overflow clipping.
        let buf = render_render_object_tab(&state, (60, 20));
        let s = buffer_to_string(&buf);
        // Both props should appear.
        assert!(s.contains("normalProp"));
        assert!(s.contains("fineProp"));
        // The divider character '─' should appear.
        assert!(
            s.contains('\u{2500}'),
            "Expected divider character '─' between sections, got: {s:?}"
        );
    }
}
