## Task: User Preferences Tab

**Objective**: Implement the User Preferences tab displaying settings from `settings.local.toml` (user-specific, gitignored).

**Depends on**: 05-tab-navigation

**Estimated Time**: 1.5-2 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: Implement `render_user_prefs_tab()`

### Details

#### 1. User Preferences Item Generation

```rust
/// Generate settings items for the User Preferences tab
fn user_prefs_items(prefs: &UserPreferences, settings: &Settings) -> Vec<SettingItem> {
    vec![
        // ─────────────────────────────────────────────────────────
        // Editor Overrides
        // ─────────────────────────────────────────────────────────
        SettingItem::new("editor.command", "Editor Command")
            .description("Override project editor setting")
            .value(SettingValue::String(
                prefs.editor.as_ref()
                    .map(|e| e.command.clone())
                    .unwrap_or_default()
            ))
            .default(SettingValue::String(settings.editor.command.clone()))
            .section("Editor Override"),

        SettingItem::new("editor.open_pattern", "Open Pattern")
            .description("Override project open pattern")
            .value(SettingValue::String(
                prefs.editor.as_ref()
                    .map(|e| e.open_pattern.clone())
                    .unwrap_or_default()
            ))
            .default(SettingValue::String(settings.editor.open_pattern.clone()))
            .section("Editor Override"),

        // ─────────────────────────────────────────────────────────
        // UI Preferences
        // ─────────────────────────────────────────────────────────
        SettingItem::new("theme", "Theme Override")
            .description("Personal theme preference")
            .value(SettingValue::Enum {
                value: prefs.theme.clone().unwrap_or_default(),
                options: vec![
                    "".to_string(),  // Use project default
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .default(SettingValue::String(String::new()))
            .section("UI Preferences"),

        // ─────────────────────────────────────────────────────────
        // Session Memory
        // ─────────────────────────────────────────────────────────
        SettingItem::new("last_device", "Last Device")
            .description("Device from last session (auto-saved)")
            .value(SettingValue::String(prefs.last_device.clone().unwrap_or_default()))
            .default(SettingValue::String(String::new()))
            .section("Session Memory"),

        SettingItem::new("last_config", "Last Config")
            .description("Launch config from last session")
            .value(SettingValue::String(prefs.last_config.clone().unwrap_or_default()))
            .default(SettingValue::String(String::new()))
            .section("Session Memory"),
    ]
}
```

#### 2. Render Function with Special Handling

```rust
impl SettingsPanel<'_> {
    fn render_user_prefs_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        // Render info banner about local settings
        let info_area = Rect::new(area.x, area.y, area.width, 3);
        self.render_user_prefs_info(info_area, buf);

        // Content area below info banner
        let content_area = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
        );

        let items = user_prefs_items(&state.user_prefs, self.settings);

        // Group items by section
        let mut current_section = String::new();
        let mut y = content_area.y;

        for (idx, item) in items.iter().enumerate() {
            if y >= content_area.bottom() {
                break;
            }

            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer
                }

                if y < content_area.bottom() {
                    self.render_section_header(content_area.x, y, content_area.width, buf, &item.section);
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row
            if y < content_area.bottom() {
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_setting_row(
                    content_area.x, y, content_area.width, buf,
                    item, is_selected, is_editing, &state.edit_buffer
                );
                y += 1;
            }
        }
    }

    fn render_user_prefs_info(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Local Settings ");

        let inner = block.inner(area);
        block.render(area, buf);

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default()),
                Span::styled(
                    "These settings are stored in .fdemon/settings.local.toml",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "They are gitignored and override project settings for you only.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ]);

        info.render(inner, buf);
    }
}
```

#### 3. Visual Distinction for Override State

```rust
impl SettingsPanel<'_> {
    /// Check if user pref overrides project setting
    fn is_override_active(&self, prefs: &UserPreferences, item_id: &str) -> bool {
        match item_id {
            "editor.command" => prefs.editor.as_ref()
                .map(|e| !e.command.is_empty())
                .unwrap_or(false),
            "editor.open_pattern" => prefs.editor.as_ref()
                .map(|e| e.open_pattern != "$EDITOR $FILE:$LINE")
                .unwrap_or(false),
            "theme" => prefs.theme.is_some(),
            _ => false,
        }
    }

    fn render_user_pref_row(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        item: &SettingItem,
        prefs: &UserPreferences,
        is_selected: bool,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        // Same as render_setting_row but with override indicator
        let indicator_width = 4; // Extra space for override marker
        let label_width = 24;
        let value_width = 15;

        // Override indicator
        let is_override = self.is_override_active(prefs, &item.id);
        let indicator = if is_selected {
            if is_override { "▶⚡" } else { "▶ " }
        } else {
            if is_override { " ⚡" } else { "  " }
        };

        let indicator_style = if is_override {
            Style::default().fg(Color::Yellow)
        } else if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        buf.set_string(x, y, indicator, indicator_style);

        // Rest same as render_setting_row...
        // (Label, Value, Description)
    }
}
```

### Acceptance Criteria

1. User prefs tab shows info banner explaining local settings
2. Settings grouped: Editor Override, UI Preferences, Session Memory
3. Override indicator (⚡) shows when local setting overrides project
4. Empty values shown distinctly (placeholder text or dimmed)
5. "Last Device" and "Last Config" are informational (auto-populated)
6. Settings can be edited (except session memory which is informational)
7. Changes marked as dirty for save prompt
8. Clear visual distinction from Project tab
9. Unit tests for item generation

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_prefs_items_count() {
        let prefs = UserPreferences::default();
        let settings = Settings::default();
        let items = user_prefs_items(&prefs, &settings);

        assert_eq!(items.len(), 5);
    }

    #[test]
    fn test_user_prefs_sections() {
        let prefs = UserPreferences::default();
        let settings = Settings::default();
        let items = user_prefs_items(&prefs, &settings);

        let sections: Vec<&str> = items.iter()
            .map(|i| i.section.as_str())
            .collect();

        assert!(sections.contains(&"Editor Override"));
        assert!(sections.contains(&"UI Preferences"));
        assert!(sections.contains(&"Session Memory"));
    }

    #[test]
    fn test_override_detection() {
        let mut prefs = UserPreferences::default();
        let settings = Settings::default();

        // No override initially
        // (Test using is_override_active method)

        // Add editor override
        prefs.editor = Some(EditorSettings {
            command: "nvim".to_string(),
            open_pattern: "nvim +$LINE $FILE".to_string(),
        });

        // Now should detect override
    }

    #[test]
    fn test_render_user_prefs_tab() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::UserPrefs;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings);
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Check info banner
        assert!(content.contains("Local Settings"));
        assert!(content.contains("gitignored"));

        // Check sections
        assert!(content.contains("[Editor Override]"));
        assert!(content.contains("[Session Memory]"));
    }
}
```

### Notes

- User preferences file is created on first save (not on app start)
- Session memory (last_device, last_config) is auto-populated and informational
- The override indicator helps users understand what they've customized
- Empty override = use project setting (no override active)

---

## Completion Summary

**Status:** Done

**Files Modified:**
| File | Changes |
|------|---------|
| `src/tui/widgets/settings_panel.rs` | Added User Preferences tab rendering with info banner, override detection, and custom row renderer |

**Implementation Details:**

1. ✅ Added `UserPreferences` import
2. ✅ Created `user_prefs_items()` function - generates 5 settings items across 3 sections (Editor Override, UI Preferences, Session Memory)
3. ✅ Implemented `render_user_prefs_tab()` - full tab renderer with info banner
4. ✅ Added `render_user_prefs_info()` - blue-bordered info box about local settings
5. ✅ Implemented `is_override_active()` helper - detects when user prefs override project settings
6. ✅ Created `render_user_pref_row()` - custom row renderer with override indicator (⚡)

**Testing Performed:**
- `cargo fmt` - PASS
- `cargo check` - PASS
- `cargo clippy -- -D warnings` - PASS
- `cargo test settings_panel` - PASS (18 tests)

**Notable Decisions:**
- Override indicator (⚡) appears for any non-empty/non-default user preference
- Empty values displayed as `<empty>` for clarity
- Session Memory items (last_device, last_config) marked as readonly
- Info banner uses blue border to distinguish from regular content
- 4-character indicator width to accommodate both cursor (▶) and override marker (⚡)
