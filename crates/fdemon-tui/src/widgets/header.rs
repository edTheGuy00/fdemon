//! Header bar widgets
//!
//! Provides the main header with project name and keybindings.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use fdemon_app::session_manager::SessionManager;

use crate::theme::{icons, palette, styles};

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
        // Render glass container with rounded borders
        let block = styles::glass_block(false).style(Style::default().bg(palette::CARD_BG));

        // Get inner content area (inside borders) before rendering
        let inner = block.inner(area);

        // Now render the block
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Check if we have multiple sessions (need to show tabs)
        let has_multiple_sessions = self.session_manager.map(|sm| sm.len() > 1).unwrap_or(false);

        if has_multiple_sessions {
            // Multi-session mode: split into title row and tabs row
            if inner.height >= 2 {
                // Title row
                let title_area = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: inner.width,
                    height: 1,
                };
                self.render_title_row(title_area, buf, false);

                // Tabs row
                let tabs_area = Rect {
                    x: inner.x,
                    y: inner.y + 1,
                    width: inner.width,
                    height: inner.height.saturating_sub(1),
                };
                if let Some(session_manager) = self.session_manager {
                    let tabs = SessionTabs::new(session_manager);
                    tabs.render(tabs_area, buf);
                }
            } else {
                // Not enough space for both rows, just render title
                self.render_title_row(inner, buf, false);
            }
        } else {
            // Single session or no session: render title + shortcuts + device pill in one row
            self.render_title_row(inner, buf, true);
        }
    }
}

impl MainHeader<'_> {
    /// Render the title row with status dot, project name, shortcuts, and optional device pill
    fn render_title_row(&self, area: Rect, buf: &mut Buffer, show_device: bool) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let project_name = self.project_name.unwrap_or("flutter");

        // Get status dot and device info from selected session
        let (status_icon, status_style, device_name, device_platform) =
            if let Some(session_manager) = self.session_manager {
                if let Some(handle) = session_manager.selected() {
                    let session = &handle.session;
                    let (icon, _label, style) = styles::phase_indicator(&session.phase);
                    (
                        icon,
                        style,
                        Some(session.device_name.as_str()),
                        Some(session.platform.as_str()),
                    )
                } else {
                    ("○", Style::default().fg(palette::TEXT_MUTED), None, None)
                }
            } else {
                ("○", Style::default().fg(palette::TEXT_MUTED), None, None)
            };

        // Build left section: status dot + "Flutter Demon" + "/" + project name
        let left_spans = vec![
            Span::raw(" "),
            Span::styled(status_icon, status_style),
            Span::raw(" "),
            Span::styled(
                "Flutter Demon",
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("/", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(" "),
            Span::styled(project_name, Style::default().fg(palette::TEXT_SECONDARY)),
        ];

        let left_line = Line::from(left_spans.clone());
        let left_width = left_line.width() as u16;

        // Build shortcut hints (center section)
        let shortcuts = vec![
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("r", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Run  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("R", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Restart  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("x", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Stop  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("d", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Debug  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("q", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Quit", Style::default().fg(palette::TEXT_MUTED)),
        ];
        let shortcuts_line = Line::from(shortcuts.clone());
        let shortcuts_width = shortcuts_line.width() as u16;

        // Build device pill (right section) if single session
        let device_content = if show_device && device_name.is_some() {
            let device_icon = device_icon_for_platform(device_platform);
            let device_spans = vec![
                Span::raw(" "),
                Span::raw(device_icon),
                Span::raw(" "),
                Span::styled(
                    device_name.unwrap_or(""),
                    Style::default().fg(palette::ACCENT),
                ),
                Span::raw(" "),
            ];
            Some(Line::from(device_spans))
        } else {
            None
        };
        let device_width = device_content
            .as_ref()
            .map(|l| l.width() as u16)
            .unwrap_or(0);

        // Calculate available space and positioning
        let total_content_width = left_width + shortcuts_width + device_width + 4; // 4 for padding

        if total_content_width <= area.width {
            // Everything fits: left | center | right layout
            buf.set_line(area.x, area.y, &left_line, area.width);

            // Center the shortcuts
            let shortcuts_x = area.x + left_width + 2;
            if shortcuts_x + shortcuts_width <= area.x + area.width {
                buf.set_line(shortcuts_x, area.y, &shortcuts_line, shortcuts_width);
            }

            // Right-align device pill
            if let Some(device_line) = device_content {
                let device_x = area.x + area.width - device_width;
                if device_x >= area.x + left_width + shortcuts_width + 4 {
                    buf.set_line(device_x, area.y, &device_line, device_width);
                }
            }
        } else if left_width + device_width + 2 <= area.width {
            // Shortcuts don't fit, but left + device does
            buf.set_line(area.x, area.y, &left_line, area.width);

            if let Some(device_line) = device_content {
                let device_x = area.x + area.width - device_width;
                if device_x >= area.x + left_width + 2 {
                    buf.set_line(device_x, area.y, &device_line, device_width);
                }
            }
        } else {
            // Only left section fits
            buf.set_line(area.x, area.y, &left_line, area.width);
        }
    }
}

/// Map platform string to device icon
fn device_icon_for_platform(platform: Option<&str>) -> &'static str {
    match platform {
        Some(p) if p.contains("ios") || p.contains("simulator") => icons::ICON_SMARTPHONE,
        Some(p) if p.contains("web") || p.contains("chrome") => icons::ICON_GLOBE,
        Some(p) if p.contains("macos") || p.contains("linux") || p.contains("windows") => {
            icons::ICON_MONITOR
        }
        _ => icons::ICON_CPU,
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
        // Use wider terminal (120 cols) to ensure shortcuts fit
        let mut term = TestTerminal::with_size(120, 24);
        let header = MainHeader::new(Some("test_project"));

        term.render_widget(header, term.area());

        // Verify keybindings are present with new format (includes labels)
        assert!(term.buffer_contains("[r] Run"), "Should show reload key");
        assert!(
            term.buffer_contains("[R] Restart"),
            "Should show restart key"
        );
        assert!(term.buffer_contains("[x] Stop"), "Should show stop key");
        assert!(
            term.buffer_contains("[d] Debug"),
            "Should show debug/device selector key"
        );
        assert!(term.buffer_contains("[q] Quit"), "Should show quit key");
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
