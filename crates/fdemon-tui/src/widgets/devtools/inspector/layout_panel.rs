//! Layout panel rendering for the widget inspector.
//!
//! Shows the selected widget's box model visualization, dimensions,
//! constraints, and flex properties.

use fdemon_core::widget_tree::{DiagnosticsNode, EdgeInsets, LayoutInfo, WidgetSize};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use super::short_path;
use super::truncate_str;
use super::WidgetInspector;
use crate::theme::palette;

/// Minimum height required to render the full box model visualization.
const BOX_MODEL_MIN_HEIGHT: u16 = 7;

/// Height threshold below which only a compact single-line summary is shown.
const COMPACT_MODE_HEIGHT: u16 = 5;

// ── impl block ────────────────────────────────────────────────────────────────

impl WidgetInspector<'_> {
    /// Render the layout panel showing box model, dimensions, and constraints
    /// for the currently selected widget tree node.
    pub(super) fn render_layout_panel(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                " Layout Explorer ",
                Style::default().fg(palette::ACCENT_DIM),
            ))
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let inspector = self.inspector_state;

        if inner.height < COMPACT_MODE_HEIGHT {
            self.render_compact_summary(inner, buf, visible, selected);
            return;
        }

        if inspector.layout_loading {
            render_centered_text(inner, buf, "Loading layout...", palette::TEXT_MUTED);
        } else if let Some(ref error) = inspector.layout_error {
            render_layout_error(inner, buf, &error.message, &error.hint);
        } else if let Some(ref layout) = inspector.layout {
            self.render_full_layout(inner, buf, visible, selected, layout);
        } else {
            render_centered_text(
                inner,
                buf,
                "Select a widget to see layout details",
                palette::TEXT_MUTED,
            );
        }
    }

    fn render_compact_summary(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
    ) {
        if area.height == 0 {
            return;
        }
        let Some(layout) = &self.inspector_state.layout else {
            return;
        };
        let node_name = visible
            .get(selected)
            .map(|(n, _)| n.display_name())
            .unwrap_or("");
        let size_str = layout
            .size
            .as_ref()
            .map(|s| format!("  {:.1} x {:.1}", s.width, s.height))
            .unwrap_or_default();
        let constraints_str = layout
            .constraints
            .as_ref()
            .map(|c| {
                format!(
                    "  min:{}x{}  max:{}x{}",
                    format_constraint_value(c.min_width),
                    format_constraint_value(c.min_height),
                    format_constraint_value(c.max_width),
                    format_constraint_value(c.max_height),
                )
            })
            .unwrap_or_default();
        let line = format!("{node_name}{size_str}{constraints_str}");
        let trunc = truncate_str(&line, area.width.saturating_sub(1) as usize);
        buf.set_string(
            area.x + 1,
            area.y,
            trunc,
            Style::default().fg(palette::TEXT_PRIMARY),
        );
    }

    fn render_full_layout(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
        layout: &LayoutInfo,
    ) {
        if area.height == 0 {
            return;
        }
        let mut y = area.y;

        // Section 1: Widget name + source location
        if let Some((node, _)) = visible.get(selected) {
            if y < area.bottom() {
                let name_trunc =
                    truncate_str(node.display_name(), area.width.saturating_sub(2) as usize);
                buf.set_string(
                    area.x + 1,
                    y,
                    name_trunc,
                    Style::default()
                        .fg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD),
                );
                y += 1;
            }
            if y < area.bottom() {
                if let Some(loc) = &node.creation_location {
                    let loc_text = format!("  {}:{}", short_path(&loc.file), loc.line);
                    let loc_trunc = truncate_str(&loc_text, area.width.saturating_sub(2) as usize);
                    buf.set_string(
                        area.x + 1,
                        y,
                        loc_trunc,
                        Style::default().fg(palette::STATUS_BLUE),
                    );
                    y += 1;
                }
            }
        }

        if y < area.bottom() {
            y += 1;
        } else {
            return;
        }

        // Section 2: Box model visualization
        if let Some(ref size) = layout.size {
            let remaining = area.bottom().saturating_sub(y);
            if let Some(ref padding) = layout.padding {
                if remaining >= BOX_MODEL_MIN_HEIGHT {
                    let h = remaining.min(BOX_MODEL_MIN_HEIGHT + 2);
                    render_box_model(Rect::new(area.x, y, area.width, h), buf, size, padding);
                    y += h;
                }
            } else if remaining >= 4 {
                let h = remaining.min(6);
                render_size_box(Rect::new(area.x, y, area.width, h), buf, size);
                y += h;
            }
        }

        if y < area.bottom() {
            y += 1;
        } else {
            return;
        }

        // Section 3: Dimensions row
        if let Some(ref size) = layout.size {
            if y < area.bottom() {
                render_dimensions_row(area.x, y, area.width, buf, size);
                y += 1;
            }
        }

        if y < area.bottom() {
            y += 1;
        } else {
            return;
        }

        // Section 4: Constraints
        if let Some(ref c) = layout.constraints {
            if y < area.bottom() {
                buf.set_string(
                    area.x + 1,
                    y,
                    "Constraints",
                    Style::default().fg(palette::STATUS_YELLOW),
                );
                y += 1;
            }
            if y < area.bottom() {
                let text = format!(
                    "  min: {} x {}  max: {} x {}",
                    format_constraint_value(c.min_width),
                    format_constraint_value(c.min_height),
                    format_constraint_value(c.max_width),
                    format_constraint_value(c.max_height),
                );
                let trunc = truncate_str(&text, area.width.saturating_sub(2) as usize);
                buf.set_string(
                    area.x + 1,
                    y,
                    trunc,
                    Style::default().fg(palette::TEXT_PRIMARY),
                );
                y += 1;
            }
            if c.is_tight_width() && c.is_tight_height() && y < area.bottom() {
                buf.set_string(
                    area.x + 3,
                    y,
                    "(tight)",
                    Style::default().fg(palette::STATUS_YELLOW),
                );
                y += 1;
            }
        }

        if y < area.bottom() {
            y += 1;
        } else {
            return;
        }

        // Section 5: Flex properties
        if (layout.flex_factor.is_some()
            || layout.flex_fit.is_some()
            || layout.description.is_some())
            && y < area.bottom()
        {
            render_flex_properties(area.x, y, area.width, buf, layout);
        }
    }
}

// ── Helper render functions ───────────────────────────────────────────────────

fn render_centered_text(area: Rect, buf: &mut Buffer, text: &str, color: Color) {
    if area.height == 0 {
        return;
    }
    Paragraph::new(text)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center)
        .render(
            Rect {
                y: area.y + area.height / 2,
                height: 1,
                ..area
            },
            buf,
        );
}

fn render_layout_error(area: Rect, buf: &mut Buffer, message: &str, hint: &str) {
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

/// Render the box model visualization with nested padding/widget blocks.
pub(super) fn render_box_model(
    area: Rect,
    buf: &mut Buffer,
    size: &WidgetSize,
    padding: &EdgeInsets,
) {
    if area.height < BOX_MODEL_MIN_HEIGHT || area.width < 10 {
        return;
    }
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::STATUS_YELLOW))
        .title(Span::styled(
            " padding ",
            Style::default().fg(palette::STATUS_YELLOW),
        ))
        .title_alignment(Alignment::Left);
    let oi = outer_block.inner(area);
    outer_block.render(area, buf);

    if oi.height == 0 || oi.width == 0 {
        return;
    }
    let mut y = oi.y;

    if y < oi.bottom() {
        let t = format!("  top: {:.1}", padding.top);
        buf.set_string(
            oi.x,
            y,
            truncate_str(&t, oi.width.saturating_sub(1) as usize),
            Style::default().fg(palette::TEXT_MUTED),
        );
        y += 1;
    }

    let mid_h = oi.height.saturating_sub(2);
    if mid_h > 0 && y < oi.bottom() {
        let inner_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::TEXT_MUTED))
            .title(Span::styled(
                " widget ",
                Style::default().fg(palette::TEXT_MUTED),
            ))
            .title_alignment(Alignment::Left);
        let off = 2u16.min(oi.width / 4);
        let iw = oi.width.saturating_sub(off * 2).max(4);
        let ih = mid_h.min(oi.bottom().saturating_sub(y + 1));

        if ih > 0 && iw > 0 {
            let ir = Rect::new(oi.x + off, y, iw, ih);
            let ca = inner_block.inner(ir);
            inner_block.render(ir, buf);

            if ca.height > 0 && ca.width > 0 {
                let st = format!("{:.1} x {:.1}", size.width, size.height);
                let cx = ca.x + (ca.width.saturating_sub(st.len() as u16)) / 2;
                let cy = ca.y + ca.height / 2;
                if cy < ca.bottom() {
                    buf.set_string(
                        cx,
                        cy,
                        &st,
                        Style::default()
                            .fg(palette::STATUS_GREEN)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
            if oi.x < oi.x + off {
                let lbl = format!("{:.0}", padding.left);
                let ly = y + ih / 2;
                if ly < oi.bottom() {
                    buf.set_string(oi.x + 1, ly, &lbl, Style::default().fg(palette::TEXT_MUTED));
                }
            }
            y += ih;
        }
    }

    if y < oi.bottom() {
        let b = format!("  bottom: {:.1}", padding.bottom);
        buf.set_string(
            oi.x,
            y,
            truncate_str(&b, oi.width.saturating_sub(1) as usize),
            Style::default().fg(palette::TEXT_MUTED),
        );
    }
}

/// Render the simplified size box without padding wrapper.
pub(super) fn render_size_box(area: Rect, buf: &mut Buffer, size: &WidgetSize) {
    if area.height < 4 || area.width < 8 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::STATUS_GREEN))
        .title(Span::styled(
            " Size ",
            Style::default().fg(palette::STATUS_GREEN),
        ))
        .title_alignment(Alignment::Left);
    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let st = format!("{:.1} x {:.1}", size.width, size.height);
    let cx = inner.x + (inner.width.saturating_sub(st.len() as u16)) / 2;
    if inner.y < inner.bottom() {
        buf.set_string(
            cx,
            inner.y,
            &st,
            Style::default()
                .fg(palette::STATUS_GREEN)
                .add_modifier(Modifier::BOLD),
        );
    }

    if inner.height > 4 && inner.width > 10 {
        let max_d = size.width.max(size.height);
        if max_d > 0.0 {
            let aw = (inner.width as f64) - 4.0;
            let ah = (inner.height as f64) - 4.0;
            let bw = ((size.width / max_d) * aw).clamp(3.0, aw) as u16;
            let bh = ((size.height / max_d) * ah).clamp(1.0, ah) as u16;
            let bx = inner.x + (inner.width.saturating_sub(bw)) / 2;
            let by = (inner.y + 2).min(inner.bottom().saturating_sub(1));
            let bh = bh.min(inner.bottom().saturating_sub(by));
            if bh > 0 && bw > 0 {
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(palette::TEXT_MUTED))
                    .render(Rect::new(bx, by, bw, bh), buf);
            }
        }
    }
}

fn render_dimensions_row(x: u16, y: u16, width: u16, buf: &mut Buffer, size: &WidgetSize) {
    let lw = "W: ";
    let vw = format!("{:.1}", size.width);
    let sep = "  ";
    let lh = "H: ";
    let vh = format!("{:.1}", size.height);
    let full = format!("{lw}{vw}{sep}{lh}{vh}");
    let max_w = width.saturating_sub(2) as usize;
    if full.len() > max_w {
        buf.set_string(
            x + 1,
            y,
            truncate_str(&full, max_w),
            Style::default().fg(palette::STATUS_GREEN),
        );
        return;
    }
    let mut cx = x + 1;
    let pieces: &[(&str, Style)] = &[
        (lw, Style::default().fg(palette::TEXT_MUTED)),
        (
            &vw,
            Style::default()
                .fg(palette::STATUS_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        (sep, Style::default().fg(palette::TEXT_MUTED)),
        (lh, Style::default().fg(palette::TEXT_MUTED)),
        (
            &vh,
            Style::default()
                .fg(palette::STATUS_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    for (txt, style) in pieces {
        buf.set_string(cx, y, txt, *style);
        cx += txt.len() as u16;
    }
}

fn render_flex_properties(x: u16, y: u16, width: u16, buf: &mut Buffer, layout: &LayoutInfo) {
    let parts: Vec<String> = [
        layout.flex_factor.map(|f| format!("flex: {f:.0}")),
        layout.flex_fit.as_ref().map(|fit| format!("fit: {fit}")),
        layout.description.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    if parts.is_empty() {
        return;
    }
    let text = parts.join("  ");
    buf.set_string(
        x + 1,
        y,
        truncate_str(&text, width.saturating_sub(2) as usize),
        Style::default().fg(palette::STATUS_INDIGO),
    );
}

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Format a constraint value: `"Inf"` for infinity or very large values, `"{:.1}"` otherwise.
pub(super) fn format_constraint_value(value: f64) -> String {
    if value == f64::INFINITY || value >= 1e10 {
        "Inf".to_string()
    } else {
        format!("{value:.1}")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "layout_panel_tests.rs"]
mod tests;
