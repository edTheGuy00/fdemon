//! Session tabs widget for multi-instance display
//!
//! Provides tab navigation for multiple running Flutter sessions.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Widget},
};

use crate::app::session_manager::SessionManager;
use crate::core::AppPhase;

/// Widget displaying session tabs in a standalone subheader row
pub struct SessionTabs<'a> {
    session_manager: &'a SessionManager,
}

impl<'a> SessionTabs<'a> {
    pub fn new(session_manager: &'a SessionManager) -> Self {
        Self { session_manager }
    }

    /// Create tab titles from sessions
    fn tab_titles(&self) -> Vec<Line<'static>> {
        self.session_manager
            .iter()
            .map(|handle| {
                let session = &handle.session;

                // Status icon with color
                let (icon, _icon_color) = match session.phase {
                    AppPhase::Running => ("●", Color::Green),
                    AppPhase::Reloading => ("↻", Color::Yellow),
                    AppPhase::Initializing => ("○", Color::DarkGray),
                    AppPhase::Stopped => ("○", Color::DarkGray),
                    AppPhase::Quitting => ("✗", Color::Red),
                };

                // Truncate device name if too long
                let name = truncate_name(&session.device_name, 12);

                Line::from(format!(" {} {} ", icon, name))
            })
            .collect()
    }
}

impl<'a> SessionTabs<'a> {
    /// Render a simplified single-session header showing device name with status icon
    fn render_single_session(&self, area: Rect, buf: &mut Buffer) {
        if let Some(handle) = self.session_manager.selected() {
            let session = &handle.session;

            let (icon, icon_color) = match session.phase {
                AppPhase::Running => ("●", Color::Green),
                AppPhase::Reloading => ("↻", Color::Yellow),
                AppPhase::Initializing => ("○", Color::DarkGray),
                AppPhase::Stopped => ("○", Color::DarkGray),
                AppPhase::Quitting => ("✗", Color::Red),
            };

            // Truncate device name if necessary
            let max_name_len = area.width.saturating_sub(4) as usize; // 2 for icon+space, 2 for padding
            let name = truncate_name(&session.device_name, max_name_len.max(8));

            let content = Line::from(vec![
                Span::styled(icon, Style::default().fg(icon_color)),
                Span::raw(" "),
                Span::raw(name),
            ]);

            // Render with left padding
            let padded_area = Rect {
                x: area.x + 1,
                y: area.y,
                width: area.width.saturating_sub(2),
                height: area.height,
            };

            Paragraph::new(content).render(padded_area, buf);
        }
    }

    /// Render full tabs UI for multiple sessions
    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles = self.tab_titles();
        let selected = self.session_manager.selected_index();

        let tabs = Tabs::new(titles)
            .select(selected)
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│");

        // Render with left padding
        let padded_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };

        tabs.render(padded_area, buf);
    }
}

impl Widget for SessionTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.session_manager.is_empty() {
            return;
        }

        if self.session_manager.len() == 1 {
            // Single session - render simplified device header
            self.render_single_session(area, buf);
        } else {
            // Multiple sessions - render full tabs UI
            self.render_tabs(area, buf);
        }
    }
}

/// Truncate a name to max length, adding ellipsis if needed
fn truncate_name(name: &str, max_len: usize) -> String {
    if name.chars().count() <= max_len {
        name.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let truncated: String = name.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

/// Header widget that conditionally shows tabs (legacy, for backward compatibility)
pub struct HeaderWithTabs<'a> {
    session_manager: Option<&'a SessionManager>,
}

impl<'a> HeaderWithTabs<'a> {
    /// Create header without tabs
    pub fn simple() -> Self {
        Self {
            session_manager: None,
        }
    }

    /// Create header with session tabs
    pub fn with_sessions(session_manager: &'a SessionManager) -> Self {
        Self {
            session_manager: Some(session_manager),
        }
    }
}

impl Widget for HeaderWithTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render the border first
        Block::default().borders(Borders::BOTTOM).render(area, buf);

        // Content area is inside the border
        let content_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        match self.session_manager {
            Some(manager) if manager.len() > 1 => {
                // Multiple sessions - show tabs in header
                render_tabs_header(manager, content_area, buf);
            }
            Some(manager) if manager.len() == 1 => {
                // Single session - show device name in header but no tabs
                render_single_session_header(manager, content_area, buf);
            }
            _ => {
                // No sessions - simple header
                render_simple_header(content_area, buf);
            }
        }
    }
}

fn render_tabs_header(manager: &SessionManager, area: Rect, buf: &mut Buffer) {
    // Layout: App title | Tabs | Keybindings
    let chunks = Layout::horizontal([
        Constraint::Length(18), // "Flutter Demon   │"
        Constraint::Min(20),    // Tabs
        Constraint::Length(20), // Keybindings
    ])
    .split(area);

    // App title
    let title = Line::from(vec![
        Span::styled(
            " Flutter Demon",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  │"),
    ]);
    buf.set_line(chunks[0].x, chunks[0].y, &title, chunks[0].width);

    // Session tabs
    if !manager.is_empty() {
        let tabs = SessionTabs::new(manager);
        tabs.render(chunks[1], buf);
    }

    // Keybindings hint
    let hints = Line::from(vec![
        Span::styled("[r]", Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        Span::styled("[R]", Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        Span::styled("[q]", Style::default().fg(Color::Yellow)),
    ]);

    // Right-align the hints
    let hints_width = 11; // "[r] [R] [q]"
    if chunks[2].width >= hints_width {
        let x = chunks[2].x + chunks[2].width - hints_width - 1;
        buf.set_line(x, chunks[2].y, &hints, hints_width);
    }
}

fn render_simple_header(area: Rect, buf: &mut Buffer) {
    let title = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let key = Style::default().fg(Color::Yellow);

    let content = Line::from(vec![
        Span::styled(" Flutter Demon", title),
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

    Paragraph::new(content).render(area, buf);
}

fn render_single_session_header(manager: &SessionManager, area: Rect, buf: &mut Buffer) {
    if let Some(handle) = manager.selected() {
        let session = &handle.session;

        let (icon, icon_color) = match session.phase {
            AppPhase::Running => ("●", Color::Green),
            AppPhase::Reloading => ("↻", Color::Yellow),
            AppPhase::Initializing => ("○", Color::DarkGray),
            AppPhase::Stopped => ("○", Color::DarkGray),
            AppPhase::Quitting => ("✗", Color::Red),
        };

        let title = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);
        let key = Style::default().fg(Color::Yellow);

        let content = Line::from(vec![
            Span::styled(" Flutter Demon", title),
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(icon_color)),
            Span::raw(" "),
            Span::raw(session.device_name.clone()),
            Span::raw("   "),
            Span::styled("[", dim),
            Span::styled("r", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("R", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("n", key),
            Span::styled("]", dim),
            Span::raw(" "),
            Span::styled("[", dim),
            Span::styled("q", key),
            Span::styled("]", dim),
        ]);

        Paragraph::new(content).render(area, buf);
    } else {
        render_simple_header(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::Device;

    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    #[test]
    fn test_truncate_name_short() {
        assert_eq!(truncate_name("Short", 10), "Short");
    }

    #[test]
    fn test_truncate_name_long() {
        assert_eq!(truncate_name("iPhone 15 Pro Max", 12), "iPhone 15 P…");
    }

    #[test]
    fn test_truncate_name_edge_cases() {
        // When name fits exactly, return it
        assert_eq!(truncate_name("A", 1), "A");
        assert_eq!(truncate_name("AB", 2), "AB");
        // When name is longer than max, truncate with ellipsis
        assert_eq!(truncate_name("ABC", 2), "A…");
        // max_len of 1 means we can only show ellipsis for longer strings
        assert_eq!(truncate_name("AB", 1), "…");
        assert_eq!(truncate_name("ABC", 1), "…");
    }

    #[test]
    fn test_truncate_name_unicode() {
        // Test with unicode characters
        assert_eq!(truncate_name("日本語テスト", 4), "日本語…");
    }

    #[test]
    fn test_session_tabs_creation() {
        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();
        manager
            .create_session(&test_device("d2", "Pixel 8"))
            .unwrap();

        let tabs = SessionTabs::new(&manager);
        let titles = tabs.tab_titles();

        assert_eq!(titles.len(), 2);
    }

    #[test]
    fn test_tab_title_includes_status_icon() {
        let mut manager = SessionManager::new();
        let id = manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();

        // Initially Initializing
        let tabs = SessionTabs::new(&manager);
        let titles = tabs.tab_titles();
        let title_str: String = titles[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(title_str.contains('○')); // Initializing icon

        // Mark as running
        manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());
        let tabs = SessionTabs::new(&manager);
        let titles = tabs.tab_titles();
        let title_str: String = titles[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(title_str.contains('●')); // Running icon
    }

    #[test]
    fn test_header_with_tabs_no_sessions() {
        let manager = SessionManager::new();
        let _header = HeaderWithTabs::with_sessions(&manager);
        // Should render simple header (no tabs)
        assert!(manager.is_empty());
    }

    #[test]
    fn test_header_with_tabs_single_session() {
        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();

        // Verify that with_sessions works with single session
        let header = HeaderWithTabs::with_sessions(&manager);
        assert!(header.session_manager.is_some());
        // Should render single session header (no tabs)
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_header_with_tabs_multiple_sessions() {
        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        manager.create_session(&test_device("d2", "Pixel")).unwrap();

        // Verify that with_sessions works with multiple sessions
        let header = HeaderWithTabs::with_sessions(&manager);
        assert!(header.session_manager.is_some());
        // Should render tabs when len > 1
        assert!(manager.len() > 1);
    }

    #[test]
    fn test_header_simple() {
        let header = HeaderWithTabs::simple();
        assert!(header.session_manager.is_none());
    }

    #[test]
    fn test_tabs_widget_rendering() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        let id1 = manager
            .create_session(&test_device("d1", "iPhone 15 Pro"))
            .unwrap();
        manager
            .create_session(&test_device("d2", "Pixel 8"))
            .unwrap();

        // Mark first as running
        manager
            .get_mut(id1)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let header = HeaderWithTabs::with_sessions(&manager);
                f.render_widget(header, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        // Verify content contains expected text
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Flutter"));
        assert!(content.contains("iPhone"));
        assert!(content.contains("Pixel"));
    }

    #[test]
    fn test_single_session_rendering() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        let id = manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();

        manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let header = HeaderWithTabs::with_sessions(&manager);
                f.render_widget(header, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Should show device name
        assert!(content.contains("iPhone 15"));
        // Should show running indicator
        assert!(content.contains('●'));
    }

    #[test]
    fn test_standalone_session_tabs() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();
        manager
            .create_session(&test_device("d2", "Pixel 8"))
            .unwrap();

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tabs = SessionTabs::new(&manager);
                f.render_widget(tabs, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Should show both device names
        assert!(content.contains("iPhone 15"));
        assert!(content.contains("Pixel 8"));
    }

    #[test]
    fn test_session_tabs_single_session_renders_device_name() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tabs = SessionTabs::new(&manager);
                f.render_widget(tabs, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Single session should show device name with status icon
        assert!(content.contains("iPhone 15"));
        assert!(content.contains('○')); // Initializing icon
    }

    #[test]
    fn test_session_tabs_single_session_running_status() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        let id = manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();

        // Mark session as running
        manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tabs = SessionTabs::new(&manager);
                f.render_widget(tabs, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Running session should show device name with running icon
        assert!(content.contains("iPhone 15"));
        assert!(content.contains('●')); // Running icon
    }
}
