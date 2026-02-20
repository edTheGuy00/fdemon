//! Tests for the [`PerformancePanel`] widget.

use super::*;
use fdemon_app::session::PerformanceState;
use fdemon_app::state::VmConnectionStatus;
use fdemon_core::performance::{FrameTiming, MemoryUsage};

fn make_test_performance() -> PerformanceState {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    perf.memory_history.push(MemoryUsage {
        heap_usage: 50_000_000,
        heap_capacity: 128_000_000,
        external_usage: 12_000_000,
        timestamp: chrono::Local::now(),
    });
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
    // Should not panic
}

#[test]
fn test_performance_panel_renders_two_sections() {
    // Normal size terminal should show both Frame Timing and Memory sections
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 30);
    assert!(
        buf_contains_text(&buf, 80, 30, "Frame Timing"),
        "Expected 'Frame Timing' section in buffer"
    );
    assert!(
        buf_contains_text(&buf, 80, 30, "Memory"),
        "Expected 'Memory' section in buffer"
    );
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
    // Stats section has been removed — no standalone "Stats" block
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
    // The frame chart summary or chart content should contain FPS-related content
    let content = collect_buf_text(&buf, 80, 24);
    assert!(content.contains("60") || content.contains("FPS") || content.contains("Frame"));
}

#[test]
fn test_performance_panel_compact_mode() {
    // Height < COMPACT_THRESHOLD (7) → compact single-line summary, no panic
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 80, 5);
    // Should not crash — compact summary shown
}

#[test]
fn test_performance_panel_compact_mode_shows_fps() {
    // Height < COMPACT_THRESHOLD should show FPS in summary line
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
fn test_performance_panel_frame_only_mode() {
    // Height between COMPACT_THRESHOLD (7) and DUAL_SECTION_MIN_HEIGHT (14)
    // should show frame chart only, no memory section
    let perf = make_test_performance();
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    let buf = render_to_buf(widget, 80, 10);
    let content = collect_buf_text(&buf, 80, 10);
    // Should show Frame Timing but not a separate Memory block
    assert!(
        content.contains("Frame Timing"),
        "Frame-only mode should still show Frame Timing block; content: {content:?}"
    );
}

#[test]
fn test_performance_panel_disconnected_state() {
    let perf = PerformanceState::default(); // Empty, no data, monitoring_active = false
    let widget = PerformancePanel::new(
        &perf,
        false,
        IconSet::default(),
        &VmConnectionStatus::Disconnected,
    );
    let buf = render_to_buf(widget, 80, 24);
    // Should render disconnected message — just check it doesn't panic
    // and that some text is present. Collect all buffer text into a flat String.
    let full = collect_buf_text(&buf, 80, 24);
    assert!(
        full.contains("VM Service") || full.contains("monitoring") || full.contains("Waiting"),
        "Expected disconnected message in buffer"
    );
}

#[test]
fn test_performance_panel_disconnected_still_works() {
    // Verify disconnected state renders a text message, not chart widgets
    let perf = PerformanceState::default();
    let widget = PerformancePanel::new(
        &perf,
        false,
        IconSet::default(),
        &VmConnectionStatus::Disconnected,
    );
    let buf = render_to_buf(widget, 80, 24);
    let full = collect_buf_text(&buf, 80, 24);
    // Should NOT try to render chart widgets
    assert!(
        full.contains("VM Service") || full.contains("not connected"),
        "Disconnected state should show VM Service message; got: {full:?}"
    );
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
    // Should not panic even in small terminal
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
    // Extremely small area — should not panic
}

#[test]
fn test_performance_panel_shows_connection_error() {
    // When vm_connection_error is set, render_disconnected should show the
    // specific error message rather than the generic "not connected" text.
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
    // Must NOT show the generic fallback when a specific error is available.
    assert!(
        !full.contains("Performance monitoring requires"),
        "Should not show generic message when specific error is available"
    );
}

#[test]
fn test_performance_panel_no_error_shows_generic_disconnected() {
    // When vm_connection_error is None and vm_connected is false, the generic
    // message should be shown.
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
    // When monitoring_active is false and vm_connected is true,
    // we should see the "starting..." message
    let mut perf = PerformanceState::default();
    perf.monitoring_active = false;
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
    // When connection_status is Reconnecting, the disconnected view should
    // show the attempt counter rather than the generic "not connected" text.
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
    // Verify frame chart shows selection without panic
    let mut perf = make_test_performance();
    perf.selected_frame = Some(5);
    let widget = PerformancePanel::new(
        &perf,
        true,
        IconSet::default(),
        &VmConnectionStatus::Connected,
    );
    render_to_buf(widget, 80, 30);
    // Should not panic with selected frame
}
