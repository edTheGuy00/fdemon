## Task: Update select_config() to apply entry_point

**Objective**: When a configuration is selected, copy its `entry_point` value to the state (following the existing pattern for mode, flavor, and dart_defines).

**Depends on**: Task 02

### Scope

- `src/app/new_session_dialog/state.rs`: Update `select_config()` method in `LaunchContextState`

### Details

The `select_config()` method currently copies `mode`, `flavor`, and `dart_defines` from the selected config to the state. Add `entry_point` to this list.

#### Current implementation (around line 488-509):

```rust
pub fn select_config(&mut self, index: Option<usize>) {
    self.selected_config_index = index;

    // Apply config values
    // Clone the config to avoid borrow checker issues
    if let Some(config) = self.selected_config().cloned() {
        self.mode = config.config.mode;

        if let Some(ref flavor) = config.config.flavor {
            self.flavor = Some(flavor.clone());
        }

        if !config.config.dart_defines.is_empty() {
            self.dart_defines = config
                .config
                .dart_defines
                .iter()
                .map(|(k, v)| DartDefine::new(k, v))
                .collect();
        }
    }
}
```

#### Updated implementation:

```rust
pub fn select_config(&mut self, index: Option<usize>) {
    self.selected_config_index = index;

    // Apply config values
    // Clone the config to avoid borrow checker issues
    if let Some(config) = self.selected_config().cloned() {
        self.mode = config.config.mode;

        if let Some(ref flavor) = config.config.flavor {
            self.flavor = Some(flavor.clone());
        }

        // Apply entry_point from config
        if let Some(ref entry_point) = config.config.entry_point {
            self.entry_point = Some(entry_point.clone());
        }

        if !config.config.dart_defines.is_empty() {
            self.dart_defines = config
                .config
                .dart_defines
                .iter()
                .map(|(k, v)| DartDefine::new(k, v))
                .collect();
        }
    }
}
```

### Acceptance Criteria

1. When `select_config(Some(idx))` is called with a config that has `entry_point`, the state's `entry_point` is updated
2. When config has no `entry_point`, state's `entry_point` is unchanged (not cleared)
3. Pattern matches existing behavior for `flavor`

### Testing

```rust
#[test]
fn test_select_config_applies_entry_point() {
    use crate::config::{ConfigSource, LaunchConfig, LoadedConfigs, SourcedConfig};
    use std::path::PathBuf;

    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig {
            name: "Dev".to_string(),
            entry_point: Some(PathBuf::from("lib/main_dev.dart")),
            ..Default::default()
        },
        source: ConfigSource::VSCode,
        display_name: "Dev".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    assert_eq!(state.entry_point, None);

    state.select_config(Some(0));
    assert_eq!(state.entry_point, Some(PathBuf::from("lib/main_dev.dart")));
}

#[test]
fn test_select_config_without_entry_point_preserves_existing() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig {
            name: "Basic".to_string(),
            entry_point: None,  // No entry point
            ..Default::default()
        },
        source: ConfigSource::FDemon,
        display_name: "Basic".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.entry_point = Some(PathBuf::from("lib/existing.dart"));

    state.select_config(Some(0));
    // Entry point should be preserved since config doesn't specify one
    assert_eq!(state.entry_point, Some(PathBuf::from("lib/existing.dart")));
}
```

### Notes

- Follow the exact same pattern as `flavor` handling
- Consider whether we should clear `entry_point` when selecting a config without one (current design: preserve existing)
- This is the key fix that makes VSCode `program` field work

---

## Completion Summary

**Status:** Not Started
