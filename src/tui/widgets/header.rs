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

use crate::app::session_manager::SessionManager;

use super::SessionTabs;

/// Main header showing app title, project name, and keybindings
/// with optional session tabs rendered inside the bordered area
pub struct MainHeader<'a> {
    project_name: Option<&'a str>,
    session_manager: Option<&'a SessionManager>,
}

impl<'a> MainHeader<'a> {
    pub fn new(project_name: Option<&'a str>) -> Self {
        Self {
            project_name,
            session_manager: None,
        }
    }

    /// Add session manager to render tabs inside the header
    pub fn with_sessions(mut self, session_manager: &'a SessionManager) -> Self {
        self.session_manager = Some(session_manager);
        self
    }
}

impl Widget for MainHeader<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render border
        Block::default().borders(Borders::ALL).render(area, buf);

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

        // Build left content (title + project name)
        let left_content = Line::from(vec![
            Span::styled(" Flutter Demon", title),
            Span::styled("  │  ", dim),
            Span::styled(project_name, project),
        ]);

        // Render title/project on the top border line (y = area.y)
        buf.set_line(area.x, area.y, &left_content, area.width);

        // Render right-aligned keybindings on the top border line
        if area.width > keybindings_width + 2 {
            let x = area.x + area.width - keybindings_width - 1;
            let right_content = Line::from(keybindings);
            buf.set_line(x, area.y, &right_content, keybindings_width);
        }

        // Render session tabs inside the bordered area (if we have sessions)
        if let Some(session_manager) = self.session_manager {
            if !session_manager.is_empty() {
                // Content area is inside the border (y + 1, with padding)
                let tabs_area = Rect {
                    x: area.x + 1,
                    y: area.y + 1,
                    width: area.width.saturating_sub(2),
                    height: area.height.saturating_sub(2),
                };

                if tabs_area.height > 0 && tabs_area.width > 0 {
                    let tabs = SessionTabs::new(session_manager);
                    tabs.render(tabs_area, buf);
                }
            }
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
