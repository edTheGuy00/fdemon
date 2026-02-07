//! Session tabs widget for multi-instance display
//!
//! Provides tab navigation for multiple running Flutter sessions.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Tabs, Widget},
};

use fdemon_app::session_manager::SessionManager;

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

                // Status icon with color from theme
                let (icon, _label, style) = crate::theme::styles::phase_indicator(&session.phase);

                // Truncate device name if too long
                let name = truncate_name(&session.device_name, 12);

                // Build line with styled icon span
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(icon, style),
                    Span::raw(format!(" {} ", name)),
                ])
            })
            .collect()
    }
}

impl<'a> SessionTabs<'a> {
    /// Render a simplified single-session header showing device name with status icon
    fn render_single_session(&self, area: Rect, buf: &mut Buffer) {
        if let Some(handle) = self.session_manager.selected() {
            let session = &handle.session;

            let (icon, _label, style) = crate::theme::styles::phase_indicator(&session.phase);

            // Truncate device name if necessary
            let max_name_len = area.width.saturating_sub(4) as usize; // 2 for icon+space, 2 for padding
            let name = truncate_name(&session.device_name, max_name_len.max(8));

            let content = Line::from(vec![
                Span::styled(icon, style),
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
            .highlight_style(crate::theme::styles::focused_selected())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_device;

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
