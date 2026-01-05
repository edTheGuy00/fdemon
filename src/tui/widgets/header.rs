//! Header bar widgets
//!
//! Provides the main header with project name and keybindings.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

/// Main header showing app title, project name, and keybindings
pub struct MainHeader<'a> {
    project_name: Option<&'a str>,
}

impl<'a> MainHeader<'a> {
    pub fn new(project_name: Option<&'a str>) -> Self {
        Self { project_name }
    }
}

impl Widget for MainHeader<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render border
        Block::default().borders(Borders::ALL).render(area, buf);

        let content_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        let title = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);
        let key = Style::default().fg(Color::Yellow);
        let project = Style::default().fg(Color::White);

        let project_name = self.project_name.unwrap_or("flutter");

        // Build keybindings
        let keybindings = vec![
            Span::styled("[", dim),
            Span::styled("r", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("R", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("x", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("d", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("q", key),
            Span::styled("]", dim),
        ];

        let keybindings_width: u16 = 23; // "[r] [R] [x] [d] [q]"

        // Build left content
        let left_content = Line::from(vec![
            Span::styled(" Flutter Demon", title),
            Span::styled("  │  ", dim),
            Span::styled(project_name, project),
        ]);

        // Render left-aligned content
        buf.set_line(
            content_area.x,
            content_area.y,
            &left_content,
            content_area.width,
        );

        // Render right-aligned keybindings
        if content_area.width > keybindings_width + 2 {
            let x = content_area.x + content_area.width - keybindings_width - 1;
            let right_content = Line::from(keybindings);
            buf.set_line(x, content_area.y, &right_content, keybindings_width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_main_header_with_project_name() {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let header = MainHeader::new(Some("my_cool_app"));
                f.render_widget(header, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Flutter Demon"));
        assert!(content.contains("my_cool_app"));
        assert!(content.contains("[r]"));
        assert!(content.contains("[d]"));
        assert!(content.contains("[q]"));
    }

    #[test]
    fn test_main_header_without_project_name() {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let header = MainHeader::new(None);
                f.render_widget(header, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Flutter Demon"));
        assert!(content.contains("flutter")); // Default fallback
    }

    #[test]
    fn test_main_header_narrow_terminal() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let header = MainHeader::new(Some("my_app"));
                f.render_widget(header, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should still contain the title
        assert!(content.contains("Flutter Demon"));
    }
}
