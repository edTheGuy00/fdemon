//! Memory chart widget for the DevTools performance panel.
//!
//! Renders a time-series chart using Unicode braille characters showing
//! stacked memory layers (Dart heap, native, raster cache) with line
//! overlays (allocated, RSS), GC event markers, a legend, and a class
//! allocation table below.
//!
//! Module wiring (Task 07) has connected `MemoryChart` to the performance panel.

mod braille_canvas;

use braille_canvas::BrailleCanvas;

use fdemon_core::performance::{AllocationProfile, GcEvent, MemorySample, MemoryUsage, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::styles::{format_number, gauge_style_for_utilization};
use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

const LEGEND_HEIGHT: u16 = 1;
const MIN_CHART_HEIGHT: u16 = 6;
const MIN_TABLE_HEIGHT: u16 = 3;
const TABLE_HEADER_HEIGHT: u16 = 2; // header + separator
const MAX_TABLE_ROWS: usize = 10;
const CHART_PROPORTION: f64 = 0.6; // 60% chart, 40% table

// ── Chart colors ─────────────────────────────────────────────────────────────

const COLOR_DART_HEAP: Color = Color::Cyan;
const COLOR_NATIVE: Color = Color::Blue;
const COLOR_RASTER: Color = Color::Magenta;
const COLOR_ALLOCATED: Color = Color::Yellow;
const COLOR_RSS: Color = Color::Gray;
const COLOR_GC_MARKER: Color = Color::Yellow;

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
    icons: bool,
}

impl<'a> MemoryChart<'a> {
    /// Create a new memory chart widget.
    pub(crate) fn new(
        memory_samples: &'a RingBuffer<MemorySample>,
        memory_history: &'a RingBuffer<MemoryUsage>,
        gc_history: &'a RingBuffer<GcEvent>,
        allocation_profile: Option<&'a AllocationProfile>,
        icons: bool,
    ) -> Self {
        Self {
            memory_samples,
            memory_history,
            gc_history,
            allocation_profile,
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
            render_allocation_table(self.allocation_profile, table_area, buf);
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
    // Y-axis label column: 7 characters wide (e.g., "128 MB ")
    let y_axis_width: u16 = 7;
    let legend_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: LEGEND_HEIGHT,
    };

    // Chart plot area (below legend, leaving bottom row for x-axis)
    let plot_top = area.y + LEGEND_HEIGHT;
    let plot_height = area.height.saturating_sub(LEGEND_HEIGHT + 1);
    let plot_left = area.x + y_axis_width;
    let plot_width = area.width.saturating_sub(y_axis_width);

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
        render_sample_chart(samples, gc_history, plot_area, area, y_axis_width, buf);
    } else if !history.is_empty() {
        render_history_chart(history, plot_area, area, y_axis_width, buf);
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

// ── Sample-based chart (rich MemorySample data) ───────────────────────────────

/// Render the stacked braille chart from `MemorySample` data.
fn render_sample_chart(
    samples: &RingBuffer<MemorySample>,
    gc_history: &RingBuffer<GcEvent>,
    plot_area: Rect,
    full_area: Rect,
    y_axis_width: u16,
    buf: &mut Buffer,
) {
    let n = samples.len();
    if n == 0 {
        return;
    }

    let pw = plot_area.width as usize;
    let ph = plot_area.height as usize;

    // Compute max value for y-axis scaling
    let max_bytes: u64 = samples
        .iter()
        .map(|s| s.rss.max(s.dart_heap + s.dart_native + s.raster_cache))
        .max()
        .unwrap_or(1)
        .max(1);

    // Render y-axis labels
    render_y_axis_labels(max_bytes, full_area, y_axis_width, buf);

    // Canvas dimensions in dot-space: width*2, height*4
    let dot_w = pw * 2;
    let dot_h = ph * 4;

    // Collect samples into fixed-width columns
    let sample_data: Vec<&MemorySample> = samples.iter().collect();

    // Helper: map a byte value to a dot-space y coordinate.
    // y=0 is the top, y=dot_h-1 is the bottom (highest memory value).
    let byte_to_dot_y = |bytes: u64| -> usize {
        let ratio = bytes as f64 / max_bytes as f64;
        let dot = (ratio * (dot_h as f64 - 1.0)) as usize;
        // Invert so that larger values appear higher (toward the top)
        dot_h.saturating_sub(1).saturating_sub(dot)
    };

    let sample_to_dot_x = |idx: usize| -> usize {
        if n <= 1 {
            dot_w.saturating_sub(1)
        } else {
            (idx * (dot_w - 1)) / (n - 1)
        }
    };

    // ── Stacked area layers ──────────────────────────────────────────────────
    //
    // For each x-column in dot space, fill dots from the bottom to each layer top.
    // Layers (bottom to top): Dart Heap, Native, Raster Cache.

    let mut canvas_heap = BrailleCanvas::new(pw, ph);
    let mut canvas_native = BrailleCanvas::new(pw, ph);
    let mut canvas_raster = BrailleCanvas::new(pw, ph);
    let mut canvas_allocated = BrailleCanvas::new(pw, ph);
    let mut canvas_rss = BrailleCanvas::new(pw, ph);

    let has_raster = samples.iter().any(|s| s.raster_cache > 0);
    let has_rss = samples.iter().any(|s| s.rss > 0);

    for (i, sample) in sample_data.iter().enumerate() {
        let dot_x = sample_to_dot_x(i);

        // Stacked layer tops (bytes)
        let heap_top = sample.dart_heap;
        let native_top = heap_top + sample.dart_native;
        let raster_top = native_top + sample.raster_cache;

        // Fill each layer: from its floor up to its ceiling in dot-space.
        // Bottom of plot = dot_h - 1 (full height). We fill upward to each top.

        let bottom_dot_y = dot_h; // one past the bottom row (exclusive)
        let heap_ceil_y = byte_to_dot_y(heap_top);
        let native_ceil_y = byte_to_dot_y(native_top);
        let raster_ceil_y = byte_to_dot_y(raster_top);

        // Heap layer: bottom to heap_ceil_y
        for dy in heap_ceil_y..bottom_dot_y {
            canvas_heap.set(dot_x, dy);
        }
        // Native layer: heap_ceil_y down to native_ceil_y
        for dy in native_ceil_y..heap_ceil_y {
            canvas_native.set(dot_x, dy);
        }
        // Raster layer: native_ceil_y down to raster_ceil_y
        if has_raster {
            for dy in raster_ceil_y..native_ceil_y {
                canvas_raster.set(dot_x, dy);
            }
        }

        // Allocated line: draw at the allocated capacity level (every dot)
        let alloc_y = byte_to_dot_y(sample.allocated);
        canvas_allocated.set(dot_x, alloc_y);
        // Dashed appearance: also skip every other column
        if dot_x.is_multiple_of(2) && alloc_y + 1 < dot_h {
            canvas_allocated.set(dot_x, alloc_y + 1);
        }

        // RSS line (skip when zero)
        if has_rss && sample.rss > 0 {
            let rss_y = byte_to_dot_y(sample.rss);
            canvas_rss.set(dot_x, rss_y);
        }
    }

    // Render canvases: order matters (topmost series drawn last overrides colors)
    canvas_heap.render_to_buffer(buf, plot_area, COLOR_DART_HEAP);
    canvas_native.render_to_buffer(buf, plot_area, COLOR_NATIVE);
    if has_raster {
        canvas_raster.render_to_buffer(buf, plot_area, COLOR_RASTER);
    }
    canvas_allocated.render_to_buffer(buf, plot_area, COLOR_ALLOCATED);
    if has_rss {
        canvas_rss.render_to_buffer(buf, plot_area, COLOR_RSS);
    }

    // ── GC markers ──────────────────────────────────────────────────────────

    if !gc_history.is_empty() && n >= 2 {
        let oldest_ts = sample_data.first().map(|s| s.timestamp);
        let newest_ts = sample_data.last().map(|s| s.timestamp);

        if let (Some(oldest), Some(newest)) = (oldest_ts, newest_ts) {
            let time_span_ms = (newest - oldest).num_milliseconds().max(1) as f64;
            let bottom_y = plot_area.y + plot_area.height.saturating_sub(1);

            for gc in gc_history.iter() {
                if gc.timestamp < oldest || gc.timestamp > newest {
                    continue;
                }
                let offset_ms = (gc.timestamp - oldest).num_milliseconds() as f64;
                let ratio = (offset_ms / time_span_ms).clamp(0.0, 1.0);
                let gx = plot_area.x + (ratio * (plot_area.width - 1) as f64) as u16;

                if gx < plot_area.right() {
                    if let Some(cell) = buf.cell_mut((gx, bottom_y)) {
                        cell.set_char('\u{25BC}') // ▼
                            .set_style(Style::default().fg(COLOR_GC_MARKER));
                    }
                }
            }
        }
    }
}

// ── History-based chart (fallback with MemoryUsage data) ─────────────────────

/// Render a simplified chart from `MemoryUsage` data (fallback when no samples).
fn render_history_chart(
    history: &RingBuffer<MemoryUsage>,
    plot_area: Rect,
    full_area: Rect,
    y_axis_width: u16,
    buf: &mut Buffer,
) {
    let n = history.len();
    if n == 0 {
        return;
    }

    let pw = plot_area.width as usize;
    let ph = plot_area.height as usize;

    let max_bytes: u64 = history
        .iter()
        .map(|m| m.heap_capacity.max(m.total()))
        .max()
        .unwrap_or(1)
        .max(1);

    render_y_axis_labels(max_bytes, full_area, y_axis_width, buf);

    let dot_w = pw * 2;
    let dot_h = ph * 4;

    let history_data: Vec<&MemoryUsage> = history.iter().collect();

    let byte_to_dot_y = |bytes: u64| -> usize {
        let ratio = bytes as f64 / max_bytes as f64;
        let dot = (ratio * (dot_h as f64 - 1.0)) as usize;
        dot_h.saturating_sub(1).saturating_sub(dot)
    };

    let sample_to_dot_x = |idx: usize| -> usize {
        if n <= 1 {
            dot_w.saturating_sub(1)
        } else {
            (idx * (dot_w - 1)) / (n - 1)
        }
    };

    let mut canvas_heap = BrailleCanvas::new(pw, ph);
    let mut canvas_allocated = BrailleCanvas::new(pw, ph);

    for (i, mem) in history_data.iter().enumerate() {
        let dot_x = sample_to_dot_x(i);
        let bottom_dot_y = dot_h;
        let heap_ceil_y = byte_to_dot_y(mem.heap_usage);

        for dy in heap_ceil_y..bottom_dot_y {
            canvas_heap.set(dot_x, dy);
        }

        // Capacity line (dashed)
        let alloc_y = byte_to_dot_y(mem.heap_capacity);
        canvas_allocated.set(dot_x, alloc_y);
        if dot_x.is_multiple_of(2) && alloc_y + 1 < dot_h {
            canvas_allocated.set(dot_x, alloc_y + 1);
        }
    }

    canvas_heap.render_to_buffer(buf, plot_area, COLOR_DART_HEAP);
    canvas_allocated.render_to_buffer(buf, plot_area, COLOR_ALLOCATED);
}

// ── Legend ────────────────────────────────────────────────────────────────────

/// Render the single-line legend for the active chart layers.
fn render_legend(
    samples: &RingBuffer<MemorySample>,
    history: &RingBuffer<MemoryUsage>,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let mut spans: Vec<Span> = Vec::new();

    if !samples.is_empty() {
        let has_raster = samples.iter().any(|s| s.raster_cache > 0);
        let has_rss = samples.iter().any(|s| s.rss > 0);

        spans.push(Span::styled(
            "\u{25A0} Heap  ",
            Style::default().fg(COLOR_DART_HEAP),
        ));
        spans.push(Span::styled(
            "\u{25A0} Native  ",
            Style::default().fg(COLOR_NATIVE),
        ));
        if has_raster {
            spans.push(Span::styled(
                "\u{25A0} Raster  ",
                Style::default().fg(COLOR_RASTER),
            ));
        }
        spans.push(Span::styled(
            "\u{2500} Allocated  ",
            Style::default().fg(COLOR_ALLOCATED),
        ));
        if has_rss {
            spans.push(Span::styled(
                "\u{2500} RSS  ",
                Style::default().fg(COLOR_RSS),
            ));
        }
        spans.push(Span::styled(
            "\u{25BC} GC",
            Style::default().fg(COLOR_GC_MARKER),
        ));
    } else if !history.is_empty() {
        spans.push(Span::styled(
            "\u{25A0} Heap  ",
            Style::default().fg(COLOR_DART_HEAP),
        ));
        spans.push(Span::styled(
            "\u{2500} Capacity",
            Style::default().fg(COLOR_ALLOCATED),
        ));
    } else {
        spans.push(Span::styled(
            "No memory data",
            Style::default().fg(palette::TEXT_MUTED),
        ));
    }

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}

// ── Y-axis labels ─────────────────────────────────────────────────────────────

/// Render 3 y-axis labels (0, mid, max) at the left of the chart area.
fn render_y_axis_labels(max_bytes: u64, area: Rect, y_axis_width: u16, buf: &mut Buffer) {
    if area.height < LEGEND_HEIGHT + 2 {
        return;
    }
    let plot_top = area.y + LEGEND_HEIGHT;
    let plot_height = area.height.saturating_sub(LEGEND_HEIGHT + 1);
    if plot_height == 0 {
        return;
    }

    let label_style = Style::default().fg(palette::TEXT_SECONDARY);

    // Max label at top
    let max_label = format!("{:>6} ", MemoryUsage::format_bytes(max_bytes));
    let max_label_trimmed = if max_label.len() > y_axis_width as usize {
        max_label[..y_axis_width as usize].to_string()
    } else {
        max_label
    };
    let max_line = Line::from(Span::styled(max_label_trimmed, label_style));
    buf.set_line(area.x, plot_top, &max_line, y_axis_width);

    // Mid label
    if plot_height >= 4 {
        let mid_y = plot_top + plot_height / 2;
        let mid_label = format!("{:>6} ", MemoryUsage::format_bytes(max_bytes / 2));
        let mid_label_trimmed = if mid_label.len() > y_axis_width as usize {
            mid_label[..y_axis_width as usize].to_string()
        } else {
            mid_label
        };
        let mid_line = Line::from(Span::styled(mid_label_trimmed, label_style));
        buf.set_line(area.x, mid_y, &mid_line, y_axis_width);
    }

    // Zero label at bottom
    let bottom_y = plot_top + plot_height - 1;
    let zero_label = format!("{:>6} ", "0 B");
    let zero_line = Line::from(Span::styled(zero_label, label_style));
    buf.set_line(area.x, bottom_y, &zero_line, y_axis_width);
}

// ── X-axis labels ─────────────────────────────────────────────────────────────

/// Render x-axis time labels: "60s ago" at left, "now" at right.
fn render_x_axis_labels(plot_x: u16, plot_width: u16, y: u16, buf: &mut Buffer) {
    if plot_width < 10 {
        return;
    }
    let label_style = Style::default().fg(palette::TEXT_MUTED);

    let left_label = "60s ago";
    let right_label = "now";

    let left_line = Line::from(Span::styled(left_label, label_style));
    buf.set_line(plot_x, y, &left_line, left_label.len() as u16);

    let right_x = plot_x + plot_width - right_label.len() as u16;
    if right_x > plot_x + left_label.len() as u16 {
        let right_line = Line::from(Span::styled(right_label, label_style));
        buf.set_line(right_x, y, &right_line, right_label.len() as u16);
    }
}

// ── Allocation table ──────────────────────────────────────────────────────────

/// Render the class allocation table below the chart.
fn render_allocation_table(
    allocation_profile: Option<&AllocationProfile>,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    // Header
    let header_line = Line::from(vec![
        Span::styled(
            format!("{:<30}", "Class"),
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
        Span::styled(
            format!("{:>12}", "Instances"),
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
        Span::styled(
            format!("{:>14}", "Shallow Size"),
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
    ]);
    buf.set_line(area.x, area.y, &header_line, area.width);

    if area.height < 2 {
        return;
    }

    // Separator
    let sep: String = "\u{2500}".repeat(area.width as usize);
    let sep_line = Line::from(Span::styled(sep, Style::default().fg(palette::BORDER_DIM)));
    buf.set_line(area.x, area.y + 1, &sep_line, area.width);

    if area.height < 3 {
        return;
    }

    let data_start_y = area.y + TABLE_HEADER_HEIGHT;
    let available_rows = area.height.saturating_sub(TABLE_HEADER_HEIGHT) as usize;

    match allocation_profile {
        None => {
            let msg = Line::from(Span::styled(
                "Allocation data loading...",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            buf.set_line(area.x, data_start_y, &msg, area.width);
        }
        Some(profile) => {
            let classes = profile.top_by_size(MAX_TABLE_ROWS);
            let display_count = classes.len().min(available_rows);

            for (i, class) in classes.iter().take(display_count).enumerate() {
                let row_y = data_start_y + i as u16;
                if row_y >= area.bottom() {
                    break;
                }

                // Truncate class name to 30 chars
                let name = if class.class_name.len() > 30 {
                    format!("{:.27}...", &class.class_name[..27])
                } else {
                    class.class_name.clone()
                };

                let row = Line::from(vec![
                    Span::styled(
                        format!("{:<30}", name),
                        Style::default().fg(palette::TEXT_PRIMARY),
                    ),
                    Span::styled(
                        format!("{:>12}", format_number(class.total_instances())),
                        Style::default().fg(palette::TEXT_SECONDARY),
                    ),
                    Span::styled(
                        format!("{:>14}", MemoryUsage::format_bytes(class.total_size())),
                        Style::default().fg(palette::TEXT_SECONDARY),
                    ),
                ]);
                buf.set_line(area.x, row_y, &row, area.width);
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
