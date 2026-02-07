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

use fdemon_app::session_manager::SessionManager;

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
    use crate::test_utils::{test_device_with_platform, TestTerminal};

    #[test]
    fn test_header_renders_title() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(None);

        term.render_widget(header, term.area());

        // Should contain app name
        assert!(
            term.buffer_contains("Flutter Demon"),
            "Header should contain app title"
        );
    }

    #[test]
    fn test_header_renders_project_name() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(Some("my_flutter_app"));

        term.render_widget(header, term.area());

        assert!(
            term.buffer_contains("my_flutter_app"),
            "Header should contain project name"
        );
    }

    #[test]
    fn test_header_without_project_name() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(None);

        term.render_widget(header, term.area());

        // Should still render without crashing
        let content = term.content();
        assert!(!content.is_empty(), "Header should render something");
        // Default fallback is "flutter"
        assert!(
            term.buffer_contains("flutter"),
            "Header should use default project name"
        );
    }

    #[test]
    fn test_header_with_sessions() {
        let mut term = TestTerminal::new();
        let mut session_manager = SessionManager::new();

        // Add mock sessions
        session_manager
            .create_session(&test_device_with_platform("device1", "iPhone 15", "ios"))
            .unwrap();
        session_manager
            .create_session(&test_device_with_platform("device2", "Pixel 7", "android"))
            .unwrap();

        let header = MainHeader::new(Some("test_app")).with_sessions(&session_manager);

        term.render_widget(header, term.area());

        // Verify session tabs appear (tabs show device names with status icons)
        assert!(
            term.buffer_contains("iPhone 15"),
            "Header should show first session device name"
        );
        assert!(
            term.buffer_contains("Pixel 7"),
            "Header should show second session device name"
        );
        // Check for status icon (○ for initializing sessions)
        assert!(
            term.buffer_contains("○"),
            "Header should show status icons for sessions"
        );
    }

    #[test]
    fn test_header_truncates_long_project_name() {
        let mut term = TestTerminal::with_size(40, 5); // Narrow terminal
        let long_name = "this_is_a_very_long_flutter_project_name_that_should_truncate";
        let header = MainHeader::new(Some(long_name));

        term.render_widget(header, term.area());

        // Should not overflow - verify no panic and content fits
        let content = term.content();
        assert!(content.len() > 0, "Should render without panic");
        // The header renders the full name but it gets truncated by the terminal width
        // Verify basic rendering worked without panic
        assert!(
            term.buffer_contains("Flutter Demon"),
            "Should still show app title"
        );
    }

    #[test]
    fn test_header_compact_mode() {
        let mut term = TestTerminal::compact();
        let header = MainHeader::new(Some("app"));

        term.render_widget(header, term.area());

        // Should adapt to compact size
        let content = term.content();
        assert!(!content.is_empty(), "Should render in compact mode");
        assert!(
            term.buffer_contains("Flutter Demon"),
            "Should contain title in compact mode"
        );
    }

    #[test]
    fn test_header_with_keybindings() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(Some("test_project"));

        term.render_widget(header, term.area());

        // Verify keybindings are present
        assert!(term.buffer_contains("[r]"), "Should show reload key");
        assert!(term.buffer_contains("[R]"), "Should show restart key");
        assert!(term.buffer_contains("[x]"), "Should show stop key");
        assert!(
            term.buffer_contains("[d]"),
            "Should show device selector key"
        );
        assert!(term.buffer_contains("[q]"), "Should show quit key");
    }

    #[test]
    fn test_header_without_sessions() {
        let mut term = TestTerminal::new();
        let session_manager = SessionManager::new(); // Empty session manager

        let header = MainHeader::new(Some("test_app")).with_sessions(&session_manager);

        term.render_widget(header, term.area());

        // Should render without tabs when no sessions
        let content = term.content();
        assert!(!content.is_empty(), "Should render without sessions");
        assert!(term.buffer_contains("test_app"), "Should show project name");
    }
}
