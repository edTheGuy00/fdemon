## Task: Project Settings Tab

**Objective**: Implement the Project Settings tab displaying settings from `config.toml` organized by section.

**Depends on**: 05-tab-navigation

**Estimated Time**: 1.5-2 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: Implement `render_project_tab()`

### Details

#### 1. Settings Item List Structure

```rust
/// Generate settings items for the Project tab from Settings struct
fn project_settings_items(settings: &Settings) -> Vec<SettingItem> {
    vec![
        // ─────────────────────────────────────────────────────────
        // Behavior Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("behavior.auto_start", "Auto Start")
            .description("Skip device selector and start immediately")
            .value(SettingValue::Bool(settings.behavior.auto_start))
            .default(SettingValue::Bool(false))
            .section("Behavior"),

        SettingItem::new("behavior.confirm_quit", "Confirm Quit")
            .description("Ask before quitting with running apps")
            .value(SettingValue::Bool(settings.behavior.confirm_quit))
            .default(SettingValue::Bool(true))
            .section("Behavior"),

        // ─────────────────────────────────────────────────────────
        // Watcher Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("watcher.paths", "Watch Paths")
            .description("Directories to watch for changes")
            .value(SettingValue::List(settings.watcher.paths.clone()))
            .default(SettingValue::List(vec!["lib".to_string()]))
            .section("Watcher"),

        SettingItem::new("watcher.debounce_ms", "Debounce (ms)")
            .description("Delay before triggering reload")
            .value(SettingValue::Number(settings.watcher.debounce_ms as i64))
            .default(SettingValue::Number(500))
            .section("Watcher"),

        SettingItem::new("watcher.auto_reload", "Auto Reload")
            .description("Hot reload on file changes")
            .value(SettingValue::Bool(settings.watcher.auto_reload))
            .default(SettingValue::Bool(true))
            .section("Watcher"),

        SettingItem::new("watcher.extensions", "Extensions")
            .description("File extensions to watch")
            .value(SettingValue::List(settings.watcher.extensions.clone()))
            .default(SettingValue::List(vec!["dart".to_string()]))
            .section("Watcher"),

        // ─────────────────────────────────────────────────────────
        // UI Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("ui.log_buffer_size", "Log Buffer Size")
            .description("Maximum log entries to keep")
            .value(SettingValue::Number(settings.ui.log_buffer_size as i64))
            .default(SettingValue::Number(10000))
            .section("UI"),

        SettingItem::new("ui.show_timestamps", "Show Timestamps")
            .description("Display timestamps in logs")
            .value(SettingValue::Bool(settings.ui.show_timestamps))
            .default(SettingValue::Bool(true))
            .section("UI"),

        SettingItem::new("ui.compact_logs", "Compact Logs")
            .description("Collapse similar consecutive logs")
            .value(SettingValue::Bool(settings.ui.compact_logs))
            .default(SettingValue::Bool(false))
            .section("UI"),

        SettingItem::new("ui.theme", "Theme")
            .description("Color theme")
            .value(SettingValue::Enum {
                value: settings.ui.theme.clone(),
                options: vec!["default".to_string(), "dark".to_string(), "light".to_string()],
            })
            .default(SettingValue::String("default".to_string()))
            .section("UI"),

        SettingItem::new("ui.stack_trace_collapsed", "Collapse Stack Traces")
            .description("Start stack traces collapsed")
            .value(SettingValue::Bool(settings.ui.stack_trace_collapsed))
            .default(SettingValue::Bool(true))
            .section("UI"),

        SettingItem::new("ui.stack_trace_max_frames", "Max Frames")
            .description("Frames shown when collapsed")
            .value(SettingValue::Number(settings.ui.stack_trace_max_frames as i64))
            .default(SettingValue::Number(3))
            .section("UI"),

        // ─────────────────────────────────────────────────────────
        // DevTools Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("devtools.auto_open", "Auto Open DevTools")
            .description("Open DevTools when app starts")
            .value(SettingValue::Bool(settings.devtools.auto_open))
            .default(SettingValue::Bool(false))
            .section("DevTools"),

        SettingItem::new("devtools.browser", "Browser")
            .description("Browser for DevTools (empty = default)")
            .value(SettingValue::String(settings.devtools.browser.clone()))
            .default(SettingValue::String(String::new()))
            .section("DevTools"),

        // ─────────────────────────────────────────────────────────
        // Editor Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("editor.command", "Editor Command")
            .description("Editor to open files (empty = auto-detect)")
            .value(SettingValue::String(settings.editor.command.clone()))
            .default(SettingValue::String(String::new()))
            .section("Editor"),

        SettingItem::new("editor.open_pattern", "Open Pattern")
            .description("Pattern for opening files ($FILE, $LINE, $COLUMN)")
            .value(SettingValue::String(settings.editor.open_pattern.clone()))
            .default(SettingValue::String("$EDITOR $FILE:$LINE".to_string()))
            .section("Editor"),
    ]
}
```

#### 2. Render Function

```rust
impl SettingsPanel<'_> {
    fn render_project_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let items = project_settings_items(self.settings);

        // Group items by section
        let mut current_section = String::new();
        let mut y = area.y;

        for (idx, item) in items.iter().enumerate() {
            if y >= area.bottom() {
                break; // Out of space
            }

            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer between sections
                }

                if y < area.bottom() {
                    self.render_section_header(area.x, y, area.width, buf, &item.section);
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row
            if y < area.bottom() {
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_setting_row(area.x, y, area.width, buf, item, is_selected, is_editing, &state.edit_buffer);
                y += 1;
            }
        }
    }

    fn render_section_header(&self, x: u16, y: u16, width: u16, buf: &mut Buffer, section: &str) {
        let header = format!("[{}]", section);
        buf.set_string(
            x + 1,
            y,
            &header,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    }

    fn render_setting_row(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        item: &SettingItem,
        is_selected: bool,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        // Layout: [indicator] [label............] [value.....] [description]
        let indicator_width = 3;
        let label_width = 25;
        let value_width = 15;
        let desc_start = indicator_width + label_width + value_width + 2;

        // Selection indicator
        let indicator = if is_selected { "▶ " } else { "  " };
        let indicator_style = if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        buf.set_string(x, y, indicator, indicator_style);

        // Label
        let label_style = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let label = truncate_str(&item.label, label_width as usize - 1);
        buf.set_string(x + indicator_width, y, &label, label_style);

        // Value
        let value_x = x + indicator_width + label_width as u16;
        let (value_str, value_style) = if is_editing {
            (
                format!("{}▌", edit_buffer), // Show cursor
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            )
        } else {
            let modified_indicator = if item.is_modified() { "*" } else { "" };
            (
                format!("{}{}", item.value.display(), modified_indicator),
                self.value_style(&item.value, is_selected),
            )
        };
        let value_display = truncate_str(&value_str, value_width as usize);
        buf.set_string(value_x, y, &value_display, value_style);

        // Description (dimmed)
        if width > desc_start as u16 + 10 {
            let desc_width = (width - desc_start as u16) as usize;
            let desc = truncate_str(&item.description, desc_width);
            buf.set_string(
                x + desc_start as u16,
                y,
                &desc,
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    fn value_style(&self, value: &SettingValue, is_selected: bool) -> Style {
        let base = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        match value {
            SettingValue::Bool(true) => base.fg(Color::Green),
            SettingValue::Bool(false) => base.fg(Color::Red),
            SettingValue::Number(_) | SettingValue::Float(_) => base.fg(Color::Cyan),
            SettingValue::String(s) if s.is_empty() => base.fg(Color::DarkGray),
            SettingValue::String(_) => base.fg(Color::White),
            SettingValue::Enum { .. } => base.fg(Color::Magenta),
            SettingValue::List(_) => base.fg(Color::Blue),
        }
    }
}

/// Truncate string with ellipsis if too long
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        s.chars().take(max_len).collect()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}
```

### Acceptance Criteria

1. Project tab displays all settings from `config.toml`
2. Settings grouped by section with headers (Behavior, Watcher, UI, DevTools, Editor)
3. Each setting shows: indicator, label, value, description
4. Selected setting highlighted with `▶` and bold text
5. Boolean values color-coded (green=true, red=false)
6. Modified values marked with `*` indicator
7. Descriptions shown in dim color
8. Long values/descriptions truncated with ellipsis
9. Edit mode shows cursor in value field
10. Unit tests for item generation and rendering

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_settings_items_count() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);

        // Should have 16 items across 5 sections
        assert_eq!(items.len(), 16);
    }

    #[test]
    fn test_project_settings_sections() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);

        let sections: Vec<&str> = items.iter().map(|i| i.section.as_str()).collect();
        assert!(sections.contains(&"Behavior"));
        assert!(sections.contains(&"Watcher"));
        assert!(sections.contains(&"UI"));
        assert!(sections.contains(&"DevTools"));
        assert!(sections.contains(&"Editor"));
    }

    #[test]
    fn test_setting_is_modified() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);

        // Default values should not be modified
        for item in &items {
            assert!(!item.is_modified(), "Item {} should not be modified", item.id);
        }
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is long", 8), "this is…");
        assert_eq!(truncate_str("ab", 2), "ab");
        assert_eq!(truncate_str("abc", 2), "a…");
    }

    #[test]
    fn test_render_project_tab() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::Project;

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

        // Check sections are rendered
        assert!(content.contains("[Behavior]"));
        assert!(content.contains("[Watcher]"));
        assert!(content.contains("[UI]"));

        // Check some settings are rendered
        assert!(content.contains("Auto Start"));
        assert!(content.contains("Debounce"));
        assert!(content.contains("Log Buffer"));
    }
}
```

### Notes

- Item generation is separate from rendering for testability
- Section headers help organize the settings visually
- The `*` indicator shows unsaved changes at a glance
- Future: Add scrolling support for terminals smaller than content height

---

## Completion Summary

**Status:** Done

**Files Modified:**

| File | Changes |
|------|---------|
| `src/tui/widgets/settings_panel.rs` | Implemented Project Settings tab with setting items generation, rendering functions, and helper utilities |
| `src/tui/render.rs` | Updated SettingsPanel::new() call to include project_path parameter |

**Implementation Details:**

1. **project_settings_items() function**: Generates 16 setting items across 5 sections (Behavior, Watcher, UI, DevTools, Editor)
2. **render_project_tab()**: Renders settings grouped by section with proper spacing
3. **render_section_header()**: Renders yellow bold section headers in `[Section]` format
4. **render_setting_row()**: Displays each setting with indicator, label, value, and description
5. **value_style()**: Color-codes values (green=true, red=false, cyan=numbers, etc.)
6. **truncate_str()**: Helper to truncate strings with ellipsis
7. Added comprehensive tests covering item count, sections, modifications, truncation, and rendering

**Testing Performed:**
- `cargo fmt` - PASS
- `cargo check` - PASS
- `cargo clippy -- -D warnings` - PASS
- `cargo test settings_panel` - PASS (18 tests passed)

**Notable Decisions:**
1. **Type annotations for layout widths**: Added explicit `u16` type annotations to avoid unnecessary casts detected by clippy
2. **Enum default value**: Used `SettingValue::Enum` for theme default (not String) to ensure proper equality comparison
3. **Allow clippy::too_many_arguments**: Added allow attribute for `render_setting_row()` as all 10 parameters are necessary for rendering
4. **Modified indicator**: Shows `*` suffix on modified values to provide visual feedback of unsaved changes
5. **Edit mode cursor**: Shows `▌` character in edit mode to indicate active editing
