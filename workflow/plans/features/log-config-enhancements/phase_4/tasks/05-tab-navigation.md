## Task: Tab Navigation

**Objective**: Implement full tab navigation functionality with keyboard shortcuts and visual feedback.

**Depends on**: 04-settings-widget

**Estimated Time**: 1.5-2 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: Enhanced tab bar rendering and state tracking

### Details

#### 1. Enhanced Tab Bar Rendering

```rust
impl SettingsPanel<'_> {
    fn render_header(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        // Header block with title
        let block = Block::default()
            .title(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    self.title,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ]))
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED);

        let inner = block.inner(area);
        block.render(area, buf);

        // Calculate tab positions
        let tab_width = inner.width / 4;
        let tabs_area = Rect::new(inner.x, inner.y, inner.width, 1);

        // Render each tab
        for (i, tab) in [
            SettingsTab::Project,
            SettingsTab::UserPrefs,
            SettingsTab::LaunchConfig,
            SettingsTab::VSCodeConfig,
        ]
        .iter()
        .enumerate()
        {
            let is_active = *tab == state.active_tab;
            let tab_area = Rect::new(
                tabs_area.x + (i as u16 * tab_width),
                tabs_area.y,
                tab_width.min(tabs_area.width.saturating_sub(i as u16 * tab_width)),
                1,
            );

            self.render_tab(tab_area, buf, tab, is_active);
        }

        // Close hint
        let close_hint = "[Esc] Close ";
        let hint_x = area.right().saturating_sub(close_hint.len() as u16 + 2);
        if hint_x > area.x + 20 {
            buf.set_string(
                hint_x,
                area.y,
                close_hint,
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    fn render_tab(&self, area: Rect, buf: &mut Buffer, tab: &SettingsTab, is_active: bool) {
        let num = format!("{}.", tab.index() + 1);
        let label = tab.label();

        // Calculate centering
        let total_len = num.len() + label.len();
        let padding = area.width.saturating_sub(total_len as u16) / 2;

        let (num_style, label_style, bg_style) = if is_active {
            (
                Style::default()
                    .fg(Color::DarkGray)
                    .bg(Color::Cyan),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                Style::default().bg(Color::Cyan),
            )
        } else {
            (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::White),
                Style::default(),
            )
        };

        // Fill background for active tab
        if is_active {
            for x in area.x..area.right() {
                buf.get_mut(x, area.y).set_style(bg_style);
            }
        }

        // Render number and label
        let x = area.x + padding;
        buf.set_string(x, area.y, &num, num_style);
        buf.set_string(x + num.len() as u16, area.y, label, label_style);
    }
}
```

#### 2. Tab Underline Indicator

Add visual underline to show active tab:

```rust
fn render_tab_underline(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
    let tab_width = area.width / 4;
    let active_idx = state.active_tab.index();

    // Draw underline for active tab
    let underline_x = area.x + (active_idx as u16 * tab_width);
    let underline_width = tab_width.min(area.width.saturating_sub(active_idx as u16 * tab_width));

    for x in underline_x..underline_x + underline_width {
        if x < area.right() {
            buf.get_mut(x, area.y)
                .set_char('─')
                .set_style(Style::default().fg(Color::Cyan));
        }
    }
}
```

#### 3. Tab Indicator Icons (Optional Enhancement)

```rust
impl SettingsTab {
    /// Icon for tab (optional visual enhancement)
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Project => "⚙",      // Gear for project settings
            Self::UserPrefs => "👤",   // Person for user prefs
            Self::LaunchConfig => "▶", // Play for launch
            Self::VSCodeConfig => "📁", // Folder for VSCode
        }
    }

    /// Whether this tab is read-only
    pub fn is_readonly(&self) -> bool {
        matches!(self, Self::VSCodeConfig)
    }
}
```

#### 4. Update Key Handlers for Tab Count Awareness

In `app/handler/update.rs`, update to use actual item counts:

```rust
Message::SettingsNextItem => {
    let item_count = get_item_count_for_tab(&state.settings, state.settings_view_state.active_tab);
    state.settings_view_state.select_next(item_count);
    UpdateResult::default()
}

Message::SettingsPrevItem => {
    let item_count = get_item_count_for_tab(&state.settings, state.settings_view_state.active_tab);
    state.settings_view_state.select_previous(item_count);
    UpdateResult::default()
}

/// Get the number of items in a settings tab
fn get_item_count_for_tab(settings: &Settings, tab: SettingsTab) -> usize {
    match tab {
        SettingsTab::Project => {
            // behavior (2) + watcher (4) + ui (6) + devtools (2) + editor (2) = 16
            16
        }
        SettingsTab::UserPrefs => {
            // editor (2) + theme (1) + last_device (1) + last_config (1) = 5
            5
        }
        SettingsTab::LaunchConfig => {
            // Dynamic based on loaded configs
            // For now, estimate
            10
        }
        SettingsTab::VSCodeConfig => {
            // Dynamic based on loaded configs
            5
        }
    }
}
```

### Acceptance Criteria

1. Tab bar renders with 4 tabs horizontally distributed
2. Active tab has distinctive visual style (background color, bold text)
3. Tab numbers (1-4) shown in dimmed color before label
4. Number keys 1-4 immediately switch to corresponding tab
5. Tab/Shift+Tab cycles through tabs correctly
6. Selection index resets to 0 when switching tabs
7. Tab underline indicator shows active tab (optional)
8. Close hint visible on header right side
9. Unit tests for tab rendering and navigation

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_navigation_wraps() {
        let mut state = SettingsViewState::new();
        assert_eq!(state.active_tab, SettingsTab::Project);

        // Forward through all tabs
        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::UserPrefs);
        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::LaunchConfig);
        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);
        state.next_tab(); // Wrap
        assert_eq!(state.active_tab, SettingsTab::Project);
    }

    #[test]
    fn test_tab_switch_resets_selection() {
        let mut state = SettingsViewState::new();
        state.selected_index = 5;

        state.next_tab();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_tab_switch_exits_edit_mode() {
        let mut state = SettingsViewState::new();
        state.editing = true;
        state.edit_buffer = "test".to_string();

        state.next_tab();
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
    }

    #[test]
    fn test_goto_tab() {
        let mut state = SettingsViewState::new();

        state.goto_tab(SettingsTab::VSCodeConfig);
        assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_tab_readonly() {
        assert!(!SettingsTab::Project.is_readonly());
        assert!(!SettingsTab::UserPrefs.is_readonly());
        assert!(!SettingsTab::LaunchConfig.is_readonly());
        assert!(SettingsTab::VSCodeConfig.is_readonly());
    }

    #[test]
    fn test_render_shows_all_tabs() {
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

        assert!(content.contains("1.Project"));
        assert!(content.contains("2.User"));
        assert!(content.contains("3.Launch"));
        assert!(content.contains("4.VSCode"));
    }
}
```

### Notes

- Tab width is calculated to distribute evenly across header
- On narrow terminals, tab labels may need truncation
- The underline indicator provides additional visual feedback
- Consider adding transition animation between tabs (future)

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
