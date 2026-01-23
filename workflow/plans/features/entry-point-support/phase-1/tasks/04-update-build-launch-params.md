## Task: Update build_launch_params() to include entry_point

**Objective**: Include `entry_point` from `LaunchContextState` when building `LaunchParams`.

**Depends on**: Task 01, Task 02

### Scope

- `src/app/new_session_dialog/state.rs`: Update `build_launch_params()` method in `NewSessionDialogState`

### Details

The `build_launch_params()` method builds a `LaunchParams` struct from the current dialog state. Add `entry_point` to the returned struct.

#### Current implementation (around line 828-846):

```rust
/// Build launch parameters
pub fn build_launch_params(&self) -> Option<LaunchParams> {
    let device = self.selected_device()?;

    Some(LaunchParams {
        device_id: device.id.clone(),
        mode: self.launch_context.mode,
        flavor: self.launch_context.flavor.clone(),
        dart_defines: self
            .launch_context
            .dart_defines
            .iter()
            .map(|d| d.to_arg())
            .collect(),
        config_name: self
            .launch_context
            .selected_config()
            .map(|c| c.display_name.clone()),
    })
}
```

#### Updated implementation:

```rust
/// Build launch parameters
pub fn build_launch_params(&self) -> Option<LaunchParams> {
    let device = self.selected_device()?;

    Some(LaunchParams {
        device_id: device.id.clone(),
        mode: self.launch_context.mode,
        flavor: self.launch_context.flavor.clone(),
        dart_defines: self
            .launch_context
            .dart_defines
            .iter()
            .map(|d| d.to_arg())
            .collect(),
        config_name: self
            .launch_context
            .selected_config()
            .map(|c| c.display_name.clone()),
        entry_point: self.launch_context.entry_point.clone(),  // ADD THIS
    })
}
```

### Acceptance Criteria

1. `build_launch_params()` includes `entry_point` from `launch_context`
2. When `entry_point` is `None`, `LaunchParams.entry_point` is `None`
3. When `entry_point` is `Some(path)`, `LaunchParams.entry_point` is `Some(path.clone())`
4. Code compiles and existing tests pass

### Testing

```rust
#[test]
fn test_build_launch_params_includes_entry_point() {
    use crate::daemon::Device;
    use std::path::PathBuf;

    let mut state = NewSessionDialogState::new(LoadedConfigs::default());

    // Add a device so we can build params
    state.target_selector.set_connected_devices(vec![Device {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        platform: "ios".to_string(),
        ..Default::default()
    }]);

    // Set entry point
    state.launch_context.entry_point = Some(PathBuf::from("lib/main_dev.dart"));

    let params = state.build_launch_params().unwrap();
    assert_eq!(params.entry_point, Some(PathBuf::from("lib/main_dev.dart")));
}

#[test]
fn test_build_launch_params_entry_point_none() {
    let mut state = NewSessionDialogState::new(LoadedConfigs::default());
    state.target_selector.set_connected_devices(vec![/* device */]);

    // No entry point set
    assert_eq!(state.launch_context.entry_point, None);

    let params = state.build_launch_params().unwrap();
    assert_eq!(params.entry_point, None);
}
```

### Notes

- Simple field addition - just clone the `entry_point` from state
- Must be done after Tasks 01 and 02 to compile

---

## Completion Summary

**Status:** Not Started
