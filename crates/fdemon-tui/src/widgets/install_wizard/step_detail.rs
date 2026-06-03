//! # Step Detail Pane
//!
//! Right pane of the Install Wizard.
//! Renders the detail for the currently selected [`WizardStep`]:
//!
//! - **Doctor step**: delegates to [`DoctorView`].
//! - **Other steps**: renders each [`ComponentCheck`] as a single row:
//!   `<glyph> <kind label>: <detail>`, colored by [`ComponentStatus`].
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
    ComponentCheck, ComponentStatus, InstallWizardState, WizardPane, WizardStepKind,
};

use crate::theme::palette;

use super::doctor_view::DoctorView;

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

/// Right pane — per-step detail renderer.
pub struct StepDetailPane<'a> {
    state: &'a InstallWizardState,
    focused: bool,
}

impl<'a> StepDetailPane<'a> {
    /// Create a new step detail pane.
    ///
    /// # Arguments
    /// * `state`   – Full install wizard state (read-only)
    /// * `focused` – Whether this pane has keyboard focus
    pub fn new(state: &'a InstallWizardState, focused: bool) -> Self {
        Self { state, focused }
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

        // Other steps: render component checks
        if step.components.is_empty() {
            // Informational step with no component checks (e.g., PathConfig)
            let msg = Line::from(Span::styled(
                "  No component checks for this step.",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(msg).render(
                Rect::new(content_area.x, content_area.y, content_area.width, 1),
                buf,
            );
            return;
        }

        // Scroll clamp (render-time safety net)
        let corrected_scroll = compute_corrected_scroll(
            self.state.detail_scroll,
            visible_height,
            step.components.len(),
        );

        let start = corrected_scroll;
        let end = (start + visible_height).min(step.components.len());

        for (i, check) in step.components[start..end].iter().enumerate() {
            let y = content_area.y + i as u16;
            Self::render_component_row(check, y, content_area, buf);
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
pub fn step_detail_pane(state: &InstallWizardState) -> StepDetailPane<'_> {
    StepDetailPane::new(state, state.focused_pane == WizardPane::Detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::install_wizard::{InstallWizardState, WizardStep, WizardStepKind};
    use fdemon_daemon::toolchain::{
        ComponentCheck, ComponentKind, ComponentStatus, DoctorLine, DoctorMarker, HostPlatform,
        HostShell, ToolchainReport,
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
        let pane = StepDetailPane::new(&state, true);
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
        let pane = StepDetailPane::new(&state, true);
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
        let pane = StepDetailPane::new(&state, true);
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
        let pane = StepDetailPane::new(&state, true);
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
    fn test_empty_step_shows_no_components_message() {
        let mut state = make_state_with_doctor_step_selected();
        // Override selected_index to PathConfig (index 2) which has no components
        state.selected_index = 2;
        let pane = StepDetailPane::new(&state, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("No component"),
            "informational step should show 'no components' message"
        );
    }

    #[test]
    fn test_no_panic_loading_state() {
        let state = InstallWizardState::opening(); // loading=true, steps empty
        let pane = StepDetailPane::new(&state, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_panic_tiny_area() {
        let state = make_state_with_doctor_step_selected();
        let pane = StepDetailPane::new(&state, true);
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

        let pane = StepDetailPane::new(&state, false);
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
        }];
        state.selected_index = 0;

        let pane = StepDetailPane::new(&state, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("✗"),
            "Missing component should render cross glyph"
        );
    }
}
