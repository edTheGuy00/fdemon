//! Performance panel widget for the DevTools TUI mode.
//!
//! Displays real-time FPS, memory usage, frame timing, and jank metrics
//! using data from Phase 3's monitoring pipeline ([`PerformanceState`]).

use fdemon_app::session::PerformanceState;
use fdemon_app::state::VmConnectionStatus;
use fdemon_core::performance::MemoryUsage;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Sparkline, Widget, Wrap},
};

use crate::theme::{icons::IconSet, palette};

// ── Layout constants ──────────────────────────────────────────────────────────

/// Minimum height for the FPS section (header + sparkline rows).
const FPS_SECTION_HEIGHT: u16 = 4;
/// Minimum height for the memory section (header + gauge + detail rows).
const MEMORY_SECTION_HEIGHT: u16 = 4;
/// Minimum height for the stats section.
const STATS_SECTION_HEIGHT: u16 = 3;

/// Width below which we use compact (sparkline-less) layout.
const COMPACT_WIDTH_THRESHOLD: u16 = 50;

// ── Style threshold constants ─────────────────────────────────────────────────

/// Cap for sparkline bar heights (2x the 16.67ms frame budget at 60fps).
const SPARKLINE_MAX_MS: u64 = 33;
/// FPS at or above this value is considered healthy (green).
const FPS_GREEN_THRESHOLD: f64 = 55.0;
/// FPS at or above this value (but below green) is degraded (yellow).
const FPS_YELLOW_THRESHOLD: f64 = 30.0;
/// Memory utilization below this is healthy (green).
const MEM_GREEN_THRESHOLD: f64 = 0.6;
/// Memory utilization below this (but above green) is elevated (yellow).
const MEM_YELLOW_THRESHOLD: f64 = 0.8;
/// Jank frame percentage below this is acceptable (yellow, not red).
const JANK_WARN_THRESHOLD: f64 = 0.05;

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

    // ── FPS section ──────────────────────────────────────────────────────────

    fn render_fps_section(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Section block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                format!(" {} Frame Timing ", self.icons.activity()),
                Style::default().fg(palette::ACCENT_DIM),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let stats = &self.performance.stats;

        // Build header line: "FPS: 60.0   Avg: 8.2ms   P95: 12.1ms   Max: 15.3ms"
        let fps_val = match stats.fps {
            Some(fps) => format!("{:.1}", fps),
            None => "\u{2014}".to_string(), // em dash — idle / no recent frames
        };
        let avg_val = stats
            .avg_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());
        let p95_val = stats
            .p95_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());
        let max_val = stats
            .max_frame_ms
            .map(|v| format!("{:.1}ms", v))
            .unwrap_or_else(|| "\u{2014}".to_string());

        let header = Line::from(vec![
            Span::styled("FPS: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(fps_val, fps_style(stats.fps)),
            Span::styled("   Avg: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(avg_val, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::styled("   P95: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(p95_val, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::styled("   Max: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(max_val, Style::default().fg(palette::TEXT_PRIMARY)),
        ]);

        buf.set_line(inner.x, inner.y, &header, inner.width);

        // Sparkline of recent frame times — skip in compact mode
        let sparkline_rows = inner.height.saturating_sub(1);
        if sparkline_rows > 0 && area.width >= COMPACT_WIDTH_THRESHOLD {
            let sparkline_area = Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: sparkline_rows,
            };
            self.render_frame_sparkline(sparkline_area, buf);
        }
    }

    fn render_frame_sparkline(&self, area: Rect, buf: &mut Buffer) {
        // Convert frame history to u64 milliseconds (capped at 33ms = 2x 60fps budget)
        let frame_data: Vec<u64> = self
            .performance
            .frame_history
            .iter()
            .map(|f| f.elapsed_micros / 1000)
            .collect();

        if frame_data.is_empty() {
            let waiting =
                Paragraph::new("No frame data yet").style(Style::default().fg(palette::TEXT_MUTED));
            waiting.render(area, buf);
            return;
        }

        let sparkline = Sparkline::default()
            .data(&frame_data)
            .max(SPARKLINE_MAX_MS)
            .style(Style::default().fg(Color::Cyan));

        sparkline.render(area, buf);
    }

    // ── Memory section ───────────────────────────────────────────────────────

    fn render_memory_section(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                format!(" {} Memory ", self.icons.cpu()),
                Style::default().fg(palette::ACCENT_DIM),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let latest = self.performance.memory_history.latest();

        if let Some(mem) = latest {
            // Distribute inner area: header line, gauge, detail line
            let available = inner.height;

            // Line 1: "Heap: 45.2 MB / 128.0 MB  (35%)"
            let usage_text = format!(
                "Heap: {} / {}  ({:.0}%)",
                MemoryUsage::format_bytes(mem.heap_usage),
                MemoryUsage::format_bytes(mem.heap_capacity),
                mem.utilization() * 100.0,
            );
            let header_line = Line::from(Span::styled(
                usage_text,
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            buf.set_line(inner.x, inner.y, &header_line, inner.width);

            // Gauge on line 2
            if available >= 2 {
                let gauge_area = Rect {
                    x: inner.x,
                    y: inner.y + 1,
                    width: inner.width,
                    height: 1,
                };
                let util = mem.utilization().clamp(0.0, 1.0);
                let gauge = Gauge::default()
                    .ratio(util)
                    .gauge_style(gauge_style_for_utilization(util))
                    .label(Span::styled(
                        format!("{:.0}%", util * 100.0),
                        Style::default().fg(palette::TEXT_BRIGHT),
                    ));
                gauge.render(gauge_area, buf);
            }

            // Line 3: "External: 12.5 MB   Total: 57.7 MB"
            if available >= 3 {
                let detail_text = format!(
                    "External: {}   Total: {}",
                    MemoryUsage::format_bytes(mem.external_usage),
                    MemoryUsage::format_bytes(mem.total()),
                );
                let detail_line = Line::from(Span::styled(
                    detail_text,
                    Style::default().fg(palette::TEXT_SECONDARY),
                ));
                buf.set_line(inner.x, inner.y + 2, &detail_line, inner.width);
            }
        } else {
            // No data yet
            let text = Paragraph::new("Waiting for memory data...")
                .style(Style::default().fg(palette::TEXT_MUTED));
            text.render(inner, buf);
        }
    }

    // ── Stats section ────────────────────────────────────────────────────────

    fn render_stats_section(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                format!(" {} Stats ", self.icons.activity()),
                Style::default().fg(palette::ACCENT_DIM),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let stats = &self.performance.stats;
        let gc_count = self.performance.gc_history.len();
        let last_gc = self.performance.gc_history.latest();

        // Jank percentage
        let jank_pct = if stats.buffered_frames > 0 {
            (stats.jank_count as f64 / stats.buffered_frames as f64) * 100.0
        } else {
            0.0
        };

        let gc_type_str = last_gc.map(|gc| gc.gc_type.as_str()).unwrap_or("\u{2014}");

        // Line 1: "Frames: 1,234   Jank: 12 (0.97%)   GC: 3 (MarkSweep)"
        let frames_str = format_number(stats.buffered_frames);
        let line1 = Line::from(vec![
            Span::styled("Frames: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(frames_str, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::styled("   Jank: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(
                format!("{}", stats.jank_count),
                jank_style(stats.jank_count, stats.buffered_frames),
            ),
            Span::styled(
                format!(" ({:.2}%)", jank_pct),
                Style::default().fg(palette::TEXT_MUTED),
            ),
            Span::styled("   GC: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(
                format!("{} ({})", gc_count, gc_type_str),
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
        ]);
        buf.set_line(inner.x, inner.y, &line1, inner.width);

        // Line 2: "Monitoring: Active"  or  "Monitoring: Inactive"
        if inner.height >= 2 {
            let (status_label, status_style) = if self.performance.monitoring_active {
                ("Active", Style::default().fg(palette::STATUS_GREEN))
            } else {
                ("Inactive", Style::default().fg(palette::TEXT_MUTED))
            };

            let line2 = Line::from(vec![
                Span::styled("Monitoring: ", Style::default().fg(palette::TEXT_SECONDARY)),
                Span::styled(status_label, status_style),
            ]);
            buf.set_line(inner.x, inner.y + 1, &line2, inner.width);
        }
    }
}

// ── Style helpers ─────────────────────────────────────────────────────────────

/// Choose a colour for the FPS value based on its magnitude.
fn fps_style(fps: Option<f64>) -> Style {
    match fps {
        Some(v) if v >= FPS_GREEN_THRESHOLD => Style::default().fg(palette::STATUS_GREEN),
        Some(v) if v >= FPS_YELLOW_THRESHOLD => Style::default().fg(palette::STATUS_YELLOW),
        Some(_) => Style::default().fg(palette::STATUS_RED),
        None => Style::default().fg(Color::DarkGray), // stale / no data
    }
}

/// Choose a gauge colour based on heap utilisation (0.0–1.0).
fn gauge_style_for_utilization(util: f64) -> Style {
    if util < MEM_GREEN_THRESHOLD {
        Style::default().fg(palette::STATUS_GREEN)
    } else if util < MEM_YELLOW_THRESHOLD {
        Style::default().fg(palette::STATUS_YELLOW)
    } else {
        Style::default().fg(palette::STATUS_RED)
    }
}

/// Choose a colour for the jank count.
fn jank_style(jank_count: u32, total_frames: u64) -> Style {
    if total_frames == 0 || jank_count == 0 {
        return Style::default().fg(palette::STATUS_GREEN);
    }
    let pct = jank_count as f64 / total_frames as f64;
    if pct < JANK_WARN_THRESHOLD {
        Style::default().fg(palette::STATUS_YELLOW)
    } else {
        Style::default().fg(palette::STATUS_RED)
    }
}

/// Format a large number with comma separators (e.g. 1234567 → "1,234,567").
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
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

        // Add some memory data
        perf.memory_history.push(MemoryUsage {
            heap_usage: 50_000_000,
            heap_capacity: 128_000_000,
            external_usage: 12_000_000,
            timestamp: chrono::Local::now(),
        });

        // Add some frame data
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

    #[test]
    fn test_performance_panel_renders_without_panic() {
        let perf = make_test_performance();
        let widget = PerformancePanel::new(
            &perf,
            true,
            IconSet::default(),
            &VmConnectionStatus::Connected,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should render disconnected message — just check it doesn't panic
        // and that some text is present. Collect all buffer text into a flat String.
        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        widget.render(Rect::new(0, 0, 10, 1), &mut buf);
        // Extremely small area — should not panic
    }

    #[test]
    fn test_fps_color_green_high_fps() {
        // FPS >= 55 should use STATUS_GREEN
        let style = fps_style(Some(60.0));
        assert_eq!(style.fg, Some(palette::STATUS_GREEN));
    }

    #[test]
    fn test_fps_color_yellow_medium_fps() {
        // FPS 30-54.9 should use STATUS_YELLOW
        let style = fps_style(Some(45.0));
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
    }

    #[test]
    fn test_fps_color_red_low_fps() {
        // FPS < 30 should use STATUS_RED
        let style = fps_style(Some(20.0));
        assert_eq!(style.fg, Some(palette::STATUS_RED));
    }

    #[test]
    fn test_fps_color_none() {
        // None fps → DarkGray
        let style = fps_style(None);
        assert_eq!(style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_memory_gauge_color_low_utilization() {
        // < 60% should be STATUS_GREEN
        let style = gauge_style_for_utilization(0.4);
        assert_eq!(style.fg, Some(palette::STATUS_GREEN));
    }

    #[test]
    fn test_memory_gauge_color_medium_utilization() {
        // 60%-79% should be STATUS_YELLOW
        let style = gauge_style_for_utilization(0.7);
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
    }

    #[test]
    fn test_memory_gauge_color_high_utilization() {
        // >= 80% should be STATUS_RED
        let style = gauge_style_for_utilization(0.85);
        assert_eq!(style.fg, Some(palette::STATUS_RED));
    }

    #[test]
    fn test_format_number_small() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn test_format_number_thousands() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234), "1,234");
    }

    #[test]
    fn test_format_number_millions() {
        assert_eq!(format_number(1_234_567), "1,234,567");
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

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
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

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Reconnecting") || full.contains("3/10"),
            "Expected reconnecting message with attempt count, got: {full:?}"
        );
    }
}
