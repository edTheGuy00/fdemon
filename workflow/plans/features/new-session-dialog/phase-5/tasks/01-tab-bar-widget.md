# Task: Tab Bar Widget

## Summary

Create a reusable tab bar widget for the Target Selector pane, displaying Connected and Bootable tabs with visual selection state.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/tab_bar.rs` | Create |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (add export) |

## Implementation

### 1. Define tab enum

```rust
// src/tui/widgets/new_session_dialog/tab_bar.rs

/// Tabs in the Target Selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetTab {
    #[default]
    Connected,
    Bootable,
}

impl TargetTab {
    pub fn label(&self) -> &'static str {
        match self {
            TargetTab::Connected => "1 Connected",
            TargetTab::Bootable => "2 Bootable",
        }
    }

    pub fn shortcut(&self) -> char {
        match self {
            TargetTab::Connected => '1',
            TargetTab::Bootable => '2',
        }
    }

    /// Get the other tab
    pub fn toggle(&self) -> Self {
        match self {
            TargetTab::Connected => TargetTab::Bootable,
            TargetTab::Bootable => TargetTab::Connected,
        }
    }
}
```

### 2. Create tab bar widget

```rust
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Widget},
};

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
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        }
    }
}

impl Widget for TabBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split area into two equal parts for tabs
        let chunks = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

        // Render Connected tab
        let connected_style = self.tab_style(TargetTab::Connected);
        let connected_block = Block::default()
            .borders(Borders::ALL)
            .border_style(connected_style);

        let connected_text = Paragraph::new(Span::styled(
            TargetTab::Connected.label(),
            connected_style,
        ))
        .alignment(Alignment::Center)
        .block(connected_block);

        connected_text.render(chunks[0], buf);

        // Render Bootable tab
        let bootable_style = self.tab_style(TargetTab::Bootable);
        let bootable_block = Block::default()
            .borders(Borders::ALL)
            .border_style(bootable_style);

        let bootable_text = Paragraph::new(Span::styled(
            TargetTab::Bootable.label(),
            bootable_style,
        ))
        .alignment(Alignment::Center)
        .block(bootable_block);

        bootable_text.render(chunks[1], buf);
    }
}
```

### 3. Alternative pill-style tabs

```rust
/// Pill-style tab bar (alternative design)
pub struct PillTabBar {
    active_tab: TargetTab,
    pane_focused: bool,
}

impl Widget for PillTabBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render as: ╭─────────────╮ ╭─────────────╮
        //            │ 1 Connected │ │ 2 Bootable  │
        //            ╰─────────────╯ ╰─────────────╯

        let chunks = Layout::horizontal([
            Constraint::Length(1),  // Left padding
            Constraint::Min(15),    // Connected tab
            Constraint::Length(1),  // Gap
            Constraint::Min(15),    // Bootable tab
            Constraint::Length(1),  // Right padding
        ])
        .split(area);

        // Render Connected pill
        self.render_pill(
            chunks[1],
            buf,
            TargetTab::Connected,
            self.active_tab == TargetTab::Connected,
        );

        // Render Bootable pill
        self.render_pill(
            chunks[3],
            buf,
            TargetTab::Bootable,
            self.active_tab == TargetTab::Bootable,
        );
    }
}

impl PillTabBar {
    fn render_pill(&self, area: Rect, buf: &mut Buffer, tab: TargetTab, is_active: bool) {
        let style = if is_active && self.pane_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let border_set = ratatui::symbols::border::ROUNDED;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set)
            .border_style(style);

        let text = Paragraph::new(Span::styled(tab.label(), style))
            .alignment(Alignment::Center)
            .block(block);

        text.render(area, buf);
    }
}
```

## Tests

```rust
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
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test tab_bar && cargo clippy -- -D warnings
```

## Notes

- Tab bar height should be 3 lines (border + text + border)
- Consider using rounded borders for a modern look
- The active tab should be clearly distinguishable
- 1/2 keyboard shortcuts should be shown in the tab labels
