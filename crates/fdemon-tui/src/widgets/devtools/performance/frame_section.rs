//! Frame timing section for the performance panel.
//!
//! Renders the FPS header line and sparkline of recent frame times.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Sparkline, Widget};

use super::styles::{fps_style, SPARKLINE_MAX_MS};
use super::PerformancePanel;
use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Minimum height for the FPS section (header + sparkline rows).
pub(super) const FPS_SECTION_HEIGHT: u16 = 4;

/// Width below which we use compact (sparkline-less) layout.
pub(super) const COMPACT_WIDTH_THRESHOLD: u16 = 50;

// ── Frame section rendering ───────────────────────────────────────────────────

impl PerformancePanel<'_> {
    // ── FPS section ──────────────────────────────────────────────────────────

    pub(super) fn render_fps_section(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Section block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                format!(" {} Frame Timing ", self.icons.activity()),
                Style::default().fg(palette::ACCENT_DIM),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let stats = &self.performance.stats;

        // Build header line: "FPS: 60.0   Avg: 8.2ms   P95: 12.1ms   Max: 15.3ms"
        let fps_val = match stats.fps {
            Some(fps) => format!("{:.1}", fps),
            None => "\u{2014}".to_string(), // em dash — idle / no recent frames
        };
        let avg_val = stats
            .avg_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());
        let p95_val = stats
            .p95_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());
        let max_val = stats
            .max_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());

        let header = Line::from(vec![
            Span::styled("FPS: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(fps_val, fps_style(stats.fps)),
            Span::styled("   Avg: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(avg_val, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::styled("   P95: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(p95_val, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::styled("   Max: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(max_val, Style::default().fg(palette::TEXT_PRIMARY)),
        ]);

        buf.set_line(inner.x, inner.y, &header, inner.width);

        // Sparkline of recent frame times — skip in compact mode
        let sparkline_rows = inner.height.saturating_sub(1);
        if sparkline_rows > 0 && area.width >= COMPACT_WIDTH_THRESHOLD {
            let sparkline_area = Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: sparkline_rows,
            };
            self.render_frame_sparkline(sparkline_area, buf);
        }
    }

    pub(super) fn render_frame_sparkline(&self, area: Rect, buf: &mut Buffer) {
        // Convert frame history to u64 milliseconds (capped at 33ms = 2x 60fps budget)
        let frame_data: Vec<u64> = self
            .performance
            .frame_history
            .iter()
            .map(|f| f.elapsed_micros / 1000)
            .collect();

        if frame_data.is_empty() {
            let waiting =
                Paragraph::new("No frame data yet").style(Style::default().fg(palette::TEXT_MUTED));
            waiting.render(area, buf);
            return;
        }

        let sparkline = Sparkline::default()
            .data(&frame_data)
            .max(SPARKLINE_MAX_MS)
            .style(Style::default().fg(Color::Cyan));

        sparkline.render(area, buf);
    }
}
