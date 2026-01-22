## Task: Auto-Create Config on Flavor Selection

**Objective**: When a user selects a flavor without having a config selected, automatically create a new default config with that flavor and trigger auto-save.

**Depends on**: Task 01 (Auto-Config Helper)

**Bug Reference**: Bug 6 - No Auto-Creation of Default Config When Flavor/Dart-Defines Set

### Scope

- `src/app/handler/new_session/launch_context.rs`: Modify `handle_flavor_selected()` to create config when none selected

### Details

**Current State:**

The flavor handler only auto-saves if a config is already selected:

```rust
// src/app/handler/new_session/launch_context.rs:136-187
pub fn handle_flavor_selected(state: &mut AppState, flavor: Option<String>) -> UpdateResult {
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

    // Apply flavor
    state.new_session_dialog_state.launch_context.set_flavor(flavor);

    if should_auto_save {
        // ... update config and return AutoSaveConfig action
    } else {
        UpdateResult::none()
    }
}
```

**Implementation:**

```rust
pub fn handle_flavor_selected(state: &mut AppState, flavor: Option<String>) -> UpdateResult {
    let launch_context = &mut state.new_session_dialog_state.launch_context;

    // Check if flavor is editable
    if !launch_context.is_flavor_editable() {
        return UpdateResult::none();
    }

    // Determine if we need to auto-create a config
    let needs_auto_create = launch_context.selected_config_index.is_none()
        && flavor.is_some();  // Only create if setting a flavor (not clearing)

    // Auto-create config if needed
    if needs_auto_create {
        launch_context.create_and_select_default_config();
        // Now selected_config_index is Some, pointing to new config
    }

    // Apply the flavor to state
    launch_context.set_flavor(flavor.clone());

    // Determine if we should auto-save
    let should_auto_save = if let Some(config_idx) = launch_context.selected_config_index {
        let config = &launch_context.configs[config_idx];
        config.source == ConfigSource::FDemon
    } else {
        false
    };

    if should_auto_save {
        // Update the config with new flavor
        if let Some(config_idx) = launch_context.selected_config_index {
            launch_context.configs[config_idx].config.flavor = flavor;
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

/// Build LoadedConfigs from current state for saving
fn build_loaded_configs_for_save(state: &AppState) -> LoadedConfigs {
    let fdemon_configs = state
        .new_session_dialog_state
        .launch_context
        .get_fdemon_configs_for_save();

    LoadedConfigs {
        configs: state
            .new_session_dialog_state
            .launch_context
            .configs
            .iter()
            .map(|c| c.clone())
            .collect(),
        // ... other fields as needed
    }
}
```

**Key Files to Reference:**
- `src/app/handler/new_session/launch_context.rs:136-187` - `handle_flavor_selected()` to modify
- `src/app/new_session_dialog/state.rs` - `LaunchContextState` and helper methods (from Task 01)
- `src/app/handler/mod.rs:99-100` - `UpdateAction::AutoSaveConfig`
- `src/tui/actions.rs:114-136` - Auto-save action handler

### Acceptance Criteria

1. Setting flavor with no config selected creates new "Default" config
2. New config is automatically selected after creation
3. Flavor is applied to the new config
4. Auto-save is triggered to persist config to `.fdemon/launch.toml`
5. Setting flavor to `None` (clearing) does NOT create a new config
6. Existing behavior preserved: editing flavor on existing FDemon config still works
7. VSCode configs remain read-only (no auto-create when VSCode config is selected)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flavor_selected_no_config_creates_default() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        // No config selected
        assert!(state.new_session_dialog_state.launch_context.selected_config_index.is_none());

        let result = handle_flavor_selected(&mut state, Some("development".to_string()));

        // Config should be created and selected
        assert!(state.new_session_dialog_state.launch_context.selected_config_index.is_some());
        let idx = state.new_session_dialog_state.launch_context.selected_config_index.unwrap();
        let config = &state.new_session_dialog_state.launch_context.configs[idx];

        assert_eq!(config.config.name, "Default");
        assert_eq!(config.config.flavor, Some("development".to_string()));
        assert_eq!(config.source, ConfigSource::FDemon);

        // Should trigger auto-save
        assert!(matches!(result.action, Some(UpdateAction::AutoSaveConfig { .. })));
    }

    #[test]
    fn test_flavor_cleared_no_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Clear flavor (set to None) - should NOT create config
        let result = handle_flavor_selected(&mut state, None);

        assert!(state.new_session_dialog_state.launch_context.selected_config_index.is_none());
        assert!(state.new_session_dialog_state.launch_context.configs.is_empty());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_flavor_selected_existing_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add and select existing config
        state.new_session_dialog_state.launch_context.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "Existing".to_string(), ..Default::default() },
            source: ConfigSource::FDemon,
        });
        state.new_session_dialog_state.launch_context.selected_config_index = Some(0);

        let result = handle_flavor_selected(&mut state, Some("staging".to_string()));

        // Should NOT create new config, just update existing
        assert_eq!(state.new_session_dialog_state.launch_context.configs.len(), 1);
        assert_eq!(
            state.new_session_dialog_state.launch_context.configs[0].config.flavor,
            Some("staging".to_string())
        );
    }

    #[test]
    fn test_flavor_selected_vscode_config_no_save() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add VSCode config (read-only)
        state.new_session_dialog_state.launch_context.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "VSCode Config".to_string(), ..Default::default() },
            source: ConfigSource::VsCode,
        });
        state.new_session_dialog_state.launch_context.selected_config_index = Some(0);

        let result = handle_flavor_selected(&mut state, Some("production".to_string()));

        // Should NOT trigger auto-save for VSCode config
        assert!(result.action.is_none());
    }
}
```

### Notes

- The auto-create should happen BEFORE applying the flavor, so the flavor gets set on the new config
- Only create config when setting a flavor (Some), not when clearing (None)
- The `build_loaded_configs_for_save()` helper may already exist or need to be extracted
- Consider logging when auto-creating: `tracing::info!("Auto-created config '{}' for flavor", name)`

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
