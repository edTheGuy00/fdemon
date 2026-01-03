//! Status bar widget

use crate::app::state::AppState;
use crate::core::AppPhase;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

/// Status bar widget showing application state
pub struct StatusBar<'a> {
    state: &'a AppState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    fn phase_display(&self) -> (Span<'static>, Style) {
        match self.state.phase {
            AppPhase::Initializing => (
                Span::raw("○ Initializing"),
                Style::default().fg(Color::Yellow),
            ),
            AppPhase::Running => (Span::raw("● Running"), Style::default().fg(Color::Green)),
            AppPhase::Reloading => (Span::raw("↻ Reloading"), Style::default().fg(Color::Cyan)),
            AppPhase::Quitting => (Span::raw("◌ Quitting"), Style::default().fg(Color::Red)),
        }
    }

    fn scroll_indicator(&self) -> Span<'static> {
        if self.state.log_view_state.auto_scroll {
            Span::styled("⬇ Auto", Style::default().fg(Color::Green))
        } else {
            Span::styled("⬆ Manual", Style::default().fg(Color::Yellow))
        }
    }

    fn log_position(&self) -> String {
        let state = &self.state.log_view_state;
        if state.total_lines == 0 {
            "0/0".to_string()
        } else {
            let current = state.offset + 1;
            let end = (state.offset + state.visible_lines).min(state.total_lines);
            format!("{}-{}/{}", current, end, state.total_lines)
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg_style = Style::default().bg(Color::DarkGray).fg(Color::White);

        let (phase_span, _) = self.phase_display();

        let content = Line::from(vec![
            Span::raw(" "),
            phase_span,
            Span::raw(" │ "),
            self.scroll_indicator(),
            Span::raw(" │ "),
            Span::raw(self.log_position()),
            Span::raw(" │ "),
            Span::styled("[q]", Style::default().fg(Color::Yellow)),
            Span::raw(" Quit "),
            Span::styled("[g/G]", Style::default().fg(Color::Yellow)),
            Span::raw(" Top/Bottom"),
        ]);

        Paragraph::new(content).style(bg_style).render(area, buf);
    }
}
