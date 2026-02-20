//! Memory gauge section for the performance panel.
//!
//! Renders the memory usage header, gauge, and detail line.

use fdemon_core::performance::MemoryUsage;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph, Widget};

use super::styles::gauge_style_for_utilization;
use super::PerformancePanel;
use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Minimum height for the memory section (header + gauge + detail rows).
pub(super) const MEMORY_SECTION_HEIGHT: u16 = 4;

// ── Memory section rendering ──────────────────────────────────────────────────

impl PerformancePanel<'_> {
    // ── Memory section ───────────────────────────────────────────────────────

    pub(super) fn render_memory_section(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                format!(" {} Memory ", self.icons.cpu()),
                Style::default().fg(palette::ACCENT_DIM),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let latest = self.performance.memory_history.latest();

        if let Some(mem) = latest {
            // Distribute inner area: header line, gauge, detail line
            let available = inner.height;

            // Line 1: "Heap: 45.2 MB / 128.0 MB  (35%)"
            let usage_text = format!(
                "Heap: {} / {}  ({:.0}%)",
                MemoryUsage::format_bytes(mem.heap_usage),
                MemoryUsage::format_bytes(mem.heap_capacity),
                mem.utilization() * 100.0,
            );
            let header_line = Line::from(Span::styled(
                usage_text,
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            buf.set_line(inner.x, inner.y, &header_line, inner.width);

            // Gauge on line 2
            if available >= 2 {
                let gauge_area = Rect {
                    x: inner.x,
                    y: inner.y + 1,
                    width: inner.width,
                    height: 1,
                };
                let util = mem.utilization().clamp(0.0, 1.0);
                let gauge = Gauge::default()
                    .ratio(util)
                    .gauge_style(gauge_style_for_utilization(util))
                    .label(Span::styled(
                        format!("{:.0}%", util * 100.0),
                        Style::default().fg(palette::TEXT_BRIGHT),
                    ));
                gauge.render(gauge_area, buf);
            }

            // Line 3: "External: 12.5 MB   Total: 57.7 MB"
            if available >= 3 {
                let detail_text = format!(
                    "External: {}   Total: {}",
                    MemoryUsage::format_bytes(mem.external_usage),
                    MemoryUsage::format_bytes(mem.total()),
                );
                let detail_line = Line::from(Span::styled(
                    detail_text,
                    Style::default().fg(palette::TEXT_SECONDARY),
                ));
                buf.set_line(inner.x, inner.y + 2, &detail_line, inner.width);
            }
        } else {
            // No data yet
            let text = Paragraph::new("Waiting for memory data...")
                .style(Style::default().fg(palette::TEXT_MUTED));
            text.render(inner, buf);
        }
    }
}
