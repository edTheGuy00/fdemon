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
/// Derived from: label(1) + command-with-inline-`[c]`-copy(1) = 2 rows minimum.
/// The `[c] copy` hint is rendered **inline on the command row** (not a separate row).
/// The optional note row adds 1 more row when present.
/// The leading blank row is **conditional**: it is skipped for command 0 when a
/// caption was already rendered above (the caption provides visual separation);
/// for i > 0 or when there is no caption, a blank row is prepended.
///
/// Real per-command height breakdown:
///   - Command 0 under a caption: label(1) + command(1) + optional note(0–1) = 2–3 rows
///   - Command i>0 (or command 0 without caption): blank(1) + label(1) + command(1) + optional note(0–1) = 3–4 rows
const GUIDED_COMMAND_MIN_HEIGHT: u16 = 2;

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
        } else if has_guided_commands
            && matches!(
                kind,
                WizardStepKind::AndroidTools | WizardStepKind::Prerequisites
            )
        {
            // AndroidTools gated (JDK missing) or Prerequisites with guided commands —
            // the guided-command section is the primary CTA; skip the "later phase" hint.
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

    /// Compute the total row height needed to render the entire guided-command section
    /// for `commands` and `step_kind`, without any clamping.
    ///
    /// Accounts for:
    /// - Section header: `GUIDED_SECTION_HEADER_HEIGHT` rows (1)
    /// - Optional per-step caption (AndroidTools / Prerequisites): `JDK_CAPTION_HEIGHT` rows (1)
    /// - Per-command blocks: label(1) + command(1) + optional note(0–1) + optional leading blank
    ///   (skipped for command 0 when a caption was rendered, i.e. `has_caption` is true).
    ///
    /// The caller is responsible for clamping this value to `content_area.height` before
    /// using it as a layout reservation — the saturating clamp in [`Widget::render`] ensures
    /// the resulting `Rect` never exceeds the content area.
    fn guided_section_full_height(commands: &[GuidedCommand], step_kind: WizardStepKind) -> u16 {
        if commands.is_empty() {
            return 0;
        }
        let has_caption = matches!(
            step_kind,
            WizardStepKind::AndroidTools | WizardStepKind::Prerequisites
        );
        let caption_rows: u16 = if has_caption { JDK_CAPTION_HEIGHT } else { 0 };

        let mut cmd_rows: u16 = 0;
        for (i, cmd) in commands.iter().enumerate() {
            // Leading blank row: skipped for command 0 when a caption is present.
            let needs_blank = i > 0 || !has_caption;
            if needs_blank {
                cmd_rows += 1;
            }
            // Label row (always 1)
            cmd_rows += 1;
            // Command row (always 1; [c] copy hint is inline, not a separate row)
            cmd_rows += 1;
            // Optional note row
            if cmd.note.is_some() {
                cmd_rows += 1;
            }
        }

        GUIDED_SECTION_HEADER_HEIGHT + caption_rows + cmd_rows
    }

    /// Render the guided-command section for a step that has guided commands.
    ///
    /// Layout (each row occupies one character-cell row):
    /// ```text
    ///   Guided steps (run these yourself, then press 'r' to re-check):
    ///   [caption — AndroidTools: "JDK 17 required …"; Prerequisites: "Install the OS build tools …"]
    ///
    ///     Install JDK 17
    ///       $ sudo apt install openjdk-17-jdk       [c] copy
    ///       or: sudo dnf install java-17-openjdk-devel
    /// ```
    ///
    /// The `[c] copy` hint is rendered **inline on the command row**, not as a separate row.
    /// The leading blank row is **conditional**: skipped for command 0 when a caption was
    /// rendered above (the caption provides visual separation from the header).
    ///
    /// The `[c] copy` hint and selection highlight follow `selected_command_index`
    /// so the visually-selected command is the one `c` will copy to the clipboard.
    ///
    /// When `area` is too small to render all commands, commands that would fall
    /// outside `area` are clipped by the `y < area.y + area.height` bounds guards.
    /// The caller must size `area` large enough (via [`guided_section_full_height`])
    /// to avoid clipping visible commands.
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

        let selected_idx = self.state.selected_command_index;
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

        // Per-step caption rendered above the command list.
        let caption_text = match step_kind {
            WizardStepKind::AndroidTools => {
                Some("  JDK 17 required before installing Android tools")
            }
            WizardStepKind::Prerequisites => {
                Some("  Install the OS build tools below, then press r to re-check")
            }
            _ => None,
        };
        if let Some(caption) = caption_text {
            if y < area.y + area.height {
                let caption_style = Style::default()
                    .fg(palette::STATUS_YELLOW)
                    .add_modifier(Modifier::BOLD);
                let caption_line = Line::from(Span::styled(caption, caption_style));
                Paragraph::new(caption_line).render(Rect::new(area.x, y, area.width, 1), buf);
                y += JDK_CAPTION_HEIGHT;
            }
        }

        // Whether the first command block needs a blank row before it.
        // When a caption was rendered there is already visual separation, so we
        // skip the leading blank for that case to avoid double-spacing.
        let has_caption = caption_text.is_some();

        for (i, cmd) in commands.iter().enumerate() {
            // Blank separator before each command block.
            // Skip the leading blank when a caption was rendered above (the
            // caption already provides visual separation from the header).
            let needs_blank = i > 0 || !has_caption;
            if needs_blank && y < area.y + area.height {
                y += 1; // blank row
            }

            let is_selected = i == selected_idx;

            // Label row: "    Install JDK 17"
            // Selected entries are rendered in the accent colour for emphasis.
            if y < area.y + area.height {
                let label_style = if is_selected {
                    Style::default()
                        .fg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(palette::TEXT_BRIGHT)
                        .add_modifier(Modifier::BOLD)
                };
                let label_line =
                    Line::from(Span::styled(format!("    {}", cmd.label), label_style));
                Paragraph::new(label_line).render(Rect::new(area.x, y, area.width, 1), buf);
                y += 1;
            }

            // Command row: "      $ <command>        [c] copy"
            // The copy hint and highlight follow `selected_command_index`.
            if y < area.y + area.height {
                let cmd_style = Style::default().fg(palette::ACCENT);
                let copy_style = Style::default().fg(palette::TEXT_MUTED);

                let copy_hint = if is_selected { "  [c] copy" } else { "" };
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
            // Compute the exact number of rows required to render all guided commands
            // (header + caption + Σ per-command blocks), then clamp to content_area.height
            // so the reservation never exceeds the available space.
            //
            // This ensures that when there are multiple guided commands (e.g. macOS
            // Prerequisites: CLT + CocoaPods + Rosetta), all commands fit in the
            // bottom section rather than only the first command being visible.
            let full_height = Self::guided_section_full_height(&step.guided_commands, step.kind);
            full_height.min(content_area.height)
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
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
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
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
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
    fn test_step_detail_shows_guided_block_for_prerequisites_step_with_commands() {
        // Prerequisites step with a Linux guided command (Prerequisites component Missing).
        // After task 05 the guided block replaces the "later phase" hint.
        let state = make_state_prerequisites_linux_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Guided steps"),
            "Prerequisites step with guided commands should show the guided-steps header: '{content}'"
        );
        assert!(
            content.contains("Install the OS build tools"),
            "Prerequisites step should show its caption: '{content}'"
        );
        assert!(
            content.contains("copy"),
            "Prerequisites step should show [c] copy affordance: '{content}'"
        );
        // The "later phase" hint must NOT appear when guided commands are present.
        assert!(
            !content.contains("later phase"),
            "Prerequisites step with guided commands must NOT show 'later phase': '{content}'"
        );
    }

    #[test]
    fn test_step_detail_shows_later_phase_for_prerequisites_step_with_no_commands() {
        // Prerequisites step with no guided commands (all Ok) still shows "later phase".
        let mut state = make_state_components();
        state.selected_index = 0; // Prerequisites step (no guided commands in this report)
        let pane = StepDetailPane::new(&state, true, 0);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("later phase"),
            "Prerequisites step with no guided commands should still show 'Available in a later phase': '{content}'"
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

    /// Build an `InstallWizardState` with the Prerequisites step selected and a
    /// Linux guided command present (simulates Prerequisites missing scenario).
    fn make_state_prerequisites_linux_missing() -> InstallWizardState {
        InstallWizardState {
            visible: true,
            steps: vec![WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: fdemon_app::install_wizard::StepStatus::Missing,
                components: vec![ComponentCheck {
                    kind: ComponentKind::Prerequisites,
                    status: ComponentStatus::Missing,
                    detail: "missing: curl, git".to_string(),
                }],
                guided_commands: vec![GuidedCommand {
                    label: "Install Linux prerequisites (apt)".to_string(),
                    command: "sudo apt-get install -y curl git unzip".to_string(),
                    note: Some("or: sudo dnf install -y curl git unzip".to_string()),
                }],
            }],
            selected_index: 0,
            ..InstallWizardState::default()
        }
    }

    /// Build an `InstallWizardState` with the Prerequisites step selected and three
    /// guided commands (macOS: CLT + CocoaPods + Rosetta all missing).
    ///
    /// The step intentionally has no component checks so the guided commands
    /// occupy the full content area (no bottom-section height clipping).
    /// This allows the test to verify all three command rows render.
    fn make_state_prerequisites_macos_three_commands() -> InstallWizardState {
        InstallWizardState {
            visible: true,
            steps: vec![WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: fdemon_app::install_wizard::StepStatus::Missing,
                // No component checks — guided commands fill the full content area.
                components: vec![],
                guided_commands: vec![
                    GuidedCommand {
                        label: "Install Xcode Command Line Tools".to_string(),
                        command: "xcode-select --install".to_string(),
                        note: Some("Opens a GUI dialog to install CLT.".to_string()),
                    },
                    GuidedCommand {
                        label: "Install CocoaPods".to_string(),
                        command: "brew install cocoapods".to_string(),
                        note: Some("or: sudo gem install cocoapods".to_string()),
                    },
                    GuidedCommand {
                        label: "Install Rosetta 2".to_string(),
                        command: "sudo softwareupdate --install-rosetta --agree-to-license"
                            .to_string(),
                        note: None,
                    },
                ],
            }],
            selected_index: 0,
            ..InstallWizardState::default()
        }
    }

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

    // --- Phase 5: Prerequisites detail-pane caption + index-aware copy hint ---

    #[test]
    fn test_prerequisites_caption_renders() {
        // Linux single-command Prerequisites step → caption + command visible.
        let state = make_state_prerequisites_linux_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Install the OS build tools"),
            "Prerequisites caption must render: '{content}'"
        );
        assert!(
            content.contains("apt-get"),
            "Prerequisites Linux command must render: '{content}'"
        );
        assert!(
            content.contains("copy"),
            "Prerequisites [c] copy affordance must render: '{content}'"
        );
    }

    #[test]
    fn test_prerequisites_macos_three_commands_render() {
        // macOS three-command Prerequisites step → all three commands visible.
        let state = make_state_prerequisites_macos_three_commands();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("xcode-select"),
            "CLT command must render: '{content}'"
        );
        assert!(
            content.contains("cocoapods"),
            "CocoaPods command must render: '{content}'"
        );
        assert!(
            content.contains("rosetta"),
            "Rosetta command must render: '{content}'"
        );
    }

    #[test]
    fn test_copy_hint_follows_selected_command_index() {
        // With three commands and selected_command_index=1, command 1 (CocoaPods)
        // gets [c] copy; command 0 (CLT) and command 2 (Rosetta) do NOT.
        // Use a tall enough area to ensure all three command rows render.
        let mut state = make_state_prerequisites_macos_three_commands();
        state.selected_command_index = 1; // CocoaPods selected
        let pane = StepDetailPane::new(&state, true, 0);
        // Use a full-width, 50-row area so all three commands fit in the bottom section.
        let area = Rect::new(0, 0, 100, 50);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        // Collect all rows so we can check which one contains "copy".
        let rows: Vec<String> = (0..area.height)
            .map(|row| {
                (0..area.width)
                    .map(|col| buf.cell((col, row)).map(|c| c.symbol()).unwrap_or(" "))
                    .collect()
            })
            .collect();

        // The copy hint must appear somewhere.
        let copy_row = rows
            .iter()
            .position(|r| r.contains("copy"))
            .expect("a row with 'copy' must exist in the rendered output");

        // The CocoaPods command must appear.
        let cocoapods_cmd_row = rows
            .iter()
            .position(|r| r.contains("cocoapods"))
            .expect("cocoapods command row must be visible");

        // The copy hint row must be at or after the CocoaPods command row
        // (command row is directly below the label row).
        assert!(
            copy_row >= cocoapods_cmd_row,
            "copy hint (row {copy_row}) must be on or after the cocoapods command (row {cocoapods_cmd_row})"
        );

        // Confirm xcode-select renders above the copy hint — it belongs to
        // command 0 (not selected) which must NOT carry the copy hint.
        if let Some(xcode_row) = rows.iter().position(|r| r.contains("xcode-select")) {
            assert!(
                xcode_row < copy_row,
                "xcode-select (row {xcode_row}) must be above the copy hint row ({copy_row})"
            );
        }
    }

    #[test]
    fn test_copy_hint_index_zero_stays_on_first_command() {
        // With selected_command_index=0 (default), first command gets [c] copy.
        let state = make_state_prerequisites_linux_missing();
        assert_eq!(state.selected_command_index, 0);
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // Single-command step → copy hint must appear
        assert!(
            content.contains("copy"),
            "single-command step at index 0 must show [c] copy: '{content}'"
        );
    }

    #[test]
    fn test_prerequisites_no_later_phase_hint_when_has_commands() {
        // When Prerequisites has guided commands, the "later phase" hint must not appear.
        let state = make_state_prerequisites_linux_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            !content.contains("later phase"),
            "Prerequisites with guided commands must NOT show 'later phase': '{content}'"
        );
    }

    #[test]
    fn test_no_panic_prerequisites_guided_tiny_area() {
        let state = make_state_prerequisites_linux_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic even in tight space
    }

    // --- Phase 4 followup: M1 regression tests ---

    /// Build an `InstallWizardState` with the Prerequisites step selected, a
    /// `Prerequisites` component present (non-empty `components`), and three
    /// guided commands (macOS: CLT + CocoaPods + Rosetta all missing).
    ///
    /// This is the production macOS path: `check_prerequisites()` always returns
    /// a `Prerequisites` `ComponentCheck`, so `components` is non-empty while
    /// `prerequisites_guided_commands()` returns 3 commands.  The combination
    /// previously triggered the M1 clipping bug because `bottom_section_height`
    /// was fixed at 6 rows (header + caption + GUIDED_COMMAND_MIN_HEIGHT) regardless
    /// of how many commands were present.
    fn make_state_prerequisites_macos_three_commands_with_component() -> InstallWizardState {
        InstallWizardState {
            visible: true,
            steps: vec![WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: fdemon_app::install_wizard::StepStatus::Missing,
                // Non-empty components — this is the key difference from the original
                // `make_state_prerequisites_macos_three_commands` helper which had
                // `components: vec![]`.  With components present the layout takes the
                // split path where the guided section is reserved at the bottom of the
                // content area and is capped to `bottom_section_height` rows.
                components: vec![ComponentCheck {
                    kind: ComponentKind::Prerequisites,
                    status: ComponentStatus::Missing,
                    detail: "missing: xcode-clt, cocoapods, rosetta".to_string(),
                }],
                guided_commands: vec![
                    GuidedCommand {
                        label: "Install Xcode Command Line Tools".to_string(),
                        command: "xcode-select --install".to_string(),
                        note: Some("Opens a GUI dialog to install CLT.".to_string()),
                    },
                    GuidedCommand {
                        label: "Install CocoaPods".to_string(),
                        command: "brew install cocoapods".to_string(),
                        note: Some("or: sudo gem install cocoapods".to_string()),
                    },
                    GuidedCommand {
                        label: "Install Rosetta 2".to_string(),
                        command: "sudo softwareupdate --install-rosetta --agree-to-license"
                            .to_string(),
                        note: None,
                    },
                ],
            }],
            selected_index: 0,
            ..InstallWizardState::default()
        }
    }

    /// Regression test for M1: Prerequisites step with components non-empty and 3
    /// guided commands — all three commands must render without clipping.
    ///
    /// Previously `bottom_section_height` was fixed at 6 rows (header + caption +
    /// GUIDED_COMMAND_MIN_HEIGHT), so only command 0 could render.  After the fix,
    /// `guided_section_full_height` computes the exact height needed and the bottom
    /// section is sized accordingly.
    #[test]
    fn test_prerequisites_three_commands_with_component_no_clipping() {
        let state = make_state_prerequisites_macos_three_commands_with_component();
        let pane = StepDetailPane::new(&state, true, 0);
        // Use a 30-row terminal — large enough to show components + all guided commands.
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("xcode-select"),
            "CLT command (command 0) must render: '{content}'"
        );
        assert!(
            content.contains("cocoapods"),
            "CocoaPods command (command 1) must render: '{content}'"
        );
        assert!(
            content.contains("softwareupdate"),
            "Rosetta command (command 2) must render: '{content}'"
        );
    }

    /// Regression test for M1: with `selected_command_index = 2` the selected
    /// command's row, its selection highlight, and its `[c] copy` hint must be
    /// visible when components are non-empty.
    ///
    /// `c` must copy a command that is currently visible (no visible/copied
    /// divergence).
    #[test]
    fn test_prerequisites_selected_command_index_2_visible_with_component() {
        let mut state = make_state_prerequisites_macos_three_commands_with_component();
        state.selected_command_index = 2; // Rosetta — the third (last) command
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // The selected command's text must be visible.
        assert!(
            content.contains("softwareupdate"),
            "Rosetta command (selected_command_index=2) must be visible: '{content}'"
        );
        // The [c] copy hint must be visible (it appears on the command row of the
        // selected command).
        assert!(
            content.contains("copy"),
            "[c] copy hint must be visible when selected_command_index=2: '{content}'"
        );

        // Collect rows to verify the copy hint appears on/after the Rosetta command row.
        let rows: Vec<String> = (0..area.height)
            .map(|row| {
                (0..area.width)
                    .map(|col| buf.cell((col, row)).map(|c| c.symbol()).unwrap_or(" "))
                    .collect()
            })
            .collect();

        let rosetta_row = rows
            .iter()
            .position(|r| r.contains("softwareupdate"))
            .expect("softwareupdate row must be visible");
        let copy_row = rows
            .iter()
            .position(|r| r.contains("copy"))
            .expect("copy hint row must be visible");

        // The copy hint must be on the same row as the selected command (inline).
        assert_eq!(
            copy_row, rosetta_row,
            "copy hint (row {copy_row}) must be on the same row as the Rosetta command (row {rosetta_row})"
        );
    }

    /// Regression test: single-command AndroidTools / Prerequisites path is visually
    /// unchanged after the fix.  The bottom section height should be the same as
    /// before for a single command with a note.
    #[test]
    fn test_single_command_android_tools_visually_unchanged() {
        let state = make_state_android_jdk_missing();
        let pane = StepDetailPane::new(&state, true, 0);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Install JDK 17"),
            "single-command AndroidTools must still render label: '{content}'"
        );
        assert!(
            content.contains("openjdk-17-jdk"),
            "single-command AndroidTools must still render command: '{content}'"
        );
        assert!(
            content.contains("copy"),
            "single-command AndroidTools must still show [c] copy: '{content}'"
        );
        assert!(
            content.contains("sudo dnf"),
            "single-command AndroidTools must still render note: '{content}'"
        );
    }

    /// Regression test: no out-of-bounds Rect on a small terminal with the new
    /// dynamic height calculation.  Must not panic even when the content area is
    /// smaller than the full guided-section height.
    #[test]
    fn test_no_panic_small_terminal_with_component_and_multiple_commands() {
        let state = make_state_prerequisites_macos_three_commands_with_component();
        let pane = StepDetailPane::new(&state, true, 0);
        // Very small terminal — the guided section will be clamped.
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    /// Unit test for `guided_section_full_height` helper: 3-command Prerequisites
    /// with notes on first two commands.
    ///
    /// Expected breakdown:
    ///   - header: 1
    ///   - caption (Prerequisites): 1
    ///   - cmd 0 (has_caption=true, no blank): label(1) + cmd(1) + note(1) = 3
    ///   - cmd 1 (blank=1): blank(1) + label(1) + cmd(1) + note(1) = 4
    ///   - cmd 2 (blank=1, no note): blank(1) + label(1) + cmd(1) = 3
    ///   Total = 1 + 1 + 3 + 4 + 3 = 12
    #[test]
    fn test_guided_section_full_height_three_commands() {
        let commands = vec![
            GuidedCommand {
                label: "A".to_string(),
                command: "cmd_a".to_string(),
                note: Some("note_a".to_string()),
            },
            GuidedCommand {
                label: "B".to_string(),
                command: "cmd_b".to_string(),
                note: Some("note_b".to_string()),
            },
            GuidedCommand {
                label: "C".to_string(),
                command: "cmd_c".to_string(),
                note: None,
            },
        ];
        let height =
            StepDetailPane::guided_section_full_height(&commands, WizardStepKind::Prerequisites);
        assert_eq!(
            height, 12,
            "3-command Prerequisites section should need 12 rows"
        );
    }

    /// Unit test for `guided_section_full_height` helper: single command with note,
    /// AndroidTools (has caption).
    ///
    /// Expected breakdown:
    ///   - header: 1
    ///   - caption (AndroidTools): 1
    ///   - cmd 0 (has_caption=true, no blank): label(1) + cmd(1) + note(1) = 3
    ///   Total = 5
    #[test]
    fn test_guided_section_full_height_single_command_with_note() {
        let commands = vec![GuidedCommand {
            label: "Install JDK 17".to_string(),
            command: "sudo apt install openjdk-17-jdk".to_string(),
            note: Some("or: sudo dnf install java-17-openjdk-devel".to_string()),
        }];
        let height =
            StepDetailPane::guided_section_full_height(&commands, WizardStepKind::AndroidTools);
        assert_eq!(
            height, 5,
            "single-command AndroidTools section with note should need 5 rows"
        );
    }

    /// Unit test for `guided_section_full_height` helper: empty command list returns 0.
    #[test]
    fn test_guided_section_full_height_empty() {
        let commands: Vec<GuidedCommand> = vec![];
        let height =
            StepDetailPane::guided_section_full_height(&commands, WizardStepKind::Prerequisites);
        assert_eq!(height, 0, "empty command list should return 0");
    }
}
