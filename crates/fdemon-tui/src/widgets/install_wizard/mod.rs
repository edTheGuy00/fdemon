//! # Install Wizard Panel
//!
//! Centered overlay panel for guiding users through Flutter toolchain setup.
//! Opens when fdemon detects a missing or broken toolchain.
//!
//! ## Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  Install Wizard                               [Esc] Close        │
//! │  Flutter toolchain setup                                         │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Setup Steps          │  Detail                                  │
//! │  ─────────────────    │  ─────────────────                       │
//! │  ✓ Prerequisites      │  ✓ Flutter SDK: 3.19.0                   │
//! │  ! Android Tools      │  ! Android Command-line Tools: missing   │
//! │  ✗ Flutter SDK        │                                          │
//! │  … Flutter Doctor     │                                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  [Tab] switch · [j/k] move · [r] re-run · [Esc] close           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod doctor_view;
mod progress;
mod step_detail;
mod step_list;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

use fdemon_app::install_wizard::InstallWizardState;

use crate::theme::palette;
use crate::widgets::modal_overlay::{self, centered_rect_percent};

use step_detail::step_detail_pane;
use step_list::step_list_pane;

/// Minimum terminal width for horizontal (side-by-side) pane layout.
///
/// Derived from: 30 chars left pane + 1 separator + 35 chars right pane + 4 border = 70.
/// Reuses the same threshold as `FlutterVersionPanel`.
const MIN_HORIZONTAL_WIDTH: u16 = 70;

/// Minimum dialog width for any rendering.
///
/// Derived from: narrowest useful display of "Running preflight checks…" + 4 border = 40.
const MIN_RENDER_WIDTH: u16 = 40;

/// Minimum dialog height for any rendering.
///
/// Derived from: 2 header + 1 sep + 5 content + 1 sep + 1 footer + 2 border = 12.
const MIN_RENDER_HEIGHT: u16 = 12;

/// Panel width as a percentage of the terminal width.
///
/// Derived from: 80% provides comfortable margins on typical 80–200 column terminals.
const PANEL_WIDTH_PERCENT: u16 = 80;

/// Panel height as a percentage of the terminal height.
///
/// Derived from: 85% maximises visible content while leaving a small margin for context.
const PANEL_HEIGHT_PERCENT: u16 = 85;

/// Width of the left (step list) pane as a percentage of the content area.
///
/// Derived from: step list never needs more than ~20 columns ("  ✓ Flutter SDK" = 15);
/// 28% provides comfortable display at typical widths and gives the detail pane more room.
const LEFT_PANE_PERCENT: u16 = 28;

/// Height of the left pane in vertical (stacked) layout.
///
/// Derived from: header(2) + 5 steps × 1 row + 2 padding = 9 rows.
const VERTICAL_STEP_LIST_HEIGHT: u16 = 9;

/// The main Install Wizard panel widget.
///
/// Renders as a centered overlay over the full terminal area.  Reads purely
/// from `&InstallWizardState`; no mutation except the `Cell<usize>` render-hint.
pub struct InstallWizardPanel<'a> {
    state: &'a InstallWizardState,
    /// Current animation frame for spinner animation in the progress view.
    /// Comes from `AppState::animation_frame`.
    animation_frame: u64,
}

impl<'a> InstallWizardPanel<'a> {
    /// Create a new Install Wizard Panel widget.
    ///
    /// # Arguments
    /// * `state`           – Panel state snapshot
    /// * `animation_frame` – Frame counter for spinner animation (from `AppState::animation_frame`)
    pub fn new(state: &'a InstallWizardState, animation_frame: u64) -> Self {
        Self {
            state,
            animation_frame,
        }
    }

    /// Render the panel header: title + close hint on row 1, subtitle on row 2.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        // Row 0: "Install Wizard" (bold) on left, "[Esc] Close" on right
        let title_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Install Wizard",
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let close_hint = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(" "),
            Span::styled("Close", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw("  "),
        ]);

        let title_area = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(title_line).render(title_area, buf);
        Paragraph::new(close_hint)
            .alignment(Alignment::Right)
            .render(title_area, buf);

        // Row 1: subtitle (dimmed)
        // When the wizard is opened informationally (UserInvoked) and every component is
        // healthy, show a reassuring "All set" hint instead of the generic subtitle.
        if area.height >= 2 {
            let subtitle_text = if !self.state.is_bootstrap() && self.state.all_components_ok() {
                "All set \u{2014} press Esc to return" // — (em dash)
            } else {
                "Flutter toolchain setup"
            };
            let subtitle = Line::from(vec![
                Span::raw("  "),
                Span::styled(subtitle_text, Style::default().fg(palette::TEXT_MUTED)),
            ]);
            let subtitle_area = Rect::new(area.x, area.y + 1, area.width, 1);
            Paragraph::new(subtitle).render(subtitle_area, buf);
        }
    }

    /// Render a horizontal separator line (─ repeated across full width).
    fn render_separator(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let sep = "\u{2500}".repeat(area.width as usize); // ─
        buf.set_string(
            area.x,
            area.y,
            &sep,
            Style::default().fg(palette::BORDER_DIM),
        );
    }

    /// Render a vertical separator line (│ from top to bottom of area).
    fn render_vertical_separator(area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_char('\u{2502}'); // │
                cell.set_style(Style::default().fg(palette::BORDER_DIM));
            }
        }
    }

    /// Render a loading placeholder in the panes area.
    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }
        let msg = "  Running preflight checks\u{2026}"; // …
        let y = area.y + area.height / 2;
        let x = area.x + area.width.saturating_sub(msg.chars().count() as u16) / 2;
        buf.set_string(
            x,
            y,
            msg,
            Style::default()
                .fg(palette::TEXT_MUTED)
                .add_modifier(Modifier::BOLD),
        );
    }

    /// Compute the `failed_step_kind` to pass to `step_list_pane`.
    ///
    /// Returns `Some(kind)` when the wizard's execution state records a `Failed`
    /// status for a step run, so the step-list badge for that step can be
    /// replaced with the run-failed indicator.
    fn failed_execution_kind(&self) -> Option<fdemon_app::install_wizard::WizardStepKind> {
        use fdemon_app::install_wizard::StepExecStatus;
        if self.state.execution.status == StepExecStatus::Failed {
            self.state.execution.kind
        } else {
            None
        }
    }

    /// Render horizontal (side-by-side) pane layout.
    fn render_horizontal_panes(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Percentage(LEFT_PANE_PERCENT),
            Constraint::Length(1),
            Constraint::Min(20),
        ])
        .split(area);

        let list_pane = step_list_pane(
            &self.state.steps,
            self.state.selected_index,
            self.state.focused_pane,
            self.failed_execution_kind(),
        );
        list_pane.render(chunks[0], buf);

        Self::render_vertical_separator(chunks[1], buf);

        let detail_pane = step_detail_pane(self.state, self.animation_frame);
        detail_pane.render(chunks[2], buf);
    }

    /// Render vertical (stacked) pane layout for narrow terminals.
    fn render_vertical_panes(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Length(VERTICAL_STEP_LIST_HEIGHT),
            Constraint::Length(1),
            Constraint::Min(5),
        ])
        .split(area);

        let list_pane = step_list_pane(
            &self.state.steps,
            self.state.selected_index,
            self.state.focused_pane,
            self.failed_execution_kind(),
        );
        list_pane.render(chunks[0], buf);

        self.render_separator(chunks[1], buf);

        let detail_pane = step_detail_pane(self.state, self.animation_frame);
        detail_pane.render(chunks[2], buf);
    }

    /// Whether a step is actively installing (live run in progress).
    ///
    /// True only for the `Running` status — terminal states
    /// (Succeeded/Failed/Cancelled) revert to the split layout so the result
    /// summary shows next to the updated step list.
    fn is_step_running(&self) -> bool {
        use fdemon_app::install_wizard::StepExecStatus;
        self.state.execution.status == StepExecStatus::Running
            && self.state.execution.kind.is_some()
    }

    /// Render the live execution view across the full content width.
    ///
    /// Used while a step is actively `Running`.  Shows an "Installing: <step>"
    /// caption + separator, then hands the rest of the area to [`StepProgress`]
    /// (phase, gauge/counter, log tail, and the `[Esc] Cancel` hint) at full
    /// width — so the progress is prominent on both wide (side-by-side) and
    /// narrow (stacked) terminals instead of being squeezed into a corner.
    fn render_running_fullwidth(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }

        // Caption: "Installing: <step title>"
        let title = self
            .state
            .steps
            .iter()
            .find(|s| Some(s.kind) == self.state.execution.kind)
            .map(|s| s.title.as_str())
            .unwrap_or("step");
        let caption = Line::from(vec![
            Span::raw("  "),
            Span::styled("Installing: ", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(
                title,
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        Paragraph::new(caption).render(Rect::new(area.x, area.y, area.width, 1), buf);

        if area.height >= 2 {
            let sep = "\u{2500}".repeat(area.width as usize); // ─
            buf.set_string(
                area.x,
                area.y + 1,
                &sep,
                Style::default().fg(palette::BORDER_DIM),
            );
        }

        // Progress occupies the rest of the content area, full width.
        let body_y = area.y + 2;
        if body_y >= area.y + area.height {
            return;
        }
        let body = Rect::new(area.x, body_y, area.width, area.height - 2);
        progress::StepProgress::new(&self.state.execution, self.animation_frame, true)
            .render(body, buf);
    }

    /// Render the footer: Phase 1 key hints (and optional status message).
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = "[Tab] switch \u{00b7} [j/k] move \u{00b7} [r] re-run \u{00b7} [Esc] close";

        let text = if let Some(ref msg) = self.state.status_message {
            format!("{msg}  \u{2502}  {hints}") // │
        } else {
            hints.to_string()
        };

        Paragraph::new(Line::from(Span::styled(
            text,
            Style::default().fg(palette::TEXT_MUTED),
        )))
        .render(area, buf);
    }

    /// Render "terminal too small" message.
    fn render_too_small(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }
        let msg = " Terminal too small for Install Wizard ";
        let x = area.x + area.width.saturating_sub(msg.chars().count() as u16) / 2;
        let y = area.y + area.height / 2;
        buf.set_string(
            x,
            y,
            msg,
            Style::default()
                .fg(palette::TEXT_MUTED)
                .add_modifier(Modifier::BOLD),
        );
    }
}

impl Widget for InstallWizardPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 1. Dim the entire background
        modal_overlay::dim_background(buf, area);

        // 2. Calculate centered dialog area
        let dialog_area = centered_rect_percent(PANEL_WIDTH_PERCENT, PANEL_HEIGHT_PERCENT, area);

        // 3. Check minimum size — render "too small" and return early
        if dialog_area.width < MIN_RENDER_WIDTH || dialog_area.height < MIN_RENDER_HEIGHT {
            self.render_too_small(dialog_area, buf);
            return;
        }

        // 4. Render drop shadow
        modal_overlay::render_shadow(buf, dialog_area);

        // 5. Clear the dialog area
        modal_overlay::clear_area(buf, dialog_area);

        // 6. Render border block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::POPUP_BG));
        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // 7. Layout: header(2) | separator(1) | panes(flex) | separator(1) | footer(1)
        //
        // The panes row is the ONLY flexible constraint, so it absorbs all
        // remaining vertical space and the footer stays anchored to the bottom
        // edge. A second flexible constraint here (e.g. a trailing `Min(0)`
        // absorber) would make the solver split the leftover space ~evenly,
        // floating the footer into the middle and leaving a dead band beneath it.
        let chunks = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        self.render_header(chunks[0], buf);
        self.render_separator(chunks[1], buf);

        // 8. Content area:
        //    - loading → placeholder
        //    - a step actively running → full-width progress (the static step
        //      list is the least useful thing on screen mid-install; handing the
        //      whole content area to the progress view keeps it prominent at any
        //      width instead of confining it to the right/bottom)
        //    - otherwise → the side-by-side / stacked step-list + detail split
        if self.state.loading {
            self.render_loading(chunks[2], buf);
        } else if self.is_step_running() {
            self.render_running_fullwidth(chunks[2], buf);
        } else if inner.width >= MIN_HORIZONTAL_WIDTH {
            self.render_horizontal_panes(chunks[2], buf);
        } else {
            self.render_vertical_panes(chunks[2], buf);
        }

        self.render_separator(chunks[3], buf);
        self.render_footer(chunks[4], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::install_wizard::{
        ComponentCheck, ComponentKind, ComponentStatus, DoctorLine, DoctorMarker, HostPlatform,
        HostShell, InstallWizardState, LinuxPackageManager, ToolchainReport, WizardOrigin,
    };
    use ratatui::{buffer::Buffer, layout::Rect};

    fn make_report() -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status: ComponentStatus::Ok,
                    detail: "3.19.0".to_string(),
                },
                ComponentCheck {
                    kind: ComponentKind::Git,
                    status: ComponentStatus::Ok,
                    detail: "2.43.0".to_string(),
                },
                ComponentCheck {
                    kind: ComponentKind::AndroidCmdlineTools,
                    status: ComponentStatus::Missing,
                    detail: "not found".to_string(),
                },
            ],
            doctor: Some(vec![
                DoctorLine {
                    marker: DoctorMarker::Ok,
                    text: "Flutter (Channel stable, 3.19.0)".to_string(),
                    indent: 0,
                },
                DoctorLine {
                    marker: DoctorMarker::Warning,
                    text: "Android toolchain".to_string(),
                    indent: 0,
                },
            ]),
            linux_package_manager: Some(LinuxPackageManager::Unknown),
            winget_available: false,
        }
    }

    fn populated_state() -> InstallWizardState {
        let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);
        state.apply_report(make_report());
        state
    }

    fn loading_state() -> InstallWizardState {
        InstallWizardState::opening(WizardOrigin::UserInvoked)
    }

    fn empty_steps_state() -> InstallWizardState {
        InstallWizardState::default()
    }

    fn running_flutter_state() -> InstallWizardState {
        use fdemon_app::install_wizard::{StepExecStatus, StepExecution, WizardStepKind};
        let mut state = populated_state();
        state.execution = StepExecution {
            kind: Some(WizardStepKind::FlutterSdk),
            status: StepExecStatus::Running,
            phase_label: Some("Cloning".to_string()),
            received: 0,
            total: None,
            log_tail: std::collections::VecDeque::new(),
            result_summary: None,
        };
        if let Some(idx) = state
            .steps
            .iter()
            .position(|s| s.kind == WizardStepKind::FlutterSdk)
        {
            state.selected_index = idx;
        }
        state
    }

    /// While a step is actively running, the wizard hands the whole content area
    /// to the progress view (full-width "Installing: <step>" caption + progress),
    /// hiding the static step list — at both wide and narrow widths.
    #[test]
    fn test_running_step_renders_fullwidth_progress() {
        for (w, h) in [(90u16, 26u16), (70, 22)] {
            let state = running_flutter_state();
            let widget = InstallWizardPanel::new(&state, 0);
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            widget.render(area, &mut buf);
            let content: String = (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
                .collect();

            assert!(
                content.contains("Installing:"),
                "{w}x{h}: full-width run view must show the 'Installing:' caption: '{content}'"
            );
            assert!(
                content.contains("Cloning"),
                "{w}x{h}: must show the live phase label: '{content}'"
            );
            // The static step list is hidden mid-run: a non-executing step's title
            // ("Android Tools") must NOT be drawn while the full-width view is up.
            assert!(
                !content.contains("Android Tools"),
                "{w}x{h}: step list must be hidden during an active run: '{content}'"
            );
            assert!(
                !content.contains("Setup Steps"),
                "{w}x{h}: step-list header must be hidden during an active run: '{content}'"
            );
        }
    }

    #[test]
    fn test_renders_loading_placeholder() {
        let state = loading_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("preflight"),
            "loading state should show preflight message"
        );
    }

    #[test]
    fn test_renders_without_panic_populated() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_renders_without_panic_empty_steps() {
        let state = empty_steps_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_renders_without_panic_step_no_components() {
        let mut state = populated_state();
        state.selected_index = 2; // PathConfig — no components
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_header_shows_title() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Install Wizard"),
            "header should contain title"
        );
    }

    #[test]
    fn test_header_shows_esc_close() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("[Esc]"), "header should contain [Esc]");
        assert!(content.contains("Close"), "header should contain Close");
    }

    #[test]
    fn test_footer_shows_key_hints() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Tab"), "footer should show Tab hint");
        assert!(content.contains("re-run"), "footer should show re-run hint");
    }

    #[test]
    fn test_footer_anchored_to_bottom_no_dead_band() {
        // Regression: a second flexible constraint (`Min(0)` absorber) used to
        // split the leftover vertical space, floating the footer into the middle
        // and leaving a large empty band beneath it. The footer key-hint row must
        // sit on the last inner row, directly above the bottom border.
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 100, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Locate the footer row (the one containing the key hints) and the
        // bottom border row (containing the rounded corner '╰').
        let row_text = |y: u16| -> String {
            (0..area.width)
                .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                .collect()
        };
        let footer_y = (0..area.height)
            .find(|&y| row_text(y).contains("re-run"))
            .expect("footer row with key hints must be rendered");
        let bottom_border_y = (0..area.height)
            .find(|&y| row_text(y).contains('\u{2570}')) // ╰
            .expect("bottom border row must be rendered");

        assert_eq!(
            footer_y + 1,
            bottom_border_y,
            "footer must be the last inner row (immediately above the bottom border); \
             no dead band of empty rows may sit between them"
        );
    }

    #[test]
    fn test_footer_shows_status_message() {
        let mut state = populated_state();
        state.status_message = Some("Checks complete.".into());
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Checks complete"),
            "footer should show status message"
        );
    }

    #[test]
    fn test_too_small_renders_message() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 30, 8); // too small
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_narrow_terminal_uses_vertical_layout() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        // Narrow area to force vertical layout
        let area = Rect::new(0, 0, 60, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_wide_terminal_uses_horizontal_layout() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    /// NEW (Phase 6): Verify the panel height constant is 85%.
    #[test]
    fn test_panel_height_percent_is_85() {
        assert_eq!(
            PANEL_HEIGHT_PERCENT, 85,
            "PANEL_HEIGHT_PERCENT must be 85 after phase-6 resize"
        );
    }

    /// NEW (Phase 6): Verify the left pane width constant is 28%.
    #[test]
    fn test_left_pane_percent_is_28() {
        assert_eq!(
            LEFT_PANE_PERCENT, 28,
            "LEFT_PANE_PERCENT must be 28 after phase-6 resize"
        );
    }

    /// NEW (Phase 6): Verify the minimum render height constant is 12 (reduced header).
    #[test]
    fn test_min_render_height_is_12() {
        assert_eq!(
            MIN_RENDER_HEIGHT, 12,
            "MIN_RENDER_HEIGHT must be 12 after header shrank to 2 rows"
        );
    }

    #[test]
    fn test_step_list_shows_status_glyphs() {
        let state = populated_state();
        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        // Some steps should have status glyphs
        assert!(
            content.contains('✓')
                || content.contains('✗')
                || content.contains('!')
                || content.contains('…'),
            "step list should show status glyphs"
        );
    }

    /// When the wizard is opened via `UserInvoked` and every component is `Ok`,
    /// the header subtitle must show the "All set" hint.
    #[test]
    fn informational_all_ok_shows_all_set_hint() {
        // Build a report where every component is Ok
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status: ComponentStatus::Ok,
                    detail: "3.19.0".to_string(),
                },
                ComponentCheck {
                    kind: ComponentKind::Git,
                    status: ComponentStatus::Ok,
                    detail: "2.43.0".to_string(),
                },
            ],
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };
        let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);
        state.apply_report(report);

        let widget = InstallWizardPanel::new(&state, 0);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("All set"),
            "informational all-Ok wizard must show 'All set' hint in header; content: {content:?}"
        );
    }

    /// When the wizard is opened via `Bootstrap`, or any component is non-Ok,
    /// or while loading, the "All set" hint must NOT appear.
    #[test]
    fn bootstrap_or_partial_does_not_show_all_set_hint() {
        let all_ok_report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::FlutterSdk,
                status: ComponentStatus::Ok,
                detail: "3.19.0".to_string(),
            }],
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };

        // Case 1: Bootstrap origin even with all-Ok report → no "All set"
        {
            let mut state = InstallWizardState::opening(WizardOrigin::Bootstrap);
            state.apply_report(all_ok_report.clone());
            let widget = InstallWizardPanel::new(&state, 0);
            let area = Rect::new(0, 0, 120, 50);
            let mut buf = Buffer::empty(area);
            widget.render(area, &mut buf);
            let content: String = buf.content().iter().map(|c| c.symbol()).collect();
            assert!(
                !content.contains("All set"),
                "Bootstrap origin must NOT show 'All set' hint; content: {content:?}"
            );
        }

        // Case 2: UserInvoked but a component is missing → no "All set"
        {
            let partial_report = ToolchainReport {
                platform: HostPlatform::Linux,
                shell: HostShell::Bash,
                components: vec![
                    ComponentCheck {
                        kind: ComponentKind::FlutterSdk,
                        status: ComponentStatus::Ok,
                        detail: "3.19.0".to_string(),
                    },
                    ComponentCheck {
                        kind: ComponentKind::AndroidCmdlineTools,
                        status: ComponentStatus::Missing,
                        detail: "not found".to_string(),
                    },
                ],
                doctor: None,
                linux_package_manager: None,
                winget_available: false,
            };
            let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);
            state.apply_report(partial_report);
            let widget = InstallWizardPanel::new(&state, 0);
            let area = Rect::new(0, 0, 120, 50);
            let mut buf = Buffer::empty(area);
            widget.render(area, &mut buf);
            let content: String = buf.content().iter().map(|c| c.symbol()).collect();
            assert!(
                !content.contains("All set"),
                "Partial report must NOT show 'All set' hint; content: {content:?}"
            );
        }

        // Case 3: UserInvoked but still loading (no report) → no "All set"
        {
            let state = InstallWizardState::opening(WizardOrigin::UserInvoked);
            let widget = InstallWizardPanel::new(&state, 0);
            let area = Rect::new(0, 0, 120, 50);
            let mut buf = Buffer::empty(area);
            widget.render(area, &mut buf);
            let content: String = buf.content().iter().map(|c| c.symbol()).collect();
            assert!(
                !content.contains("All set"),
                "Loading state must NOT show 'All set' hint; content: {content:?}"
            );
        }
    }
}
