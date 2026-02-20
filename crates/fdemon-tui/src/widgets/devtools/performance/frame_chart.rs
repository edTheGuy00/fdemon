//! # Frame Bar Chart Widget
//!
//! Renders each Flutter frame as a pair of vertical bars (UI thread + Raster thread)
//! with colour coding for jank and shader compilation, a 16ms budget line,
//! frame selection with highlight, and a 3-line detail panel below the chart.
//!
//! Module wiring (Task 07) has connected `FrameChart` to the performance panel.

use fdemon_core::performance::{FramePhases, FrameTiming, PerformanceStats, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::styles::{fps_style, jank_style};

// ── Layout constants ──────────────────────────────────────────────────────────

/// Number of terminal rows reserved for the detail panel at the bottom.
const DETAIL_PANEL_HEIGHT: u16 = 3;

/// Terminal columns consumed by each frame (UI bar + Raster bar + gap).
const CHARS_PER_FRAME: u16 = 3;

/// Minimum bar chart area height below which we skip the bar chart entirely.
const MIN_CHART_HEIGHT: u16 = 4;

/// Minimum y-axis range in milliseconds — prevents flat charts for fast apps.
const MIN_Y_RANGE_MS: f64 = 20.0;

/// 16.667ms frame budget line (60 FPS).
const BUDGET_LINE_MS: f64 = 16.667;

// ── Colour helpers ────────────────────────────────────────────────────────────

/// Bar colour for a normal (non-jank) UI thread bar.
const COLOR_UI_NORMAL: Color = Color::Cyan;

/// Bar colour for a normal (non-jank) Raster thread bar.
const COLOR_RASTER_NORMAL: Color = Color::Green;

/// Bar colour for a janky frame (either thread, total > 16ms).
const COLOR_JANK: Color = Color::Red;

/// Bar colour for a frame with shader compilation detected.
const COLOR_SHADER: Color = Color::Magenta;

/// Colour of the 16ms budget dashed line and its label.
const COLOR_BUDGET_LINE: Color = Color::DarkGray;

// ── FrameChart ────────────────────────────────────────────────────────────────

/// Frame timing bar chart with selectable frames.
///
/// Renders each frame as a pair of vertical bars (UI thread + Raster thread)
/// with colour coding for jank/shader compilation, a 16ms budget line,
/// and a detail panel below for the selected frame.
pub(crate) struct FrameChart<'a> {
    frame_history: &'a RingBuffer<FrameTiming>,
    selected_frame: Option<usize>,
    stats: &'a PerformanceStats,
    icons: bool,
}

impl<'a> FrameChart<'a> {
    /// Create a new [`FrameChart`] widget.
    ///
    /// # Arguments
    /// * `frame_history` - Rolling history of frame timings from `PerformanceState`.
    /// * `selected_frame` - Optional index into `frame_history` for the selected frame.
    /// * `stats` - Aggregated performance statistics for the summary line.
    /// * `icons` - Whether to use Unicode icon characters (disabled for narrow/ASCII terminals).
    pub fn new(
        frame_history: &'a RingBuffer<FrameTiming>,
        selected_frame: Option<usize>,
        stats: &'a PerformanceStats,
        icons: bool,
    ) -> Self {
        Self {
            frame_history,
            selected_frame,
            stats,
            icons,
        }
    }
}

impl Widget for FrameChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let total_h = area.height;

        // Compact mode: area is too small for chart + detail panel
        if total_h < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT {
            self.render_summary_line(area, buf);
            return;
        }

        let chart_h = total_h - DETAIL_PANEL_HEIGHT;
        let chart_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: chart_h,
        };
        let detail_area = Rect {
            x: area.x,
            y: area.y + chart_h,
            width: area.width,
            height: DETAIL_PANEL_HEIGHT,
        };

        self.render_bar_chart(chart_area, buf);
        self.render_detail_panel(detail_area, buf);
    }
}

// ── Bar chart rendering ───────────────────────────────────────────────────────

impl FrameChart<'_> {
    /// Render the bar chart section.
    fn render_bar_chart(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let max_visible = (area.width / CHARS_PER_FRAME) as usize;
        if max_visible == 0 {
            return;
        }

        let total_frames = self.frame_history.len();
        if total_frames == 0 {
            return;
        }

        // Determine the visible window of frames.
        // Prefer: show the most recent N frames.
        // Exception: if a frame is selected, scroll so the selected frame is visible.
        let (start_idx, end_idx) = self.compute_visible_range(total_frames, max_visible);

        // Collect visible frames (oldest first so they render left-to-right)
        let visible: Vec<&FrameTiming> = self
            .frame_history
            .iter()
            .skip(start_idx)
            .take(end_idx - start_idx)
            .collect();

        // Compute y-axis scale based on max elapsed_ms in visible frames
        let max_ms = visible
            .iter()
            .map(|f| f.elapsed_ms())
            .fold(MIN_Y_RANGE_MS, f64::max);

        // Round up to nearest 10ms boundary for a clean axis
        let y_range_ms = (max_ms / 10.0).ceil() * 10.0;

        // Each character row represents 2 "half-block" units.
        // Total half-block units available = chart_height * 2.
        let total_half_blocks = (area.height as f64) * 2.0;

        // Budget line y position (in chart row coordinates from top)
        let budget_frac = BUDGET_LINE_MS / y_range_ms;
        let budget_row_from_bottom = (budget_frac * total_half_blocks / 2.0).round() as u16;
        let budget_y = area
            .bottom()
            .saturating_sub(1)
            .saturating_sub(budget_row_from_bottom);
        let budget_y = budget_y.clamp(area.y, area.bottom().saturating_sub(1));

        // Draw budget dashed line
        self.render_budget_line(area, buf, budget_y);

        // Render each visible frame as a pair of bars
        for (slot, frame) in visible.iter().enumerate() {
            let global_idx = start_idx + slot;
            let x = area.x + (slot as u16) * CHARS_PER_FRAME;

            if x + 1 >= area.right() {
                break;
            }

            let is_selected = self.selected_frame == Some(global_idx);

            let (ui_color, raster_color) = bar_colors(frame);

            // UI bar height in half-block units
            let ui_ms = frame.build_ms();
            let ui_half_blocks =
                ms_to_half_blocks(ui_ms, y_range_ms, total_half_blocks).min(area.height * 2);

            // Raster bar height in half-block units
            let raster_ms = frame.raster_ms();
            let raster_half_blocks =
                ms_to_half_blocks(raster_ms, y_range_ms, total_half_blocks).min(area.height * 2);

            let bottom_y = area.bottom().saturating_sub(1);

            render_bar(buf, x, bottom_y, ui_half_blocks, ui_color, area.y);
            render_bar(
                buf,
                x + 1,
                bottom_y,
                raster_half_blocks,
                raster_color,
                area.y,
            );

            // Selection highlight: draw a `▔` (upper one-eighth block) above the taller bar
            if is_selected && area.y < area.bottom() {
                let highlight_y = area.y;
                // Write highlight indicator on the top row for both bar columns
                let hl_style = Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD);
                if let Some(cell) = buf.cell_mut((x, highlight_y)) {
                    cell.set_char('▔').set_style(hl_style);
                }
                if x + 1 < area.right() {
                    if let Some(cell) = buf.cell_mut((x + 1, highlight_y)) {
                        cell.set_char('▔').set_style(hl_style);
                    }
                }
            }
        }
    }

    /// Compute which slice of `frame_history` to display.
    ///
    /// Returns `(start_idx, end_idx)` — exclusive end, i.e. `frame_history[start..end]`.
    fn compute_visible_range(&self, total_frames: usize, max_visible: usize) -> (usize, usize) {
        let visible_count = max_visible.min(total_frames);

        match self.selected_frame {
            None => {
                // Show the most recent frames
                let end = total_frames;
                let start = end.saturating_sub(visible_count);
                (start, end)
            }
            Some(sel) => {
                // Keep selected frame in view — prefer showing it at the right side
                // but scroll left if near the start.
                let end = (sel + 1).min(total_frames);
                let start = end.saturating_sub(visible_count);
                (start, end)
            }
        }
    }

    /// Draw the 16ms dashed budget line across the chart area.
    fn render_budget_line(&self, area: Rect, buf: &mut Buffer, budget_y: u16) {
        if budget_y < area.y || budget_y >= area.bottom() {
            return;
        }

        // Label: "16ms" at the left edge
        let label = "16ms";
        let label_style = Style::default().fg(COLOR_BUDGET_LINE);
        let line_style = Style::default().fg(COLOR_BUDGET_LINE);

        // Write label
        for (i, ch) in label.chars().enumerate() {
            let lx = area.x + i as u16;
            if lx >= area.right() {
                break;
            }
            if let Some(cell) = buf.cell_mut((lx, budget_y)) {
                cell.set_char(ch).set_style(label_style);
            }
        }

        // Draw dashed line after label
        let line_start_x = area.x + label.len() as u16;
        let mut x = line_start_x;
        while x < area.right() {
            if let Some(cell) = buf.cell_mut((x, budget_y)) {
                // Skip cells that are part of bar columns (avoid overwriting bars)
                // Use dashed '╌' for every other cell to create a dashed effect
                if (x - line_start_x).is_multiple_of(2) {
                    cell.set_char('╌').set_style(line_style);
                }
            }
            x += 1;
        }
    }
}

// ── Detail panel ─────────────────────────────────────────────────────────────

impl FrameChart<'_> {
    /// Render the 3-line detail panel below the chart.
    fn render_detail_panel(&self, area: Rect, buf: &mut Buffer) {
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
    fn render_frame_detail(&self, area: Rect, buf: &mut Buffer, frame: &FrameTiming) {
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
    fn render_summary_line(&self, area: Rect, buf: &mut Buffer) {
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

/// Determine the UI and Raster bar colours for a frame.
fn bar_colors(frame: &FrameTiming) -> (Color, Color) {
    if frame.has_shader_compilation() {
        (COLOR_SHADER, COLOR_SHADER)
    } else if frame.is_janky() {
        (COLOR_JANK, COLOR_JANK)
    } else {
        (COLOR_UI_NORMAL, COLOR_RASTER_NORMAL)
    }
}

/// Convert a frame time in milliseconds to a number of half-block units.
///
/// Each terminal row is 2 half-block units tall, so using half-blocks
/// doubles the vertical resolution.
fn ms_to_half_blocks(ms: f64, y_range_ms: f64, total_half_blocks: f64) -> u16 {
    if y_range_ms <= 0.0 || ms <= 0.0 {
        return 0;
    }
    ((ms / y_range_ms) * total_half_blocks).round() as u16
}

/// Render a vertical bar using half-block Unicode characters for 2× vertical resolution.
///
/// - `█` = full block (both top and bottom halves filled)
/// - `▄` = lower half block (bottom half only — used for odd pixel at top of bar)
/// - ` ` = empty space
///
/// The bar grows upward from `bottom_y`. Rows outside `[top_y, bottom_y]` are skipped.
fn render_bar(
    buf: &mut Buffer,
    x: u16,
    bottom_y: u16,
    height_half_blocks: u16,
    color: Color,
    top_y: u16,
) {
    if height_half_blocks == 0 {
        return;
    }

    let full_rows = height_half_blocks / 2;
    let has_half = height_half_blocks % 2 == 1;
    let style = Style::default().fg(color);

    // Draw full-block rows from the bottom upward
    for row in 0..full_rows {
        let y = bottom_y.saturating_sub(row);
        if y < top_y {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('\u{2588}').set_style(style); // █
        }
    }

    // Draw the half-block at the top of the bar (if the height is odd)
    if has_half {
        let y = bottom_y.saturating_sub(full_rows);
        if y >= top_y {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char('\u{2584}').set_style(style); // ▄
            }
        }
    }
}

/// Build the status label and style for the detail panel header line.
fn frame_status_label_and_style(frame: &FrameTiming) -> (&'static str, Style) {
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
fn render_ui_phase_line(ui_ms: f64, phases: &FramePhases) -> Line<'static> {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
