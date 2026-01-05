## Task: VSCode Config Tab (Read-Only)

**Objective**: Implement the VSCode Config tab displaying Flutter/Dart configurations from `.vscode/launch.json` in read-only mode.

**Depends on**: 05-tab-navigation

**Estimated Time**: 1-1.5 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: Implement `render_vscode_tab()`
- Use existing `src/config/vscode.rs` for parsing (already filters for Dart configs)

### Details

#### Important: Flutter/Dart Only

The existing `config/vscode.rs` already filters for Flutter/Dart configurations only:

```rust
// From src/config/vscode.rs:120-122
fn is_dart_config(config: &VSCodeConfiguration) -> bool {
    config.config_type.to_lowercase() == "dart"
}
```

This means configurations like Node.js, Python, Go, etc. are automatically excluded. Only configurations with `"type": "dart"` are displayed.

#### 1. Load VSCode Configurations

Reuse the existing loader which handles filtering:

```rust
use crate::config::vscode::load_vscode_configs;
use crate::config::types::ResolvedLaunchConfig;

/// Load VSCode launch configurations for display (Dart/Flutter only)
fn load_vscode_display_configs(project_path: &std::path::Path) -> Vec<ResolvedLaunchConfig> {
    // This already filters for type="dart" configurations
    load_vscode_configs(project_path)
}
```

#### 2. VSCode Config Item Generation

```rust
/// Generate read-only settings items for VSCode launch config
fn vscode_config_items(config: &VSCodeLaunchConfig, idx: usize) -> Vec<SettingItem> {
    let prefix = format!("vscode.{}", idx);

    vec![
        SettingItem::new(format!("{}.name", prefix), "Name")
            .description("Configuration name")
            .value(SettingValue::String(config.name.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),

        SettingItem::new(format!("{}.type", prefix), "Type")
            .description("Debugger type")
            .value(SettingValue::String(config.config_type.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),

        SettingItem::new(format!("{}.request", prefix), "Request")
            .description("Launch or attach")
            .value(SettingValue::String(config.request.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),

        SettingItem::new(format!("{}.program", prefix), "Program")
            .description("Entry point file")
            .value(SettingValue::String(
                config.program.clone().unwrap_or_else(|| "default".to_string())
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),

        SettingItem::new(format!("{}.device_id", prefix), "Device ID")
            .description("Target device")
            .value(SettingValue::String(
                config.device_id.clone().unwrap_or_else(|| "auto".to_string())
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),

        SettingItem::new(format!("{}.flutter_mode", prefix), "Flutter Mode")
            .description("Build mode")
            .value(SettingValue::String(
                config.flutter_mode.clone().unwrap_or_else(|| "debug".to_string())
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),

        SettingItem::new(format!("{}.args", prefix), "Arguments")
            .description("Additional arguments")
            .value(SettingValue::List(config.args.clone().unwrap_or_default()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
    ]
}
```

#### 3. Render Function

```rust
impl SettingsPanel<'_> {
    fn render_vscode_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        // Info banner about read-only nature
        let info_area = Rect::new(area.x, area.y, area.width, 3);
        self.render_vscode_info(info_area, buf);

        // Content area
        let content_area = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
        );

        // Load configs
        let configs = load_vscode_display_configs(&self.project_path);

        match configs {
            None => self.render_vscode_not_found(content_area, buf),
            Some(configs) if configs.is_empty() => self.render_vscode_empty(content_area, buf),
            Some(configs) => self.render_vscode_configs(content_area, buf, state, &configs),
        }
    }

    fn render_vscode_info(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" VSCode Launch Configurations ");

        let inner = block.inner(area);
        block.render(area, buf);

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("🔒 ", Style::default()),
                Span::styled(
                    "Read-only view of .vscode/launch.json",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Edit this file directly in VSCode for changes.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ]);

        info.render(inner, buf);
    }

    fn render_vscode_not_found(&self, area: Rect, buf: &mut Buffer) {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "No .vscode/launch.json found",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Create launch configurations in VSCode:",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Run > Add Configuration > Dart & Flutter",
                    Style::default().fg(Color::Cyan),
                ),
            ]),
        ])
        .alignment(Alignment::Center);

        msg.render(area, buf);
    }

    fn render_vscode_empty(&self, area: Rect, buf: &mut Buffer) {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "launch.json exists but has no Dart configurations",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ])
        .alignment(Alignment::Center);

        msg.render(area, buf);
    }

    fn render_vscode_configs(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SettingsViewState,
        configs: &[VSCodeLaunchConfig],
    ) {
        // Generate all items
        let mut all_items: Vec<SettingItem> = Vec::new();
        for (idx, config) in configs.iter().enumerate() {
            all_items.extend(vscode_config_items(config, idx));
        }

        // Render with sections (read-only styling)
        let mut current_section = String::new();
        let mut y = area.y;

        for (idx, item) in all_items.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }

            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1;
                }

                if y < area.bottom() {
                    self.render_vscode_config_header(area.x, y, area.width, buf, &item.section);
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row (read-only)
            if y < area.bottom() {
                let is_selected = idx == state.selected_index;
                self.render_readonly_row(area.x, y, area.width, buf, item, is_selected);
                y += 1;
            }
        }
    }

    fn render_vscode_config_header(&self, x: u16, y: u16, width: u16, buf: &mut Buffer, section: &str) {
        let header_line = format!("─── {} ", section);
        let padding = "─".repeat((width as usize).saturating_sub(header_line.len() + 2));
        let full_header = format!("{}{}", header_line, padding);

        buf.set_string(
            x + 1,
            y,
            &full_header,
            Style::default().fg(Color::Blue), // Blue for VSCode configs
        );
    }

    fn render_readonly_row(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        item: &SettingItem,
        is_selected: bool,
    ) {
        let indicator_width = 3;
        let label_width = 20;
        let value_width = 20;

        // Selection indicator (dimmed for readonly)
        let indicator = if is_selected { "› " } else { "  " };
        let indicator_style = Style::default().fg(Color::DarkGray);
        buf.set_string(x, y, indicator, indicator_style);

        // Label (dimmed)
        let label_style = if is_selected {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };
        let label = truncate_str(&item.label, label_width - 1);
        buf.set_string(x + indicator_width, y, &label, label_style);

        // Value (read-only styling)
        let value_x = x + indicator_width + label_width as u16;
        let value_str = item.value.display();
        let value_style = Style::default().fg(Color::DarkGray);
        let value_display = truncate_str(&value_str, value_width);
        buf.set_string(value_x, y, &value_display, value_style);

        // Lock icon to indicate read-only
        if is_selected {
            let lock_x = value_x + value_display.len() as u16 + 1;
            if lock_x < x + width - 2 {
                buf.set_string(lock_x, y, "🔒", Style::default());
            }
        }
    }
}
```

### Acceptance Criteria

1. VSCode tab shows info banner explaining read-only nature
2. Loads and displays configurations from `.vscode/launch.json`
3. All fields are read-only (no edit mode)
4. "Not found" message if file doesn't exist
5. "Empty" message if file has no Dart configurations
6. Visual distinction from editable tabs (dimmed colors, lock icon)
7. Selection still works for visual feedback (but no editing)
8. Configuration sections match VSCode config structure
9. Unit tests for loading and display

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_vscode_config_items_readonly() {
        let config = VSCodeLaunchConfig {
            name: "Flutter".to_string(),
            config_type: "dart".to_string(),
            request: "launch".to_string(),
            program: Some("lib/main.dart".to_string()),
            device_id: None,
            flutter_mode: Some("debug".to_string()),
            args: None,
        };

        let items = vscode_config_items(&config, 0);

        // All items should be readonly
        for item in &items {
            assert!(item.readonly, "Item {} should be readonly", item.id);
        }
    }

    #[test]
    fn test_load_vscode_not_found() {
        let temp = tempdir().unwrap();
        let configs = load_vscode_display_configs(temp.path());
        assert!(configs.is_none());
    }

    #[test]
    fn test_load_vscode_configs() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let launch_json = r#"{
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "Flutter",
                    "type": "dart",
                    "request": "launch"
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), launch_json).unwrap();

        let configs = load_vscode_display_configs(temp.path());
        assert_eq!(configs.len(), 1);
    }

    #[test]
    fn test_load_vscode_filters_non_dart() {
        // This test verifies that non-Dart configs are filtered out
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let launch_json = r#"{
            "configurations": [
                { "name": "Node.js", "type": "node", "request": "launch" },
                { "name": "Python", "type": "python", "request": "launch" },
                { "name": "Flutter Debug", "type": "dart", "request": "launch" },
                { "name": "Go", "type": "go", "request": "launch" },
                { "name": "Flutter Profile", "type": "dart", "request": "launch" }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), launch_json).unwrap();

        let configs = load_vscode_display_configs(temp.path());

        // Only the 2 Dart configs should be loaded
        assert_eq!(configs.len(), 2);
        assert!(configs.iter().all(|c| c.config.name.contains("Flutter")));
    }

    #[test]
    fn test_render_vscode_tab_not_found() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::VSCodeConfig;

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

        assert!(content.contains("Read-only"));
        assert!(content.contains("No .vscode/launch.json"));
    }
}
```

### Notes

- This tab is purely informational - all editing happens in VSCode
- The lock icon (🔒) provides visual feedback that editing is disabled
- Navigation (j/k) still works for viewing purposes
- Consider adding "Open in VSCode" shortcut (future enhancement)
- The read-only nature prevents accidental modification of team-shared VSCode configs

---

## Completion Summary

**Status:** Done

**Files Modified:**

| File | Changes |
|------|---------|
| `src/tui/widgets/settings_panel.rs` | Added VSCode tab rendering with read-only info banner, config display, and lock icon indicators |

**Implementation Details:**

1. ✅ Implemented `render_vscode_tab()` - full tab renderer with info banner and conditional states
2. ✅ Added `render_vscode_info()` - blue-bordered info box explaining read-only nature
3. ✅ Implemented `render_vscode_not_found()` - message when .vscode/launch.json doesn't exist
4. ✅ Implemented `render_vscode_empty()` - message when file exists but has no Dart configs
5. ✅ Created `render_vscode_config_header()` - blue-styled section headers
6. ✅ Implemented `render_readonly_row()` - dimmed row styling with lock icon
7. ✅ Created `vscode_config_items()` function - generates 6 readonly items per config

**Testing Performed:**
- `cargo fmt` - PASS
- `cargo check` - PASS
- `cargo clippy -- -D warnings` - PASS
- `cargo test settings_panel` - PASS (18 tests)

**Notable Decisions:**
1. Used existing `load_vscode_configs()` from config/vscode.rs which already filters for Dart configs only
2. Detects if launch.json file exists to show appropriate empty state message
3. VSCode configs are read-only with dimmed styling and lock icon (🔒) when selected
4. Uses different selection indicator (`›` vs `▶`) for read-only items
5. Blue color scheme for VSCode tab to differentiate from other tabs
