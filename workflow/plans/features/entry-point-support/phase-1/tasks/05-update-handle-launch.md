## Task: Update handle_launch() to use entry_point from params

**Objective**: Pass `entry_point` from `LaunchParams` to the `LaunchConfig` when spawning a session.

**Depends on**: Task 01, Task 04

### Scope

- `src/app/handler/new_session/launch_context.rs`: Update `handle_launch()` function

### Details

The `handle_launch()` function builds a `LaunchConfig` from `LaunchParams` but currently omits `entry_point`. This is the critical fix.

#### Current implementation (around line 347-366):

```rust
let config = if params.config_name.is_some()
    || params.flavor.is_some()
    || !params.dart_defines.is_empty()
{
    let mut cfg = LaunchConfig {
        name: params.config_name.unwrap_or_else(|| "Session".to_string()),
        device: device.id.clone(),
        mode: params.mode,
        flavor: params.flavor,
        ..Default::default()
    };

    // Parse dart_defines from "KEY=VALUE" format
    for define in &params.dart_defines {
        if let Some((key, value)) = define.split_once('=') {
            cfg.dart_defines.insert(key.to_string(), value.to_string());
        }
    }

    Some(Box::new(cfg))
} else {
    None
};
```

#### Updated implementation:

```rust
let config = if params.config_name.is_some()
    || params.flavor.is_some()
    || !params.dart_defines.is_empty()
    || params.entry_point.is_some()  // ADD THIS CONDITION
{
    let mut cfg = LaunchConfig {
        name: params.config_name.unwrap_or_else(|| "Session".to_string()),
        device: device.id.clone(),
        mode: params.mode,
        flavor: params.flavor,
        entry_point: params.entry_point,  // ADD THIS FIELD
        ..Default::default()
    };

    // Parse dart_defines from "KEY=VALUE" format
    for define in &params.dart_defines {
        if let Some((key, value)) = define.split_once('=') {
            cfg.dart_defines.insert(key.to_string(), value.to_string());
        }
    }

    Some(Box::new(cfg))
} else {
    None
};
```

### Key Changes

1. **Add condition**: `|| params.entry_point.is_some()` to the `if` check
   - This ensures we create a config when entry_point is specified, even if nothing else is set

2. **Add field**: `entry_point: params.entry_point` in the `LaunchConfig` initializer
   - This passes the entry point through to the spawn

### Acceptance Criteria

1. `handle_launch()` includes `entry_point` in the condition check
2. `handle_launch()` passes `entry_point` to `LaunchConfig`
3. When `entry_point` is `Some(path)`, the spawned Flutter process receives `-t path`
4. When `entry_point` is `None`, no `-t` flag is added (default `lib/main.dart`)
5. Existing tests continue to pass

### Testing

```rust
#[test]
fn test_handle_launch_with_entry_point() {
    use std::path::PathBuf;

    let mut state = create_test_app_state();
    // Setup: device selected, entry_point set
    state.new_session_dialog.as_mut().unwrap()
        .launch_context.entry_point = Some(PathBuf::from("lib/main_dev.dart"));

    let result = handle_launch(&mut state);

    // Verify SpawnSession action has config with entry_point
    match result.action {
        Some(UpdateAction::SpawnSession { config, .. }) => {
            let cfg = config.expect("should have config");
            assert_eq!(cfg.entry_point, Some(PathBuf::from("lib/main_dev.dart")));
        }
        _ => panic!("Expected SpawnSession action"),
    }
}

#[test]
fn test_handle_launch_entry_point_creates_config() {
    // Even if no flavor/defines, entry_point alone should create a config
    let mut state = create_test_app_state();
    state.new_session_dialog.as_mut().unwrap()
        .launch_context.entry_point = Some(PathBuf::from("lib/main_test.dart"));
    state.new_session_dialog.as_mut().unwrap()
        .launch_context.flavor = None;
    state.new_session_dialog.as_mut().unwrap()
        .launch_context.dart_defines.clear();

    let result = handle_launch(&mut state);

    match result.action {
        Some(UpdateAction::SpawnSession { config, .. }) => {
            assert!(config.is_some(), "entry_point alone should create config");
        }
        _ => panic!("Expected SpawnSession action"),
    }
}
```

### Integration Test

After this task, manually verify end-to-end:

1. Create `.vscode/launch.json` with `"program": "lib/main_dev.dart"`
2. Run fdemon, open NewSessionDialog
3. Select the VSCode config
4. Launch
5. Verify Flutter command includes `-t lib/main_dev.dart`

### Notes

- This is the critical fix that makes the feature work end-to-end
- The `LaunchConfig.build_flutter_args()` already handles `-t` flag generation
- Must add both the condition AND the field assignment

---

## Completion Summary

**Status:** Not Started
