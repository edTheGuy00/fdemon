## Task: Settings Panel Widget (Base)

**Objective**: Create the full-screen settings panel widget with tab bar header and content area.

**Depends on**: 03-ui-mode-settings

**Estimated Time**: 2-3 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: **NEW** - Full-screen settings widget
- `src/tui/widgets/mod.rs`: Export new widget
- `src/tui/render.rs`: Render settings panel when `UiMode::Settings`

### Details

#### 1. Widget Structure

```rust
//! Settings panel widget - full-screen settings UI
//!
//! Displays a tabbed interface for managing:
//! - Project settings (config.toml)
//! - User preferences (settings.local.toml)
//! - Launch configurations (launch.toml)
//! - VSCode configurations (launch.json, read-only)

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Widget, StatefulWidget},
};

use crate::app::state::SettingsViewState;
use crate::config::{Settings, SettingsTab};

/// Full-screen settings panel widget
pub struct SettingsPanel<'a> {
    /// Reference to application settings
    settings: &'a Settings,

    /// Title to display in header
    title: &'a str,
}

impl<'a> SettingsPanel<'a> {
    pub fn new(settings: &'a Settings) -> Self {
        Self {
            settings,
            title: "Settings",
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
}
```

#### 2. Layout Structure

```
┌──────────────────────────────────────────────────────────────────────┐
│  Settings                                          [Esc] Close       │
├──────────────────────────────────────────────────────────────────────┤
│  ┌──────────┬──────────┬──────────┬──────────┐                       │
│  │ 1.Project│ 2.User   │ 3.Launch │ 4.VSCode │                       │
│  └──────────┴──────────┴──────────┴──────────┘                       │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  [Behavior]                                                          │
│                                                                       │
│  ▶ Auto Start              false                     Toggle on start │
│    Confirm Quit            true                      Ask before quit │
│                                                                       │
│  [Watcher]                                                           │
│                                                                       │
│    Watch Paths             lib                       Dirs to watch   │
│    Debounce (ms)           500                       Delay before... │
│    Auto Reload             true                      Reload on save  │
│    Extensions              dart                      File extensions │
│                                                                       │
│  [UI]                                                                │
│                                                                       │
│    Log Buffer Size         10000                     Max log entries │
│    Show Timestamps         true                      Time in logs    │
│    ...                                                               │
│                                                                       │
├──────────────────────────────────────────────────────────────────────┤
│  Tab/Shift+Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save│
└──────────────────────────────────────────────────────────────────────┘
```

#### 3. StatefulWidget Implementation

```rust
impl StatefulWidget for SettingsPanel<'_> {
    type State = SettingsViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Create main layout
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header with tabs
            Constraint::Min(5),    // Content area
            Constraint::Length(2), // Footer with shortcuts
        ])
        .split(area);

        // Render header with tabs
        self.render_header(chunks[0], buf, state);

        // Render content based on active tab
        self.render_content(chunks[1], buf, state);

        // Render footer with keyboard shortcuts
        self.render_footer(chunks[2], buf, state);
    }
}

impl SettingsPanel<'_> {
    fn render_header(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        // Header block
        let header_block = Block::default()
            .title(format!(" {} ", self.title))
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED);

        let inner = header_block.inner(area);
        header_block.render(area, buf);

        // Tab bar
        let tab_titles: Vec<Line> = [
            SettingsTab::Project,
            SettingsTab::UserPrefs,
            SettingsTab::LaunchConfig,
            SettingsTab::VSCodeConfig,
        ]
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let num = format!("{}.", i + 1);
            Line::from(vec![
                Span::styled(num, Style::default().fg(Color::DarkGray)),
                Span::raw(tab.label()),
            ])
        })
        .collect();

        let tabs = Tabs::new(tab_titles)
            .select(state.active_tab.index())
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(symbols::line::VERTICAL);

        tabs.render(inner, buf);

        // Close hint on right
        let close_hint = " [Esc] Close ";
        let hint_x = area.right().saturating_sub(close_hint.len() as u16 + 1);
        if hint_x > area.x + 10 {
            buf.set_string(
                hint_x,
                area.y,
                close_hint,
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let content_block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_set(symbols::border::ROUNDED);

        let inner = content_block.inner(area);
        content_block.render(area, buf);

        // Dispatch to tab-specific renderer
        match state.active_tab {
            SettingsTab::Project => self.render_project_tab(inner, buf, state),
            SettingsTab::UserPrefs => self.render_user_prefs_tab(inner, buf, state),
            SettingsTab::LaunchConfig => self.render_launch_tab(inner, buf, state),
            SettingsTab::VSCodeConfig => self.render_vscode_tab(inner, buf, state),
        }
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        let footer_block = Block::default()
            .borders(Borders::ALL ^ Borders::TOP)
            .border_set(symbols::border::ROUNDED);

        let inner = footer_block.inner(area);
        footer_block.render(area, buf);

        // Build shortcut text
        let shortcuts = if state.editing {
            "Enter: Confirm  Esc: Cancel"
        } else if state.dirty {
            "Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save (unsaved changes)"
        } else {
            "Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save"
        };

        let footer = Paragraph::new(shortcuts)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));

        footer.render(inner, buf);
    }

    // Placeholder renderers - implemented in subsequent tasks
    fn render_project_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let placeholder = Paragraph::new("Project Settings (config.toml)")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        placeholder.render(area, buf);
    }

    fn render_user_prefs_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let placeholder = Paragraph::new("User Preferences (settings.local.toml)")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        placeholder.render(area, buf);
    }

    fn render_launch_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let placeholder = Paragraph::new("Launch Configurations (launch.toml)")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        placeholder.render(area, buf);
    }

    fn render_vscode_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let placeholder = Paragraph::new("VSCode Configurations (launch.json) - Read Only")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        placeholder.render(area, buf);
    }
}
```

#### 4. Update mod.rs

```rust
// src/tui/widgets/mod.rs
mod settings_panel;
pub use settings_panel::SettingsPanel;
```

#### 5. Update render.rs

```rust
// In render.rs view() function, add match arm:
UiMode::Settings => {
    // Full-screen settings panel
    let settings_panel = widgets::SettingsPanel::new(&state.settings);
    frame.render_stateful_widget(
        settings_panel,
        area,
        &mut state.settings_view_state
    );
}
```

### Acceptance Criteria

1. `SettingsPanel` widget created in new file `tui/widgets/settings_panel.rs`
2. Widget renders full-screen (uses entire terminal area)
3. Header shows title and tab bar with 4 tabs
4. Active tab is visually highlighted
5. Number keys (1-4) shown in tab labels
6. Footer shows context-sensitive keyboard shortcuts
7. Footer shows "unsaved changes" indicator when dirty
8. Placeholder content renders for each tab
9. Widget exported from `tui/widgets/mod.rs`
10. render.rs handles `UiMode::Settings` case
11. Widget compiles without errors

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_settings_panel_renders() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings);
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Settings"));
        assert!(content.contains("Project"));
        assert!(content.contains("User"));
        assert!(content.contains("Launch"));
        assert!(content.contains("VSCode"));
    }

    #[test]
    fn test_settings_panel_shows_active_tab() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::LaunchConfig;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings);
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        // Verify Launch tab content placeholder is shown
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Launch Configurations"));
    }

    #[test]
    fn test_settings_panel_dirty_indicator() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.dirty = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings);
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("unsaved"));
    }
}
```

### Notes

- This task creates the shell; actual tab content is implemented in tasks 06-09
- The widget uses `StatefulWidget` trait to access mutable state for selection tracking
- Consider adding animation for tab switching (future enhancement)
- The layout should gracefully handle small terminal sizes

---

## Completion Summary

**Status:** (Not Started)

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**

(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy -- -D warnings` -
- `cargo test settings_panel` -

**Notable Decisions:**
- (To be filled after implementation)
