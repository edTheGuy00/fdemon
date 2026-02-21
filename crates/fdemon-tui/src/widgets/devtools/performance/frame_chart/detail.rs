//! Detail panel rendering for [`FrameChart`].
//!
//! Contains the 3-line detail panel below the bar chart, individual frame
//! breakdown rendering, the summary line (no selection), and status helpers.

use super::*;

use fdemon_core::performance::{FramePhases, FrameTiming};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::super::styles::{fps_style, jank_style};

// ── Detail panel methods ──────────────────────────────────────────────────────

impl FrameChart<'_> {
    /// Render the 3-line detail panel below the chart.
    pub(super) fn render_detail_panel(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        match self
            .selected_frame
            .and_then(|i| self.frame_history.iter().nth(i))
        {
            Some(frame) => self.render_frame_detail(area, buf, frame),
            None => self.render_summary_line(area, buf),
        }
    }

    /// Render the detail lines for a selected frame.
    pub(super) fn render_frame_detail(&self, area: Rect, buf: &mut Buffer, frame: &FrameTiming) {
        // Line 0: "Frame #1234  Total: 18.2ms (JANK)" or "(SHADER)"
        let total_ms = frame.elapsed_ms();
        let frame_label = format!("Frame #{}", frame.number);
        let total_label = format!("  Total: {:.1}ms", total_ms);

        let (status_label, status_style) = frame_status_label_and_style(frame);

        let line0 = Line::from(vec![
            Span::styled(
                frame_label,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(total_label, Style::default().fg(Color::Gray)),
            Span::styled(status_label, status_style),
        ]);
        buf.set_line(area.x, area.y, &line0, area.width);

        if area.height < 2 {
            return;
        }

        // Line 1: UI thread breakdown
        let ui_ms = frame.build_ms();
        let line1 = if let Some(phases) = &frame.phases {
            render_ui_phase_line(ui_ms, phases)
        } else {
            Line::from(vec![
                Span::styled("UI: ", Style::default().fg(Color::Gray)),
                Span::styled(format!("{:.1}ms", ui_ms), Style::default().fg(Color::Cyan)),
            ])
        };
        buf.set_line(area.x, area.y + 1, &line1, area.width);

        if area.height < 3 {
            return;
        }

        // Line 2: Raster thread
        let raster_ms = frame.raster_ms();
        let line2 = Line::from(vec![
            Span::styled("Raster: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.1}ms", raster_ms),
                Style::default().fg(Color::Green),
            ),
        ]);
        buf.set_line(area.x, area.y + 2, &line2, area.width);
    }

    /// Render the single-line summary when no frame is selected.
    pub(super) fn render_summary_line(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let stats = self.stats;

        let fps_str = match stats.fps {
            Some(fps) => format!("{:.0}", fps),
            None => "\u{2014}".to_string(),
        };
        let avg_str = stats
            .avg_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());

        let jank_pct = if stats.buffered_frames > 0 {
            (stats.jank_count as f64 / stats.buffered_frames as f64) * 100.0
        } else {
            0.0
        };

        let shader_count: usize = self
            .frame_history
            .iter()
            .filter(|f| f.has_shader_compilation())
            .count();

        // Format: "FPS: 60  Avg: 8.2ms  Jank: 2 (1.3%)  Shader: 0"
        let _use_icons = self.icons; // kept for future icon expansion
        let line = Line::from(vec![
            Span::styled("FPS: ", Style::default().fg(Color::DarkGray)),
            Span::styled(fps_str, fps_style(stats.fps)),
            Span::styled("  Avg: ", Style::default().fg(Color::DarkGray)),
            Span::styled(avg_str, Style::default().fg(Color::Gray)),
            Span::styled("  Jank: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} ({:.1}%)", stats.jank_count, jank_pct),
                jank_style(stats.jank_count, stats.buffered_frames),
            ),
            Span::styled("  Shader: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", shader_count),
                if shader_count > 0 {
                    Style::default().fg(COLOR_SHADER)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]);

        buf.set_line(area.x, area.y, &line, area.width);
    }
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Build the status label and style for the detail panel header line.
pub(super) fn frame_status_label_and_style(frame: &FrameTiming) -> (&'static str, Style) {
    if frame.has_shader_compilation() {
        (
            "  (SHADER)",
            Style::default()
                .fg(COLOR_SHADER)
                .add_modifier(Modifier::BOLD),
        )
    } else if frame.is_janky() {
        (
            "  (JANK)",
            Style::default().fg(COLOR_JANK).add_modifier(Modifier::BOLD),
        )
    } else {
        ("", Style::default())
    }
}

/// Build the UI thread breakdown line for the detail panel.
pub(super) fn render_ui_phase_line(ui_ms: f64, phases: &FramePhases) -> Line<'static> {
    let build_ms = phases.build_micros as f64 / 1000.0;
    let layout_ms = phases.layout_micros as f64 / 1000.0;
    let paint_ms = phases.paint_micros as f64 / 1000.0;

    let phase_style = Style::default().fg(Color::DarkGray);

    Line::from(vec![
        Span::styled("UI: ", Style::default().fg(Color::Gray)),
        Span::styled(format!("{:.1}ms", ui_ms), Style::default().fg(Color::Cyan)),
        Span::styled("  (", phase_style),
        Span::styled("Build: ", phase_style),
        Span::styled(
            format!("{:.1}ms", build_ms),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("  Layout: ", phase_style),
        Span::styled(
            format!("{:.1}ms", layout_ms),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("  Paint: ", phase_style),
        Span::styled(
            format!("{:.1}ms", paint_ms),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(")", phase_style),
    ])
}
