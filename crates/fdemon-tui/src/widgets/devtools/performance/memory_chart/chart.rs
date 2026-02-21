//! Chart rendering helpers for the memory chart.
//!
//! Contains the braille-based sample chart, the history fallback chart,
//! the legend, and the y/x-axis label renderers.

use super::*;

// ── Sample-based chart (rich MemorySample data) ───────────────────────────────

/// Render the stacked braille chart from `MemorySample` data.
pub(super) fn render_sample_chart(
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
pub(super) fn render_history_chart(
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
pub(super) fn render_legend(
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
pub(super) fn render_y_axis_labels(
    max_bytes: u64,
    area: Rect,
    y_axis_width: u16,
    buf: &mut Buffer,
) {
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
pub(super) fn render_x_axis_labels(plot_x: u16, plot_width: u16, y: u16, buf: &mut Buffer) {
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
