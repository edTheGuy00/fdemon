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
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 30);
    assert!(
        !buf_contains_text(&buf, 80, 30, " Stats "),
        "Stats section should be removed"
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
