//! Performance panel widget for the DevTools TUI mode.
//!
//! Displays real-time FPS, memory usage, frame timing, and jank metrics
//! using data from Phase 3's monitoring pipeline ([`PerformanceState`]).

mod frame_section;
mod memory_section;
mod stats_section;
pub(super) mod styles;

use fdemon_app::session::PerformanceState;
use fdemon_app::state::VmConnectionStatus;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::theme::{icons::IconSet, palette};

use frame_section::FPS_SECTION_HEIGHT;
use memory_section::MEMORY_SECTION_HEIGHT;
use stats_section::STATS_SECTION_HEIGHT;
use styles::fps_style;

// ── PerformancePanel ─────────────────────────────────────────────────────────

/// Performance panel widget for the DevTools mode.
///
/// Displays FPS, memory usage, frame timing, and jank metrics
/// using data from Phase 3's monitoring pipeline.
pub struct PerformancePanel<'a> {
    performance: &'a PerformanceState,
    vm_connected: bool,
    /// Optional connection error from `DevToolsViewState::vm_connection_error`.
    /// When `Some`, the disconnected state shows the specific failure reason instead
    /// of the generic "VM Service not connected" message.
    vm_connection_error: Option<&'a str>,
    /// Rich VM connection status for displaying more detailed messages in the
    /// disconnected/reconnecting state.
    connection_status: &'a VmConnectionStatus,
    icons: IconSet,
}

impl<'a> PerformancePanel<'a> {
    /// Create a new performance panel widget.
    pub fn new(
        performance: &'a PerformanceState,
        vm_connected: bool,
        icons: IconSet,
        connection_status: &'a VmConnectionStatus,
    ) -> Self {
        Self {
            performance,
            vm_connected,
            vm_connection_error: None,
            connection_status,
            icons,
        }
    }

    /// Attach the optional VM connection error string (from `DevToolsViewState`).
    ///
    /// When set, the disconnected view shows the specific failure reason instead of
    /// the generic "VM Service not connected" message.
    pub fn with_connection_error(mut self, error: Option<&'a str>) -> Self {
        self.vm_connection_error = error;
        self
    }
}

impl Widget for PerformancePanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        // Show disconnected/no-data state if VM is not connected
        if !self.vm_connected || !self.performance.monitoring_active {
            self.render_disconnected(area, buf);
            return;
        }

        // Compute section heights based on available area
        let total_h = area.height;
        let min_required = FPS_SECTION_HEIGHT + MEMORY_SECTION_HEIGHT + STATS_SECTION_HEIGHT;

        if total_h < min_required {
            // Very small terminal — just show a compact single-line summary
            self.render_compact_summary(area, buf);
            return;
        }

        // Distribute remaining height to FPS section (sparkline benefits from more height)
        let fps_h = FPS_SECTION_HEIGHT + (total_h - min_required) / 2;
        let mem_h = MEMORY_SECTION_HEIGHT + (total_h - min_required + 1) / 4;
        let stats_h = total_h - fps_h - mem_h;

        let chunks = Layout::vertical([
            Constraint::Length(fps_h),
            Constraint::Length(mem_h),
            Constraint::Min(stats_h),
        ])
        .split(area);

        self.render_fps_section(chunks[0], buf);
        self.render_memory_section(chunks[1], buf);
        self.render_stats_section(chunks[2], buf);
    }
}

impl PerformancePanel<'_> {
    // ── Disconnected / no-data state ─────────────────────────────────────────

    fn render_disconnected(&self, area: Rect, buf: &mut Buffer) {
        // If a specific connection error was recorded, prefer that over the
        // generic "not connected" message so the user sees an actionable reason.
        let error_owned: String;
        let message: &str = if !self.vm_connected {
            match self.connection_status {
                VmConnectionStatus::Reconnecting {
                    attempt,
                    max_attempts,
                } => {
                    error_owned = format!(
                        "Reconnecting to VM Service... ({attempt}/{max_attempts})\n\
                         Performance monitoring will resume when connected."
                    );
                    &error_owned
                }
                _ => {
                    if let Some(err) = self.vm_connection_error {
                        error_owned = err.to_string();
                        &error_owned
                    } else {
                        "VM Service not connected. Performance monitoring requires a debug connection."
                    }
                }
            }
        } else if !self.performance.monitoring_active {
            "Performance monitoring starting..."
        } else {
            "Waiting for data..."
        };

        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        // Centre vertically within the area
        let y_offset = area.height.saturating_sub(1) / 2;
        let centered = Rect {
            y: area.y + y_offset,
            height: 1,
            ..area
        };
        paragraph.render(centered, buf);
    }

    // ── Compact summary for very small terminals ──────────────────────────────

    fn render_compact_summary(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let stats = &self.performance.stats;
        let fps_str = match stats.fps {
            Some(fps) => format!("{:.1} FPS", fps),
            None => "\u{2014} FPS".to_string(),
        };
        let jank_str = format!("  Jank: {}", stats.jank_count);
        let line = Line::from(vec![
            Span::styled(fps_str, fps_style(stats.fps)),
            Span::styled(jank_str, Style::default().fg(palette::TEXT_SECONDARY)),
        ]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
    fn test_performance_panel_shows_fps() {
        let perf = make_test_performance();
        let widget = PerformancePanel::new(
            &perf,
            true,
            IconSet::default(),
            &VmConnectionStatus::Connected,
        );
        let buf = render_to_buf(widget, 80, 24);
        // Collect content from row 0
        let content: String = (0u16..80)
            .filter_map(|x| {
                buf.cell((x, 0u16))
                    .map(|c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect();
        assert!(content.contains("60.0") || content.contains("FPS") || content.contains("Frame"));
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
}
