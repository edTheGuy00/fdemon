//! # Step List Pane
//!
//! Left pane of the Install Wizard.
//! Renders an ordered list of [`WizardStep`] items, each with a status glyph
//! and title.  The currently selected step is highlighted.
//!
//! ## Status Glyphs
//!
//! | `StepStatus` | Glyph | Color  |
//! |--------------|-------|--------|
//! | `Ok`         | `✓`   | green  |
//! | `Partial`    | `!`   | yellow |
//! | `Missing`    | `✗`   | red    |
//! | `Pending`    | `…`   | dim    |

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use fdemon_app::install_wizard::{StepStatus, WizardPane, WizardStep};

use crate::theme::palette;

/// Glyph for `StepStatus::Ok`.
const GLYPH_OK: &str = "✓";
/// Glyph for `StepStatus::Partial`.
const GLYPH_PARTIAL: &str = "!";
/// Glyph for `StepStatus::Missing`.
const GLYPH_MISSING: &str = "✗";
/// Glyph for `StepStatus::Pending`.
const GLYPH_PENDING: &str = "…";

/// Height of the pane title header (label + separator).
///
/// Derived from: 1 title row + 1 separator row = 2 rows.
const HEADER_HEIGHT: u16 = 2;

/// Left pane — ordered step list.
pub struct StepListPane<'a> {
    steps: &'a [WizardStep],
    selected_index: usize,
    focused: bool,
}

impl<'a> StepListPane<'a> {
    /// Create a new step list pane.
    ///
    /// # Arguments
    /// * `steps`          – Ordered list of wizard steps
    /// * `selected_index` – Currently selected step index
    /// * `focused`        – Whether this pane has keyboard focus
    pub fn new(steps: &'a [WizardStep], selected_index: usize, focused: bool) -> Self {
        Self {
            steps,
            selected_index,
            focused,
        }
    }

    /// Return the glyph character for the given step status.
    fn status_glyph(status: StepStatus) -> &'static str {
        match status {
            StepStatus::Ok => GLYPH_OK,
            StepStatus::Partial => GLYPH_PARTIAL,
            StepStatus::Missing => GLYPH_MISSING,
            StepStatus::Pending => GLYPH_PENDING,
        }
    }

    /// Return the color for the given step status.
    fn status_color(status: StepStatus) -> ratatui::style::Color {
        match status {
            StepStatus::Ok => palette::STATUS_GREEN,
            StepStatus::Partial => palette::STATUS_YELLOW,
            StepStatus::Missing => palette::STATUS_RED,
            StepStatus::Pending => palette::TEXT_MUTED,
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

        let label = Line::from(vec![
            Span::raw("  "),
            Span::styled("Setup Steps", title_style),
        ]);
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

    /// Render a single step row at absolute `y` position.
    fn render_step_row(
        &self,
        step: &WizardStep,
        index: usize,
        y: u16,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let is_selected = index == self.selected_index;
        let glyph = Self::status_glyph(step.status);
        let glyph_color = Self::status_color(step.status);

        let row_style = if is_selected && self.focused {
            Style::default()
                .fg(palette::CONTRAST_FG)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .bg(palette::SELECTED_ROW_BG)
        } else {
            Style::default().fg(palette::TEXT_PRIMARY)
        };

        let glyph_style = if is_selected && self.focused {
            // Use row style for glyph too when fully selected+focused
            Style::default()
                .fg(palette::CONTRAST_FG)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(glyph_color)
                .bg(palette::SELECTED_ROW_BG)
        } else {
            Style::default().fg(glyph_color)
        };

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(glyph, glyph_style),
            Span::raw(" "),
            Span::styled(step.title.as_str(), row_style),
        ]);

        let row_area = Rect::new(area.x, y, area.width, 1);
        Paragraph::new(line).render(row_area, buf);

        // Fill rest of row with row background to avoid stray characters
        if is_selected {
            let text_len = (2 + glyph.chars().count() + 1 + step.title.chars().count()) as u16;
            if text_len < area.width {
                let padding = " ".repeat((area.width - text_len) as usize);
                buf.set_string(area.x + text_len, y, &padding, row_style);
            }
        }
    }
}

impl Widget for StepListPane<'_> {
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

        if self.steps.is_empty() {
            let msg = Line::from(Span::styled(
                "  No steps available.",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(msg).render(
                Rect::new(content_area.x, content_area.y, content_area.width, 1),
                buf,
            );
            return;
        }

        let visible_height = content_area.height as usize;
        for (i, step) in self.steps.iter().take(visible_height).enumerate() {
            let y = content_area.y + i as u16;
            self.render_step_row(step, i, y, content_area, buf);
        }
    }
}

/// Construct a [`StepListPane`] from the install wizard state fields.
///
/// This is a convenience constructor used by [`super::InstallWizardPanel`].
pub fn step_list_pane<'a>(
    steps: &'a [WizardStep],
    selected_index: usize,
    focused_pane: WizardPane,
) -> StepListPane<'a> {
    StepListPane::new(steps, selected_index, focused_pane == WizardPane::StepList)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::install_wizard::{StepStatus, WizardStepKind};
    use ratatui::{buffer::Buffer, layout::Rect};

    fn make_steps() -> Vec<WizardStep> {
        vec![
            WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: StepStatus::Ok,
                components: vec![],
                guided_commands: vec![],
            },
            WizardStep {
                kind: WizardStepKind::AndroidTools,
                title: "Android Tools".to_string(),
                status: StepStatus::Partial,
                components: vec![],
                guided_commands: vec![],
            },
            WizardStep {
                kind: WizardStepKind::FlutterSdk,
                title: "Flutter SDK".to_string(),
                status: StepStatus::Missing,
                components: vec![],
                guided_commands: vec![],
            },
            WizardStep {
                kind: WizardStepKind::Doctor,
                title: "Flutter Doctor".to_string(),
                status: StepStatus::Pending,
                components: vec![],
                guided_commands: vec![],
            },
        ]
    }

    fn make_area() -> Rect {
        Rect::new(0, 0, 40, 20)
    }

    #[test]
    fn test_renders_step_list_with_status_glyphs() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains('✓'), "Ok step should show checkmark glyph");
        assert!(content.contains('!'), "Partial step should show ! glyph");
        assert!(content.contains('✗'), "Missing step should show ✗ glyph");
        assert!(content.contains('…'), "Pending step should show … glyph");
    }

    #[test]
    fn test_renders_step_titles() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Prerequisites"),
            "should show first step title"
        );
        assert!(
            content.contains("Android Tools"),
            "should show android tools title"
        );
        assert!(
            content.contains("Flutter SDK"),
            "should show flutter sdk title"
        );
        assert!(
            content.contains("Flutter Doctor"),
            "should show doctor title"
        );
    }

    #[test]
    fn test_selected_step_highlighted() {
        let steps = make_steps();
        // Select step 1 (index 1)
        let pane = StepListPane::new(&steps, 1, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Row at y=3 (header=2 + index=1) should have accent background
        // We verify the row contains the selected step title
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Android Tools"),
            "selected step title should appear"
        );
        // Check the cell at row y=3 for the selected background
        // Step index 1 = row y = header(2) + 1 = y=3
        let cell = &buf[(2, 3)];
        assert_eq!(
            cell.bg,
            palette::ACCENT,
            "selected+focused row should have accent background"
        );
    }

    #[test]
    fn test_unfocused_selected_uses_subtle_highlight() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, false); // not focused
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Step 0 is selected but pane is unfocused — should use SELECTED_ROW_BG
        // Row y = header(2) + 0 = 2
        let cell = &buf[(2, 2)];
        assert_eq!(
            cell.bg,
            palette::SELECTED_ROW_BG,
            "selected but unfocused row should use subtle highlight"
        );
    }

    #[test]
    fn test_renders_without_panic_empty_steps() {
        let steps: Vec<WizardStep> = vec![];
        let pane = StepListPane::new(&steps, 0, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_renders_without_panic_tiny_area() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_header_shows_in_focused_state() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Setup Steps"),
            "header label should appear"
        );
    }
}
