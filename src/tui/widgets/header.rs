//! Header bar widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Header widget displaying app title and shortcuts
pub struct Header;

impl Header {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Header {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);
        let key = Style::default().fg(Color::Yellow);

        let content = Line::from(vec![
            Span::styled(" Flutter Demon 😈", title),
            Span::raw("   "),
            Span::styled("[", dim),
            Span::styled("r", key),
            Span::styled("] Reload  ", dim),
            Span::styled("[", dim),
            Span::styled("R", key),
            Span::styled("] Restart  ", dim),
            Span::styled("[", dim),
            Span::styled("q", key),
            Span::styled("] Quit", dim),
        ]);

        Paragraph::new(content)
            .block(Block::default().borders(Borders::BOTTOM))
            .render(area, buf);
    }
}
