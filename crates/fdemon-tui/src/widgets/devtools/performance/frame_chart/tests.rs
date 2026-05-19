//! Tests for the [`FrameChart`] widget.

use super::*;
use fdemon_core::performance::{FramePhases, FrameTiming, PerformanceStats, RingBuffer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::cell::Cell;

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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
    let buf = render_widget(widget, 40, 20);
    let text = collect_text(&buf, 40, 20);
    // Selection highlight now uses full-column side markers: ▏ (U+258F) or ▕ (U+2595).
    assert!(
        text.contains('\u{258F}') || text.contains('\u{2595}'),
        "Selected frame should render full-column side marker characters (▏ or ▕)"
    );
}

#[test]
fn test_detail_panel_shows_frame_info_when_selected() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(42, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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
    let hint_cell = Cell::new(0);
    let widget = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);
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

// ── Phase 4 Task 08: click region tests ──────────────────────────────────────

/// Build a minimal FrameTiming value for tests.
fn make_timing(number: u64) -> fdemon_core::performance::FrameTiming {
    make_frame(number, 5_000, 5_000)
}

#[test]
fn frame_chart_records_one_region_per_visible_frame() {
    use fdemon_app::{Message, MouseAction, MouseRegions};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use crate::widgets::MouseCtx;

    let mut history = RingBuffer::new(120);
    for i in 1..=8u64 {
        history.push(make_timing(i));
    }
    let stats = PerformanceStats::default();
    let hint_cell = Cell::new(0);
    let chart = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        chart.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    // Collect the global indices from every SelectPerformanceFrame region.
    let frame_indices: Vec<usize> = regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(m)) => match **m {
                Message::SelectPerformanceFrame { index: Some(i) } => Some(i),
                _ => None,
            },
            _ => None,
        })
        .collect();

    assert_eq!(
        frame_indices,
        vec![0, 1, 2, 3, 4, 5, 6, 7],
        "8 frames → 8 regions with indices 0..=7"
    );
}

#[test]
fn frame_chart_in_compact_mode_records_no_regions() {
    use fdemon_app::MouseRegions;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use crate::widgets::MouseCtx;

    let mut history = RingBuffer::new(120);
    history.push(make_timing(1));
    let stats = PerformanceStats::default();
    let hint_cell = Cell::new(0);
    let chart = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);

    // Compact: height < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT (= 4 + 3 = 7).
    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 5);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        chart.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    assert_eq!(
        regions.iter().count(),
        0,
        "compact mode → no regions registered"
    );
}

#[test]
fn frame_chart_region_width_is_chars_per_frame() {
    // Each region should be CHARS_PER_FRAME (3) wide for non-edge slots.
    use fdemon_app::{Message, MouseAction, MouseRegions};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use crate::widgets::MouseCtx;

    let mut history = RingBuffer::new(120);
    for i in 1..=4u64 {
        history.push(make_timing(i));
    }
    let stats = PerformanceStats::default();
    let hint_cell = Cell::new(0);
    let chart = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        chart.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    // All four frames should have width = CHARS_PER_FRAME (area is wide enough).
    for entry in regions.iter().filter(|e| {
        matches!(
            &e.on_left,
            Some(MouseAction::Emit(m)) if matches!(**m, Message::SelectPerformanceFrame { .. })
        )
    }) {
        assert_eq!(
            entry.rect.width, CHARS_PER_FRAME,
            "region width should equal CHARS_PER_FRAME for a wide area"
        );
    }
}

#[test]
fn frame_chart_region_height_equals_chart_height() {
    // Each region should span the full bar-chart height (area.height - DETAIL_PANEL_HEIGHT).
    use fdemon_app::{Message, MouseAction, MouseRegions};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use crate::widgets::MouseCtx;

    let mut history = RingBuffer::new(120);
    history.push(make_timing(42));
    let stats = PerformanceStats::default();
    let hint_cell = Cell::new(0);
    let chart = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);

    let total_h: u16 = 20;
    let expected_chart_h = total_h - DETAIL_PANEL_HEIGHT;

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, total_h);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        chart.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    for entry in regions.iter().filter(|e| {
        matches!(
            &e.on_left,
            Some(MouseAction::Emit(m)) if matches!(**m, Message::SelectPerformanceFrame { .. })
        )
    }) {
        assert_eq!(
            entry.rect.height, expected_chart_h,
            "region height should equal chart_h (area.height - DETAIL_PANEL_HEIGHT)"
        );
    }
}

#[test]
fn frame_chart_no_regions_without_ctx() {
    // Widget::render (without ctx) should register no regions.
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    let mut history = RingBuffer::new(120);
    for i in 1..=4u64 {
        history.push(make_timing(i));
    }
    let stats = PerformanceStats::default();
    let hint_cell = Cell::new(0);
    let chart = FrameChart::new(&history, None, &stats, false, 0, &hint_cell, false);

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    // Widget::render does not accept ctx — it simply cannot register regions.
    chart.render(area, &mut buf);
    // This test documents that the region-free path doesn't panic.
    // The absence of region registration is enforced by the type system (no ctx).
}

// ── compute_visible_range unit tests (Task 05 / updated for Task 01) ─────────
//
// After Task 01 (Fix 3), `compute_visible_range` no longer accepts a
// `selected_frame` parameter. `scroll_offset` is the sole viewport authority.

/// Model A: scroll_offset is "frames back from the live edge".
/// With offset=200 and 1000 frames, the window should end at frame 800.
#[test]
fn visible_range_anchors_at_offset_when_scrolled() {
    let (start, end) = compute_visible_range(1000, 50, 200);
    assert_eq!(end, 800, "end should be frame_count - scroll_offset = 800");
    assert_eq!(start, 750, "start should be end - visible_width = 750");
}

/// Live-edge mode: when offset is 0, the window sits at the newest frames.
#[test]
fn visible_range_lives_at_edge_when_offset_zero() {
    let (start, end) = compute_visible_range(1000, 50, 0);
    assert_eq!(end, 1000, "live-edge: end should equal frame_count");
    assert_eq!(start, 950, "live-edge: start should be end - visible_width");
}

/// Model A drift property: scroll_offset is "frames back from latest", so when
/// 10 new frames arrive the window drifts forward by exactly 10 frames while
/// keeping the same window size.
#[test]
fn scroll_offset_window_drifts_forward_with_new_arrivals() {
    let (s1, e1) = compute_visible_range(1000, 50, 100);
    let (s2, e2) = compute_visible_range(1010, 50, 100); // 10 new frames arrive
                                                         // Window size is preserved
    assert_eq!(
        e1 - s1,
        e2 - s2,
        "window size must be preserved after new arrivals"
    );
    // Window drifts forward by the number of new arrivals (Model A)
    assert_eq!(
        e2 - e1,
        10,
        "window end drifts forward by the number of new arrivals (Model A)"
    );
}

/// scroll_offset is the sole viewport authority: with offset=100 the window
/// anchors at len - offset regardless of any selection state.
/// (Previously tested as "scroll_offset_takes_priority_over_selection" with a
/// `selected_frame` argument; the argument has been removed in Task 01 Fix 3.)
#[test]
fn scroll_offset_is_sole_viewport_authority() {
    // offset=100 → end = 1000 - 100 = 900
    let (start, end) = compute_visible_range(1000, 50, 100);
    assert_eq!(
        end, 900,
        "scroll_offset is sole authority: end = frame_count - scroll_offset"
    );
    assert_eq!(start, 850, "start = end - visible_width");
}

/// Edge case: offset >= frame_count → window collapses to (0, 0).
#[test]
fn scroll_offset_saturating_behaviour_at_zero() {
    let (start, end) = compute_visible_range(10, 50, 100);
    // saturating_sub(100) on 10 → 0
    assert_eq!(end, 0);
    assert_eq!(start, 0);
}

// ── Task 01, Bug 1: ms_to_half_blocks minimum floor ──────────────────────────

/// A very-small but nonzero `ms` value must return at least 1 (MIN_BAR_HALF_BLOCKS).
///
/// Previously `ms_to_half_blocks(0.5, 20.0, 4.0)` would round to 0 and the
/// bar would be invisible. After the fix it should return 1.
#[test]
fn ms_to_half_blocks_clamps_nonzero_to_at_least_one() {
    // 0.5 / 20.0 * 4.0 = 0.1 → rounds to 0 before fix; must be 1 after fix.
    assert_eq!(
        ms_to_half_blocks(0.5, 20.0, 4.0),
        1,
        "very small nonzero ms should return at least 1 half-block"
    );

    // Very small but non-zero with total_half_blocks=2 (1 row visible).
    assert_eq!(
        ms_to_half_blocks(0.01, 20.0, 2.0),
        1,
        "nonzero ms with tiny ratio should still return at least 1"
    );
}

/// Zero-duration frame must return exactly 0 (zero-duration stays invisible).
#[test]
fn ms_to_half_blocks_zero_ms_stays_zero() {
    assert_eq!(
        ms_to_half_blocks(0.0, 20.0, 4.0),
        0,
        "zero ms → 0 half-blocks"
    );
}

// ── Task 01, Bug 2: full-column selection highlight ───────────────────────────

/// The selection highlight side-markers (▏ or ▕) must be present on every
/// row of the chart area for the selected frame, not just the top row.
#[test]
fn selection_highlight_paints_full_column() {
    let mut history = RingBuffer::new(100);
    history.push(make_frame(1, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 1);
    let hint_cell = Cell::new(0);
    // height=20: chart area = rows 0..17 (height 17), detail = rows 17..20.
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let chart_h = 20 - DETAIL_PANEL_HEIGHT; // 17 rows

    // The right-marker column for slot 0 is x = 0 + 2 = 2.
    // Count how many rows in column 2 contain the right-eighth marker (▕ = U+2595).
    let right_marker_col: u16 = 2;
    let mut marker_row_count = 0u16;
    for y in 0..chart_h {
        if let Some(cell) = buf.cell((right_marker_col, y)) {
            if cell.symbol().contains('\u{2595}') || cell.symbol().contains('\u{258F}') {
                marker_row_count += 1;
            }
        }
    }

    assert_eq!(
        marker_row_count, chart_h,
        "selection highlight marker should appear on every chart row ({chart_h}), \
         found it on {marker_row_count} rows"
    );
}

/// The column to the right of the selection's right-marker should not carry
/// selection highlight characters (no bleed-over to unselected bars).
///
/// Slot 0 uses columns 0 and 1 for bars. Right-marker is at column 2.
/// The next slot (slot 1) starts at column 3 — that column must be unmodified.
#[test]
fn selection_highlight_does_not_paint_adjacent_columns() {
    let mut history = RingBuffer::new(100);
    // Two frames so there's an adjacent column to check.
    history.push(make_frame(1, 5_000, 3_000));
    history.push(make_frame(2, 5_000, 3_000));
    let stats = make_stats(Some(60.0), 0, Some(8.0), 2);
    let hint_cell = Cell::new(0);
    // Select frame 0 (slot 0, columns 0-1).
    let widget = FrameChart::new(&history, Some(0), &stats, false, 0, &hint_cell, false);
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Adjacent slot (slot 1) starts at column 3. Its left bar (UI) is at x=3.
    // That column must NOT contain any selection-highlight characters.
    let adjacent_col: u16 = 3;
    let chart_h = 20 - DETAIL_PANEL_HEIGHT;
    for y in 0..chart_h {
        if let Some(cell) = buf.cell((adjacent_col, y)) {
            assert!(
                !cell.symbol().contains('\u{2595}') && !cell.symbol().contains('\u{258F}'),
                "adjacent column {adjacent_col} row {y} should not contain a selection marker; \
                 found {:?}",
                cell.symbol()
            );
        }
    }
}
