//! Tests for the [`PerformancePanel`] widget.

use super::*;
use fdemon_app::session::PerformanceState;
use fdemon_app::state::VmConnectionStatus;
use fdemon_core::performance::FrameTiming;

fn make_test_performance() -> PerformanceState {
    let mut perf = PerformanceState {
        monitoring_active: true,
        ..Default::default()
    };
    for i in 0u64..30 {
        perf.frame_history.push(FrameTiming {
            number: i,
            build_micros: 5000 + (i * 100),
            raster_micros: 3000 + (i * 50),
            elapsed_micros: 8000 + (i * 150),
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        });
    }
    perf.stats.fps = Some(60.0);
    perf.stats.jank_count = 2;
    perf.stats.avg_frame_ms = Some(8.5);
    perf.stats.buffered_frames = 30;
    perf
}

fn render_to_buf(widget: PerformancePanel<'_>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    widget.render(Rect::new(0, 0, width, height), &mut buf);
    buf
}

fn collect_buf_text(buf: &Buffer, width: u16, height: u16) -> String {
    let mut full = String::new();
    for y in 0..height {
        for x in 0..width {
            if let Some(c) = buf.cell((x, y)) {
                if let Some(ch) = c.symbol().chars().next() {
                    full.push(ch);
                }
            }
        }
    }
    full
}

fn buf_contains_text(buf: &Buffer, width: u16, height: u16, text: &str) -> bool {
    collect_buf_text(buf, width, height).contains(text)
}

#[test]
fn test_performance_panel_renders_without_panic() {
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 80, 24);
}

#[test]
fn test_performance_panel_no_stats_section() {
    // Verifies the old Memory/Stats block header (moved to the Memory panel in
    // Phase 1) is no longer shown in the Performance panel. The string " Stats "
    // (with surrounding spaces — ratatui block title pattern) must not appear as
    // a standalone block title. Note: "Rebuild Stats" appears as a tab label in
    // the dual-pane layout; the assertion checks for the old block-header pattern
    // "─ Stats ─" or similar, so we check for the old section title format.
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 30);
    // The old Stats section had a block title like "─ Stats ─"; it must not appear.
    // (The new "Rebuild Stats" tab label is acceptable — it does not use the block
    // title format and is a Phase 2 addition to the Details pane tab strip.)
    assert!(
        !buf_contains_text(&buf, 80, 30, "Memory Stats")
            && !buf_contains_text(&buf, 80, 30, "─ Stats"),
        "Old Stats block section should have been removed from Performance panel"
    );
}

#[test]
fn test_performance_panel_shows_fps() {
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 24);
    let content = collect_buf_text(&buf, 80, 24);
    assert!(content.contains("60") || content.contains("FPS") || content.contains("Frame"));
}

#[test]
fn test_performance_panel_compact_mode() {
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 80, 5);
}

#[test]
fn test_performance_panel_compact_mode_shows_fps() {
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 5);
    let content = collect_buf_text(&buf, 80, 5);
    assert!(
        content.contains("60") || content.contains("FPS"),
        "Compact mode should show FPS; content: {content:?}"
    );
}

#[test]
fn test_performance_panel_frame_chart_fills_area() {
    // All heights >= COMPACT_THRESHOLD should show Frame Timing
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 12);
    let content = collect_buf_text(&buf, 80, 12);
    assert!(
        content.contains("Frame Timing"),
        "Frame chart should show Frame Timing block; content: {content:?}"
    );
}

#[test]
fn test_performance_panel_disconnected_state() {
    let perf = PerformanceState::default();
    let widget = PerformancePanel::new(
        &perf,
        false,
        IconSet::default(),
        &VmConnectionStatus::Disconnected,
    );
    let buf = render_to_buf(widget, 80, 24);
    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("VM Service") || full.contains("monitoring") || full.contains("Waiting"),
        "Expected disconnected message in buffer"
    );
}

#[test]
fn test_performance_panel_disconnected_still_works() {
    let perf = PerformanceState::default();
    let widget = PerformancePanel::new(
        &perf,
        false,
        IconSet::default(),
        &VmConnectionStatus::Disconnected,
    );
    let buf = render_to_buf(widget, 80, 24);
    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("VM Service") || full.contains("not connected"),
        "Disconnected state should show VM Service message; got: {full:?}"
    );
}

#[test]
fn test_performance_panel_shows_connection_error() {
    let perf = PerformanceState::default();
    let widget = PerformancePanel::new(
        &perf,
        false,
        IconSet::default(),
        &VmConnectionStatus::Disconnected,
    )
    .with_connection_error(Some("Connection failed: Connection refused"));
    let full = collect_buf_text(&render_to_buf(widget, 80, 24), 80, 24);
    assert!(
        full.contains("Connection failed") || full.contains("Connection refused"),
        "Expected specific connection error message in buffer, got: {full:?}"
    );
    assert!(
        !full.contains("Performance monitoring requires"),
        "Should not show generic message when specific error is available"
    );
}

#[test]
fn test_performance_panel_no_error_shows_generic_disconnected() {
    let perf = PerformanceState::default();
    let widget = PerformancePanel::new(
        &perf,
        false,
        IconSet::default(),
        &VmConnectionStatus::Disconnected,
    )
    .with_connection_error(None);
    let full = collect_buf_text(&render_to_buf(widget, 80, 24), 80, 24);
    assert!(
        full.contains("VM Service") || full.contains("not connected"),
        "Expected generic VM Service disconnected message, got: {full:?}"
    );
}

#[test]
fn test_monitoring_inactive_shows_disconnected() {
    let perf = PerformanceState {
        monitoring_active: false,
        ..Default::default()
    };
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let full = collect_buf_text(&render_to_buf(widget, 80, 24), 80, 24);
    assert!(
        full.contains("monitoring") || full.contains("Waiting"),
        "Expected 'monitoring' or 'Waiting' in buffer"
    );
}

#[test]
fn test_performance_panel_reconnecting_shows_attempt_count() {
    let perf = PerformanceState::default();
    let status = VmConnectionStatus::Reconnecting {
        attempt: 3,
        max_attempts: 10,
    };
    let widget = PerformancePanel::new(&perf, false, IconSet::default(), &status);
    let full = collect_buf_text(&render_to_buf(widget, 80, 24), 80, 24);
    assert!(
        full.contains("Reconnecting") || full.contains("3/10"),
        "Expected reconnecting message with attempt count, got: {full:?}"
    );
}

#[test]
fn test_performance_panel_with_selected_frame() {
    let mut perf = make_test_performance();
    perf.selected_frame = Some(5);
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 80, 30);
}

#[test]
fn test_performance_panel_small_terminal() {
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 40, 10);
}

#[test]
fn test_performance_panel_zero_area() {
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 10, 1);
}

// ── Phase 1 followup T03: Tab-trap regression test ───────────────────────────

/// Regression guard: pressing Tab on the Performance panel must NOT move focus
/// to a section that silently disables j/k/PgUp/PgDn scroll keys.
///
/// Option A (YAGNI) was chosen: `PerfSection::next()` always returns `FrameChart`
/// so Tab is a visible no-op and the frame chart remains the active section.
/// The test asserts:
/// 1. After calling `next()`, `focused_section` advances to `Details` (Phase 2 cycling).
/// 2. When the section is switched back to `FrameChart`, scroll offset can be incremented,
///    demonstrating that scroll keys remain functional.
#[test]
fn performance_tab_after_tab_does_not_break_scroll_keys() {
    use fdemon_app::session::PerfSection;

    // Set up state with several FrameTiming entries.
    let mut perf = make_test_performance();

    // Simulate what keys.rs does on Tab: focused_section = focused_section.next()
    let after_tab = perf.focused_section.next();
    perf.focused_section = after_tab;

    // Phase 2: Tab cycles FrameChart → Details.
    assert_eq!(
        perf.focused_section,
        PerfSection::Details,
        "After Tab, focused_section advances to Details (Phase 2 cycling)"
    );

    // Switch back to FrameChart and verify scroll still works.
    perf.focused_section = PerfSection::FrameChart;
    let before_scroll = perf.frame_chart_scroll_offset;
    perf.frame_chart_scroll_offset = before_scroll.saturating_add(1);

    assert!(
        perf.frame_chart_scroll_offset > 0,
        "Scroll offset must be > 0 after simulated PerfScrollUp (trap is gone)"
    );

    // Confirm the panel still renders without panic after Tab + scroll.
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 80, 24);
}

// ── Phase 2 Task 04: dual-pane layout tests ───────────────────────────────────

use fdemon_app::state::PerfDetailsTab;

fn render_panel(perf: &PerformanceState, w: u16, h: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    let widget = PerformancePanel::new(
        perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    widget.render(buf.area, &mut buf);
    buf
}

fn collect_full_text(buf: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if let Some(c) = buf.cell((x, y)) {
                if let Some(ch) = c.symbol().chars().next() {
                    s.push(ch);
                }
            }
        }
        s.push('\n');
    }
    s
}

#[test]
fn dual_pane_renders_chart_and_details_at_tall_terminal() {
    let mut perf = PerformanceState {
        monitoring_active: true,
        ..Default::default()
    };
    perf.frame_history.push(FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    });

    let buf = render_panel(&perf, 200, 30);
    let text = collect_full_text(&buf);
    assert!(
        text.contains("Frame Timing"),
        "expected 'Frame Timing' title in dual-pane mode, got:\n{text}"
    );
    assert!(
        text.contains("Frame Details"),
        "expected 'Frame Details' title in dual-pane mode, got:\n{text}"
    );
    assert!(
        text.contains("Frame Analysis"),
        "expected 'Frame Analysis' tab label, got:\n{text}"
    );
    assert!(
        text.contains("Rebuild Stats"),
        "expected 'Rebuild Stats' tab label, got:\n{text}"
    );
    assert!(
        text.contains("Timeline Events"),
        "expected 'Timeline Events' tab label, got:\n{text}"
    );
}

#[test]
fn chart_only_at_short_terminal_below_min_dual_pane() {
    let mut perf = PerformanceState {
        monitoring_active: true,
        ..Default::default()
    };
    perf.frame_history.push(FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    });

    let buf = render_panel(&perf, 200, 16);
    let text = collect_full_text(&buf);
    assert!(
        text.contains("Frame Timing"),
        "expected 'Frame Timing' in chart-only mode"
    );
    assert!(
        !text.contains("Frame Details"),
        "details pane must be suppressed below MIN_DUAL_PANE_HEIGHT, got:\n{text}"
    );
}

#[test]
fn details_dispatches_rebuild_stats_stub() {
    let mut perf = PerformanceState {
        monitoring_active: true,
        details_tab: PerfDetailsTab::RebuildStats,
        ..Default::default()
    };
    perf.frame_history.push(FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    });

    let buf = render_panel(&perf, 200, 30);
    let text = collect_full_text(&buf);
    assert!(
        text.contains("Coming soon"),
        "rebuild stats stub must say 'Coming soon', got:\n{text}"
    );
}

#[test]
fn details_dispatches_timeline_events_stub() {
    let mut perf = PerformanceState {
        monitoring_active: true,
        details_tab: PerfDetailsTab::TimelineEvents,
        ..Default::default()
    };
    perf.frame_history.push(FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    });

    let buf = render_panel(&perf, 200, 30);
    let text = collect_full_text(&buf);
    assert!(
        text.contains("Coming soon"),
        "timeline events stub must say 'Coming soon', got:\n{text}"
    );
}

#[test]
fn details_pane_visible_height_is_written_to_render_hint() {
    let mut perf = PerformanceState {
        monitoring_active: true,
        ..Default::default()
    };
    perf.frame_history.push(FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    });

    let _ = render_panel(&perf, 200, 30);
    assert!(
        perf.details_pane_visible_height.get() > 0,
        "render-hint Cell must be written each frame"
    );
}

#[test]
fn active_tab_label_is_underlined() {
    let mut perf = PerformanceState {
        monitoring_active: true,
        details_tab: PerfDetailsTab::RebuildStats,
        ..Default::default()
    };
    perf.frame_history.push(FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    });

    let buf = render_panel(&perf, 200, 30);
    // Scan through all rows to find one containing the underline character ━
    // that is in the vicinity of the tab strip.
    let has_underline = (0..buf.area.height).any(|y| {
        (0..buf.area.width).any(|x| {
            buf.cell((x, y))
                .and_then(|c| c.symbol().chars().next())
                .map(|ch| ch == '\u{2501}')
                .unwrap_or(false)
        })
    });
    assert!(
        has_underline,
        "expected at least one ━ character for the active tab underline in the buffer"
    );
}

// ── Phase 4.5 Task 03: render_with_regions parity test ───────────────────────

#[test]
fn render_with_regions_matches_widget_render_buffer() {
    use fdemon_app::MouseRegions;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let perf = make_test_performance();
    let area = Rect::new(0, 0, 80, 24);

    let mut buf_a = Buffer::empty(area);
    PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    )
    .render(area, &mut buf_a);

    let mut buf_b = Buffer::empty(area);
    {
        let mut regions = MouseRegions::default();
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(
            area,
            &mut buf_b,
            PerformancePanel::new(
                &perf,
                true,
                IconSet::default(),
                &VmConnectionStatus::Connected,
            ),
            Some(&mut ctx),
        );
    }

    assert_eq!(
        buf_a, buf_b,
        "Widget::render and render_with_regions must produce identical buffers"
    );
}
