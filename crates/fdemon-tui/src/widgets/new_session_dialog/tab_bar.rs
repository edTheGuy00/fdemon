//! Tab bar widget for Target Selector pane
//!
//! Provides tab navigation between Connected and Bootable device views.

use super::TargetTab;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::theme::palette;

/// Tab bar widget for switching between Connected and Bootable views
pub struct TabBar {
    active_tab: TargetTab,
    /// Whether this pane is focused
    pane_focused: bool,
}

impl TabBar {
    pub fn new(active_tab: TargetTab, pane_focused: bool) -> Self {
        Self {
            active_tab,
            pane_focused,
        }
    }

    fn tab_style(&self, tab: TargetTab) -> Style {
        let is_active = self.active_tab == tab;

        if is_active && self.pane_focused {
            Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        }
    }
}

impl Widget for TabBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split area into two equal parts for tabs
        let chunks = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Render Connected tab
        let connected_style = self.tab_style(TargetTab::Connected);
        let connected_block = Block::default()
            .borders(Borders::ALL)
            .border_style(connected_style);

        let connected_text =
            Paragraph::new(Span::styled(TargetTab::Connected.label(), connected_style))
                .alignment(Alignment::Center)
                .block(connected_block);

        connected_text.render(chunks[0], buf);

        // Render Bootable tab
        let bootable_style = self.tab_style(TargetTab::Bootable);
        let bootable_block = Block::default()
            .borders(Borders::ALL)
            .border_style(bootable_style);

        let bootable_text =
            Paragraph::new(Span::styled(TargetTab::Bootable.label(), bootable_style))
                .alignment(Alignment::Center)
                .block(bootable_block);

        bootable_text.render(chunks[1], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_target_tab_label() {
        assert_eq!(TargetTab::Connected.label(), "1 Connected");
        assert_eq!(TargetTab::Bootable.label(), "2 Bootable");
    }

    #[test]
    fn test_target_tab_toggle() {
        assert_eq!(TargetTab::Connected.toggle(), TargetTab::Bootable);
        assert_eq!(TargetTab::Bootable.toggle(), TargetTab::Connected);
    }

    #[test]
    fn test_target_tab_shortcut() {
        assert_eq!(TargetTab::Connected.shortcut(), '1');
        assert_eq!(TargetTab::Bootable.shortcut(), '2');
    }

    #[test]
    fn test_target_tab_default() {
        let tab: TargetTab = Default::default();
        assert_eq!(tab, TargetTab::Connected);
    }

    #[test]
    fn test_tab_bar_renders() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }

    #[test]
    fn test_tab_bar_renders_with_bootable_active() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }

    #[test]
    fn test_tab_bar_unfocused() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, false);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should still render both tabs
        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }
}
