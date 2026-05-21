//! Bar chart rendering for [`FrameChart`].
//!
//! Contains the main bar chart rendering loop, visible range computation,
//! the 16ms budget line, and pure helper functions for bar height/colour.

use super::*;

use fdemon_app::{MouseAction, MouseRect};
use fdemon_core::performance::FrameTiming;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::widgets::MouseCtx;

/// Z-index for per-bar click regions.
///
/// Must be higher than the section-level focus region (z=0) registered by
/// [`PerformancePanel`] so per-bar selection wins on overlap.
const BAR_CLICK_Z_INDEX: u8 = 1;

/// Minimum half-block height for a nonzero frame.
///
/// Prevents fast frames (e.g. ~1 ms) from becoming invisible at small terminal
/// heights. A nonzero `ms` value always renders at least one half-block.
const MIN_BAR_HALF_BLOCKS: u16 = 1;

// ── Bar chart methods ─────────────────────────────────────────────────────────

impl FrameChart<'_> {
    /// Render the bar chart section.
    ///
    /// Pass `Some(ctx)` to record one click region per visible frame slot.
    /// Pass `None` (as in the [`Widget::render`] impl) to skip region recording.
    pub(super) fn render_bar_chart(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mut ctx: Option<&mut MouseCtx<'_>>,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let max_visible = (area.width / CHARS_PER_FRAME) as usize;
        if max_visible == 0 {
            return;
        }

        // EXCEPTION (TEA): render-hint Cell write-back — see docs/CODE_STANDARDS.md
        // Principle 3 and "Region Registry Pattern".
        self.frame_chart_visible_width.set(max_visible);

        let total_frames = self.frame_history.len();
        if total_frames == 0 {
            return;
        }

        // Determine the visible window of frames.
        // scroll_offset is always the viewport authority (Model A — "frames back
        // from the live edge"). The selection highlight is applied on top of
        // whatever window scroll_offset designates.
        let (start_idx, end_idx) =
            compute_visible_range(total_frames, max_visible, self.scroll_offset);

        // Collect visible frames (oldest first so they render left-to-right)
        let visible: Vec<&FrameTiming> = self
            .frame_history
            .iter()
            .skip(start_idx)
            .take(end_idx - start_idx)
            .collect();

        // Compute y-axis scale: scale to ~1.5× the slowest visible frame so
        // bars use the chart's full vertical range, with a small floor
        // (MIN_Y_RANGE_MS) to avoid flat charts when the visible frames are
        // uniformly tiny. Picking the rounding unit by magnitude keeps axis
        // labels readable across very different frame rates.
        let max_ms_observed = visible
            .iter()
            .map(|f| f.elapsed_ms())
            .fold(0.0_f64, f64::max);
        let target = (max_ms_observed * 1.5).max(MIN_Y_RANGE_MS);
        let unit = if target <= 5.0 {
            1.0
        } else if target <= 20.0 {
            2.0
        } else {
            10.0
        };
        let y_range_ms = (target / unit).ceil() * unit;

        // Each character row represents 2 "half-block" units.
        // Total half-block units available = chart_height * 2.
        let total_half_blocks = (area.height as f64) * 2.0;

        // Budget line: only render when 16.667ms falls inside the visible range.
        // When all visible frames are well under the 60-FPS budget (fast app on
        // a fast device), the line would sit above the chart and just clutter
        // the view; suppressing it makes more vertical space available for the
        // bars.
        if BUDGET_LINE_MS <= y_range_ms {
            let budget_frac = BUDGET_LINE_MS / y_range_ms;
            let budget_row_from_bottom = (budget_frac * total_half_blocks / 2.0).round() as u16;
            let budget_y = area
                .bottom()
                .saturating_sub(1)
                .saturating_sub(budget_row_from_bottom);
            let budget_y = budget_y.clamp(area.y, area.bottom().saturating_sub(1));
            self.render_budget_line(area, buf, budget_y);
        }

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

            // Selection highlight: paint left-eighth (▏) and right-eighth (▕) side
            // markers on every row of the chart area to frame the selected bar
            // without obscuring its content (Option A).
            if is_selected {
                let hl_style = Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD);
                // Left marker column: one column to the left of the UI bar (if in bounds)
                let left_marker_x = x.saturating_sub(1);
                // Right marker column: one column to the right of the Raster bar
                let right_marker_x = x + 2;

                // Paint markers on every row of the chart area.
                // left_marker_x is only distinct from x when x > 0 (saturating_sub).
                let has_left_marker = left_marker_x < x && left_marker_x >= area.x;
                let has_right_marker = right_marker_x < area.right();
                for row_y in area.y..area.bottom() {
                    if has_left_marker {
                        if let Some(cell) = buf.cell_mut((left_marker_x, row_y)) {
                            cell.set_char('\u{258F}').set_style(hl_style); // ▏ left eighth
                        }
                    }
                    if has_right_marker {
                        if let Some(cell) = buf.cell_mut((right_marker_x, row_y)) {
                            cell.set_char('\u{2595}').set_style(hl_style); // ▕ right eighth
                        }
                    }
                }
            }

            // Register a click region covering the full slot width and chart height.
            // Clicking anywhere in the bar pair (UI + Raster + gap) selects the frame.
            // z=BAR_CLICK_Z_INDEX (1) so these win over the section-level focus region at z=0.
            if let Some(c) = ctx.as_deref_mut() {
                // Width: CHARS_PER_FRAME (3) per slot, but clamp to available columns
                // at the right edge of the chart so we never exceed the area bounds.
                let avail = area.right().saturating_sub(x);
                let rect_w = CHARS_PER_FRAME.min(avail);
                if rect_w > 0 && area.height > 0 {
                    let rect = MouseRect::new(x, area.y, rect_w, area.height);
                    c.click_at_z(
                        rect,
                        MouseAction::emit(fdemon_app::Message::SelectPerformanceFrame {
                            index: Some(global_idx),
                        }),
                        BAR_CLICK_Z_INDEX,
                    );
                }
            }
        }
    }

    // MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
    #[allow(clippy::manual_is_multiple_of)]
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
                if (x - line_start_x) % 2 == 0 {
                    cell.set_char('╌').set_style(line_style);
                }
            }
            x += 1;
        }
    }
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Compute which slice of `frame_history` to display.
///
/// Returns `(start_idx, end_idx)` — exclusive end, i.e. `frame_history[start..end]`.
///
/// `scroll_offset` is the sole viewport authority (Model A — "frames back from the
/// live edge"). Selection highlighting is applied on top of whatever window
/// `scroll_offset` designates; this function does not need to know about the
/// selected frame index.
///
/// - `scroll_offset == 0`: live-edge mode — always shows the most recent
///   `visible_width` frames.
/// - `scroll_offset > 0`: frozen-scroll mode. The window anchors at
///   `frame_count - scroll_offset`. As new frames arrive the absolute window
///   drifts forward by the same amount, preserving the "N frames back from
///   latest" mental model.
pub fn compute_visible_range(
    frame_count: usize,
    visible_width: usize,
    scroll_offset: usize,
) -> (usize, usize) {
    let end = frame_count.saturating_sub(scroll_offset);
    let start = end.saturating_sub(visible_width);
    (start, end)
}

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
/// Nonzero `ms` values are clamped to at least [`MIN_BAR_HALF_BLOCKS`] (1) so
/// that fast frames (~1 ms) never become invisible at small terminal heights.
///
/// `pub(crate)` to allow re-export from `mod.rs` into tests via `use super::*`.
pub(crate) fn ms_to_half_blocks(ms: f64, y_range_ms: f64, total_half_blocks: f64) -> u16 {
    if y_range_ms <= 0.0 || ms <= 0.0 {
        return 0;
    }
    let raw = ((ms / y_range_ms) * total_half_blocks).round() as u16;
    // Never let a nonzero frame become invisible — clamp to at least MIN_BAR_HALF_BLOCKS.
    raw.max(MIN_BAR_HALF_BLOCKS)
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
