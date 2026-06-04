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
/// Derived from: 3 header + 1 sep + 5 content + 1 sep + 1 footer + 2 border = 13.
const MIN_RENDER_HEIGHT: u16 = 13;

/// Panel width as a percentage of the terminal width.
///
/// Derived from: 80% provides comfortable margins on typical 80–200 column terminals.
const PANEL_WIDTH_PERCENT: u16 = 80;

/// Panel height as a percentage of the terminal height.
///
/// Derived from: 70% reserves header/footer space while showing all panel content.
const PANEL_HEIGHT_PERCENT: u16 = 70;

/// Width of the left (step list) pane as a percentage of the content area.
///
/// Derived from: step list needs ~35% for comfortable title display at typical widths.
const LEFT_PANE_PERCENT: u16 = 35;

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
        if area.height >= 2 {
            let subtitle = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Flutter toolchain setup",
                    Style::default().fg(palette::TEXT_SECONDARY),
                ),
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
        );
        list_pane.render(chunks[0], buf);

        self.render_separator(chunks[1], buf);

        let detail_pane = step_detail_pane(self.state, self.animation_frame);
        detail_pane.render(chunks[2], buf);
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

        // 7. Layout: header(3) | separator(1) | panes(flex) | separator(1) | footer(1) | absorber(0)
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0), // absorber
        ])
        .split(inner);

        self.render_header(chunks[0], buf);
        self.render_separator(chunks[1], buf);

        // 8. Loading state — show placeholder; skip pane split
        if self.state.loading {
            self.render_loading(chunks[2], buf);
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
        HostShell, InstallWizardState, LinuxPackageManager, ToolchainReport,
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
        let mut state = InstallWizardState::opening();
        state.apply_report(make_report());
        state
    }

    fn loading_state() -> InstallWizardState {
        InstallWizardState::opening()
    }

    fn empty_steps_state() -> InstallWizardState {
        InstallWizardState::default()
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
}
