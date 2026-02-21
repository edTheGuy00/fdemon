//! # Frame Bar Chart Widget
//!
//! Renders each Flutter frame as a pair of vertical bars (UI thread + Raster thread)
//! with colour coding for jank and shader compilation, a 16ms budget line,
//! frame selection with highlight, and a 3-line detail panel below the chart.
//!
//! Module wiring (Task 07) has connected `FrameChart` to the performance panel.

mod bars;
mod detail;

use fdemon_core::performance::{FrameTiming, PerformanceStats, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

// Re-export pure helpers so tests.rs (which uses `use super::*;`) can access them.
// bar_colors and ms_to_half_blocks are pub(crate) in bars.rs to allow this re-export.
// The cfg(test) guard prevents an unused-import warning in non-test builds.
#[cfg(test)]
pub(super) use bars::{bar_colors, ms_to_half_blocks};

// Layout constants

/// Number of terminal rows reserved for the detail panel at the bottom.
pub(super) const DETAIL_PANEL_HEIGHT: u16 = 3;

/// Terminal columns consumed by each frame (UI bar + Raster bar + gap).
pub(super) const CHARS_PER_FRAME: u16 = 3;

/// Minimum bar chart area height below which we skip the bar chart entirely.
pub(super) const MIN_CHART_HEIGHT: u16 = 4;

/// Minimum y-axis range in milliseconds — prevents flat charts for fast apps.
pub(super) const MIN_Y_RANGE_MS: f64 = 20.0;

/// 16.667ms frame budget line (60 FPS).
pub(super) const BUDGET_LINE_MS: f64 = 16.667;

// Colour helpers

/// Bar colour for a normal (non-jank) UI thread bar.
pub(super) const COLOR_UI_NORMAL: Color = Color::Cyan;

/// Bar colour for a normal (non-jank) Raster thread bar.
pub(super) const COLOR_RASTER_NORMAL: Color = Color::Green;

/// Bar colour for a janky frame (either thread, total > 16ms).
pub(super) const COLOR_JANK: Color = Color::Red;

/// Bar colour for a frame with shader compilation detected.
pub(super) const COLOR_SHADER: Color = Color::Magenta;

/// Colour of the 16ms budget dashed line and its label.
pub(super) const COLOR_BUDGET_LINE: Color = Color::DarkGray;

// FrameChart

/// Frame timing bar chart with selectable frames.
///
/// Renders each frame as a pair of vertical bars (UI thread + Raster thread)
/// with colour coding for jank/shader compilation, a 16ms budget line,
/// and a detail panel below for the selected frame.
pub(crate) struct FrameChart<'a> {
    pub(super) frame_history: &'a RingBuffer<FrameTiming>,
    pub(super) selected_frame: Option<usize>,
    pub(super) stats: &'a PerformanceStats,
    pub(super) icons: bool,
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

// Tests

#[cfg(test)]
mod tests;
