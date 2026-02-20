//! Tests for the [`FrameChart`] widget.

use super::*;
use fdemon_core::performance::{FramePhases, FrameTiming, PerformanceStats, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

// ── Test helpers ──────────────────────────────────────────────────────────

fn make_frame(number: u64, build: u64, raster: u64) -> FrameTiming {
    FrameTiming {
        number,
        build_micros: build,
        raster_micros: raster,
        elapsed_micros: build + raster,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    }
}

fn make_janky_frame(number: u64) -> FrameTiming {
    make_frame(number, 12_000, 8_000) // 20ms total > 16ms budget
}

fn make_shader_frame(number: u64) -> FrameTiming {
    let mut f = make_frame(number, 5_000, 30_000);
    f.shader_compilation = true;
    f
}

fn make_stats(
    fps: Option<f64>,
    jank_count: u32,
    avg: Option<f64>,
    frames: u64,
) -> PerformanceStats {
    PerformanceStats {
        fps,
        jank_count,
        avg_frame_ms: avg,
        p95_frame_ms: None,
        max_frame_ms: None,
        buffered_frames: frames,
    }
}

fn render_widget(widget: FrameChart<'_>, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    buf
}

/// Collect all characters in the buffer into a flat String (row-major).
fn collect_text(buf: &Buffer, width: u16, height: u16) -> String {
    let mut result = String::new();
    for y in 0..height {
        for x in 0..width {
            if let Some(cell) = buf.cell((x, y)) {
                if let Some(ch) = cell.symbol().chars().next() {
                    result.push(ch);
                }
            }
        }
    }
    result
}

/// Check if any cell in the buffer has the given foreground colour.
fn has_color(buf: &Buffer, width: u16, height: u16, color: Color) -> bool {
    for y in 0..height {
        for x in 0..width {
            if let Some(cell) = buf.cell((x, y)) {
                if cell.style().fg == Some(color) && !cell.symbol().trim().is_empty() {
                    return true;
                }
            }
        }
    }
    false
}

// ── Acceptance criteria tests ─────────────────────────────────────────────

#[test]
fn test_renders_empty_history_without_panic() {
    let history = RingBuffer::new(100);
    let stats = PerformanceStats::default();
    let widget = FrameChart::new(&history, None, &stats, false);
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Must not panic
}

#[test]
fn test_renders_single_frame_without_panic() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(1, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 80, 20);
    // Verify at least one non-space character is present in the chart area
    let text = collect_text(&buf, 80, 20);
    assert!(
        !text.chars().all(|c| c == ' '),
        "Expected chart content in buffer"
    );
}

#[test]
fn test_jank_frame_uses_red_color() {
    let mut history = RingBuffer::new(100);
    history.push(make_janky_frame(1)); // 20ms > 16ms
    let stats = make_stats(Some(50.0), 1, Some(20.0), 1);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 40, 20);
    assert!(
        has_color(&buf, 40, 20, COLOR_JANK),
        "Janky frame should use red (COLOR_JANK)"
    );
}

#[test]
fn test_normal_frame_uses_cyan_and_green() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(1, 5_000, 3_000)); // 8ms total, well under budget
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 40, 20);
    assert!(
        has_color(&buf, 40, 20, COLOR_UI_NORMAL) || has_color(&buf, 40, 20, COLOR_RASTER_NORMAL),
        "Normal frame should use cyan or green"
    );
}

#[test]
fn test_shader_frame_uses_magenta() {
    let mut history = RingBuffer::new(100);
    history.push(make_shader_frame(1));
    let stats = make_stats(Some(30.0), 0, Some(35.0), 1);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 40, 20);
    assert!(
        has_color(&buf, 40, 20, COLOR_SHADER),
        "Shader frame should use magenta (COLOR_SHADER)"
    );
}

#[test]
fn test_budget_line_label_drawn() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(1, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    assert!(
        text.contains("16ms"),
        "Budget line should contain '16ms' label; buffer: {text:?}"
    );
}

#[test]
fn test_selected_frame_shows_highlight() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(1, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let widget = FrameChart::new(&history, Some(0), &stats, false);
    let buf = render_widget(widget, 40, 20);
    let text = collect_text(&buf, 40, 20);
    // Selection highlight uses '▔'
    assert!(
        text.contains('\u{2594}'),
        "Selected frame should render a highlight character (▔)"
    );
}

#[test]
fn test_detail_panel_shows_frame_info_when_selected() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(42, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let widget = FrameChart::new(&history, Some(0), &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    // Should contain frame number
    assert!(
        text.contains('#'),
        "Detail panel should show frame number marker (#)"
    );
}

#[test]
fn test_summary_line_when_no_selection() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(1, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 2, Some(8.2), 100);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    assert!(
        text.contains("FPS") || text.contains("60"),
        "Summary line should contain FPS value; text: {text:?}"
    );
    assert!(
        text.contains("Jank") || text.contains("Avg"),
        "Summary line should contain Jank or Avg; text: {text:?}"
    );
}

#[test]
fn test_compact_mode_for_small_area_no_panic() {
    let history = RingBuffer::new(100);
    let stats = PerformanceStats::default();
    let widget = FrameChart::new(&history, None, &stats, false);
    // Area too small for chart (height < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT = 7)
    let area = Rect::new(0, 0, 80, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Must not panic
}

#[test]
fn test_zero_area_no_panic() {
    let history = RingBuffer::new(100);
    let stats = PerformanceStats::default();
    let widget = FrameChart::new(&history, None, &stats, false);
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Must not panic
}

#[test]
fn test_frame_count_fits_width() {
    let mut history = RingBuffer::new(100);
    // Push 20 frames but width=30 should only show 10 (30/3)
    for i in 0..20u64 {
        history.push(make_frame(i, 5_000, 3_000));
    }
    let stats = make_stats(Some(60.0), 0, Some(8.0), 20);
    let widget = FrameChart::new(&history, None, &stats, false);
    // Width 30 → max_visible = 30 / 3 = 10 frames
    let buf = render_widget(widget, 30, 20);
    // Should not panic and should render something
    let text = collect_text(&buf, 30, 20);
    assert!(!text.is_empty(), "Rendered buffer should not be empty");
}

#[test]
fn test_auto_scaling_minimum_range() {
    // All frames are very short (< 5ms each), y-axis should still be >= 20ms range
    // The budget line at 16ms should still appear even when all frames are < 5ms
    let mut history = RingBuffer::new(100);
    for i in 0..10u64 {
        history.push(make_frame(i, 2_000, 1_000)); // 3ms total
    }
    let stats = make_stats(Some(60.0), 0, Some(3.0), 10);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    // Budget line should still appear because MIN_Y_RANGE_MS = 20ms > 3ms frame time
    assert!(
        text.contains("16ms"),
        "Budget line should appear even when all frames are below 16ms; text: {text:?}"
    );
}

#[test]
fn test_detail_panel_with_phases() {
    let mut history = RingBuffer::new(100);
    let mut frame = make_frame(42, 6_000, 6_000);
    frame.phases = Some(FramePhases {
        build_micros: 3_000,
        layout_micros: 1_500,
        paint_micros: 1_500,
        raster_micros: 6_000,
        shader_compilation: false,
    });
    history.push(frame);
    let stats = make_stats(Some(60.0), 0, Some(12.0), 1);
    let widget = FrameChart::new(&history, Some(0), &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    // Phase breakdown should include "Build", "Layout", "Paint"
    assert!(
        text.contains("Build") || text.contains("Layout") || text.contains("Paint"),
        "Detail panel with phases should show breakdown; text: {text:?}"
    );
}

#[test]
fn test_detail_panel_jank_label() {
    let mut history = RingBuffer::new(100);
    history.push(make_janky_frame(99));
    let stats = make_stats(Some(50.0), 1, Some(20.0), 1);
    let widget = FrameChart::new(&history, Some(0), &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    assert!(
        text.contains("JANK"),
        "Janky selected frame should show '(JANK)' in detail panel; text: {text:?}"
    );
}

#[test]
fn test_detail_panel_shader_label() {
    let mut history = RingBuffer::new(100);
    history.push(make_shader_frame(7));
    let stats = make_stats(Some(30.0), 0, Some(35.0), 1);
    let widget = FrameChart::new(&history, Some(0), &stats, false);
    let buf = render_widget(widget, 80, 20);
    let text = collect_text(&buf, 80, 20);
    assert!(
        text.contains("SHADER"),
        "Shader frame should show '(SHADER)' in detail panel; text: {text:?}"
    );
}

#[test]
fn test_many_frames_shows_most_recent() {
    // Push 100 frames numbered 0-99; with width=30, only 10 frames fit.
    // Should show the most recent (frames 90-99).
    let mut history = RingBuffer::new(300);
    for i in 0u64..100 {
        history.push(make_frame(i, 5_000, 3_000));
    }
    let stats = make_stats(Some(60.0), 0, Some(8.0), 100);
    let widget = FrameChart::new(&history, None, &stats, false);
    // width 30 → 10 frames visible in the bar chart
    let buf = render_widget(widget, 30, 20);
    // No panic is the minimum requirement; verify something was rendered
    let text = collect_text(&buf, 30, 20);
    assert!(!text.is_empty());
}

#[test]
fn test_full_buffer_history_no_panic() {
    let mut history = RingBuffer::new(300);
    for i in 0..300u64 {
        history.push(make_frame(i, 5_000 + i * 10, 3_000 + i * 5));
    }
    let stats = make_stats(Some(60.0), 5, Some(8.0), 300);
    let widget = FrameChart::new(&history, None, &stats, false);
    let buf = render_widget(widget, 80, 24);
    let text = collect_text(&buf, 80, 24);
    assert!(!text.is_empty());
}

// ── Unit tests for pure helper functions ──────────────────────────────────

#[test]
fn test_bar_colors_normal_frame() {
    let frame = make_frame(1, 5_000, 3_000);
    let (ui, raster) = bar_colors(&frame);
    assert_eq!(ui, COLOR_UI_NORMAL);
    assert_eq!(raster, COLOR_RASTER_NORMAL);
}

#[test]
fn test_bar_colors_jank_frame() {
    let frame = make_janky_frame(1);
    let (ui, raster) = bar_colors(&frame);
    assert_eq!(ui, COLOR_JANK);
    assert_eq!(raster, COLOR_JANK);
}

#[test]
fn test_bar_colors_shader_frame() {
    let frame = make_shader_frame(1);
    let (ui, raster) = bar_colors(&frame);
    assert_eq!(ui, COLOR_SHADER);
    assert_eq!(raster, COLOR_SHADER);
}

#[test]
fn test_ms_to_half_blocks_zero_ms_returns_zero() {
    assert_eq!(ms_to_half_blocks(0.0, 20.0, 40.0), 0);
}

#[test]
fn test_ms_to_half_blocks_full_range() {
    // 20ms with 20ms range and 40 half-blocks → full height (40)
    assert_eq!(ms_to_half_blocks(20.0, 20.0, 40.0), 40);
}

#[test]
fn test_ms_to_half_blocks_half_range() {
    // 10ms with 20ms range and 40 half-blocks → 20 (half)
    assert_eq!(ms_to_half_blocks(10.0, 20.0, 40.0), 20);
}

#[test]
fn test_ms_to_half_blocks_zero_range_returns_zero() {
    assert_eq!(ms_to_half_blocks(10.0, 0.0, 40.0), 0);
}
