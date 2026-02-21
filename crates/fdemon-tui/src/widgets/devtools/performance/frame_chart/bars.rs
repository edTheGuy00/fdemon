//! Bar chart rendering for [`FrameChart`].
//!
//! Contains the main bar chart rendering loop, visible range computation,
//! the 16ms budget line, and pure helper functions for bar height/colour.

use super::*;

use fdemon_core::performance::FrameTiming;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

// ── Bar chart methods ─────────────────────────────────────────────────────────

impl FrameChart<'_> {
    /// Render the bar chart section.
    pub(super) fn render_bar_chart(&self, area: Rect, buf: &mut Buffer) {
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
    pub(super) fn compute_visible_range(
        &self,
        total_frames: usize,
        max_visible: usize,
    ) -> (usize, usize) {
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
    pub(super) fn render_budget_line(&self, area: Rect, buf: &mut Buffer, budget_y: u16) {
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

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Determine the UI and Raster bar colours for a frame.
///
/// `pub(crate)` to allow re-export from `mod.rs` into tests via `use super::*`.
pub(crate) fn bar_colors(frame: &FrameTiming) -> (Color, Color) {
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
///
/// `pub(crate)` to allow re-export from `mod.rs` into tests via `use super::*`.
pub(crate) fn ms_to_half_blocks(ms: f64, y_range_ms: f64, total_half_blocks: f64) -> u16 {
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
pub(super) fn render_bar(
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
