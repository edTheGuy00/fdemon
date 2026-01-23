## Task: Add Auto-Config Creation Helper Methods

**Objective**: Add helper methods to create a new default config, add it to the dialog state, and select it - providing infrastructure for auto-config creation in flavor and dart-defines handlers.

**Depends on**: None

**Bug Reference**: Bug 6 - No Auto-Creation of Default Config When Flavor/Dart-Defines Set

### Scope

- `src/app/new_session_dialog/state.rs`: Add `create_and_select_default_config()` helper method to `LaunchContextState`
- `src/config/launch.rs`: Verify existing helper functions work for this use case

### Details

**Current State:**

The dialog state (`LaunchContextState`) manages configs but has no method to create a new config on-the-fly:

```rust
// src/app/new_session_dialog/state.rs
pub struct LaunchContextState {
    pub configs: Vec<ConfigWithSource>,
    pub selected_config_index: Option<usize>,
    pub mode: FlutterMode,
    pub flavor: Option<String>,
    pub dart_defines: Vec<DartDefine>,
    // ...
}
```

Helper functions exist in `config/launch.rs` but aren't wired to the dialog state:
- `create_default_launch_config()` - creates a config template
- `add_launch_config()` - adds config with unique naming

**Implementation:**

**Step 1:** Add helper method to `LaunchContextState` (`src/app/new_session_dialog/state.rs`):

```rust
impl LaunchContextState {
    /// Creates a new default config, adds it to the config list, and selects it.
    /// Returns the index of the newly created config.
    ///
    /// This is used when the user sets flavor or dart-defines without having
    /// a config selected - we auto-create a config to persist their choices.
    pub fn create_and_select_default_config(&mut self) -> usize {
        use crate::config::launch::{add_launch_config, create_default_launch_config};
        use crate::config::types::ConfigSource;

        // Create a new default config with current mode
        let mut new_config = create_default_launch_config();
        new_config.mode = self.mode;

        // Generate unique name if "Default" already exists
        let existing_names: Vec<&str> = self.configs
            .iter()
            .map(|c| c.config.name.as_str())
            .collect();

        let unique_name = generate_unique_name("Default", &existing_names);
        new_config.name = unique_name;

        // Wrap in ConfigWithSource (FDemon source so it's editable and saveable)
        let config_with_source = ConfigWithSource {
            config: new_config,
            source: ConfigSource::FDemon,
        };

        // Add to configs list
        self.configs.push(config_with_source);

        // Select the new config
        let new_index = self.configs.len() - 1;
        self.selected_config_index = Some(new_index);

        new_index
    }
}

/// Generate a unique name by appending numbers if needed.
/// "Default" -> "Default", "Default 2", "Default 3", etc.
fn generate_unique_name(base_name: &str, existing_names: &[&str]) -> String {
    if !existing_names.contains(&base_name) {
        return base_name.to_string();
    }

    let mut counter = 2;
    loop {
        let candidate = format!("{} {}", base_name, counter);
        if !existing_names.contains(&candidate.as_str()) {
            return candidate;
        }
        counter += 1;
    }
}
```

**Step 2:** Add method to get updated `LoadedConfigs` for saving:

```rust
impl LaunchContextState {
    /// Returns the current configs as LoadedConfigs for saving.
    /// Only includes FDemon configs (VSCode configs are read-only).
    pub fn get_fdemon_configs_for_save(&self) -> Vec<LaunchConfig> {
        self.configs
            .iter()
            .filter(|c| c.source == ConfigSource::FDemon)
            .map(|c| c.config.clone())
            .collect()
    }
}
```

**Key Files to Reference:**
- `src/app/new_session_dialog/state.rs:404-558` - `LaunchContextState` definition
- `src/config/launch.rs:151-163` - `create_default_launch_config()`
- `src/config/launch.rs:165-186` - `add_launch_config()` (for reference on unique naming)
- `src/config/types.rs:89-95` - `ConfigWithSource` struct
- `src/config/types.rs:97-102` - `ConfigSource` enum

### Acceptance Criteria

1. `create_and_select_default_config()` creates a new config with name "Default"
2. If "Default" exists, names are "Default 2", "Default 3", etc.
3. New config inherits current `mode` from dialog state
4. New config is added to `configs` list with `ConfigSource::FDemon`
5. `selected_config_index` is updated to point to new config
6. Method returns the index of the new config
7. `get_fdemon_configs_for_save()` returns only FDemon configs for persistence

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{ConfigSource, ConfigWithSource, FlutterMode, LaunchConfig};

    #[test]
    fn test_create_and_select_default_config_empty_list() {
        let mut state = LaunchContextState::default();
        state.mode = FlutterMode::Profile;

        let index = state.create_and_select_default_config();

        assert_eq!(index, 0);
        assert_eq!(state.configs.len(), 1);
        assert_eq!(state.selected_config_index, Some(0));
        assert_eq!(state.configs[0].config.name, "Default");
        assert_eq!(state.configs[0].config.mode, FlutterMode::Profile);
        assert_eq!(state.configs[0].source, ConfigSource::FDemon);
    }

    #[test]
    fn test_create_and_select_default_config_unique_naming() {
        let mut state = LaunchContextState::default();

        // Add existing "Default" config
        state.configs.push(ConfigWithSource {
            config: LaunchConfig {
                name: "Default".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
        });

        let index = state.create_and_select_default_config();

        assert_eq!(state.configs.len(), 2);
        assert_eq!(state.configs[index].config.name, "Default 2");
    }

    #[test]
    fn test_create_and_select_default_config_multiple_defaults() {
        let mut state = LaunchContextState::default();

        // Add existing "Default" and "Default 2" configs
        state.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "Default".to_string(), ..Default::default() },
            source: ConfigSource::FDemon,
        });
        state.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "Default 2".to_string(), ..Default::default() },
            source: ConfigSource::FDemon,
        });

        let index = state.create_and_select_default_config();

        assert_eq!(state.configs[index].config.name, "Default 3");
    }

    #[test]
    fn test_generate_unique_name() {
        assert_eq!(generate_unique_name("Default", &[]), "Default");
        assert_eq!(generate_unique_name("Default", &["Default"]), "Default 2");
        assert_eq!(generate_unique_name("Default", &["Default", "Default 2"]), "Default 3");
        assert_eq!(generate_unique_name("Default", &["Other"]), "Default");
    }

    #[test]
    fn test_get_fdemon_configs_for_save() {
        let mut state = LaunchContextState::default();

        state.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "FDemon Config".to_string(), ..Default::default() },
            source: ConfigSource::FDemon,
        });
        state.configs.push(ConfigWithSource {
            config: LaunchConfig { name: "VSCode Config".to_string(), ..Default::default() },
            source: ConfigSource::VsCode,
        });

        let fdemon_configs = state.get_fdemon_configs_for_save();

        assert_eq!(fdemon_configs.len(), 1);
        assert_eq!(fdemon_configs[0].name, "FDemon Config");
    }
}
```

### Notes

- The `create_default_launch_config()` function in `config/launch.rs` may need to be made public if it's not already
- Consider whether the new config should also inherit `flavor` and `dart_defines` from current state (probably not - those will be set by the caller)
- The helper doesn't trigger auto-save - that's the responsibility of the caller (Tasks 02 and 03)
- VSCode configs are read-only, so we only save FDemon configs

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/state.rs` | Added `create_and_select_default_config()` method to `LaunchContextState`, added `get_fdemon_configs_for_save()` helper, added `generate_unique_name()` function, added comprehensive unit tests |

### Notable Decisions/Tradeoffs

1. **Used SourcedConfig instead of ConfigWithSource**: The task document referenced `ConfigWithSource` but the actual codebase uses `SourcedConfig` from `config::priority`. Updated implementation to match existing codebase conventions.
2. **Unique naming follows "Default 2, 3, 4..." pattern**: The implementation generates names like "Default 2" instead of "Default (1)" to match common naming conventions in similar tools.
3. **display_name synchronization**: When creating a new config, the `display_name` field is set to match the config name (without source suffix) since new configs are always FDemon source.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed
- `cargo test --lib app::new_session_dialog::state::tests` - Passed (5/5 tests)
- `cargo test --lib app::` - Passed (396/396 tests)

All new tests pass:
- `test_create_and_select_default_config_empty_list` - Verifies config creation with empty list
- `test_create_and_select_default_config_unique_naming` - Verifies "Default 2" naming
- `test_create_and_select_default_config_multiple_defaults` - Verifies "Default 3" naming with multiple existing
- `test_generate_unique_name` - Verifies unique name generation function
- `test_get_fdemon_configs_for_save` - Verifies filtering of FDemon vs VSCode configs

### Risks/Limitations

1. **No automatic persistence**: The helper methods create and manipulate configs in memory but do not automatically save to disk. Callers (Tasks 02 and 03) are responsible for triggering `save_launch_configs()` when appropriate.
2. **Assumes LoadedConfigs mutability**: The implementation modifies `configs.configs` directly. This is safe in the current architecture but could cause issues if LoadedConfigs becomes immutable in the future.
