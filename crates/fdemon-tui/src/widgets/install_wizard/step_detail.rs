//! # Step Detail Pane
//!
//! Right pane of the Install Wizard.
//! Renders the detail for the currently selected [`WizardStep`]:
//!
//! - **Doctor step**: delegates to [`DoctorView`].
//! - **Executable steps** (`FlutterSdk`, `PathConfig`, `AndroidTools` when JDK present):
//!   shows component checks plus an "▶ Press Enter to …" action hint; switches to the live
//!   [`StepProgress`] view while a run is in progress.
//! - **AndroidTools with JDK missing**: shows component checks plus a guided-command
//!   section (label, command, optional note) and a `[c] copy` affordance, with a
//!   "JDK 17 required" caption.
//! - **Non-executable steps** (`Prerequisites`, `Doctor`):
//!   shows component checks plus "Available in a later phase" note.
//!
//! Vertical scroll is driven by `state.detail_scroll`.  The actual visible
//! height is written back to `state.last_known_visible_height` each frame
//! (Cell render-hint pattern; see docs/CODE_STANDARDS.md Principle 3).
//! A render-time scroll clamp ensures the content stays in view even when the
//! handler has not yet received the updated hint.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use fdemon_app::install_wizard::{
    ComponentCheck, ComponentStatus, GuidedCommand, InstallWizardState, StepExecStatus, WizardPane,
    WizardStepKind,
};

use crate::theme::palette;

use super::doctor_view::DoctorView;
use super::progress::StepProgress;

/// Glyph for `ComponentStatus::Ok`.
const COMP_OK: &str = "✓";
/// Glyph for `ComponentStatus::Partial`.
const COMP_PARTIAL: &str = "!";
/// Glyph for `ComponentStatus::Missing`.
const COMP_MISSING: &str = "✗";
/// Glyph for `ComponentStatus::Error`.
const COMP_ERROR: &str = "⚠";
/// Glyph for `ComponentStatus::Unknown`.
const COMP_UNKNOWN: &str = "?";

/// Height of the pane title header (label + separator).
///
/// Derived from: 1 title row + 1 separator row = 2 rows.
const HEADER_HEIGHT: u16 = 2;

/// Height of the action hint row at the bottom of the detail body.
///
/// Derived from: 1 row for the "▶ Press Enter to …" / "Available in a later phase" hint.
const ACTION_HINT_HEIGHT: u16 = 1;

/// Height of the guided-command section header line.
///
/// Derived from: 1 row for the "Guided steps (run these yourself…)" label.
const GUIDED_SECTION_HEADER_HEIGHT: u16 = 1;

/// Minimum height required to render a single guided command block.
///
/// Derived from: 1 blank row + 1 label + 1 command + 1 copy hint = 4 rows.
/// The note row is optional and only rendered when space permits.
const GUIDED_COMMAND_MIN_HEIGHT: u16 = 4;

/// Minimum height required to render the JDK-required caption.
///
/// Derived from: 1 row for "JDK 17 required before installing Android tools".
const JDK_CAPTION_HEIGHT: u16 = 1;

/// Right pane — per-step detail renderer.
pub struct StepDetailPane<'a> {
    state: &'a InstallWizardState,
    focused: bool,
    /// Animation frame for spinner in progress view.
    animation_frame: u64,
}

impl<'a> StepDetailPane<'a> {
    /// Create a new step detail pane.
    ///
    /// # Arguments
    /// * `state`           – Full install wizard state (read-only)
    /// * `focused`         – Whether this pane has keyboard focus
    /// * `animation_frame` – Frame counter for spinner animation
    pub fn new(state: &'a InstallWizardState, focused: bool, animation_frame: u64) -> Self {
        Self {
            state,
            focused,
            animation_frame,
        }
    }

    /// Render the pane header: title + separator underline.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let title_style = if self.focused {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        };

        let label = Line::from(vec![Span::raw("  "), Span::styled("Detail", title_style)]);
        Paragraph::new(label).render(Rect::new(area.x, area.y, area.width, 1), buf);

        if area.height >= 2 {
            let separator = "\u{2500}".repeat(area.width as usize); // ─
            buf.set_string(
                area.x,
                area.y + 1,
                &separator,
                Style::default().fg(palette::BORDER_DIM),
            );
        }
    }

    /// Glyph for the given component status.
    fn component_glyph(status: &ComponentStatus) -> &'static str {
        match status {
            ComponentStatus::Ok => COMP_OK,
            ComponentStatus::Partial => COMP_PARTIAL,
            ComponentStatus::Missing => COMP_MISSING,
            ComponentStatus::Error => COMP_ERROR,
            ComponentStatus::Unknown => COMP_UNKNOWN,
        }
    }

    /// Color for the given component status.
    fn component_color(status: &ComponentStatus) -> ratatui::style::Color {
        match status {
            ComponentStatus::Ok => palette::STATUS_GREEN,
            ComponentStatus::Partial => palette::STATUS_YELLOW,
            ComponentStatus::Missing => palette::STATUS_RED,
            ComponentStatus::Error => palette::STATUS_YELLOW,
            ComponentStatus::Unknown => palette::TEXT_MUTED,
        }
    }

    /// Render a single `ComponentCheck` row.
    ///
    /// Format: `  <glyph> <kind label>: <detail>`
    fn render_component_row(check: &ComponentCheck, y: u16, area: Rect, buf: &mut Buffer) {
        let glyph = Self::component_glyph(&check.status);
        let color = Self::component_color(&check.status);

        let kind_label = check.kind.to_string();
        let text = if check.detail.is_empty() {
            format!("  {glyph} {kind_label}")
        } else {
            format!("  {glyph} {kind_label}: {}", check.detail)
        };

        let line = Line::from(vec![Span::styled(text, Style::default().fg(color))]);
        Paragraph::new(line).render(Rect::new(area.x, y, area.width, 1), buf);
    }

    /// Whether this step kind is executable (can be triggered with Enter).
    ///
    /// `AndroidTools` is executable only when JDK is present (no guided commands).
    /// When guided commands are present (JDK missing), it is not immediately runnable
    /// — the user must install JDK first.
    fn is_executable(kind: WizardStepKind, has_guided_commands: bool) -> bool {
        match kind {
            WizardStepKind::FlutterSdk | WizardStepKind::PathConfig => true,
            WizardStepKind::AndroidTools => !has_guided_commands,
            _ => false,
        }
    }

    /// Action hint text for an executable step.
    fn action_hint_text(kind: WizardStepKind) -> &'static str {
        match kind {
            WizardStepKind::FlutterSdk => "\u{25b6} Press Enter to install Flutter SDK", // ▶
            WizardStepKind::PathConfig => "\u{25b6} Press Enter to add Flutter to PATH", // ▶
            WizardStepKind::AndroidTools => "\u{25b6} Press Enter to install Android tools", // ▶
            _ => "",
        }
    }

    /// Render the action hint line for the bottom of the content area.
    ///
    /// Shows "▶ Press Enter to …" for executable steps, or
    /// "Available in a later phase" for non-executable steps.
    /// `has_guided_commands` controls whether `AndroidTools` is treated as executable.
    fn render_action_hint(
        &self,
        kind: WizardStepKind,
        has_guided_commands: bool,
        y: u16,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if y >= area.y + area.height {
            return;
        }

        let executable = Self::is_executable(kind, has_guided_commands);

        let (text, color, bold) = if executable {
            (
                Self::action_hint_text(kind).to_string(),
                palette::ACCENT,
                true,
            )
        } else if kind == WizardStepKind::Doctor {
            // Doctor step is a display-only view; no action
            return;
        } else if kind == WizardStepKind::AndroidTools && has_guided_commands {
            // AndroidTools gated — guided command section handles the CTA
            return;
        } else {
            (
                "  Available in a later phase".to_string(),
                palette::TEXT_MUTED,
                false,
            )
        };

        let style = if bold {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        let prefix = if executable { "  " } else { "" };
        let line = Line::from(Span::styled(format!("{prefix}{text}"), style));
        Paragraph::new(line).render(Rect::new(area.x, y, area.width, 1), buf);
    }

    /// Render the guided-command section for a step that has guided commands.
    ///
    /// Layout (each row occupies one character-cell row):
    /// ```text
    ///   [blank row]
    ///   Guided steps (run these yourself, then press 'r' to re-check):
    ///   [JDK 17 required caption — AndroidTools only]
    ///
    ///     Install JDK 17
    ///       $ sudo apt install openjdk-17-jdk
    ///       or: sudo dnf install java-17-openjdk-devel        [c] copy
    /// ```
    ///
    /// Returns the number of rows consumed (for callers that track y-offset).
    fn render_guided_commands(
        &self,
        commands: &[GuidedCommand],
        step_kind: WizardStepKind,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if commands.is_empty() || area.height < GUIDED_COMMAND_MIN_HEIGHT {
            return;
        }

        let mut y = area.y;

        // Section header
        if y < area.y + area.height {
            let header_style = Style::default().fg(palette::TEXT_SECONDARY);
            let header = Line::from(Span::styled(
                "  Guided steps (run these yourself, then press 'r' to re-check):",
                header_style,
            ));
            Paragraph::new(header).render(Rect::new(area.x, y, area.width, 1), buf);
            y += GUIDED_SECTION_HEADER_HEIGHT;
        }

        // JDK-required caption for the AndroidTools step
        if step_kind == WizardStepKind::AndroidTools && y < area.y + area.height {
            let caption_style = Style::default()
                .fg(palette::STATUS_YELLOW)
                .add_modifier(Modifier::BOLD);
            let caption = Line::from(Span::styled(
                "  JDK 17 required before installing Android tools",
                caption_style,
            ));
            Paragraph::new(caption).render(Rect::new(area.x, y, area.width, 1), buf);
            y += JDK_CAPTION_HEIGHT;
        }

        for (i, cmd) in commands.iter().enumerate() {
            // Blank separator before each command block (except for the very first
            // when there is no caption, to avoid a double blank)
            let needs_blank = i > 0 || step_kind != WizardStepKind::AndroidTools;
            if needs_blank && y < area.y + area.height {
                y += 1; // blank row
            }

            // Label row: "    Install JDK 17"
            if y < area.y + area.height {
                let label_style = Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD);
                let label_line =
                    Line::from(Span::styled(format!("    {}", cmd.label), label_style));
                Paragraph::new(label_line).render(Rect::new(area.x, y, area.width, 1), buf);
                y += 1;
            }

            // Command row: "      $ <command>        [c] copy" (first cmd gets the hint)
            if y < area.y + area.height {
                let cmd_style = Style::default().fg(palette::ACCENT);
                let copy_style = Style::default().fg(palette::TEXT_MUTED);

                let copy_hint = if i == 0 { "  [c] copy" } else { "" };
                let cmd_text = format!("      $ {}", cmd.command);

                let line = Line::from(vec![
                    Span::styled(cmd_text, cmd_style),
                    Span::styled(copy_hint.to_string(), copy_style),
                ]);
                Paragraph::new(line).render(Rect::new(area.x, y, area.width, 1), buf);
                y += 1;
            }

            // Note row (optional): "      or: sudo dnf install …"
            if let Some(ref note) = cmd.note {
                if y < area.y + area.height {
                    let note_style = Style::default().fg(palette::TEXT_SECONDARY);
                    let note_line = Line::from(Span::styled(format!("      {note}"), note_style));
                    Paragraph::new(note_line).render(Rect::new(area.x, y, area.width, 1), buf);
                    y += 1;
                }
            }
        }
        let _ = y; // silence unused warning — final y not needed by callers
    }

    /// Whether the current execution is active for the given step kind.
    ///
    /// Returns `true` when `execution.kind == Some(kind)` and the status is
    /// Running, Succeeded, or Failed (i.e. there is something live to show).
    fn is_execution_active_for(&self, kind: WizardStepKind) -> bool {
        if self.state.execution.kind != Some(kind) {
            return false;
        }
        matches!(
            self.state.execution.status,
            StepExecStatus::Running | StepExecStatus::Succeeded | StepExecStatus::Failed
        )
    }
}

impl Widget for StepDetailPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_header(area, buf);

        let content_y = area.y + HEADER_HEIGHT;
        if content_y >= area.y + area.height {
            return; // No space for content
        }
        let content_area = Rect::new(
            area.x,
            content_y,
            area.width,
            area.height.saturating_sub(HEADER_HEIGHT),
        );

        let visible_height = content_area.height as usize;

        // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
        self.state.last_known_visible_height.set(visible_height);

        let Some(step) = self.state.selected_step() else {
            let msg = Line::from(Span::styled(
                "  Select a step to view details.",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(msg).render(
                Rect::new(content_area.x, content_area.y, content_area.width, 1),
                buf,
            );
            return;
        };

        // --- Live execution view ---
        // When a run is active for the selected step, replace the static detail
        // with the StepProgress view (occupies the full content_area).
        if self.is_execution_active_for(step.kind) {
            let progress = StepProgress::new(&self.state.execution, self.animation_frame);
            progress.render(content_area, buf);
            return;
        }

        // Doctor step: delegate to DoctorView (with scroll applied)
        if step.kind == WizardStepKind::Doctor {
            let doctor_lines = self.state.report.as_ref().and_then(|r| r.doctor.as_ref());

            if let Some(lines) = doctor_lines {
                // Scroll clamp (render-time safety net)
                let corrected_scroll =
                    compute_corrected_scroll(self.state.detail_scroll, visible_height, lines.len());
                // corrected_scroll is guaranteed <= lines.len() by compute_corrected_scroll
                DoctorView::new(Some(&lines[corrected_scroll..])).render(content_area, buf);
            } else {
                DoctorView::new(None).render(content_area, buf);
            }
            return;
        }

        let has_guided_commands = !step.guided_commands.is_empty();

        // Other steps: render component checks + action hint / guided commands.
        if step.components.is_empty() {
            if has_guided_commands {
                // No component checks but guided commands present — render them
                // directly in the content area (e.g. a future prerequisites step).
                self.render_guided_commands(&step.guided_commands, step.kind, content_area, buf);
            } else if content_area.height >= ACTION_HINT_HEIGHT {
                // Informational step with no component checks (e.g., PathConfig)
                self.render_action_hint(
                    step.kind,
                    has_guided_commands,
                    content_area.y,
                    content_area,
                    buf,
                );
            }
            return;
        }

        // Components are present: decide how much space to allocate.
        // When guided commands exist we need room for them below the components.
        // When not, we need just ACTION_HINT_HEIGHT at the bottom.
        let bottom_section_height: u16 = if has_guided_commands {
            // Guided-command block: header(1) + caption(1 for AndroidTools) + blank(1)
            // + label(1) + command(1) + optional note(1) = up to 6 rows, minimum 4.
            // We reserve GUIDED_COMMAND_MIN_HEIGHT + GUIDED_SECTION_HEADER_HEIGHT rows.
            // For AndroidTools also add JDK_CAPTION_HEIGHT.
            let caption_rows = if step.kind == WizardStepKind::AndroidTools {
                JDK_CAPTION_HEIGHT
            } else {
                0
            };
            GUIDED_SECTION_HEADER_HEIGHT + caption_rows + GUIDED_COMMAND_MIN_HEIGHT
        } else {
            ACTION_HINT_HEIGHT
        };

        let component_height = content_area.height.saturating_sub(bottom_section_height) as usize;
        let effective_visible = if component_height > 0 {
            component_height
        } else {
            visible_height
        };

        // Scroll clamp (render-time safety net)
        let corrected_scroll = compute_corrected_scroll(
            self.state.detail_scroll,
            effective_visible,
            step.components.len(),
        );

        let start = corrected_scroll;
        let end = (start + effective_visible).min(step.components.len());

        for (i, check) in step.components[start..end].iter().enumerate() {
            let y = content_area.y + i as u16;
            Self::render_component_row(check, y, content_area, buf);
        }

        // Bottom section: guided commands or action hint
        let bottom_y = content_area.y + content_area.height.saturating_sub(bottom_section_height);
        if has_guided_commands {
            // Height derivation: total content height minus the rows already consumed
            // by the component list above (bottom_y - content_area.y rows used).
            let bottom_area = Rect::new(
                content_area.x,
                bottom_y,
                content_area.width,
                content_area
                    .height
                    .saturating_sub(bottom_y.saturating_sub(content_area.y)),
            );
            self.render_guided_commands(&step.guided_commands, step.kind, bottom_area, buf);
        } else if content_area.height >= ACTION_HINT_HEIGHT {
            self.render_action_hint(step.kind, has_guided_commands, bottom_y, content_area, buf);
        }
    }
}

/// Compute a render-time corrected scroll offset (safety net).
///
/// Does not mutate state — returns a local corrected offset for this frame.
/// The handler will use `last_known_visible_height` to update the real scroll
/// on future keystrokes.
///
/// The returned value is guaranteed to be `<= total_lines` (i.e. a valid
/// slice start), so callers do not need an additional `.min(len)` guard.
fn compute_corrected_scroll(
    scroll_offset: usize,
    visible_height: usize,
    total_lines: usize,
) -> usize {
    if visible_height == 0 || total_lines == 0 {
        return 0;
    }
    let max_scroll = total_lines.saturating_sub(visible_height);
    scroll_offset.min(max_scroll)
}

/// Construct a [`StepDetailPane`] from the install wizard state.
pub fn step_detail_pane(state: &InstallWizardState, animation_frame: u64) -> StepDetailPane<'_> {
    StepDetailPane::new(
        state,
        state.focused_pane == WizardPane::Detail,
        animation_frame,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::install_wizard::{
        ComponentCheck, ComponentKind, ComponentStatus, DoctorLine, DoctorMarker, GuidedCommand,
        HostPlatform, HostShell, InstallWizardState, StepExecStatus, StepExecution,
        ToolchainReport, WizardStep, WizardStepKind,
    };
    use ratatui::{buffer::Buffer, layout::Rect};

    fn make_area() -> Rect {
        Rect::new(0, 0, 60, 20)
    }

    fn make_report_with_doctor() -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::FlutterSdk,
                status: ComponentStatus::Ok,
                detail: "3.19.0".to_string(),
            }],
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
        }
    }

    fn make_report_no_doctor() -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::FlutterSdk,
                status: ComponentStatus::Missing,
                detail: String::new(),
            }],
            doctor: None,
        }
    }

    fn make_state_with_doctor_step_selected() -> InstallWizardState {
        let mut state = InstallWizardState::opening();
        state.apply_report(make_report_with_doctor());
        // Select the Doctor step (index 4 in the 5-step list)
        state.selected_index = 4;
        state
    }

    fn make_state_no_doctor() -> InstallWizardState {
        let mut state = InstallWizardState::opening();
        state.apply_report(make_report_no_doctor());
        state.selected_index = 4; // Doctor step
        state
    }

    fn make_state_components() -> InstallWizardState {
        let mut state = InstallWizardState::opening();
        state.apply_report(make_report_with_doctor());
        state.selected_index = 3; // FlutterSdk step (has components)
        state
    }

    #[test]
    fn test_doctor_view_renders_markers() {
        let state = make_state_with_doctor_step_selected();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Flutter"), "should render doctor output");
        assert!(content.contains("✓"), "Ok marker should be visible");
    }

    #[test]
    fn test_doctor_view_none_shows_unavailable() {
        let state = make_state_no_doctor();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("unavailable"),
            "should show unavailable placeholder when doctor=None"
        );
    }

    #[test]
    fn test_detail_pane_writes_visible_height_hint() {
        let state = make_state_with_doctor_step_selected();
        assert_eq!(
            state.last_known_visible_height.get(),
            0,
            "hint should start at 0"
        );
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        assert!(
            state.last_known_visible_height.get() > 0,
            "render should write visible height hint"
        );
    }

    #[test]
    fn test_component_step_renders_check_rows() {
        let state = make_state_components();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Flutter SDK"),
            "should show component kind label"
        );
        assert!(content.contains("3.19.0"), "should show component detail");
        assert!(
            content.contains("✓"),
            "Ok component should show checkmark glyph"
        );
    }

    #[test]
    fn test_step_detail_shows_enter_hint_for_flutter_step() {
        let state = make_state_components(); // FlutterSdk step selected (index 3)
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Press Enter"),
            "FlutterSdk step should show 'Press Enter' hint: '{content}'"
        );
        assert!(
            content.contains("install Flutter SDK"),
            "FlutterSdk step should mention 'install Flutter SDK': '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_enter_hint_for_path_config_step() {
        let mut state = make_state_components();
        state.selected_index = 2; // PathConfig step
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Press Enter"),
            "PathConfig step should show 'Press Enter' hint: '{content}'"
        );
        assert!(
            content.contains("PATH"),
            "PathConfig step hint should mention PATH: '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_enter_hint_for_android_step_when_jdk_present() {
        // AndroidTools with JDK Ok (guided_commands is empty) → executable.
        // Uses a hand-crafted state to ensure the JDK component is explicitly Ok,
        // since a report without any Jdk entry now correctly produces a guided command
        // (m2 fix: is_jdk_actionable returns true when no Jdk entry).
        let state = make_state_android_jdk_present();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Press Enter"),
            "AndroidTools step without guided commands should show 'Press Enter' hint: '{content}'"
        );
        assert!(
            content.contains("Android tools"),
            "AndroidTools Enter hint should mention 'Android tools': '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_phase_for_prerequisites_step() {
        let mut state = make_state_components();
        state.selected_index = 0; // Prerequisites step
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("later phase"),
            "Prerequisites step should show 'Available in a later phase': '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_progress_view_when_running() {
        let mut state = make_state_components(); // FlutterSdk selected (index 3)
                                                 // Start a run for FlutterSdk
        state.execution = StepExecution {
            kind: Some(WizardStepKind::FlutterSdk),
            status: StepExecStatus::Running,
            phase_label: Some("Downloading".to_string()),
            received: 50 * 1_048_576,
            total: Some(100 * 1_048_576),
            log_tail: std::collections::VecDeque::from(vec!["Fetching...".to_string()]),
            result_summary: None,
        };

        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // Progress view should show phase label and gauge
        assert!(
            content.contains("Downloading"),
            "should show phase label when running: '{content}'"
        );
        // Normal component detail should NOT appear (progress replaced it)
        // The "Press Enter" hint also should not appear (replaced by progress)
        assert!(
            !content.contains("Press Enter"),
            "should not show Enter hint while running: '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_success_summary_after_run() {
        let mut state = make_state_components(); // FlutterSdk selected
        state.execution = StepExecution {
            kind: Some(WizardStepKind::FlutterSdk),
            status: StepExecStatus::Succeeded,
            phase_label: Some("Complete".to_string()),
            received: 100 * 1_048_576,
            total: Some(100 * 1_048_576),
            log_tail: std::collections::VecDeque::new(),
            result_summary: Some("Flutter SDK installed successfully.".to_string()),
        };

        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("installed successfully"),
            "should show success summary: '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_error_summary_on_failure() {
        let mut state = make_state_components(); // FlutterSdk selected
        state.execution = StepExecution {
            kind: Some(WizardStepKind::FlutterSdk),
            status: StepExecStatus::Failed,
            phase_label: Some("Failed".to_string()),
            received: 0,
            total: None,
            log_tail: std::collections::VecDeque::from(vec!["Error: network timeout".to_string()]),
            result_summary: Some("Installation failed: network timeout".to_string()),
        };

        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("network timeout"),
            "should show error summary on failure: '{content}'"
        );
    }

    #[test]
    fn test_step_detail_progress_not_shown_for_different_step() {
        let mut state = make_state_components(); // FlutterSdk selected (index 3)
                                                 // But execution is for a different step (PathConfig)
        state.execution = StepExecution {
            kind: Some(WizardStepKind::PathConfig),
            status: StepExecStatus::Running,
            phase_label: Some("Configuring PATH".to_string()),
            ..StepExecution::default()
        };

        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // FlutterSdk step is selected, not PathConfig — so static detail appears
        assert!(
            content.contains("Flutter SDK"),
            "should show FlutterSdk component detail, not PathConfig progress: '{content}'"
        );
    }

    #[test]
    fn test_empty_step_shows_no_components_message() {
        let mut state = make_state_with_doctor_step_selected();
        // Override selected_index to PathConfig (index 2) which has no components
        state.selected_index = 2;
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // PathConfig is executable and has no component checks: shows Enter hint
        assert!(
            content.contains("Press Enter") || content.contains("PATH"),
            "PathConfig informational step should show action hint: '{content}'"
        );
    }

    #[test]
    fn test_no_panic_loading_state() {
        let state = InstallWizardState::opening(); // loading=true, steps empty
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_panic_tiny_area() {
        let state = make_state_with_doctor_step_selected();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_compute_corrected_scroll_clamps_to_max() {
        // scroll_offset=100 with 5 total lines, 3 visible → max_scroll = 2
        let corrected = compute_corrected_scroll(100, 3, 5);
        assert_eq!(corrected, 2, "scroll should be clamped to max_scroll");
    }

    #[test]
    fn test_compute_corrected_scroll_zero_height() {
        let corrected = compute_corrected_scroll(5, 0, 10);
        assert_eq!(corrected, 0, "zero visible_height should return 0");
    }

    #[test]
    fn test_detail_pane_writes_hint_even_for_empty_state() {
        let state = InstallWizardState::default(); // visible=false, no steps
        assert_eq!(state.last_known_visible_height.get(), 0);

        let pane = StepDetailPane::new(&state, false, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        // Even when no step is selected, the hint should be written
        assert!(
            state.last_known_visible_height.get() > 0,
            "hint should be written even when no step selected"
        );
    }

    #[test]
    fn test_component_missing_status_renders_cross_glyph() {
        let mut state = make_state_with_doctor_step_selected();
        // Manually build a state with a missing component in FlutterSdk step
        state.steps = vec![WizardStep {
            kind: WizardStepKind::FlutterSdk,
            title: "Flutter SDK".to_string(),
            status: fdemon_app::install_wizard::StepStatus::Missing,
            components: vec![ComponentCheck {
                kind: ComponentKind::FlutterSdk,
                status: ComponentStatus::Missing,
                detail: "not found".to_string(),
            }],
            guided_commands: vec![],
        }];
        state.selected_index = 0;

        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("✗"),
            "Missing component should render cross glyph"
        );
    }

    #[test]
    fn test_no_panic_small_terminal() {
        let state = make_state_components();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    // --- Phase 3: Guided command rendering tests ---

    /// Build an `InstallWizardState` with the AndroidTools step selected and a JDK
    /// guided command present (simulates JDK missing scenario).
    fn make_state_android_jdk_missing() -> InstallWizardState {
        InstallWizardState {
            visible: true,
            steps: vec![WizardStep {
                kind: WizardStepKind::AndroidTools,
                title: "Android Tools".to_string(),
                status: fdemon_app::install_wizard::StepStatus::Missing,
                components: vec![ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Missing,
                    detail: "not found".to_string(),
                }],
                guided_commands: vec![GuidedCommand {
                    label: "Install JDK 17".to_string(),
                    command: "sudo apt install openjdk-17-jdk".to_string(),
                    note: Some("or: sudo dnf install java-17-openjdk-devel".to_string()),
                }],
            }],
            selected_index: 0,
            ..InstallWizardState::default()
        }
    }

    /// Build an `InstallWizardState` with the AndroidTools step selected, JDK present
    /// (no guided commands), simulating a ready-to-run Android install.
    fn make_state_android_jdk_present() -> InstallWizardState {
        InstallWizardState {
            visible: true,
            steps: vec![WizardStep {
                kind: WizardStepKind::AndroidTools,
                title: "Android Tools".to_string(),
                status: fdemon_app::install_wizard::StepStatus::Missing,
                components: vec![
                    ComponentCheck {
                        kind: ComponentKind::Jdk,
                        status: ComponentStatus::Ok,
                        detail: "17.0.9".to_string(),
                    },
                    ComponentCheck {
                        kind: ComponentKind::AndroidCmdlineTools,
                        status: ComponentStatus::Missing,
                        detail: "not found".to_string(),
                    },
                ],
                guided_commands: vec![], // JDK is Ok → no guided command
            }],
            selected_index: 0,
            ..InstallWizardState::default()
        }
    }

    #[test]
    fn test_detail_renders_jdk_guided_command() {
        // AndroidTools selected + JDK GuidedCommand present
        let state = make_state_android_jdk_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Install JDK 17"),
            "should render the guided command label: '{content}'"
        );
        assert!(
            content.contains("openjdk-17-jdk"),
            "should render the guided command text: '{content}'"
        );
        assert!(
            content.contains("copy"),
            "should render the [c] copy affordance: '{content}'"
        );
        assert!(
            content.contains("JDK 17 required"),
            "should show the JDK-required caption: '{content}'"
        );
        // The Enter-to-run hint should NOT be the primary CTA when gated
        assert!(
            !content.contains("Press Enter"),
            "should NOT show 'Press Enter' when JDK is missing: '{content}'"
        );
    }

    #[test]
    fn test_detail_android_enter_hint_when_jdk_present() {
        // AndroidTools with no guided commands (JDK present) → normal Enter hint
        let state = make_state_android_jdk_present();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Press Enter"),
            "should show '[Enter] Install Android tools' hint when JDK is present: '{content}'"
        );
        assert!(
            content.contains("Android tools"),
            "Enter hint should mention 'Android tools': '{content}'"
        );
        // No guided command block when JDK is present
        assert!(
            !content.contains("Guided steps"),
            "should NOT show guided-steps section when JDK is present: '{content}'"
        );
    }

    #[test]
    fn test_guided_command_with_note_renders_note() {
        let state = make_state_android_jdk_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("sudo dnf"),
            "should render optional note line: '{content}'"
        );
    }

    #[test]
    fn test_no_panic_guided_command_tiny_area() {
        let state = make_state_android_jdk_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic even in tight space
    }
}
