//! Memory panel widget for the DevTools TUI mode.
//!
//! Displays the memory usage time-series chart and class allocation table
//! using data from `MemoryState` (rich memory samples, allocation profile,
//! GC events). This widget gets the full panel inner area — chart on top,
//! allocation table below.

mod braille_canvas;
mod chart;
mod table;

use std::cell::Cell;

use braille_canvas::BrailleCanvas;
use chart::{render_history_chart, render_legend, render_sample_chart, render_x_axis_labels};
use table::{render_allocation_table, AllocationTable};

use fdemon_app::session::memory::{MemorySection, MemoryState};
use fdemon_core::performance::{AllocationProfile, GcEvent, MemorySample, MemoryUsage, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::widgets::MouseCtx;

use crate::theme::palette;

// Re-export the scroll window helper so tests.rs can use it via `use super::*`.
#[cfg(test)]
pub(super) use chart::visible_memory_window;

// ── Layout constants ──────────────────────────────────────────────────────────

pub(super) const LEGEND_HEIGHT: u16 = 1;
pub(super) const MIN_CHART_HEIGHT: u16 = 6;
/// Minimum inner height for the allocation table section.
/// Accepts header (1 row) + 1 data row, which is the smallest useful view.
pub(super) const MIN_TABLE_HEIGHT: u16 = 2;
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

// ── Helpers used by performance/mod.rs (no longer needed here, but kept for format_number) ──

fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}G", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

// ── MemoryPanel widget ────────────────────────────────────────────────────────

/// Memory panel widget for the DevTools mode.
///
/// Displays the memory usage time-series chart and class allocation table
/// using data from `MemoryState`. Gets the full panel inner area — chart on top,
/// allocation table below.
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
pub struct MemoryPanel<'a> {
    memory: &'a MemoryState,
    /// Whether the entire memory panel has focus (for outer border colour).
    #[allow(dead_code)]
    focused: bool,
    // ── Time-series chart scroll / focus ──────────────────────────────────────
    chart_scroll_offset: usize,
    #[allow(dead_code)]
    chart_focused: bool,
    chart_visible_width_cell: Option<&'a Cell<usize>>,
    // ── Alloc table interactivity ──────────────────────────────────────────────
    alloc_scroll_offset: usize,
    alloc_selected_row: Option<usize>,
    alloc_focused: bool,
    alloc_visible_height_cell: Option<&'a Cell<usize>>,
}

impl<'a> MemoryPanel<'a> {
    /// Create a new memory panel widget from `MemoryState`.
    pub fn new(memory: &'a MemoryState, focused: bool) -> Self {
        Self {
            memory,
            focused,
            chart_scroll_offset: memory.memory_chart_scroll_offset,
            chart_focused: memory.focused_section == MemorySection::Chart,
            chart_visible_width_cell: Some(&memory.memory_chart_visible_width),
            alloc_scroll_offset: memory.alloc_table_scroll_offset,
            alloc_selected_row: memory.alloc_table_selected_row,
            alloc_focused: memory.focused_section == MemorySection::AllocationList,
            alloc_visible_height_cell: Some(&memory.alloc_table_visible_height),
        }
    }

    // ── Shared render implementation ──────────────────────────────────────────

    /// Core render logic shared by [`Widget::render`] and [`render_with_regions`].
    fn render_impl(self, area: Rect, buf: &mut Buffer, ctx: Option<&mut MouseCtx<'_>>) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Very small area: single-line summary
        if area.height < MIN_CHART_HEIGHT {
            render_compact_summary(
                &self.memory.memory_samples,
                &self.memory.memory_history,
                area,
                buf,
            );
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
                &self.memory.memory_samples,
                &self.memory.memory_history,
                &self.memory.gc_history,
                self.chart_scroll_offset,
                self.chart_visible_width_cell,
                chart_area,
                buf,
            );

            // Render allocation table
            match (
                self.memory.allocation_profile.as_ref(),
                self.alloc_visible_height_cell,
            ) {
                (Some(profile), Some(cell)) => {
                    let table = AllocationTable {
                        profile,
                        sort_column: self.memory.allocation_sort,
                        scroll_offset: self.alloc_scroll_offset,
                        selected_row: self.alloc_selected_row,
                        focused: self.alloc_focused,
                        visible_height_cell: cell,
                    };
                    table.render(table_area, buf, ctx);
                }
                (profile, _) => {
                    render_allocation_table(profile, self.memory.allocation_sort, table_area, buf);
                }
            }
        } else {
            render_chart_area(
                &self.memory.memory_samples,
                &self.memory.memory_history,
                &self.memory.gc_history,
                self.chart_scroll_offset,
                self.chart_visible_width_cell,
                area,
                buf,
            );
        }
    }
}

impl Widget for MemoryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_impl(area, buf, None);
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

fn gauge_style_for_utilization(utilization: f64) -> Style {
    let color = if utilization > 0.9 {
        palette::STATUS_RED
    } else if utilization > 0.7 {
        palette::STATUS_YELLOW
    } else {
        palette::TEXT_PRIMARY
    };
    Style::default().fg(color)
}

// ── Chart area ────────────────────────────────────────────────────────────────

/// Render the braille time-series chart into `area`.
fn render_chart_area(
    samples: &RingBuffer<MemorySample>,
    history: &RingBuffer<MemoryUsage>,
    gc_history: &RingBuffer<GcEvent>,
    scroll_offset: usize,
    visible_width_cell: Option<&Cell<usize>>,
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
        render_sample_chart(
            samples,
            gc_history,
            scroll_offset,
            visible_width_cell,
            plot_area,
            area,
            Y_AXIS_WIDTH,
            buf,
        );
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

/// Render the memory panel, optionally recording clickable regions.
///
/// This is the click-aware entry point used by `devtools::render_with_regions`.
/// Passing `None` for `ctx` produces output byte-identical to `Widget::render`.
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    widget: MemoryPanel<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    widget.render_impl(area, buf, ctx);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
