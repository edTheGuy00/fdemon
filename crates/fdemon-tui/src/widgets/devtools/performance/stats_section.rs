//! Stats section for the performance panel.
//!
//! Renders frames, jank, and GC counts.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use super::styles::{format_number, jank_style};
use super::PerformancePanel;
use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Minimum height for the stats section.
pub(super) const STATS_SECTION_HEIGHT: u16 = 3;

// ── Stats section rendering ───────────────────────────────────────────────────

impl PerformancePanel<'_> {
    // ── Stats section ────────────────────────────────────────────────────────

    pub(super) fn render_stats_section(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                format!(" {} Stats ", self.icons.activity()),
                Style::default().fg(palette::ACCENT_DIM),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let stats = &self.performance.stats;
        let gc_count = self.performance.gc_history.len();
        let last_gc = self.performance.gc_history.latest();

        // Jank percentage
        let jank_pct = if stats.buffered_frames > 0 {
            (stats.jank_count as f64 / stats.buffered_frames as f64) * 100.0
        } else {
            0.0
        };

        let gc_type_str = last_gc.map(|gc| gc.gc_type.as_str()).unwrap_or("\u{2014}");

        // Line 1: "Frames: 1,234   Jank: 12 (0.97%)   GC: 3 (MarkSweep)"
        let frames_str = format_number(stats.buffered_frames);
        let line1 = Line::from(vec![
            Span::styled("Frames: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(frames_str, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::styled("   Jank: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(
                format!("{}", stats.jank_count),
                jank_style(stats.jank_count, stats.buffered_frames),
            ),
            Span::styled(
                format!(" ({:.2}%)", jank_pct),
                Style::default().fg(palette::TEXT_MUTED),
            ),
            Span::styled("   GC: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(
                format!("{} ({})", gc_count, gc_type_str),
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
        ]);
        buf.set_line(inner.x, inner.y, &line1, inner.width);

        // Line 2: "Monitoring: Active"  or  "Monitoring: Inactive"
        if inner.height >= 2 {
            let (status_label, status_style) = if self.performance.monitoring_active {
                ("Active", Style::default().fg(palette::STATUS_GREEN))
            } else {
                ("Inactive", Style::default().fg(palette::TEXT_MUTED))
            };

            let line2 = Line::from(vec![
                Span::styled("Monitoring: ", Style::default().fg(palette::TEXT_SECONDARY)),
                Span::styled(status_label, status_style),
            ]);
            buf.set_line(inner.x, inner.y + 1, &line2, inner.width);
        }
    }
}
