# Task: Field Widgets

## Summary

Create individual field widgets for the Launch Context pane: dropdown field, mode radio buttons, and launch button.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Create |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (add export) |

## Implementation

### 1. Field styles

```rust
// src/tui/widgets/new_session_dialog/launch_context.rs

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Styles for Launch Context fields
pub struct LaunchContextStyles {
    pub label: Style,
    pub value_normal: Style,
    pub value_focused: Style,
    pub value_disabled: Style,
    pub placeholder: Style,
    pub button_normal: Style,
    pub button_focused: Style,
    pub mode_selected: Style,
    pub mode_unselected: Style,
    pub suffix: Style,
}

impl Default for LaunchContextStyles {
    fn default() -> Self {
        Self {
            label: Style::default().fg(Color::Gray),
            value_normal: Style::default().fg(Color::White),
            value_focused: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            value_disabled: Style::default().fg(Color::DarkGray),
            placeholder: Style::default().fg(Color::DarkGray),
            button_normal: Style::default().fg(Color::Green),
            button_focused: Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
            mode_selected: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            mode_unselected: Style::default().fg(Color::DarkGray),
            suffix: Style::default().fg(Color::DarkGray),
        }
    }
}
```

### 2. Dropdown field widget

```rust
/// A dropdown-style field that opens a fuzzy modal
pub struct DropdownField {
    label: String,
    value: String,
    is_focused: bool,
    is_disabled: bool,
    suffix: Option<String>,
    styles: LaunchContextStyles,
}

impl DropdownField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            is_focused: false,
            is_disabled: false,
            suffix: None,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }
}

impl Widget for DropdownField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Layout: label + value box
        let chunks = Layout::horizontal([
            Constraint::Length(15), // Label
            Constraint::Min(20),    // Value
        ])
        .split(area);

        // Render label
        let label = Paragraph::new(format!("  {}:", self.label))
            .style(self.styles.label);
        label.render(chunks[0], buf);

        // Determine value style
        let value_style = if self.is_disabled {
            self.styles.value_disabled
        } else if self.is_focused {
            self.styles.value_focused
        } else {
            self.styles.value_normal
        };

        // Format value with dropdown indicator and suffix
        let display_value = if self.value.is_empty() || self.value == "(none)" {
            "(none)".to_string()
        } else {
            self.value.clone()
        };

        let dropdown_indicator = if self.is_disabled { " " } else { " ▼" };
        let suffix_text = self.suffix
            .map(|s| format!("  {}", s))
            .unwrap_or_default();

        let value_line = Line::from(vec![
            Span::styled(format!("[ {}", display_value), value_style),
            Span::styled(dropdown_indicator, value_style),
            Span::styled(" ]", value_style),
            Span::styled(suffix_text, self.styles.suffix),
        ]);

        Paragraph::new(value_line).render(chunks[1], buf);
    }
}
```

### 3. Mode radio buttons widget

```rust
use crate::config::FlutterMode;

/// Radio button group for Flutter mode selection
pub struct ModeSelector {
    selected: FlutterMode,
    is_focused: bool,
    is_disabled: bool,
    styles: LaunchContextStyles,
}

impl ModeSelector {
    pub fn new(selected: FlutterMode) -> Self {
        Self {
            selected,
            is_focused: false,
            is_disabled: false,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    fn mode_style(&self, mode: FlutterMode) -> Style {
        if self.is_disabled {
            self.styles.value_disabled
        } else if mode == self.selected && self.is_focused {
            self.styles.value_focused
        } else if mode == self.selected {
            self.styles.mode_selected
        } else {
            self.styles.mode_unselected
        }
    }

    fn mode_indicator(&self, mode: FlutterMode) -> &'static str {
        if mode == self.selected {
            "(●)"
        } else {
            "(○)"
        }
    }
}

impl Widget for ModeSelector {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Length(15), // Label
            Constraint::Min(40),    // Radio buttons
        ])
        .split(area);

        // Render label
        let label = Paragraph::new("  Mode:")
            .style(self.styles.label);
        label.render(chunks[0], buf);

        // Render radio buttons
        let debug_style = self.mode_style(FlutterMode::Debug);
        let profile_style = self.mode_style(FlutterMode::Profile);
        let release_style = self.mode_style(FlutterMode::Release);

        let line = Line::from(vec![
            Span::styled(
                format!("{} Debug", self.mode_indicator(FlutterMode::Debug)),
                debug_style,
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Profile", self.mode_indicator(FlutterMode::Profile)),
                profile_style,
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Release", self.mode_indicator(FlutterMode::Release)),
                release_style,
            ),
        ]);

        Paragraph::new(line).render(chunks[1], buf);
    }
}
```

### 4. Action field widget (for Dart Defines)

```rust
/// A field that opens a modal when activated
pub struct ActionField {
    label: String,
    value: String,
    action_indicator: &'static str,
    is_focused: bool,
    is_disabled: bool,
    styles: LaunchContextStyles,
}

impl ActionField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            action_indicator: "▶",
            is_focused: false,
            is_disabled: false,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }
}

impl Widget for ActionField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Length(15), // Label
            Constraint::Min(20),    // Value
        ])
        .split(area);

        // Render label
        let label = Paragraph::new(format!("  {}:", self.label))
            .style(self.styles.label);
        label.render(chunks[0], buf);

        // Determine value style
        let value_style = if self.is_disabled {
            self.styles.value_disabled
        } else if self.is_focused {
            self.styles.value_focused
        } else {
            self.styles.value_normal
        };

        let indicator = if self.is_disabled { " " } else { self.action_indicator };

        let value_line = Line::from(vec![
            Span::styled(format!("[ {} ", self.value), value_style),
            Span::styled(indicator, value_style),
            Span::styled(" ]", value_style),
        ]);

        Paragraph::new(value_line).render(chunks[1], buf);
    }
}
```

### 5. Launch button widget

```rust
/// The launch button at the bottom of Launch Context
pub struct LaunchButton {
    is_focused: bool,
    is_enabled: bool,
    styles: LaunchContextStyles,
}

impl LaunchButton {
    pub fn new() -> Self {
        Self {
            is_focused: false,
            is_enabled: true,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
}

impl Widget for LaunchButton {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = if !self.is_enabled {
            Style::default().fg(Color::DarkGray)
        } else if self.is_focused {
            self.styles.button_focused
        } else {
            self.styles.button_normal
        };

        let text = if self.is_enabled {
            "    LAUNCH (Enter)    "
        } else {
            "    SELECT DEVICE     "
        };

        let button = Paragraph::new(format!("[{}]", text))
            .style(style)
            .alignment(Alignment::Center);

        button.render(area, buf);
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
    fn test_dropdown_field_renders() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = DropdownField::new("Config", "Development")
                    .focused(true);
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Config"));
        assert!(content.contains("Development"));
    }

    #[test]
    fn test_mode_selector_renders() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = ModeSelector::new(FlutterMode::Debug)
                    .focused(true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Debug"));
        assert!(content.contains("Profile"));
        assert!(content.contains("Release"));
    }

    #[test]
    fn test_launch_button_renders() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let button = LaunchButton::new().focused(true);
                f.render_widget(button, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("LAUNCH"));
    }

    #[test]
    fn test_disabled_field_styling() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = DropdownField::new("Flavor", "dev")
                    .disabled(true)
                    .suffix("(from config)");
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("from config"));
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test field_widgets && cargo clippy -- -D warnings
```

## Notes

- All widgets respect disabled state with visual feedback
- Dropdown indicator (▼) shows field opens a modal
- Action indicator (▶) shows field opens a modal
- Focused state uses high-contrast styling
- Mode selector shows filled/empty radio buttons
