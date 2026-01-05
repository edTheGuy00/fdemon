## Task: Launch Config Tab

**Objective**: Implement the Launch Config tab displaying and editing configurations from `launch.toml`.

**Depends on**: 05-tab-navigation

**Estimated Time**: 1.5-2 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: Implement `render_launch_tab()`
- `src/config/launch.rs`: Add method to load configs for display

### Details

#### 1. Load Launch Configurations

Add to `settings_panel.rs` or create helper:

```rust
use crate::config::{LaunchFile, LaunchConfig, ResolvedLaunchConfig, ConfigSource};

/// Load launch configurations for the settings panel
fn load_launch_configs(project_path: &std::path::Path) -> Vec<ResolvedLaunchConfig> {
    let launch_path = project_path.join(".fdemon").join("launch.toml");

    if !launch_path.exists() {
        return vec![]; // No launch.toml yet
    }

    match std::fs::read_to_string(&launch_path) {
        Ok(content) => match toml::from_str::<LaunchFile>(&content) {
            Ok(file) => file.configurations.into_iter()
                .map(|config| ResolvedLaunchConfig {
                    config,
                    source: ConfigSource::FDemon,
                })
                .collect(),
            Err(_) => vec![],
        },
        Err(_) => vec![],
    }
}
```

#### 2. Launch Config Item Generation

```rust
/// Generate settings items for a single launch configuration
fn launch_config_items(config: &LaunchConfig, idx: usize) -> Vec<SettingItem> {
    let prefix = format!("launch.{}", idx);

    vec![
        SettingItem::new(format!("{}.name", prefix), "Name")
            .description("Configuration display name")
            .value(SettingValue::String(config.name.clone()))
            .section(format!("Configuration {}", idx + 1)),

        SettingItem::new(format!("{}.device", prefix), "Device")
            .description("Target device ID or 'auto'")
            .value(SettingValue::String(config.device.clone()))
            .default(SettingValue::String("auto".to_string()))
            .section(format!("Configuration {}", idx + 1)),

        SettingItem::new(format!("{}.mode", prefix), "Mode")
            .description("Flutter build mode")
            .value(SettingValue::Enum {
                value: config.mode.to_string(),
                options: vec![
                    "debug".to_string(),
                    "profile".to_string(),
                    "release".to_string(),
                ],
            })
            .default(SettingValue::String("debug".to_string()))
            .section(format!("Configuration {}", idx + 1)),

        SettingItem::new(format!("{}.flavor", prefix), "Flavor")
            .description("Build flavor (optional)")
            .value(SettingValue::String(config.flavor.clone().unwrap_or_default()))
            .default(SettingValue::String(String::new()))
            .section(format!("Configuration {}", idx + 1)),

        SettingItem::new(format!("{}.auto_start", prefix), "Auto Start")
            .description("Start this config automatically")
            .value(SettingValue::Bool(config.auto_start))
            .default(SettingValue::Bool(false))
            .section(format!("Configuration {}", idx + 1)),

        SettingItem::new(format!("{}.dart_defines", prefix), "Dart Defines")
            .description("--dart-define values")
            .value(SettingValue::List(
                config.dart_defines.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect()
            ))
            .default(SettingValue::List(vec![]))
            .section(format!("Configuration {}", idx + 1)),

        SettingItem::new(format!("{}.extra_args", prefix), "Extra Args")
            .description("Additional flutter run arguments")
            .value(SettingValue::List(config.extra_args.clone()))
            .default(SettingValue::List(vec![]))
            .section(format!("Configuration {}", idx + 1)),
    ]
}
```

#### 3. Render Function

```rust
impl SettingsPanel<'_> {
    fn render_launch_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        // Load configs (in real impl, this would be cached in state)
        let configs = load_launch_configs(&self.project_path);

        if configs.is_empty() {
            self.render_launch_empty_state(area, buf);
            return;
        }

        // Generate all items
        let mut all_items: Vec<SettingItem> = Vec::new();
        for (idx, resolved) in configs.iter().enumerate() {
            all_items.extend(launch_config_items(&resolved.config, idx));
        }

        // Render with sections
        let mut current_section = String::new();
        let mut y = area.y;

        for (idx, item) in all_items.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }

            // Section header (configuration separator)
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer
                }

                if y < area.bottom() {
                    self.render_config_header(area.x, y, area.width, buf, &item.section);
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row
            if y < area.bottom() {
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_setting_row(
                    area.x, y, area.width, buf,
                    item, is_selected, is_editing, &state.edit_buffer
                );
                y += 1;
            }
        }

        // Add "New Configuration" option at bottom
        if y + 2 < area.bottom() {
            y += 1; // Spacer
            let is_selected = state.selected_index == all_items.len();
            self.render_add_config_option(area.x, y, area.width, buf, is_selected);
        }
    }

    fn render_launch_empty_state(&self, area: Rect, buf: &mut Buffer) {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "No launch configurations found",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Create .fdemon/launch.toml or press ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("n", Style::default().fg(Color::Cyan)),
                Span::styled(
                    " to create one.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ])
        .alignment(Alignment::Center);

        empty.render(area, buf);
    }

    fn render_config_header(&self, x: u16, y: u16, width: u16, buf: &mut Buffer, section: &str) {
        // Configuration header with visual separator
        let header_line = format!("─── {} ", section);
        let padding = "─".repeat((width as usize).saturating_sub(header_line.len() + 2));
        let full_header = format!("{}{}", header_line, padding);

        buf.set_string(
            x + 1,
            y,
            &full_header,
            Style::default().fg(Color::Cyan),
        );
    }

    fn render_add_config_option(&self, x: u16, y: u16, width: u16, buf: &mut Buffer, is_selected: bool) {
        let indicator = if is_selected { "▶ " } else { "  " };
        let style = if is_selected {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };

        buf.set_string(x, y, indicator, style);
        buf.set_string(x + 2, y, "+ Add New Configuration", style);
    }
}
```

#### 4. Add State for Configs

```rust
// In SettingsViewState
pub struct SettingsViewState {
    // ... existing fields ...

    /// Loaded launch configurations (cached)
    pub launch_configs: Vec<ResolvedLaunchConfig>,
}

impl SettingsViewState {
    /// Load launch configs from disk
    pub fn load_launch_configs(&mut self, project_path: &std::path::Path) {
        self.launch_configs = load_launch_configs(project_path);
    }
}
```

### Acceptance Criteria

1. Launch tab loads configurations from `.fdemon/launch.toml`
2. Each configuration displayed as a collapsible section
3. Configuration fields editable: name, device, mode, flavor, auto_start
4. Dart defines shown as key=value list
5. Extra args shown as list
6. Empty state message when no configs exist
7. "Add New Configuration" option at bottom
8. Enum fields (mode) show dropdown options
9. Changes marked as dirty for save prompt
10. Unit tests for config loading and item generation

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_launch_config_items() {
        let config = LaunchConfig {
            name: "Development".to_string(),
            device: "iphone".to_string(),
            mode: FlutterMode::Debug,
            flavor: Some("dev".to_string()),
            auto_start: true,
            dart_defines: [("API_URL".to_string(), "https://dev.api.com".to_string())]
                .into_iter().collect(),
            extra_args: vec!["--verbose".to_string()],
            entry_point: None,
        };

        let items = launch_config_items(&config, 0);

        assert_eq!(items.len(), 7);
        assert!(items.iter().any(|i| i.id == "launch.0.name"));
        assert!(items.iter().any(|i| i.id == "launch.0.mode"));
    }

    #[test]
    fn test_load_launch_configs_empty() {
        let temp = tempdir().unwrap();
        let configs = load_launch_configs(temp.path());
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_launch_configs_from_file() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        let launch_toml = r#"
[[configurations]]
name = "Test Config"
device = "auto"
mode = "debug"
"#;
        std::fs::write(fdemon_dir.join("launch.toml"), launch_toml).unwrap();

        let configs = load_launch_configs(temp.path());
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.name, "Test Config");
    }

    #[test]
    fn test_render_launch_tab_empty() {
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

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("No launch configurations"));
    }
}
```

### Notes

- Configurations are loaded once when entering the tab, cached in state
- Consider adding delete confirmation for removing configurations
- The "Add New Configuration" option will be implemented in Phase 5 (Startup Configuration UI)
- Dart defines editing uses list editor (implemented in task 10)

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
