//! Layout explorer panel for the DevTools TUI mode.
//!
//! Renders an ASCII visualization of the selected widget's box constraints,
//! actual size, and flex properties. Handles loading, error, and empty states
//! when no layout data is available.

use fdemon_app::state::{DevToolsError, LayoutExplorerState, VmConnectionStatus};
use fdemon_core::widget_tree::{BoxConstraints, LayoutInfo, WidgetSize};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use super::truncate_str;
use crate::theme::palette;

// ── LayoutExplorer ────────────────────────────────────────────────────────────

/// Layout explorer panel for the DevTools mode.
///
/// Renders an ASCII visualization of the selected widget's box constraints,
/// actual size, and flex properties.
pub struct LayoutExplorer<'a> {
    layout_state: &'a LayoutExplorerState,
    /// The currently selected widget name (from inspector).
    selected_widget_name: Option<&'a str>,
    /// Whether the VM Service WebSocket is currently connected.
    /// When `false`, the panel renders a dedicated "VM Service disconnected"
    /// state instead of the generic empty/error state.
    vm_connected: bool,
    /// Rich connection status for contextual disconnected messaging.
    connection_status: &'a VmConnectionStatus,
}

impl<'a> LayoutExplorer<'a> {
    /// Create a new `LayoutExplorer` widget.
    pub fn new(
        layout_state: &'a LayoutExplorerState,
        selected_widget_name: Option<&'a str>,
        vm_connected: bool,
        connection_status: &'a VmConnectionStatus,
    ) -> Self {
        Self {
            layout_state,
            selected_widget_name,
            vm_connected,
            connection_status,
        }
    }
}

impl Widget for LayoutExplorer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(" Layout Explorer ")
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        if !self.vm_connected {
            self.render_disconnected(inner, buf);
        } else if self.layout_state.loading {
            self.render_loading(inner, buf);
        } else if let Some(ref error) = self.layout_state.error {
            self.render_error_box(inner, buf, error);
        } else if let Some(ref layout) = self.layout_state.layout {
            self.render_layout(inner, buf, layout);
        } else {
            self.render_no_selection(inner, buf);
        }
    }
}

impl LayoutExplorer<'_> {
    // ── Loading / Error / Empty / Disconnected states ─────────────────────────

    fn render_disconnected(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let status_line = match self.connection_status {
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts,
            } => {
                format!("Reconnecting to VM Service... ({attempt}/{max_attempts})")
            }
            VmConnectionStatus::TimedOut => "Layout data fetch timed out.".to_string(),
            _ => "VM Service disconnected.".to_string(),
        };

        let lines = vec![
            Line::from(Span::styled(
                status_line,
                Style::default().fg(palette::STATUS_RED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Layout data is unavailable while disconnected.",
                Style::default().fg(palette::TEXT_MUTED),
            )),
            Line::from(Span::styled(
                "Waiting for reconnection...",
                Style::default().fg(palette::TEXT_MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [r] to retry  |  Press [b] to open browser DevTools  |  Press [Esc] to return to logs",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

        let y_offset = area.height.saturating_sub(6) / 2;
        let render_area = Rect {
            y: area.y + y_offset,
            height: 6.min(area.height),
            ..area
        };
        paragraph.render(render_area, buf);
    }

    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let text = Paragraph::new("Loading layout data...")
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center);
        let y_offset = area.height / 2;
        text.render(
            Rect {
                y: area.y + y_offset,
                height: 1,
                ..area
            },
            buf,
        );
    }

    fn render_error_box(&self, area: Rect, buf: &mut Buffer, error: &DevToolsError) {
        if area.height == 0 {
            return;
        }

        let lines = vec![
            Line::from(Span::styled(
                format!("\u{26a0} {}", error.message),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                error.hint.as_str(),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[r] Retry   [b] Browser DevTools   [Esc] Return to logs",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

        let content_height = 5u16;
        let y_offset = area.height.saturating_sub(content_height) / 2;
        paragraph.render(
            Rect {
                y: area.y + y_offset,
                height: content_height.min(area.height),
                ..area
            },
            buf,
        );
    }

    fn render_no_selection(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let lines = vec![
            Line::from(Span::styled(
                "Select a widget in the Inspector panel first",
                Style::default().fg(palette::TEXT_MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Switch to Inspector (press 'i'), select a widget,",
                Style::default().fg(palette::TEXT_MUTED),
            )),
            Line::from(Span::styled(
                "then return here to view its layout.",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);

        let y_offset = area.height.saturating_sub(4) / 2;
        paragraph.render(
            Rect {
                y: area.y + y_offset,
                height: 4.min(area.height),
                ..area
            },
            buf,
        );
    }

    // ── Layout rendering ──────────────────────────────────────────────────────

    fn render_layout(&self, area: Rect, buf: &mut Buffer, layout: &LayoutInfo) {
        if area.height == 0 {
            return;
        }

        let mut y = area.y;

        // Widget name header
        if let Some(name) = self.selected_widget_name {
            if y < area.bottom() {
                let header = format!("Widget: {name}");
                let header_trunc = truncate_str(&header, area.width as usize);
                buf.set_string(
                    area.x + 1,
                    y,
                    header_trunc,
                    Style::default()
                        .fg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD),
                );
                y += 1;
            }
        }

        // Spacer
        if y < area.bottom() {
            y += 1;
        } else {
            return;
        }

        // Constraints section (4 rows + 2 borders = 6 rows minimum)
        if let Some(ref constraints) = layout.constraints {
            let constraints_height = 6u16.min(area.bottom().saturating_sub(y));
            if constraints_height >= 3 {
                let constraints_rect = Rect::new(area.x, y, area.width, constraints_height);
                self.render_constraints(constraints_rect, buf, constraints);
                y += constraints_height;

                // Spacer
                if y < area.bottom() {
                    y += 1;
                } else {
                    return;
                }
            }
        }

        // Size section (proportional box)
        if let Some(ref size) = layout.size {
            let remaining = area.bottom().saturating_sub(y);
            let size_height = if layout.flex_factor.is_some() || layout.flex_fit.is_some() {
                // Leave 2 rows for flex info below
                remaining.saturating_sub(3).max(4)
            } else {
                remaining
            };
            if size_height >= 4 {
                let size_rect = Rect::new(area.x, y, area.width, size_height);
                self.render_size_box(size_rect, buf, size);
                y += size_height;
            }
        }

        // Flex properties
        if layout.flex_factor.is_some() || layout.flex_fit.is_some() || layout.description.is_some()
        {
            if y < area.bottom() {
                // Spacer
                y += 1;
            }
            if y < area.bottom() {
                let flex_rect = Rect::new(area.x, y, area.width, 1);
                self.render_flex_properties(flex_rect, buf, layout);
            }
        }
    }

    // ── Constraints section ───────────────────────────────────────────────────

    fn render_constraints(&self, area: Rect, buf: &mut Buffer, constraints: &BoxConstraints) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::STATUS_BLUE))
            .title(" Constraints ")
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // min row
        let min_text = format!(
            "  min: {:.1} x {:.1}",
            constraints.min_width, constraints.min_height
        );
        if inner.y < inner.bottom() {
            let trunc = truncate_str(&min_text, inner.width as usize);
            buf.set_string(
                inner.x,
                inner.y,
                trunc,
                Style::default().fg(palette::STATUS_BLUE),
            );
        }

        // max row
        let max_text = format!(
            "  max: {} x {}",
            format_constraint_value(constraints.max_width),
            format_constraint_value(constraints.max_height),
        );
        if inner.y + 1 < inner.bottom() {
            let trunc = truncate_str(&max_text, inner.width as usize);
            buf.set_string(
                inner.x,
                inner.y + 1,
                trunc,
                Style::default().fg(palette::STATUS_BLUE),
            );
        }

        // Tight constraint indicator
        if constraints.min_width == constraints.max_width
            && constraints.min_height == constraints.max_height
            && inner.y + 2 < inner.bottom()
        {
            buf.set_string(
                inner.x + 2,
                inner.y + 2,
                "(tight)",
                Style::default().fg(palette::STATUS_YELLOW),
            );
        }
    }

    // ── Size box section ──────────────────────────────────────────────────────

    fn render_size_box(&self, area: Rect, buf: &mut Buffer, size: &WidgetSize) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::STATUS_GREEN))
            .title(" Size ")
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Size label centered on top
        let size_text = format!("{:.1} x {:.1}", size.width, size.height);
        let x = inner.x + (inner.width.saturating_sub(size_text.len() as u16)) / 2;
        if inner.y < inner.bottom() {
            buf.set_string(
                x,
                inner.y,
                &size_text,
                Style::default()
                    .fg(palette::STATUS_GREEN)
                    .add_modifier(Modifier::BOLD),
            );
        }

        // Proportional inner box visualization
        if inner.height > 4 && inner.width > 10 {
            let max_dim = size.width.max(size.height);
            if max_dim > 0.0 {
                let available_w = (inner.width as f64) - 4.0;
                let available_h = (inner.height as f64) - 4.0;

                let box_w = ((size.width / max_dim) * available_w).clamp(3.0, available_w) as u16;
                let box_h = ((size.height / max_dim) * available_h).clamp(1.0, available_h) as u16;

                let box_x = inner.x + (inner.width.saturating_sub(box_w)) / 2;
                let box_y = inner.y + 2;

                // Clamp to available area
                let box_y = box_y.min(inner.bottom().saturating_sub(1));
                let box_h = box_h.min(inner.bottom().saturating_sub(box_y));

                if box_h > 0 && box_w > 0 {
                    let box_rect = Rect::new(box_x, box_y, box_w, box_h);
                    let inner_box = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(palette::TEXT_MUTED));
                    inner_box.render(box_rect, buf);
                }
            }
        }
    }

    // ── Flex properties ───────────────────────────────────────────────────────

    fn render_flex_properties(&self, area: Rect, buf: &mut Buffer, layout: &LayoutInfo) {
        let mut parts = Vec::new();

        if let Some(factor) = layout.flex_factor {
            parts.push(format!("flex: {:.1}", factor));
        }

        if let Some(ref fit) = layout.flex_fit {
            parts.push(format!("fit: {}", fit));
        }

        if let Some(ref desc) = layout.description {
            // Only show description if different from selected_widget_name
            if Some(desc.as_str()) != self.selected_widget_name {
                parts.push(desc.clone());
            }
        }

        if parts.is_empty() {
            return;
        }

        let text = parts.join("  ");
        let trunc = truncate_str(&text, area.width.saturating_sub(1) as usize);
        buf.set_string(
            area.x + 1,
            area.y,
            trunc,
            Style::default().fg(palette::STATUS_INDIGO),
        );
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Format a layout constraint value for display. Infinite values render as "Inf".
pub fn format_constraint_value(value: f64) -> String {
    if value == f64::INFINITY || value >= 1e10 {
        "Inf".to_string()
    } else {
        format!("{:.1}", value)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::state::{DevToolsError, LayoutExplorerState, VmConnectionStatus};
    use fdemon_core::widget_tree::{BoxConstraints, LayoutInfo, WidgetSize};

    fn make_test_layout() -> LayoutInfo {
        LayoutInfo {
            constraints: Some(BoxConstraints {
                min_width: 0.0,
                max_width: 414.0,
                min_height: 0.0,
                max_height: 896.0,
            }),
            size: Some(WidgetSize {
                width: 414.0,
                height: 896.0,
            }),
            flex_factor: Some(1.0),
            flex_fit: Some("tight".to_string()),
            description: None,
        }
    }

    #[test]
    fn test_layout_explorer_renders_with_data() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(make_test_layout());
        let widget = LayoutExplorer::new(
            &state,
            Some("Scaffold"),
            true,
            &VmConnectionStatus::Connected,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_layout_explorer_no_selection() {
        let state = LayoutExplorerState::default();
        let widget = LayoutExplorer::new(&state, None, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_layout_explorer_loading() {
        let mut state = LayoutExplorerState::default();
        state.loading = true;
        let widget =
            LayoutExplorer::new(&state, Some("Column"), true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_layout_explorer_error_state() {
        let mut state = LayoutExplorerState::default();
        state.error = Some(DevToolsError::new(
            "VM Service not available",
            "Ensure the app is running in debug mode",
        ));
        let widget = LayoutExplorer::new(&state, None, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_format_infinity_constraint() {
        assert_eq!(format_constraint_value(f64::INFINITY), "Inf");
        assert_eq!(format_constraint_value(414.0), "414.0");
        assert_eq!(format_constraint_value(0.0), "0.0");
    }

    #[test]
    fn test_format_large_value_shows_inf() {
        // Values >= 1e10 are treated as "Inf"
        assert_eq!(format_constraint_value(1e10), "Inf");
        assert_eq!(format_constraint_value(1e15), "Inf");
    }

    #[test]
    fn test_tight_constraints_detected() {
        let constraints = BoxConstraints {
            min_width: 100.0,
            max_width: 100.0,
            min_height: 50.0,
            max_height: 50.0,
        };
        assert_eq!(constraints.min_width, constraints.max_width);
        assert_eq!(constraints.min_height, constraints.max_height);
    }

    #[test]
    fn test_layout_explorer_small_terminal() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(make_test_layout());
        let widget = LayoutExplorer::new(
            &state,
            Some("Scaffold"),
            true,
            &VmConnectionStatus::Connected,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
    }

    #[test]
    fn test_layout_explorer_minimum_terminal() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(make_test_layout());
        let widget = LayoutExplorer::new(
            &state,
            Some("Scaffold"),
            true,
            &VmConnectionStatus::Connected,
        );
        // Minimum per acceptance criteria: 40x10
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_layout_explorer_zero_size_no_panic() {
        let state = LayoutExplorerState::default();
        let widget = LayoutExplorer::new(&state, None, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        widget.render(Rect::new(0, 0, 1, 1), &mut buf);
    }

    #[test]
    fn test_layout_explorer_loading_contains_message() {
        let mut state = LayoutExplorerState::default();
        state.loading = true;
        let widget =
            LayoutExplorer::new(&state, Some("Column"), true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Loading"),
            "Expected 'Loading' in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_error_contains_error_text() {
        let mut state = LayoutExplorerState::default();
        state.error = Some(DevToolsError::new(
            "VM Service connection lost",
            "Reconnecting automatically...",
        ));
        let widget = LayoutExplorer::new(&state, None, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("VM Service") || full.contains("Reconnecting"),
            "Expected user-friendly error message in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_no_selection_contains_prompt() {
        let state = LayoutExplorerState::default();
        let widget = LayoutExplorer::new(&state, None, true, &VmConnectionStatus::Connected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Select a widget") || full.contains("Inspector"),
            "Expected no-selection message in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_renders_constraint_values() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(LayoutInfo {
            constraints: Some(BoxConstraints {
                min_width: 0.0,
                max_width: f64::INFINITY,
                min_height: 0.0,
                max_height: f64::INFINITY,
            }),
            size: None,
            flex_factor: None,
            flex_fit: None,
            description: None,
        });
        let widget = LayoutExplorer::new(
            &state,
            Some("Container"),
            true,
            &VmConnectionStatus::Connected,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Inf"),
            "Expected 'Inf' for infinity constraint, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_renders_tight_indicator() {
        let mut state = LayoutExplorerState::default();
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
            flex_factor: None,
            flex_fit: None,
            description: None,
        });
        let widget = LayoutExplorer::new(
            &state,
            Some("SizedBox"),
            true,
            &VmConnectionStatus::Connected,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("tight"),
            "Expected '(tight)' indicator for tight constraints, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_flex_properties_shown() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(LayoutInfo {
            constraints: None,
            size: None,
            flex_factor: Some(2.0),
            flex_fit: Some("loose".to_string()),
            description: None,
        });
        let widget = LayoutExplorer::new(
            &state,
            Some("Flexible"),
            true,
            &VmConnectionStatus::Connected,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("flex") || full.contains("loose"),
            "Expected flex properties in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_disconnected_shows_vm_message() {
        let state = LayoutExplorerState::default();
        let widget = LayoutExplorer::new(&state, None, false, &VmConnectionStatus::Disconnected);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("disconnected")
                || full.contains("Disconnected")
                || full.contains("VM Service"),
            "Expected VM Service disconnected message in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_layout_explorer_reconnecting_shows_attempt_count() {
        let state = LayoutExplorerState::default();
        let status = VmConnectionStatus::Reconnecting {
            attempt: 3,
            max_attempts: 10,
        };
        let widget = LayoutExplorer::new(&state, None, false, &status);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Reconnecting") || full.contains("3"),
            "Expected reconnecting message with attempt count, got: {full:?}"
        );
    }
}
