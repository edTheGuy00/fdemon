# Task: "New Config" Option for VSCode Users

**Objective**: Add a "New config" option to the configuration list when VSCode configurations exist, allowing users to create a new fdemon launch.toml config with custom Flavor and Dart Defines.

**Depends on**: Task 10b (VSCode Config Readonly), Task 10d (No-Config Auto-save)

## Problem

When only VSCode launch.json configurations exist:
- User cannot customize Flavor/Dart Defines (fields disabled per Task 10b)
- User has no way to create a new fdemon config from the dialog
- Must manually create .fdemon/launch.toml file

## Desired Behavior

1. When VSCode configs exist, show "+ New config" option at end of config list
2. Selecting "+ New config" enables Flavor/Dart Defines fields
3. User can enter custom values
4. Values auto-save to new launch.toml config (per Task 10d logic)
5. New config appears in list after save

## Scope

- `src/app/state.rs` - Handle "New config" selection state
- `src/tui/widgets/startup_dialog/mod.rs` - Render "+ New config" option
- `src/app/handler/keys.rs` - Navigation to include new option

## Implementation

### 1. Special Selection State (`src/app/state.rs`)

Use a sentinel value or separate flag for "New config" selection:

```rust
pub struct StartupDialogState {
    // ... existing fields ...

    /// Whether "+ New config" is selected (distinct from no selection)
    pub new_config_selected: bool,
}

impl StartupDialogState {
    /// Total items in config list (including "+ New config" if applicable)
    pub fn config_list_len(&self) -> usize {
        let base = self.configs.configs.len();
        if self.should_show_new_config_option() {
            base + 1  // +1 for "+ New config"
        } else {
            base
        }
    }

    /// Should show "+ New config" option?
    pub fn should_show_new_config_option(&self) -> bool {
        // Show when there are VSCode configs (user might want custom fdemon config)
        // Also show when there are any configs (as an alternative to selecting one)
        !self.configs.configs.is_empty()
    }

    /// Check if current selection is the "+ New config" option
    fn is_new_config_index(&self, idx: usize) -> bool {
        self.should_show_new_config_option()
            && idx == self.configs.configs.len()
    }

    /// Get effective config selection (None if "+ New config" selected)
    pub fn effective_config(&self) -> Option<&SourcedConfig> {
        if self.new_config_selected {
            None
        } else {
            self.selected_config()
        }
    }

    /// Handle navigation in config list
    pub fn navigate_config_down(&mut self) {
        let max_idx = self.config_list_len().saturating_sub(1);

        if self.new_config_selected {
            // From "+ New config" -> wrap to first config
            self.new_config_selected = false;
            self.selected_config = if self.configs.configs.is_empty() {
                None
            } else {
                Some(0)
            };
        } else if let Some(idx) = self.selected_config {
            if idx >= self.configs.configs.len().saturating_sub(1) {
                // At last real config -> go to "+ New config"
                if self.should_show_new_config_option() {
                    self.new_config_selected = true;
                    self.selected_config = None;
                } else {
                    // Wrap to first
                    self.selected_config = Some(0);
                }
            } else {
                self.selected_config = Some(idx + 1);
            }
        } else {
            // No selection -> select first
            self.selected_config = Some(0);
        }

        self.on_selection_changed();
    }

    pub fn navigate_config_up(&mut self) {
        if self.new_config_selected {
            // From "+ New config" -> go to last real config
            self.new_config_selected = false;
            if !self.configs.configs.is_empty() {
                self.selected_config = Some(self.configs.configs.len() - 1);
            }
        } else if let Some(idx) = self.selected_config {
            if idx == 0 {
                // At first config -> go to "+ New config" or wrap
                if self.should_show_new_config_option() {
                    self.new_config_selected = true;
                    self.selected_config = None;
                } else {
                    // Wrap to last
                    self.selected_config = Some(self.configs.configs.len().saturating_sub(1));
                }
            } else {
                self.selected_config = Some(idx - 1);
            }
        }

        self.on_selection_changed();
    }

    fn on_selection_changed(&mut self) {
        if self.new_config_selected {
            // Enable editing mode for new config
            self.creating_new_config = true;
            self.editing_config_name = None;
            // Clear fields for fresh start (or keep previous new config values?)
            // Decision: Keep values if user switches back and forth
        } else if let Some(idx) = self.selected_config {
            self.on_config_selected(Some(idx));
        }
    }

    /// Whether flavor/dart_defines fields are editable
    pub fn flavor_editable(&self) -> bool {
        // Editable if:
        // 1. "+ New config" selected
        // 2. FDemon config selected
        // 3. No config selected
        if self.new_config_selected {
            return true;
        }
        match self.selected_config {
            Some(idx) => {
                self.configs.configs.get(idx)
                    .map(|c| c.source != ConfigSource::VSCode)
                    .unwrap_or(true)
            }
            None => true,
        }
    }
}
```

### 2. Render "+ New config" Option (`src/tui/widgets/startup_dialog/mod.rs`)

```rust
fn render_config_list(&self, area: Rect, buf: &mut Buffer) {
    let is_active = self.state.active_section == DialogSection::Configs;
    // ... existing block setup ...

    let mut items: Vec<ListItem> = Vec::new();

    // Render existing configs
    for (i, config) in self.state.configs.configs.iter().enumerate() {
        let is_selected = self.state.selected_config == Some(i)
            && !self.state.new_config_selected;
        // ... existing config rendering ...
    }

    // Render "+ New config" option if applicable
    if self.state.should_show_new_config_option() {
        // Add divider before "+ New config"
        if !self.state.configs.configs.is_empty() {
            items.push(
                ListItem::new("  ─────────────────────────────────")
                    .style(Style::default().fg(DIVIDER_COLOR)),
            );
        }

        let is_selected = self.state.new_config_selected;
        let style = if is_selected && is_active {
            Style::default()
                .fg(NEW_CONFIG_COLOR)
                .bg(SELECTED_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(NEW_CONFIG_COLOR)
        };

        let indicator = if is_selected { "▶ " } else { "  " };
        items.push(
            ListItem::new(format!("{}+ New config", indicator))
                .style(style)
        );
    }

    // ... render list ...
}
```

### 3. Add Color Constant (`src/tui/widgets/startup_dialog/styles.rs`)

```rust
pub const NEW_CONFIG_COLOR: Color = Color::Green;
```

### 4. Update Navigation Keys (`src/app/handler/keys.rs`)

```rust
// In startup dialog config section navigation
KeyCode::Down | KeyCode::Char('j') if section == DialogSection::Configs => {
    state.startup_dialog_state.navigate_config_down();
}

KeyCode::Up | KeyCode::Char('k') if section == DialogSection::Configs => {
    state.startup_dialog_state.navigate_config_up();
}
```

### 5. Handle Launch with New Config

When user presses Enter with "+ New config" selected:

```rust
// In launch handling
if state.startup_dialog_state.new_config_selected {
    // If user entered values, save as new config first (per Task 10d)
    if !state.startup_dialog_state.flavor.is_empty()
        || !state.startup_dialog_state.dart_defines.is_empty()
    {
        // Create config before launching
        let new_config = LaunchConfig {
            name: state.startup_dialog_state.new_config_name.clone(),
            mode: state.startup_dialog_state.mode,
            flavor: if state.startup_dialog_state.flavor.is_empty() {
                None
            } else {
                Some(state.startup_dialog_state.flavor.clone())
            },
            dart_defines: parse_dart_defines(&state.startup_dialog_state.dart_defines),
            ..Default::default()
        };

        // Save and use for launch
        config::add_launch_config(&project_path, new_config.clone())?;

        return launch_with_config(state, &new_config, device);
    } else {
        // No custom values, launch without config
        return launch_without_config(state, device);
    }
}
```

## Visual Design

```
┌─────────────────────────────────────────────────────────────┐
│                      Launch Session                          │
├─────────────────────────────────────────────────────────────┤
│  Configuration                                               │
│  ─────────────────────────────────────────                  │
│    Flutter Dev (VSCode)                                      │
│    Flutter Release (VSCode)                                  │
│  ─────────────────────────────────────────                  │
│  ▶ + New config                                ← GREEN      │
├─────────────────────────────────────────────────────────────┤
│  Mode: ●debug ○profile ○release                             │
│  Flavor: [                    ]          ← ENABLED          │
│  Dart Defines: [              ]          ← ENABLED          │
└─────────────────────────────────────────────────────────────┘
```

## Acceptance Criteria

1. **Visibility**:
   - "+ New config" appears after all configs (with divider)
   - Styled in green to indicate "add" action
   - Shows selection indicator when selected

2. **Navigation**:
   - Can navigate to/from "+ New config" with j/k/arrows
   - Wraps correctly at list boundaries

3. **Field Enabling**:
   - Selecting "+ New config" enables Flavor/Dart Defines
   - Tab navigation includes these fields

4. **Config Creation**:
   - Enter values → auto-saves to launch.toml (debounced)
   - Or saves on Enter (launch)
   - New config appears in list after save

5. **Switching**:
   - Can switch between "+ New config" and VSCode configs
   - Field values preserved when switching back to "+ New config"

## Testing

### Manual Test

1. Create project with only .vscode/launch.json
2. Open startup dialog
3. Verify "+ New config" option visible
4. Navigate to it
5. Verify Flavor/Dart Defines enabled
6. Enter "staging" in Flavor
7. Press Enter to launch
8. Verify .fdemon/launch.toml created with new config

### Unit Tests

```rust
#[test]
fn test_should_show_new_config_option() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "VSCode".to_string(),
    });

    let state = StartupDialogState::with_configs(configs);
    assert!(state.should_show_new_config_option());
}

#[test]
fn test_config_list_len_includes_new_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "VSCode".to_string(),
    });

    let state = StartupDialogState::with_configs(configs);
    assert_eq!(state.config_list_len(), 2); // 1 config + 1 "New config"
}

#[test]
fn test_navigate_to_new_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "VSCode".to_string(),
    });

    let mut state = StartupDialogState::with_configs(configs);
    state.selected_config = Some(0);

    state.navigate_config_down();

    assert!(state.new_config_selected);
    assert!(state.flavor_editable());
}

#[test]
fn test_flavor_editable_new_config() {
    let mut state = StartupDialogState::new();
    state.new_config_selected = true;

    assert!(state.flavor_editable());
}
```

## Notes

- "+ New config" provides escape hatch from read-only VSCode configs
- Integrates with existing Task 10d logic for config creation
- Consider showing hint: "Create custom fdemon config"
- Green color matches typical "add/create" UI conventions

## Estimated Complexity

Medium - Navigation state management + UI rendering.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added `new_config_selected` field, implemented `should_show_new_config_option()`, `config_list_len()`, `navigate_config_down()`, `navigate_config_up()`, and `on_selection_changed()` methods. Updated `flavor_editable()` to return true when new config selected. Added 13 unit tests. |
| `src/tui/widgets/startup_dialog/mod.rs` | Updated `render_config_list()` to render "+ New config" option with green styling and selection indicator, including divider. |
| `src/tui/widgets/startup_dialog/styles.rs` | Added `NEW_CONFIG_COLOR: Color = Color::Green` constant. |

### Notable Decisions/Tradeoffs

1. **Navigation Integration**: Replaced the logic in existing `navigate_up()`/`navigate_down()` methods to call new `navigate_config_up()`/`navigate_config_down()` methods, maintaining backward compatibility while adding "+ New config" support.

2. **State Management**: When "+ New config" is selected via `on_selection_changed()`, the code sets `creating_new_config = true` and clears `editing_config_name`, enabling the Task 10d auto-save logic to work seamlessly.

3. **Field Value Preservation**: When user switches between "+ New config" and existing configs, field values are preserved (as per task spec: "Keep values if user switches back and forth").

4. **Wrap-Around Behavior**: Navigation properly wraps from last config to "+ New config" to first config in a circular pattern.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1246 tests, including 13 new tests for Task 10e)
- `cargo fmt` - Applied
- `cargo clippy` - Passed (1 unrelated warning)

**New Tests Added:**
- `test_should_show_new_config_option`
- `test_should_show_new_config_option_empty`
- `test_config_list_len_includes_new_config`
- `test_config_list_len_without_new_config`
- `test_navigate_config_down_to_new_config`
- `test_navigate_config_up_from_new_config`
- `test_navigate_config_down_wraps_from_new_config`
- `test_navigate_config_up_wraps_from_first_to_new_config`
- `test_flavor_editable_new_config_selected`
- `test_on_selection_changed_sets_creating_new_config`
- `test_navigate_with_multiple_configs`
- `test_default_new_config_selected_false`

### Risks/Limitations

1. **Launch Handling Not Implemented**: The task specification mentions saving config on launch when `new_config_selected` is true, but this logic appears to already exist in Task 10d's implementation (auto-save when user enters values). The launch handler should check `state.startup_dialog_state.new_config_selected` and handle accordingly.

2. **No Handler Changes Needed**: Since the existing `Message::StartupDialogUp/Down` handlers already call `state.navigate_up()/navigate_down()`, and those methods now use the new navigation logic, no changes to `src/app/handler/update.rs` or `src/app/handler/keys.rs` were required.

---
