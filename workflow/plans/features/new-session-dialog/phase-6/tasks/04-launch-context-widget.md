# Task: Launch Context Widget

## Summary

Create the main Launch Context widget that combines all field widgets into the right pane of the NewSessionDialog.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Modify (add main widget) |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (add export) |

## Implementation

### 1. Launch context widget

```rust
// src/tui/widgets/new_session_dialog/launch_context.rs

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};

use super::state::{LaunchContextState, LaunchContextField};
use crate::config::ConfigSource;

/// The Launch Context widget (right pane of NewSessionDialog)
pub struct LaunchContext<'a> {
    state: &'a LaunchContextState,
    is_focused: bool,
}

impl<'a> LaunchContext<'a> {
    pub fn new(state: &'a LaunchContextState, is_focused: bool) -> Self {
        Self { state, is_focused }
    }

    /// Check if a field should show disabled suffix
    fn should_show_disabled_suffix(&self, field: LaunchContextField) -> bool {
        !self.state.is_field_editable(field) &&
        matches!(self.state.selected_config_source(), Some(ConfigSource::VSCode))
    }
}

impl Widget for LaunchContext<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Main block
        let border_color = if self.is_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .title(" Launch Context ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout fields
        let chunks = Layout::vertical([
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Config field
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Mode field
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Flavor field
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Dart Defines field
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Launch button
            Constraint::Min(0),     // Rest (empty)
        ])
        .split(inner);

        // Render Config dropdown
        let config_focused = self.is_focused &&
            self.state.focused_field == LaunchContextField::Config;
        let config_field = DropdownField::new("Configuration", self.state.config_display())
            .focused(config_focused);
        config_field.render(chunks[1], buf);

        // Render Mode selector
        let mode_focused = self.is_focused &&
            self.state.focused_field == LaunchContextField::Mode;
        let mode_disabled = !self.state.is_mode_editable();
        let mode_selector = ModeSelector::new(self.state.mode)
            .focused(mode_focused)
            .disabled(mode_disabled);
        mode_selector.render(chunks[3], buf);

        // Render Flavor dropdown
        let flavor_focused = self.is_focused &&
            self.state.focused_field == LaunchContextField::Flavor;
        let flavor_disabled = !self.state.is_flavor_editable();
        let flavor_suffix = if self.should_show_disabled_suffix(LaunchContextField::Flavor) {
            Some("(from config)")
        } else {
            None
        };
        let mut flavor_field = DropdownField::new("Flavor", self.state.flavor_display())
            .focused(flavor_focused)
            .disabled(flavor_disabled);
        if let Some(suffix) = flavor_suffix {
            flavor_field = flavor_field.suffix(suffix);
        }
        flavor_field.render(chunks[5], buf);

        // Render Dart Defines action field
        let defines_focused = self.is_focused &&
            self.state.focused_field == LaunchContextField::DartDefines;
        let defines_disabled = !self.state.are_dart_defines_editable();
        let defines_field = ActionField::new("Dart Defines", self.state.dart_defines_display())
            .focused(defines_focused)
            .disabled(defines_disabled);
        defines_field.render(chunks[7], buf);

        // Render Launch button
        let launch_focused = self.is_focused &&
            self.state.focused_field == LaunchContextField::Launch;
        let launch_button = LaunchButton::new()
            .focused(launch_focused);
        launch_button.render(chunks[9], buf);
    }
}
```

### 2. Widget with device awareness

```rust
/// Launch Context with device selection awareness
pub struct LaunchContextWithDevice<'a> {
    state: &'a LaunchContextState,
    is_focused: bool,
    has_device_selected: bool,
}

impl<'a> LaunchContextWithDevice<'a> {
    pub fn new(
        state: &'a LaunchContextState,
        is_focused: bool,
        has_device_selected: bool,
    ) -> Self {
        Self {
            state,
            is_focused,
            has_device_selected,
        }
    }
}

impl Widget for LaunchContextWithDevice<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Similar to LaunchContext but with device-aware launch button
        let border_color = if self.is_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .title(" Launch Context ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout fields (same as LaunchContext)
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

        // ... render other fields same as above ...

        // Render Launch button with device awareness
        let launch_focused = self.is_focused &&
            self.state.focused_field == LaunchContextField::Launch;
        let launch_button = LaunchButton::new()
            .focused(launch_focused)
            .enabled(self.has_device_selected);
        launch_button.render(chunks[9], buf);
    }
}
```

### 3. Compact rendering for small terminals

```rust
impl LaunchContext<'_> {
    /// Calculate minimum height needed
    pub fn min_height() -> u16 {
        12 // 1 border + 10 content + 1 border
    }

    /// Render in compact mode (smaller terminals)
    pub fn render_compact(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Launch ")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        block.render(area, buf);

        // Compact layout: fewer spacers
        let chunks = Layout::vertical([
            Constraint::Length(1), // Config
            Constraint::Length(1), // Mode
            Constraint::Length(1), // Flavor
            Constraint::Length(1), // Dart Defines
            Constraint::Length(1), // Launch
            Constraint::Min(0),
        ])
        .split(inner);

        // Render fields without spacers
        // ... (same field widgets, just denser layout)
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use crate::config::LoadedConfigs;

    #[test]
    fn test_launch_context_renders() {
        let state = LaunchContextState::new(LoadedConfigs::default());

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Launch Context"));
        assert!(content.contains("Configuration"));
        assert!(content.contains("Mode"));
        assert!(content.contains("Flavor"));
        assert!(content.contains("Dart Defines"));
        assert!(content.contains("LAUNCH"));
    }

    #[test]
    fn test_launch_context_shows_disabled_suffix() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                flavor: Some("prod".to_string()),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.select_config(Some(0));

        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("from config"));
    }

    #[test]
    fn test_launch_context_focused_field() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.focused_field = LaunchContextField::Flavor;

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        // Verify Flavor field is highlighted
        // (visual verification via styling)
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test launch_context && cargo clippy -- -D warnings
```

## Notes

- Widget respects focused state for individual fields
- Disabled fields show "(from config)" suffix for VSCode configs
- Launch button changes text based on device selection
- Compact mode available for small terminals
