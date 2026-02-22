//! Memory chart widget for the DevTools performance panel.
//!
//! Renders a time-series chart using Unicode braille characters showing
//! stacked memory layers (Dart heap, native, raster cache) with line
//! overlays (allocated, RSS), GC event markers, a legend, and a class
//! allocation table below.
//!
//! Module wiring (Task 07) has connected `MemoryChart` to the performance panel.

mod braille_canvas;
mod chart;
mod table;

use braille_canvas::BrailleCanvas;
use chart::{render_history_chart, render_legend, render_sample_chart, render_x_axis_labels};
use table::render_allocation_table;

use fdemon_app::session::AllocationSortColumn;
use fdemon_core::performance::{AllocationProfile, GcEvent, MemorySample, MemoryUsage, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::styles::{format_number, gauge_style_for_utilization};
use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

pub(super) const LEGEND_HEIGHT: u16 = 1;
pub(super) const MIN_CHART_HEIGHT: u16 = 6;
/// Minimum inner height for the allocation table section.
/// Accepts header (1 row) + 1 data row, which is the smallest useful view.
pub(super) const MIN_TABLE_HEIGHT: u16 = 2;
pub(super) const TABLE_HEADER_HEIGHT: u16 = 2; // header + separator
pub(super) const MAX_TABLE_ROWS: usize = 10;
const CHART_PROPORTION: f64 = 0.6; // 60% chart, 40% table
/// Width of the Y-axis label column in characters (e.g., "128 MB ").
const Y_AXIS_WIDTH: u16 = 7;

// ── Chart colors ─────────────────────────────────────────────────────────────

pub(super) const COLOR_DART_HEAP: Color = Color::Cyan;
pub(super) const COLOR_NATIVE: Color = Color::Blue;
pub(super) const COLOR_RASTER: Color = Color::Magenta;
pub(super) const COLOR_ALLOCATED: Color = Color::Yellow;
pub(super) const COLOR_RSS: Color = Color::Gray;
pub(super) const COLOR_GC_MARKER: Color = Color::Yellow;

// ── MemoryChart widget ────────────────────────────────────────────────────────

/// Time-series memory chart with stacked area layers, GC markers, and
/// an allocation table.
///
/// # Layout
///
/// ```text
/// ┌─ Memory ──────────────────────────────────────┐
/// │ Legend: ■ Dart Heap  ■ Native  ■ Raster       │
/// │                                               │
/// │ 128MB ┤                     ╭──── RSS         │
/// │       │              ╭──────╯                 │
/// │  64MB ┤       ╭──────╯                        │
/// │       │ ╭─────╯                               │
/// │  32MB ┤─╯                                     │
/// │       └────────────────────────────────────── │
/// │        60s ago             30s         now    │
/// ├────────────────────────────────────────────── ┤
/// │ Class              Instances   Size            │
/// │ _String            12,345      2.4 MB          │
/// └───────────────────────────────────────────────┘
/// ```
pub(crate) struct MemoryChart<'a> {
    memory_samples: &'a RingBuffer<MemorySample>,
    memory_history: &'a RingBuffer<MemoryUsage>,
    gc_history: &'a RingBuffer<GcEvent>,
    allocation_profile: Option<&'a AllocationProfile>,
    allocation_sort: AllocationSortColumn,
    icons: bool,
}

impl<'a> MemoryChart<'a> {
    /// Create a new memory chart widget.
    pub(crate) fn new(
        memory_samples: &'a RingBuffer<MemorySample>,
        memory_history: &'a RingBuffer<MemoryUsage>,
        gc_history: &'a RingBuffer<GcEvent>,
        allocation_profile: Option<&'a AllocationProfile>,
        allocation_sort: AllocationSortColumn,
        icons: bool,
    ) -> Self {
        Self {
            memory_samples,
            memory_history,
            gc_history,
            allocation_profile,
            allocation_sort,
            icons,
        }
    }
}

impl Widget for MemoryChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let _use_icons = self.icons; // kept for future icon expansion

        // Very small area: single-line summary
        if area.height < MIN_CHART_HEIGHT {
            render_compact_summary(self.memory_samples, self.memory_history, area, buf);
            return;
        }

        // Determine whether to show the allocation table
        let show_table = area.height >= MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT;

        if show_table {
            let chart_height =
                ((area.height as f64 * CHART_PROPORTION) as u16).max(MIN_CHART_HEIGHT);
            let table_height = area.height.saturating_sub(chart_height);

            let chart_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: chart_height,
            };
            let table_area = Rect {
                x: area.x,
                y: area.y + chart_height,
                width: area.width,
                height: table_height,
            };

            render_chart_area(
                self.memory_samples,
                self.memory_history,
                self.gc_history,
                chart_area,
                buf,
            );
            render_allocation_table(
                self.allocation_profile,
                self.allocation_sort,
                table_area,
                buf,
            );
        } else {
            render_chart_area(
                self.memory_samples,
                self.memory_history,
                self.gc_history,
                area,
                buf,
            );
        }
    }
}

// ── Compact summary ───────────────────────────────────────────────────────────

/// Render a single-line memory summary when the area is too small for a chart.
fn render_compact_summary(
    samples: &RingBuffer<MemorySample>,
    history: &RingBuffer<MemoryUsage>,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let (text, style) = if let Some(s) = samples.latest() {
        let util = if s.allocated > 0 {
            (s.dart_heap + s.dart_native + s.raster_cache) as f64 / s.allocated as f64
        } else {
            0.0
        };
        let formatted = format!(
            "Heap: {}  Native: {}  RSS: {}",
            MemoryUsage::format_bytes(s.dart_heap),
            MemoryUsage::format_bytes(s.dart_native),
            MemoryUsage::format_bytes(s.rss),
        );
        (formatted, gauge_style_for_utilization(util.clamp(0.0, 1.0)))
    } else if let Some(m) = history.latest() {
        let util = m.utilization();
        let formatted = format!(
            "Heap: {} / {}  ({:.0}%)",
            MemoryUsage::format_bytes(m.heap_usage),
            MemoryUsage::format_bytes(m.heap_capacity),
            util * 100.0,
        );
        (formatted, gauge_style_for_utilization(util.clamp(0.0, 1.0)))
    } else {
        (
            "No memory data".to_string(),
            Style::default().fg(palette::TEXT_PRIMARY),
        )
    };

    let line = Line::from(Span::styled(text, style));
    buf.set_line(area.x, area.y, &line, area.width);
}

// ── Chart area ────────────────────────────────────────────────────────────────

/// Render the braille time-series chart into `area`.
fn render_chart_area(
    samples: &RingBuffer<MemorySample>,
    history: &RingBuffer<MemoryUsage>,
    gc_history: &RingBuffer<GcEvent>,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.width < 4 || area.height < 2 {
        return;
    }

    // Reserve the first row for the legend and last row for x-axis labels.
    let legend_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: LEGEND_HEIGHT,
    };

    // Chart plot area (below legend, leaving bottom row for x-axis)
    let plot_top = area.y + LEGEND_HEIGHT;
    let plot_height = area.height.saturating_sub(LEGEND_HEIGHT + 1);
    let plot_left = area.x + Y_AXIS_WIDTH;
    let plot_width = area.width.saturating_sub(Y_AXIS_WIDTH);

    if plot_height == 0 || plot_width == 0 {
        // Not enough space — just render the legend and return
        render_legend(samples, history, legend_area, buf);
        return;
    }

    let plot_area = Rect {
        x: plot_left,
        y: plot_top,
        width: plot_width,
        height: plot_height,
    };

    // Decide which data source to use
    if !samples.is_empty() {
        render_sample_chart(samples, gc_history, plot_area, area, Y_AXIS_WIDTH, buf);
    } else if !history.is_empty() {
        render_history_chart(history, plot_area, area, Y_AXIS_WIDTH, buf);
    } else {
        // No data at all
        let msg = Span::styled(
            "No memory data yet",
            Style::default().fg(palette::TEXT_MUTED),
        );
        let line = Line::from(msg);
        buf.set_line(plot_area.x, plot_area.y, &line, plot_area.width);
    }

    // Legend (computed after we know which layers are active)
    render_legend(samples, history, legend_area, buf);

    // X-axis labels
    let xaxis_y = area.y + area.height - 1;
    render_x_axis_labels(plot_left, plot_width, xaxis_y, buf);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
