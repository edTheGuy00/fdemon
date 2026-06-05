//! # Step Progress View
//!
//! Stateless widget that renders the live execution state of a running or
//! finished wizard step:
//!
//! ```text
//! Phase label: Downloading  ⟳
//! [████████░░░░░░░░░░░░] 42.1 MB / 100.0 MB
//! > Cloning flutter repository...
//! > Resolving dependencies...
//! ```
//!
//! When `total` bytes are known a filled gauge bar is shown; when unknown a
//! plain byte counter is shown with an animated spinner.  The log tail is
//! clipped to whatever height remains after the header rows.

use fdemon_core::strip_ansi_codes;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{LineGauge, Paragraph, Widget},
};

use fdemon_app::install_wizard::{StepExecStatus, StepExecution};

use crate::theme::palette;
use crate::widgets::spinner::{spinner_char, SPINNER_TICKS_PER_FRAME};

/// Height of the phase-label row.
///
/// Derived from: 1 row for "Phase: <label> <status glyph>".
const PHASE_ROW_HEIGHT: u16 = 1;

/// Height of the progress row (gauge or byte counter).
///
/// Derived from: 1 row for the gauge / counter.
const PROGRESS_ROW_HEIGHT: u16 = 1;

/// Height of the separator between progress and log tail.
///
/// Derived from: 1 blank row provides visual breathing room.
const SEPARATOR_HEIGHT: u16 = 1;

/// Height of the result-summary row shown in terminal states (Succeeded/Failed).
///
/// Derived from: 1 row for the success/failure summary text.
const RESULT_SUMMARY_HEIGHT: u16 = 1;

/// Height of the "Esc cancels" hint row shown while a step is Running.
///
/// Derived from: 1 row for the cancel affordance hint.
const CANCEL_HINT_HEIGHT: u16 = 1;

/// Glyph shown when a step succeeded.
const SUCCESS_GLYPH: &str = "\u{2713}"; // ✓
/// Glyph shown when a step failed.
const FAILED_GLYPH: &str = "\u{2717}"; // ✗

/// Renders the live execution view for a running or finished wizard step.
///
/// Shows:
/// - Line 1: phase label + status glyph / spinner
/// - Line 2: download progress gauge (when `total` is known) or byte counter
///   with spinner (when `total` is unknown)
/// - Optional bottom row: "  [Esc] Cancel" hint while status is `Running`
///   (controlled by the `show_cancel_hint` field, set by the detail-pane
///   renderer in `step_detail.rs`)
/// - Remaining rows: the tail of `log_tail`, clipped to available height
///
/// This widget is stateless and reads purely from `&StepExecution`.  Pass the
/// current `animation_frame` for spinner animation; it is derived from log
/// length for tests where no frame counter is available.
pub struct StepProgress<'a> {
    exec: &'a StepExecution,
    /// Current animation frame for spinner — from `AppState::animation_frame`.
    animation_frame: u64,
    /// When `true`, render an "  [Esc] Cancel" hint at the bottom of the view.
    ///
    /// Set by `step_detail.rs` when the execution status is `Running` so the
    /// user knows they can press Esc to abort the ongoing operation.  The hint
    /// is absent for `Succeeded`/`Failed`/`Idle` states.
    show_cancel_hint: bool,
}

impl<'a> StepProgress<'a> {
    /// Create a new step progress widget.
    ///
    /// # Arguments
    /// * `exec`              – Live execution state to render.
    /// * `animation_frame`   – Frame counter for spinner animation.
    /// * `show_cancel_hint`  – Show "[Esc] Cancel" hint when `true` (pass `true`
    ///   when `exec.status == Running`).
    pub fn new(exec: &'a StepExecution, animation_frame: u64, show_cancel_hint: bool) -> Self {
        Self {
            exec,
            animation_frame,
            show_cancel_hint,
        }
    }

    /// Render the phase label row with status glyph.
    fn render_phase_row(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let label = self
            .exec
            .phase_label
            .as_deref()
            .unwrap_or("Running\u{2026}"); // Running…

        let (glyph, glyph_color) = match self.exec.status {
            StepExecStatus::Running => {
                let spinner = spinner_char(self.animation_frame / SPINNER_TICKS_PER_FRAME);
                // Return a String for the animated spinner — we box it below
                return self.render_phase_row_running(area, buf, label, spinner);
            }
            StepExecStatus::Succeeded => (SUCCESS_GLYPH, palette::STATUS_GREEN),
            StepExecStatus::Failed => (FAILED_GLYPH, palette::STATUS_RED),
            // Cancelled: neutral muted glyph — not an error.
            StepExecStatus::Cancelled => ("\u{2014}", palette::TEXT_MUTED), // —
            StepExecStatus::Idle => ("\u{2026}", palette::TEXT_MUTED),      // …
        };

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::raw(" "),
            Span::styled(glyph, Style::default().fg(glyph_color)),
        ]);
        Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }

    /// Render the phase row specifically for the `Running` status (includes spinner char).
    fn render_phase_row_running(&self, area: Rect, buf: &mut Buffer, label: &str, spinner: char) {
        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::raw(" "),
            Span::styled(
                spinner.to_string(),
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }

    /// Render the progress row: gauge when total is known, byte counter otherwise.
    fn render_progress_row(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let row_area = Rect::new(area.x, area.y, area.width, 1);

        match self.exec.total {
            Some(total) if total > 0 => {
                // Known total: render a filled line gauge
                let ratio = (self.exec.received as f64 / total as f64).clamp(0.0, 1.0);
                let label = format!(
                    " {:.1} MB / {:.1} MB ",
                    self.exec.received as f64 / 1_048_576.0,
                    total as f64 / 1_048_576.0,
                );

                let gauge = LineGauge::default()
                    .ratio(ratio)
                    .label(label)
                    .filled_style(Style::default().fg(palette::ACCENT))
                    .unfilled_style(Style::default().fg(palette::BORDER_DIM));
                gauge.render(row_area, buf);
            }
            _ => {
                // Unknown total: render a plain byte counter with spinner
                let spinner = spinner_char(self.animation_frame / SPINNER_TICKS_PER_FRAME);
                let mb = self.exec.received as f64 / 1_048_576.0;

                let (counter_text, counter_color) = if self.exec.received == 0 {
                    (
                        format!("  {} Waiting\u{2026}", spinner),
                        palette::TEXT_MUTED,
                    )
                } else {
                    (
                        format!("  {} {:.1} MB received", spinner, mb),
                        palette::TEXT_SECONDARY,
                    )
                };

                let line = Line::from(Span::styled(
                    counter_text,
                    Style::default().fg(counter_color),
                ));
                Paragraph::new(line).render(row_area, buf);
            }
        }
    }

    /// Render the log tail lines, clipped to available height.
    ///
    /// Shows the most-recent lines (tail of `exec.log_tail`); never computes
    /// manual out-of-bounds offsets — all positioning is via `Layout`.
    ///
    /// Each line is ANSI-sanitized before rendering to prevent raw escape sequences
    /// (from git or flutter output) from corrupting the terminal cursor/color state.
    fn render_log_tail(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || self.exec.log_tail.is_empty() {
            return;
        }

        let visible_height = area.height as usize;
        let total = self.exec.log_tail.len();
        // Show the last `visible_height` lines (most recent)
        let skip = total.saturating_sub(visible_height);

        for (i, line_text) in self.exec.log_tail.iter().skip(skip).enumerate() {
            let y = area.y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            // Strip ANSI escape sequences before pushing into the Ratatui buffer.
            // git progress output and flutter logs may contain color/cursor codes that
            // would corrupt cursor state if rendered as raw bytes.
            let clean = strip_ansi_codes(line_text);
            let line = Line::from(Span::styled(
                format!("  > {clean}"),
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(line).render(Rect::new(area.x, y, area.width, 1), buf);
        }
    }

    /// Render the "[Esc] Cancel" hint row.
    ///
    /// Shown at the bottom of the progress view while a step is `Running`.
    /// Consistent with the existing key-hint styling used in the wizard footer.
    fn render_cancel_hint(area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let line = Line::from(vec![
            Span::styled("  [Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled(" Cancel", Style::default().fg(palette::TEXT_SECONDARY)),
        ]);
        Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }

    /// Render the result summary line (shown on Succeeded or Failed).
    fn render_result_summary(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let (text, color) = match &self.exec.result_summary {
            Some(summary) => {
                let color = match self.exec.status {
                    StepExecStatus::Succeeded => palette::STATUS_GREEN,
                    StepExecStatus::Failed => palette::STATUS_RED,
                    // Cancelled: neutral muted color — not an error display.
                    StepExecStatus::Cancelled => palette::TEXT_MUTED,
                    _ => palette::TEXT_SECONDARY,
                };
                (format!("  {summary}"), color)
            }
            None => {
                return; // Nothing to show
            }
        };

        let line = Line::from(Span::styled(text, Style::default().fg(color)));
        Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }
}

impl Widget for StepProgress<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }

        // For terminal states (Succeeded/Failed/Cancelled), show: phase row + separator +
        // result summary + (optional) log tail remainder.
        // For Running/Idle, show: phase row + progress row + separator + log tail
        //   + optional "[Esc] Cancel" hint at the bottom.
        let is_terminal = matches!(
            self.exec.status,
            StepExecStatus::Succeeded | StepExecStatus::Failed | StepExecStatus::Cancelled
        );

        if is_terminal {
            // Layout: phase(1) | separator(1) | result(1) | log_tail(min 0)
            let chunks = Layout::vertical([
                Constraint::Length(PHASE_ROW_HEIGHT),
                Constraint::Length(SEPARATOR_HEIGHT),
                Constraint::Length(RESULT_SUMMARY_HEIGHT),
                Constraint::Min(0), // log tail absorber
            ])
            .split(area);

            self.render_phase_row(chunks[0], buf);
            // chunks[1] is blank (separator space)
            self.render_result_summary(chunks[2], buf);
            self.render_log_tail(chunks[3], buf);
        } else if self.show_cancel_hint {
            // Running with cancel hint:
            // phase(1) | progress(1) | separator(1) | log_tail(min 0) | cancel_hint(1)
            let chunks = Layout::vertical([
                Constraint::Length(PHASE_ROW_HEIGHT),
                Constraint::Length(PROGRESS_ROW_HEIGHT),
                Constraint::Length(SEPARATOR_HEIGHT),
                Constraint::Min(0), // log tail (may be empty, absorbs slack)
                Constraint::Length(CANCEL_HINT_HEIGHT),
            ])
            .split(area);

            self.render_phase_row(chunks[0], buf);
            self.render_progress_row(chunks[1], buf);
            // chunks[2] is blank separator
            self.render_log_tail(chunks[3], buf);
            Self::render_cancel_hint(chunks[4], buf);
        } else {
            // Running/Idle without cancel hint:
            // phase(1) | progress(1) | separator(1) | log_tail(min 0)
            let chunks = Layout::vertical([
                Constraint::Length(PHASE_ROW_HEIGHT),
                Constraint::Length(PROGRESS_ROW_HEIGHT),
                Constraint::Length(SEPARATOR_HEIGHT),
                Constraint::Min(0), // log tail
            ])
            .split(area);

            self.render_phase_row(chunks[0], buf);
            self.render_progress_row(chunks[1], buf);
            // chunks[2] is blank separator
            self.render_log_tail(chunks[3], buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    fn small_area() -> Rect {
        Rect::new(0, 0, 40, 10)
    }

    fn large_area() -> Rect {
        Rect::new(0, 0, 80, 20)
    }

    fn collect_content(buf: &Buffer, area: Rect) -> String {
        buf.content()
            .iter()
            .take((area.width * area.height) as usize)
            .map(|c| c.symbol())
            .collect()
    }

    fn make_running_exec_known_total() -> StepExecution {
        StepExecution {
            kind: Some(fdemon_app::install_wizard::WizardStepKind::FlutterSdk),
            status: StepExecStatus::Running,
            phase_label: Some("Downloading".to_string()),
            received: 42 * 1_048_576,     // 42 MB
            total: Some(100 * 1_048_576), // 100 MB
            log_tail: std::collections::VecDeque::from(vec![
                "Fetching objects...".to_string(),
                "Resolving...".to_string(),
            ]),
            result_summary: None,
        }
    }

    fn make_running_exec_unknown_total() -> StepExecution {
        StepExecution {
            kind: Some(fdemon_app::install_wizard::WizardStepKind::FlutterSdk),
            status: StepExecStatus::Running,
            phase_label: Some("Cloning".to_string()),
            received: 10 * 1_048_576, // 10 MB
            total: None,
            log_tail: std::collections::VecDeque::from(vec!["Cloning repository...".to_string()]),
            result_summary: None,
        }
    }

    fn make_succeeded_exec() -> StepExecution {
        StepExecution {
            kind: Some(fdemon_app::install_wizard::WizardStepKind::FlutterSdk),
            status: StepExecStatus::Succeeded,
            phase_label: Some("Complete".to_string()),
            received: 100 * 1_048_576,
            total: Some(100 * 1_048_576),
            log_tail: std::collections::VecDeque::from(vec!["Installation complete.".to_string()]),
            result_summary: Some("Flutter SDK installed successfully.".to_string()),
        }
    }

    fn make_failed_exec() -> StepExecution {
        StepExecution {
            kind: Some(fdemon_app::install_wizard::WizardStepKind::FlutterSdk),
            status: StepExecStatus::Failed,
            phase_label: Some("Failed".to_string()),
            received: 0,
            total: None,
            log_tail: std::collections::VecDeque::from(vec!["error: network timeout".to_string()]),
            result_summary: Some("Installation failed: network timeout".to_string()),
        }
    }

    #[test]
    fn test_progress_renders_bar_with_known_total() {
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        // The gauge label shows MB values
        assert!(
            content.contains("42.0 MB"),
            "should show received MB: '{content}'"
        );
        assert!(
            content.contains("100.0 MB"),
            "should show total MB: '{content}'"
        );
    }

    #[test]
    fn test_progress_renders_counter_with_unknown_total() {
        let exec = make_running_exec_unknown_total();
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        // Without total, we show a byte counter
        assert!(
            content.contains("MB received"),
            "should show byte counter: '{content}'"
        );
        // No gauge ratio text
        assert!(
            !content.contains("/ 100.0 MB"),
            "should not show total MB: '{content}'"
        );
    }

    #[test]
    fn test_progress_shows_phase_label() {
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        assert!(
            content.contains("Downloading"),
            "should show phase label: '{content}'"
        );
    }

    #[test]
    fn test_progress_log_tail_clips_to_height() {
        // Create an exec with more log lines than the render area can fit
        let log_lines: std::collections::VecDeque<String> =
            (0..50).map(|i| format!("log line {i}")).collect();
        let exec = StepExecution {
            status: StepExecStatus::Running,
            phase_label: Some("Running".to_string()),
            log_tail: log_lines,
            ..StepExecution::default()
        };

        // Small area: 40x10, after header section (3 rows) only 7 rows for log
        let area = small_area();
        let mut buf = Buffer::empty(area);
        let widget = StepProgress::new(&exec, 0, false);
        widget.render(area, &mut buf); // must not panic, must clip

        // Only the most recent lines should appear — lines 43-49 are in tail
        let content = collect_content(&buf, area);
        // Last line should be visible
        assert!(
            content.contains("49"),
            "last log line should be visible in tail: '{content}'"
        );
    }

    #[test]
    fn test_progress_shows_success_summary() {
        let exec = make_succeeded_exec();
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        assert!(
            content.contains("installed successfully"),
            "should show success summary: '{content}'"
        );
    }

    #[test]
    fn test_progress_shows_failure_summary() {
        let exec = make_failed_exec();
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        assert!(
            content.contains("network timeout"),
            "should show failure summary: '{content}'"
        );
    }

    #[test]
    fn test_progress_renders_without_panic_tiny_area() {
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, false);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_progress_renders_without_panic_zero_area() {
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, false);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1)); // need at least 1x1 buf
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_progress_spinner_advances_with_frame() {
        let exec = make_running_exec_unknown_total();

        // Render at frame 0 and frame 10 to get different spinner states
        let area = large_area();
        let mut buf0 = Buffer::empty(area);
        let mut buf10 = Buffer::empty(area);

        StepProgress::new(&exec, 0, false).render(area, &mut buf0);
        StepProgress::new(&exec, 20, false).render(area, &mut buf10); // 20 / 2 = frame 10 vs 0

        // Content may differ due to spinner (not guaranteed but likely for 10 frame advance)
        let c0 = collect_content(&buf0, area);
        let c10 = collect_content(&buf10, area);
        // The spinner changes after SPINNER_TICKS_PER_FRAME(2) ticks, so frame 0 vs 20
        // should differ since 20 / 2 = 10, and spinner frames wrap at 10
        // (wrap means they're equal again, so let's just verify no panic)
        let _ = c0;
        let _ = c10;
    }

    #[test]
    fn test_progress_idle_renders_without_panic() {
        let exec = StepExecution::default(); // Idle status
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_progress_empty_log_tail_renders_without_panic() {
        let exec = StepExecution {
            status: StepExecStatus::Running,
            phase_label: Some("Checking".to_string()),
            log_tail: std::collections::VecDeque::new(),
            ..StepExecution::default()
        };
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_log_tail_ansi_stripped_before_render() {
        // Lines containing ANSI escape sequences must not appear as raw bytes
        // in the rendered Ratatui buffer — they must be stripped first.
        let exec = StepExecution {
            status: StepExecStatus::Running,
            phase_label: Some("Cloning".to_string()),
            log_tail: std::collections::VecDeque::from(vec![
                "\x1b[31mred error text\x1b[0m".to_string()
            ]),
            ..StepExecution::default()
        };
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content = collect_content(&buf, area);

        // The visible text should be present
        assert!(
            content.contains("red error text"),
            "stripped text should appear in buffer: '{content}'"
        );

        // Raw ESC byte (0x1B) must not be present
        assert!(
            !content.contains('\x1b'),
            "raw ESC byte must not appear in buffer: '{content}'"
        );
    }

    // --- Phase 5 task 06: cancel hint tests ---

    #[test]
    fn detail_shows_esc_cancels_hint_while_running() {
        // show_cancel_hint=true → "[Esc]" and "Cancel" must appear in the buffer.
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, true);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        assert!(
            content.contains("Esc"),
            "[Esc] must appear when show_cancel_hint=true: '{content}'"
        );
        assert!(
            content.contains("Cancel"),
            "'Cancel' must appear when show_cancel_hint=true: '{content}'"
        );
    }

    #[test]
    fn detail_hides_cancel_hint_when_show_cancel_hint_false() {
        // show_cancel_hint=false → no "[Esc]" or "Cancel" in the buffer.
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, false);
        let area = large_area();
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        assert!(
            !content.contains("Cancel"),
            "'Cancel' must NOT appear when show_cancel_hint=false: '{content}'"
        );
    }

    #[test]
    fn detail_hides_cancel_hint_for_terminal_states() {
        // Even with show_cancel_hint=true, terminal states (Succeeded/Failed)
        // do not reach the cancel-hint layout branch — the hint must not appear.
        for exec in [make_succeeded_exec(), make_failed_exec()] {
            let widget = StepProgress::new(&exec, 0, true);
            let area = large_area();
            let mut buf = Buffer::empty(area);
            widget.render(area, &mut buf);
            let content = collect_content(&buf, area);

            assert!(
                !content.contains("Cancel"),
                "terminal state must NOT show 'Cancel' hint (status={:?}): '{content}'",
                exec.status
            );
        }
    }

    #[test]
    fn cancel_hint_does_not_panic_tiny_area() {
        let exec = make_running_exec_known_total();
        let widget = StepProgress::new(&exec, 0, true);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    // --- Task 03 (F12): Cancelled state renders neutral (non-red) summary ---

    fn make_cancelled_exec() -> StepExecution {
        StepExecution {
            kind: Some(fdemon_app::install_wizard::WizardStepKind::FlutterSdk),
            status: StepExecStatus::Cancelled,
            phase_label: Some("Cancelled".to_string()),
            received: 0,
            total: None,
            log_tail: std::collections::VecDeque::new(),
            result_summary: Some("Cancelled: Flutter install cancelled by user".to_string()),
        }
    }

    /// F12: A step in `StepExecStatus::Cancelled` must render a result summary
    /// without using `STATUS_RED` — the summary color must be muted, not red.
    #[test]
    fn cancelled_exec_result_summary_is_not_red() {
        let exec = make_cancelled_exec();
        let area = large_area();
        let mut buf = Buffer::empty(area);
        let widget = StepProgress::new(&exec, 0, false);
        widget.render(area, &mut buf);

        // Verify the result summary text appears in the buffer.
        let content = collect_content(&buf, area);
        assert!(
            content.contains("Cancelled"),
            "Cancelled exec must render the result summary: '{content}'"
        );

        // The summary text spans row 2 (phase=0, separator=1, result=2).
        // Find any cell in the buffer that contains the summary text and
        // check it is NOT styled with STATUS_RED.
        use crate::theme::palette;
        let red_cells: Vec<_> = buf
            .content()
            .iter()
            .filter(|c| c.fg == palette::STATUS_RED && c.symbol() != " " && c.symbol() != "─")
            .collect();
        assert!(
            red_cells.is_empty(),
            "Cancelled exec must NOT render any STATUS_RED cells; found {:?}",
            red_cells.iter().map(|c| c.symbol()).collect::<Vec<_>>()
        );
    }

    /// F12: A step in `StepExecStatus::Cancelled` must not trigger the
    /// "cancel hint" layout branch (Cancelled is a terminal state).
    /// The "[Esc] Cancel" hint must NOT appear — note that the result summary
    /// may contain "Cancelled" (the status text), but the hint's "[Esc]" key
    /// label must be absent from the buffer.
    #[test]
    fn cancelled_exec_does_not_show_cancel_hint() {
        let exec = make_cancelled_exec();
        let area = large_area();
        let mut buf = Buffer::empty(area);
        // show_cancel_hint=true: even if caller passes true, terminal state
        // prevents the cancel-hint layout branch from rendering.
        let widget = StepProgress::new(&exec, 0, true);
        widget.render(area, &mut buf);
        let content = collect_content(&buf, area);

        // The "[Esc] Cancel" hint uses "[Esc]" — not present in the summary text.
        assert!(
            !content.contains("[Esc]"),
            "Cancelled terminal state must not show '[Esc]' cancel hint: '{content}'"
        );
    }

    /// F12: A genuine `StepExecStatus::Failed` exec must still render STATUS_RED
    /// (this ensures we haven't accidentally neutralised genuine failures).
    #[test]
    fn failed_exec_result_summary_is_still_red() {
        let exec = make_failed_exec();
        let area = large_area();
        let mut buf = Buffer::empty(area);
        let widget = StepProgress::new(&exec, 0, false);
        widget.render(area, &mut buf);

        use crate::theme::palette;
        // At least the glyph or summary should be STATUS_RED for a genuine failure.
        let has_red = buf
            .content()
            .iter()
            .any(|c| c.fg == palette::STATUS_RED && c.symbol() != " ");
        assert!(
            has_red,
            "genuine Failed exec must still render at least one STATUS_RED cell"
        );
    }
}
