//! Performance panel widget for the DevTools TUI mode.
//!
//! Renders the Flutter Frames bar chart in the top section, and a tabbed
//! Details pane in the bottom section (dual-pane layout).
//!
//! # Layout
//!
//! ```text
//! ┌─ Frame Timing ──────────────────────────┐
//! │                                         │
//! │  [frame chart — FRAME_CHART_PCT %]      │
//! │                                         │
//! └─────────────────────────────────────────┘
//! ┌─ ⚡ Frame Details ──────────────────────┐
//! │ Frame Analysis  Rebuild Stats  Timeline │
//! │ ━━━━━━━━━━━━━━                          │
//! │  [tab content]                          │
//! └─────────────────────────────────────────┘
//! ```
//!
//! At short terminals (`inner_h < MIN_DUAL_PANE_HEIGHT`) the chart fills the
//! full usable area (Phase 1 behaviour). At very short terminals
//! (`total_h < COMPACT_THRESHOLD`) a single compact summary line is shown.
//!
//! Memory data and allocation profiling live in the dedicated Memory panel
//! (`DevToolsPanel::Memory`); see [`super::memory`].

mod details;
mod frame_chart;
pub(super) mod styles;

use fdemon_app::session::{PerfSection, PerformanceState};
use fdemon_app::state::VmConnectionStatus;
use fdemon_app::{MouseAction, MouseRect};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use crate::widgets::MouseCtx;

use crate::theme::{icons::IconSet, palette};

use frame_chart::FrameChart;
use styles::fps_style;

// ── Responsive layout thresholds ─────────────────────────────────────────────

/// Below this height, show compact summary only (FPS + Jank single line).
const COMPACT_THRESHOLD: u16 = 7;

/// Minimum total inner height to show the dual-pane layout (chart + details).
///
/// Derivation: FrameChart requires ≥ `MIN_CHART_HEIGHT (4) + DETAIL_PANEL_HEIGHT (3) = 7`
/// rows internally. Details pane requires ≥ `MIN_DETAILS_HEIGHT (8)` rows. Inner area
/// is `area.height - 1` (footer) - 2 (chart block borders). So we need 10 inner
/// rows for the chart + 8 for details = 18 rows.
const MIN_DUAL_PANE_HEIGHT: u16 = 18;

/// Minimum details pane height — tab strip (2) + content (≥ 6).
const MIN_DETAILS_HEIGHT: u16 = 8;

/// Minimum content-area width to show the proportional phase bar in the
/// Frame Analysis tab. T05 consumes this constant; T04 defines it.
///
/// Derivation: 4 phase labels × ~9 chars each + 3 separators = 39 cols. Round
/// up to 40 to leave room for borders and padding.
const MIN_PHASE_BAR_WIDTH: u16 = 40;

/// Percentage of the dual-pane inner area allocated to the Frame Chart.
const FRAME_CHART_PCT: u16 = 55;

// ── Focus / border styling constants ─────────────────────────────────────────

/// Border colour for the focused section (brighter, draws the eye).
const COLOR_FOCUSED_BORDER: Color = Color::Cyan;

/// Border colour for unfocused sections (dim, recedes).
const COLOR_UNFOCUSED_BORDER: Color = Color::DarkGray;

// ── PerformancePanel ─────────────────────────────────────────────────────────

/// Performance panel widget for the DevTools mode.
///
/// Displays FPS, frame timing, and jank metrics using data from Phase 3's
/// monitoring pipeline. In Phase 2, the panel uses a dual-pane layout: the
/// Frame Chart in the upper section and a tabbed Details pane below.
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
        self.render_impl(area, buf, None);
    }
}

impl PerformancePanel<'_> {
    // ── Shared render entry point ─────────────────────────────────────────────

    /// Shared implementation called by both `Widget::render` and
    /// `render_with_regions`.
    ///
    /// When `ctx` is `None` the behaviour is identical to the old
    /// `Widget::render` implementation. When `ctx` is `Some`, click regions
    /// are forwarded into the FrameChart section.
    /// The compact-summary and disconnected paths receive `None`.
    fn render_impl(self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
        // Clear background
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        // Show disconnected/no-data state if VM is not connected or monitoring not started.
        if !self.vm_connected || !self.performance.monitoring_active {
            self.render_disconnected(area, buf);
            return;
        }

        let total_h = area.height;

        if total_h < COMPACT_THRESHOLD {
            // Very small terminal — compact summary, no regions.
            self.render_compact_summary(area, buf);
            return;
        }

        // Reserve 1 row at the bottom for the DevTools footer (unchanged).
        let usable = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        if usable.height < MIN_DUAL_PANE_HEIGHT {
            // Short terminals — Frame Chart fills the entire usable area, same as Phase 1.
            self.render_chart_only(usable, buf, ctx.as_deref_mut());
            return;
        }

        // Dual-pane layout.
        let chart_h = usable.height.saturating_mul(FRAME_CHART_PCT) / 100;
        let chunks = Layout::vertical([
            Constraint::Length(chart_h),
            Constraint::Min(MIN_DETAILS_HEIGHT),
        ])
        .split(usable);

        self.render_chart_only(chunks[0], buf, ctx.as_deref_mut());
        self.render_details_pane(chunks[1], buf, ctx);
    }

    // ── Chart-only rendering ──────────────────────────────────────────────────

    /// Render the frame chart into `area`, optionally registering click regions.
    ///
    /// Called both as the sole content on short terminals and as the upper half
    /// of the dual-pane layout on taller terminals.
    fn render_chart_only(&self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
        let frame_focused = self.performance.focused_section == PerfSection::FrameChart;
        let frame_border_color = if frame_focused {
            COLOR_FOCUSED_BORDER
        } else {
            COLOR_UNFOCUSED_BORDER
        };
        let frame_block = Block::default()
            .title(format!(" {} Frame Timing ", self.icons.activity()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(frame_border_color))
            .title_style(Style::default().fg(palette::ACCENT_DIM));
        let frame_inner = frame_block.inner(area);
        frame_block.render(area, buf);

        // Section-level focus region: clicking anywhere in the frame chart area
        // focuses this section. Per-bar clicks (z=1) win over this region (z=0).
        if let Some(c) = ctx.as_deref_mut() {
            // EXCEPTION (TEA): mouse_regions is a render-hint cell. See docs/CODE_STANDARDS.md
            // "Region Registry Pattern" and docs/REVIEW_FOCUS.md approved-exceptions list.
            let section_rect = MouseRect::new(area.x, area.y, area.width, area.height);
            c.click(
                section_rect,
                MouseAction::emit(fdemon_app::Message::PerfFocusSection(
                    PerfSection::FrameChart,
                )),
            );
        }

        FrameChart::new(
            &self.performance.frame_history,
            self.performance.selected_frame,
            &self.performance.stats,
            false,
            self.performance.frame_chart_scroll_offset,
            &self.performance.frame_chart_visible_width,
        )
        .render_with_regions(frame_inner, buf, ctx);
    }

    // ── Details pane rendering ────────────────────────────────────────────────

    /// Render the tabbed details pane into `area`.
    ///
    /// Draws the surrounding block (with focus-aware border colour) and
    /// delegates the inner content to [`details::render`].
    fn render_details_pane(&self, area: Rect, buf: &mut Buffer, _ctx: Option<&mut MouseCtx<'_>>) {
        // Details pane block — same focus-aware border styling as the chart.
        let details_focused = self.performance.focused_section == PerfSection::Details;
        let border_color = if details_focused {
            COLOR_FOCUSED_BORDER
        } else {
            COLOR_UNFOCUSED_BORDER
        };
        let block = Block::default()
            .title(format!(" {} Frame Details ", self.icons.activity()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title_style(Style::default().fg(palette::ACCENT_DIM));
        let inner = block.inner(area);
        block.render(area, buf);

        // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
        self.performance
            .details_pane_visible_height
            .set(inner.height as usize);

        details::render(inner, buf, self.performance);
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

/// Render the performance panel, optionally recording clickable regions.
///
/// This is the click-aware entry point used by `devtools::render_with_regions`.
/// Delegates to `PerformancePanel::render_impl` — the single authoritative
/// implementation shared with `Widget::render`.  Passing `None` for `ctx`
/// produces output byte-identical to `Widget::render`.
///
/// `ctx` is forwarded into the frame-chart section. The
/// compact-summary and disconnected paths do not record click regions.
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    widget: PerformancePanel<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    widget.render_impl(area, buf, ctx);
}

// Suppress the dead-code warning for MIN_PHASE_BAR_WIDTH, which is defined here
// for co-location of thresholds but consumed by T05.
#[allow(dead_code)]
const _: u16 = MIN_PHASE_BAR_WIDTH;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
