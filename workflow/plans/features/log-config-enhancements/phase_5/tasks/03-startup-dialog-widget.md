# Task: New Startup Dialog Widget

**Objective**: Create a centered modal widget for comprehensive session launching with config selection, mode, flavor, dart-defines, and device selection.

**Depends on**: Task 01 (Config Priority), Task 02 (Dialog State)

## Scope

- `src/tui/widgets/startup_dialog/mod.rs` — **NEW** Main widget
- `src/tui/widgets/startup_dialog/styles.rs` — **NEW** Style constants
- `src/tui/widgets/mod.rs` — Export new widget

## Module Structure

```
src/tui/widgets/startup_dialog/
├── mod.rs      # Widget implementation, render logic
└── styles.rs   # Color/style constants
```

## Details

### Visual Design

```
┌───────────────────── Launch Session ─────────────────────┐
│                                                          │
│  Configuration                                           │
│  ┌────────────────────────────────────────────────────┐  │
│  │ ▶ Debug                                            │  │
│  │   Profile                                          │  │
│  │   ──────────────────────────                       │  │
│  │   Flutter App (VSCode)                             │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  Mode: [●Debug] ○Profile ○Release                        │
│                                                          │
│  Flavor: [development_______] (optional)                 │
│                                                          │
│  Dart Defines: [API_URL=https://dev.api.com] (optional)  │
│                                                          │
│  Device                                                  │
│  ┌────────────────────────────────────────────────────┐  │
│  │ ▶ iPhone 15 Pro (simulator)                        │  │
│  │   Pixel 8 (emulator)                               │  │
│  │   macOS (desktop)                                  │  │
│  │   Chrome (web)                                     │  │
│  │   ──────────────────────────                       │  │
│  │   + Launch Android Emulator                        │  │
│  │   + Launch iOS Simulator                           │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  [Tab] Section  [↑↓] Navigate  [Enter] Launch  [Esc]     │
└──────────────────────────────────────────────────────────┘
```

### Widget Implementation

```rust
// src/tui/widgets/startup_dialog/mod.rs

mod styles;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use crate::app::state::{DialogSection, StartupDialogState};
use crate::config::ConfigSource;

pub use styles::*;

/// Startup dialog widget for session launching
pub struct StartupDialog<'a> {
    state: &'a StartupDialogState,
    /// Whether there are running sessions (affects Esc behavior)
    has_running_sessions: bool,
}

impl<'a> StartupDialog<'a> {
    pub fn new(state: &'a StartupDialogState) -> Self {
        Self {
            state,
            has_running_sessions: false,
        }
    }

    pub fn with_session_state(state: &'a StartupDialogState, has_running_sessions: bool) -> Self {
        Self {
            state,
            has_running_sessions,
        }
    }

    /// Calculate centered modal area (80% width, 70% height)
    fn centered_rect(area: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(area);

        Layout::horizontal([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(popup_layout[1])[1]
    }

    /// Render config list section
    fn render_config_list(&self, area: Rect, buf: &mut Buffer) {
        let is_active = self.state.active_section == DialogSection::Configs;
        let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

        let block = Block::default()
            .title(" Configuration ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.configs.is_empty {
            let no_configs = Paragraph::new("No configurations found")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            no_configs.render(inner, buf);
            return;
        }

        let mut items: Vec<ListItem> = Vec::new();

        for (i, config) in self.state.configs.configs.iter().enumerate() {
            let is_selected = self.state.selected_config == Some(i);
            let is_vscode_start = self.state.configs.vscode_start_index == Some(i);

            // Add divider before VSCode configs
            if is_vscode_start && i > 0 {
                items.push(ListItem::new("  ─────────────────────────────────"));
            }

            let style = if is_selected && is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indicator = if is_selected { "▶ " } else { "  " };
            let source_tag = match config.source {
                ConfigSource::VSCode => " (VSCode)",
                _ => "",
            };

            let line = format!("{}{}{}", indicator, config.config.name, source_tag);
            items.push(ListItem::new(line).style(style));
        }

        let list = List::new(items);
        list.render(inner, buf);
    }

    /// Render mode selector (horizontal radio buttons)
    fn render_mode_selector(&self, area: Rect, buf: &mut Buffer) {
        let is_active = self.state.active_section == DialogSection::Mode;

        let debug_style = self.mode_style(FlutterMode::Debug, is_active);
        let profile_style = self.mode_style(FlutterMode::Profile, is_active);
        let release_style = self.mode_style(FlutterMode::Release, is_active);

        let line = Line::from(vec![
            Span::raw("  Mode: "),
            Span::styled(self.mode_indicator(FlutterMode::Debug), debug_style),
            Span::raw(" "),
            Span::styled(self.mode_indicator(FlutterMode::Profile), profile_style),
            Span::raw(" "),
            Span::styled(self.mode_indicator(FlutterMode::Release), release_style),
        ]);

        Paragraph::new(line).render(area, buf);
    }

    fn mode_indicator(&self, mode: FlutterMode) -> String {
        let selected = self.state.mode == mode;
        let icon = if selected { "●" } else { "○" };
        format!("{}{}", icon, mode)
    }

    fn mode_style(&self, mode: FlutterMode, section_active: bool) -> Style {
        let selected = self.state.mode == mode;
        if selected && section_active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }

    /// Render text input field
    fn render_input_field(&self, area: Rect, buf: &mut Buffer, label: &str, value: &str, section: DialogSection) {
        let is_active = self.state.active_section == section;
        let is_editing = is_active && self.state.editing;

        let border_color = if is_active { Color::Cyan } else { Color::DarkGray };
        let value_style = if is_editing {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let display_value = if value.is_empty() && !is_editing {
            "(optional)".to_string()
        } else {
            value.to_string()
        };

        let line = Line::from(vec![
            Span::raw(format!("  {}: ", label)),
            Span::styled(format!("[{}]", display_value), value_style),
        ]);

        Paragraph::new(line)
            .style(Style::default().fg(if is_active { Color::White } else { Color::Gray }))
            .render(area, buf);
    }

    /// Render device list section
    fn render_device_list(&self, area: Rect, buf: &mut Buffer) {
        let is_active = self.state.active_section == DialogSection::Devices;
        let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

        let block = Block::default()
            .title(" Device ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.loading {
            let loading = Paragraph::new("Discovering devices...")
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            loading.render(inner, buf);
            return;
        }

        if let Some(ref error) = self.state.error {
            let error_text = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center);
            error_text.render(inner, buf);
            return;
        }

        let mut items: Vec<ListItem> = Vec::new();

        for (i, device) in self.state.devices.iter().enumerate() {
            let is_selected = self.state.selected_device == Some(i);

            let style = if is_selected && is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indicator = if is_selected { "▶ " } else { "  " };
            let device_type = if device.emulator {
                device.emulator_type()
            } else {
                "physical"
            };

            let line = format!("{}{} ({})", indicator, device.name, device_type);
            items.push(ListItem::new(line).style(style));
        }

        // Add emulator launch options
        if !self.state.devices.is_empty() {
            items.push(ListItem::new("  ─────────────────────────────────"));
        }

        let android_idx = self.state.devices.len();
        let ios_idx = android_idx + 1;

        let android_style = Style::default().fg(Color::Green);
        items.push(ListItem::new("  + Launch Android Emulator").style(android_style));

        let ios_style = Style::default().fg(Color::Blue);
        items.push(ListItem::new("  + Launch iOS Simulator").style(ios_style));

        let list = List::new(items);
        list.render(inner, buf);
    }

    /// Render footer with keybindings
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = if self.has_running_sessions {
            "[Tab] Section  [↑↓] Navigate  [Enter] Launch  [Esc] Cancel  [r] Refresh"
        } else {
            "[Tab] Section  [↑↓] Navigate  [Enter] Launch  [r] Refresh"
        };

        let can_launch = self.state.can_launch();
        let enter_hint = if can_launch { "Enter" } else { "Enter (select device)" };

        Paragraph::new(hints)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray))
            .render(area, buf);
    }
}

impl Widget for StartupDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_area = Self::centered_rect(area);

        // Clear background
        Clear.render(modal_area, buf);

        // Modal block
        let block = Block::default()
            .title(" Launch Session ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout sections
        let chunks = Layout::vertical([
            Constraint::Length(8),  // Config list
            Constraint::Length(2),  // Mode selector
            Constraint::Length(1),  // Flavor input
            Constraint::Length(1),  // Dart defines input
            Constraint::Length(1),  // Spacer
            Constraint::Min(6),     // Device list
            Constraint::Length(2),  // Footer
        ])
        .split(inner);

        self.render_config_list(chunks[0], buf);
        self.render_mode_selector(chunks[1], buf);
        self.render_input_field(chunks[2], buf, "Flavor", &self.state.flavor, DialogSection::Flavor);
        self.render_input_field(chunks[3], buf, "Dart Defines", &self.state.dart_defines, DialogSection::DartDefines);
        self.render_device_list(chunks[5], buf);
        self.render_footer(chunks[6], buf);
    }
}
```

### Styles Module

```rust
// src/tui/widgets/startup_dialog/styles.rs

use ratatui::style::{Color, Modifier, Style};

// Section colors
pub const ACTIVE_BORDER: Color = Color::Cyan;
pub const INACTIVE_BORDER: Color = Color::DarkGray;

// Selection colors
pub const SELECTED_BG: Color = Color::Cyan;
pub const SELECTED_FG: Color = Color::Black;

// List colors
pub const DIVIDER_COLOR: Color = Color::DarkGray;
pub const EMULATOR_ANDROID: Color = Color::Green;
pub const EMULATOR_IOS: Color = Color::Blue;

// Text colors
pub const LABEL_COLOR: Color = Color::Gray;
pub const VALUE_COLOR: Color = Color::White;
pub const PLACEHOLDER_COLOR: Color = Color::DarkGray;
pub const ERROR_COLOR: Color = Color::Red;
pub const LOADING_COLOR: Color = Color::Yellow;
```

### Module Export

Update `src/tui/widgets/mod.rs`:

```rust
mod startup_dialog;

pub use startup_dialog::StartupDialog;
```

## Acceptance Criteria

1. Modal centers correctly at ~80% width, ~70% height
2. Config list shows launch.toml configs first, divider, then launch.json
3. Mode selector cycles through Debug/Profile/Release
4. Flavor and Dart Defines show as text input fields
5. Device list shows devices with emulator launch options
6. Active section highlighted with cyan border
7. Selected items highlighted with inverse colors
8. Footer shows appropriate keybindings
9. Loading state shows "Discovering devices..." with animation
10. Error state displays error message in red

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_startup_dialog_renders() {
        let state = StartupDialogState::new();
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Launch Session"));
        assert!(content.contains("Configuration"));
        assert!(content.contains("Device"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = StartupDialog::centered_rect(area);

        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.width < area.width);
        assert!(centered.height < area.height);
    }
}
```

## Notes

- Reuses patterns from `DeviceSelector` widget
- State is passed by reference, not owned
- Widget is stateless - all state in `StartupDialogState`
- Layout uses ratatui's `Layout::vertical` for sectioning

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**
(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy -- -D warnings` - Pending
- `cargo test startup_dialog` - Pending
