//! Performance panel widget for the DevTools TUI mode.
//!
//! Displays real-time FPS, memory usage, frame timing, and jank metrics
//! using data from Phase 3's monitoring pipeline ([`PerformanceState`]).
//!
//! # Layout
//!
//! The panel uses a two-section layout:
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │                                         │
//! │           Frame Timing (~45%)           │
//! │  [bar chart + detail panel]             │
//! │                                         │
//! ├─────────────────────────────────────────┤
//! │                                         │
//! │           Memory (~55%)                 │
//! │  [time-series chart + alloc table]      │
//! │                                         │
//! └─────────────────────────────────────────┘
//! ```

mod frame_chart;
mod memory_chart;
pub(super) mod styles;

use fdemon_app::session::PerformanceState;
use fdemon_app::state::VmConnectionStatus;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use crate::theme::{icons::IconSet, palette};

use frame_chart::FrameChart;
use memory_chart::MemoryChart;
use styles::fps_style;

// ── Responsive layout thresholds ─────────────────────────────────────────────

/// Minimum terminal height to show both sections.
/// At 16 rows, each section gets 8 outer rows: frame inner = 6 (Borders::ALL removes 2),
/// memory inner = 7 (Borders::TOP removes 1). Both exceed their minimum chart height.
const DUAL_SECTION_MIN_HEIGHT: u16 = 16;

/// Below this height, show compact summary only.
const COMPACT_THRESHOLD: u16 = 7;

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

        self.render_content(area, buf);
    }
}

impl PerformancePanel<'_> {
    // ── Main content rendering ────────────────────────────────────────────────

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        let total_h = area.height;

        if total_h < COMPACT_THRESHOLD {
            // Very small terminal — show a compact single-line summary
            self.render_compact_summary(area, buf);
            return;
        }

        if total_h < DUAL_SECTION_MIN_HEIGHT {
            // Small terminal — show frame chart only
            let frame_block = Block::default()
                .title(format!(" {} Frame Timing ", self.icons.activity()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::BORDER_DIM))
                .title_style(Style::default().fg(palette::ACCENT_DIM));
            let frame_inner = frame_block.inner(area);
            frame_block.render(area, buf);

            FrameChart::new(
                &self.performance.frame_history,
                self.performance.selected_frame,
                &self.performance.stats,
                false,
            )
            .render(frame_inner, buf);
            return;
        }

        // Two-section split. Reserve 1 row at the bottom for the DevTools
        // footer that the parent DevToolsView renders over this area.
        // Memory gets 55% so that on odd-height areas ratatui's rounding
        // favours the memory section (which needs the larger inner area for the
        // allocation table).  Frame gets 45%, which still yields at least
        // MIN_CHART_HEIGHT inner rows at DUAL_SECTION_MIN_HEIGHT.
        let usable_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(usable_area);

        // Frame timing section (with block border)
        let frame_block = Block::default()
            .title(format!(" {} Frame Timing ", self.icons.activity()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title_style(Style::default().fg(palette::ACCENT_DIM));
        let frame_inner = frame_block.inner(chunks[0]);
        frame_block.render(chunks[0], buf);

        FrameChart::new(
            &self.performance.frame_history,
            self.performance.selected_frame,
            &self.performance.stats,
            false,
        )
        .render(frame_inner, buf);

        // Memory section — use Borders::TOP only to maximise inner height.
        // The top border carries the title; no bottom/side borders are needed
        // because the footer hint line occupies the row below.
        let memory_block = Block::default()
            .title(format!(" {} Memory ", self.icons.cpu()))
            .borders(Borders::TOP)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title_style(Style::default().fg(palette::ACCENT_DIM));
        let memory_inner = memory_block.inner(chunks[1]);
        memory_block.render(chunks[1], buf);

        MemoryChart::new(
            &self.performance.memory_samples,
            &self.performance.memory_history,
            &self.performance.gc_history,
            self.performance.allocation_profile.as_ref(),
            false,
        )
        .render(memory_inner, buf);
    }

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
mod tests;
