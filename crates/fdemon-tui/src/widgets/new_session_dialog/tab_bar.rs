//! Tab bar widget for Target Selector pane
//!
//! Provides tab navigation between Connected and Bootable device views.

use super::TargetTab;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::theme::{icons::IconSet, palette};

/// Tab bar widget for switching between Connected and Bootable views
pub struct TabBar<'a> {
    active_tab: TargetTab,
    /// Whether this pane is focused
    pane_focused: bool,
    /// Refresh-in-flight indicator for the Connected tab.
    connected_refreshing: bool,
    /// Refresh-in-flight indicator for the Bootable tab.
    bootable_refreshing: bool,
    /// Icon set for resolving glyphs (Unicode vs Nerd Fonts).
    icons: &'a IconSet,
}

impl<'a> TabBar<'a> {
    pub fn new(
        active_tab: TargetTab,
        pane_focused: bool,
        connected_refreshing: bool,
        bootable_refreshing: bool,
        icons: &'a IconSet,
    ) -> Self {
        Self {
            active_tab,
            pane_focused,
            connected_refreshing,
            bootable_refreshing,
            icons,
        }
    }
}

impl Widget for TabBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Outer container: dark background with rounded border
        let container_bg = palette::DEEPEST_BG;
        let container_block = Block::default()
            .style(Style::default().bg(container_bg))
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM));

        let inner = container_block.inner(area);
        container_block.render(area, buf);

        // Split into two equal halves for tabs
        let tabs = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        // Render each tab
        for (i, tab) in [TargetTab::Connected, TargetTab::Bootable]
            .iter()
            .enumerate()
        {
            let is_active = *tab == self.active_tab;
            let refreshing = match tab {
                TargetTab::Connected => self.connected_refreshing,
                TargetTab::Bootable => self.bootable_refreshing,
            };

            let label = if refreshing {
                format!("{} {}", tab.label(), self.icons.refresh())
            } else {
                tab.label().to_string()
            };

            let style = if is_active && self.pane_focused {
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .bg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };

            let paragraph = Paragraph::new(label)
                .style(style)
                .alignment(Alignment::Center);
            paragraph.render(tabs[i], buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::icons::IconSet;
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
        let icons = IconSet::default();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, false, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, false, false, false, &icons);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should still render both tabs
        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }

    #[test]
    fn test_tab_bar_renders_connected_refreshing_indicator() {
        let icons = IconSet::default();
        let glyph = icons.refresh();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, true, false, &icons);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            rendered.contains(glyph),
            "expected refresh glyph on Connected tab, got: {rendered}"
        );
    }

    #[test]
    fn test_tab_bar_renders_bootable_refreshing_indicator() {
        let icons = IconSet::default();
        let glyph = icons.refresh();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, true, &icons);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            rendered.contains(glyph),
            "expected refresh glyph on Bootable tab, got: {rendered}"
        );
    }

    #[test]
    fn test_tab_bar_no_indicator_when_not_refreshing() {
        let icons = IconSet::default();
        let glyph = icons.refresh();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(!rendered.contains(glyph));
    }
}
