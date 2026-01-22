## Task: Auto-Create Config on Dart-Defines Update

**Objective**: When a user sets dart-defines without having a config selected, automatically create a new default config with those dart-defines and trigger auto-save.

**Depends on**: Task 01 (Auto-Config Helper)

**Bug Reference**: Bug 6 - No Auto-Creation of Default Config When Flavor/Dart-Defines Set

### Scope

- `src/app/handler/new_session/launch_context.rs`: Modify `handle_dart_defines_updated()` to create config when none selected

### Details

**Current State:**

The dart-defines handler only auto-saves if a config is already selected:

```rust
// src/app/handler/new_session/launch_context.rs:193-249
pub fn handle_dart_defines_updated(state: &mut AppState, defines: Vec<DartDefine>) -> UpdateResult {
    // ... validation ...

    // Current auto-save check - requires selected_config_index to be Some
    let should_auto_save = if let Some(config_idx) = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
    {
        // Check if FDemon source...
        config.source == ConfigSource::FDemon
    } else {
        false  // ← NO AUTO-SAVE when no config selected
    };

    // Apply dart-defines
    state.new_session_dialog_state.launch_context.set_dart_defines(defines);

    if should_auto_save {
        // ... update config and return AutoSaveConfig action
    } else {
        UpdateResult::none()
    }
}
```

**Implementation:**

```rust
pub fn handle_dart_defines_updated(state: &mut AppState, defines: Vec<DartDefine>) -> UpdateResult {
    let launch_context = &mut state.new_session_dialog_state.launch_context;

    // Check if dart-defines are editable
    if !launch_context.are_dart_defines_editable() {
        return UpdateResult::none();
    }

    // Determine if we need to auto-create a config
    let needs_auto_create = launch_context.selected_config_index.is_none()
        && !defines.is_empty();  // Only create if adding defines (not clearing)

    // Auto-create config if needed
    if needs_auto_create {
        launch_context.create_and_select_default_config();
        // Now selected_config_index is Some, pointing to new config
    }

    // Apply the dart-defines to state
    launch_context.set_dart_defines(defines.clone());

    // Determine if we should auto-save
    let should_auto_save = if let Some(config_idx) = launch_context.selected_config_index {
        let config = &launch_context.configs[config_idx];
        config.source == ConfigSource::FDemon
    } else {
        false
    };

    if should_auto_save {
        // Update the config with new dart-defines
        if let Some(config_idx) = launch_context.selected_config_index {
            // Convert DartDefine to the format stored in LaunchConfig
            let dart_define_strings: Vec<String> = defines
                .iter()
                .map(|d| {
                    if let Some(ref value) = d.value {
                        format!("{}={}", d.key, value)
                    } else {
                        d.key.clone()
                    }
                })
                .collect();

            launch_context.configs[config_idx].config.dart_defines = dart_define_strings;
        }

        // Get configs for saving
        let configs_to_save = build_loaded_configs_for_save(state);

        UpdateResult::action(UpdateAction::AutoSaveConfig {
            configs: configs_to_save,
        })
    } else {
        UpdateResult::none()
    }
}
```

**Key Files to Reference:**
- `src/app/handler/new_session/launch_context.rs:193-249` - `handle_dart_defines_updated()` to modify
- `src/app/new_session_dialog/state.rs` - `LaunchContextState` and helper methods (from Task 01)
- `src/app/new_session_dialog/state.rs` - `DartDefine` struct definition
- `src/config/types.rs:12-61` - `LaunchConfig` struct (dart_defines is `Vec<String>`)

### DartDefine Conversion

The dialog uses `DartDefine` struct:
```rust
pub struct DartDefine {
    pub key: String,
    pub value: Option<String>,
}
```

But `LaunchConfig` stores dart_defines as `Vec<String>` in "KEY=VALUE" format:
```rust
pub struct LaunchConfig {
    pub dart_defines: Vec<String>,  // ["KEY1=value1", "KEY2=value2"]
}
```

The handler needs to convert between these formats when saving.

### Acceptance Criteria

1. Setting dart-defines with no config selected creates new "Default" config
2. New config is automatically selected after creation
3. Dart-defines are applied to the new config in correct format
4. Auto-save is triggered to persist config to `.fdemon/launch.toml`
5. Clearing all dart-defines (empty vec) does NOT create a new config
6. Existing behavior preserved: editing dart-defines on existing FDemon config still works
7. VSCode configs remain read-only (no auto-create when VSCode config is selected)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dart_defines_updated_no_config_creates_default() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        // No config selected
        assert!(state.new_session_dialog_state.launch_context.selected_config_index.is_none());

        let defines = vec![
            DartDefine { key: "API_URL".to_string(), value: Some("https://api.dev".to_string()) },
            DartDefine { key: "DEBUG_MODE".to_string(), value: Some("true".to_string()) },
        ];

        let result = handle_dart_defines_updated(&mut state, defines);

        // Config should be created and selected
        assert!(state.new_session_dialog_state.launch_context.selected_config_index.is_some());
        let idx = state.new_session_dialog_state.launch_context.selected_config_index.unwrap();
        let config = &state.new_session_dialog_state.launch_context.configs[idx];

        assert_eq!(config.config.name, "Default");
        assert_eq!(config.config.dart_defines.len(), 2);
        assert!(config.config.dart_defines.contains(&"API_URL=https://api.dev".to_string()));
        assert!(config.config.dart_defines.contains(&"DEBUG_MODE=true".to_string()));
        assert_eq!(config.source, ConfigSource::FDemon);

        // Should trigger auto-save
        assert!(matches!(result.action, Some(UpdateAction::AutoSaveConfig { .. })));
    }

    #[test]
    fn test_dart_defines_cleared_no_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Clear dart-defines (empty vec) - should NOT create config
        let result = handle_dart_defines_updated(&mut state, vec![]);

        assert!(state.new_session_dialog_state.launch_context.selected_config_index.is_none());
        assert!(state.new_session_dialog_state.launch_context.configs.is_empty());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dart_defines_updated_existing_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add and select existing config
        state.new_session_dialog_state.launch_context.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "Existing".to_string(), ..Default::default() },
            source: ConfigSource::FDemon,
        });
        state.new_session_dialog_state.launch_context.selected_config_index = Some(0);

        let defines = vec![
            DartDefine { key: "ENV".to_string(), value: Some("staging".to_string()) },
        ];

        let result = handle_dart_defines_updated(&mut state, defines);

        // Should NOT create new config, just update existing
        assert_eq!(state.new_session_dialog_state.launch_context.configs.len(), 1);
        assert_eq!(
            state.new_session_dialog_state.launch_context.configs[0].config.dart_defines,
            vec!["ENV=staging".to_string()]
        );
    }

    #[test]
    fn test_dart_define_without_value() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        let defines = vec![
            DartDefine { key: "FEATURE_FLAG".to_string(), value: None },
        ];

        let result = handle_dart_defines_updated(&mut state, defines);

        let idx = state.new_session_dialog_state.launch_context.selected_config_index.unwrap();
        let config = &state.new_session_dialog_state.launch_context.configs[idx];

        // Key without value should be stored as just the key
        assert_eq!(config.config.dart_defines, vec!["FEATURE_FLAG".to_string()]);
    }

    #[test]
    fn test_dart_defines_vscode_config_no_save() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add VSCode config (read-only)
        state.new_session_dialog_state.launch_context.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "VSCode Config".to_string(), ..Default::default() },
            source: ConfigSource::VsCode,
        });
        state.new_session_dialog_state.launch_context.selected_config_index = Some(0);

        let defines = vec![
            DartDefine { key: "KEY".to_string(), value: Some("value".to_string()) },
        ];

        let result = handle_dart_defines_updated(&mut state, defines);

        // Should NOT trigger auto-save for VSCode config
        assert!(result.action.is_none());
    }
}
```

### Notes

- Similar pattern to Task 02 (flavor handler) - consider extracting common auto-create logic
- The `DartDefine` to `String` conversion may already exist somewhere - check for existing helpers
- Only create config when adding defines, not when clearing (empty vec)
- Consider logging when auto-creating: `tracing::info!("Auto-created config '{}' for dart-defines", name)`
- The dart-defines modal closes with "Save & Close" which triggers this handler

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (to be filled after implementation)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy` -
- `cargo test` -

**Notable Decisions:**
- (to be filled after implementation)

**Risks/Limitations:**
- (to be filled after implementation)
