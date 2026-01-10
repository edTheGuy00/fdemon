## Task: Add Integration Tests for Auto-Launch Flow

**Objective**: Create integration tests that verify the complete auto-launch message flow from `StartAutoLaunch` through `AutoLaunchResult`.

**Depends on**: 01-update-device-cache, 02-handle-edge-cases

**Estimated Time**: 1.5 hours

### Scope

- `src/app/handler/tests.rs`: Add integration tests for auto-launch flow

### Details

#### Test Strategy

Since the actual device discovery is async and requires Flutter SDK, we'll test the handler flow by simulating the message sequence:

1. **Unit test handlers individually** (already done in Phase 1)
2. **Integration test the message flow** (this task)
3. **Manual E2E test with real devices** (Task 4)

#### Test Cases

##### Test 1: Successful Auto-Launch Flow

```rust
#[test]
fn test_auto_launch_flow_success() {
    let mut state = AppState::new();
    let project_path = PathBuf::from("/tmp/test");
    state.project_path = project_path.clone();

    // Step 1: StartAutoLaunch
    let configs = LoadedConfigs::default();
    let result = update(&mut state, Message::StartAutoLaunch { configs: configs.clone() });

    assert_eq!(state.ui_mode, UiMode::Loading);
    assert!(state.loading_state.is_some());
    assert!(matches!(
        result.action,
        Some(UpdateAction::DiscoverDevicesAndAutoLaunch { .. })
    ));

    // Step 2: Progress update
    let _ = update(&mut state, Message::AutoLaunchProgress {
        message: "Detecting devices...".to_string(),
    });

    // Loading message should be updated (hard to verify exact text due to randomization)
    assert!(state.loading_state.is_some());

    // Step 3: Successful result
    let device = Device {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        platform: "android".to_string(),
        emulator: false,
        sdk: "30".to_string(),
        category: DeviceCategory::Mobile,
    };

    let result = update(&mut state, Message::AutoLaunchResult {
        result: Ok(AutoLaunchSuccess {
            device: device.clone(),
            config: None,
        }),
    });

    // Verify final state
    assert!(state.loading_state.is_none()); // Loading cleared
    assert_eq!(state.ui_mode, UiMode::Normal);
    assert_eq!(state.session_manager.len(), 1); // Session created
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnSession { .. })
    ));
}
```

##### Test 2: Auto-Launch with Config

```rust
#[test]
fn test_auto_launch_with_config() {
    let mut state = AppState::new();
    state.project_path = PathBuf::from("/tmp/test");

    // Skip to result with config
    let device = Device { /* ... */ };
    let config = LaunchConfig {
        name: "debug".to_string(),
        device: "auto".to_string(),
        mode: FlutterMode::Debug,
        flavor: Some("dev".to_string()),
        dart_defines: vec![],
        auto_start: false,
    };

    let result = update(&mut state, Message::AutoLaunchResult {
        result: Ok(AutoLaunchSuccess {
            device: device.clone(),
            config: Some(config.clone()),
        }),
    });

    // Verify session was created with config
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnSession { config: Some(_), .. })
    ));
}
```

##### Test 3: Auto-Launch Failure (No Devices)

```rust
#[test]
fn test_auto_launch_no_devices_shows_dialog() {
    let mut state = AppState::new();
    state.project_path = PathBuf::from("/tmp/test");
    state.set_loading_phase("Testing...");

    let result = update(&mut state, Message::AutoLaunchResult {
        result: Err("No devices found".to_string()),
    });

    assert!(state.loading_state.is_none()); // Loading cleared
    assert_eq!(state.ui_mode, UiMode::StartupDialog);
    assert!(state.startup_dialog_state.error.is_some());
    assert!(state.startup_dialog_state.error.as_ref().unwrap().contains("No devices"));
}
```

##### Test 4: Auto-Launch Failure (Discovery Error)

```rust
#[test]
fn test_auto_launch_discovery_error() {
    let mut state = AppState::new();
    state.project_path = PathBuf::from("/tmp/test");
    state.set_loading_phase("Testing...");

    let result = update(&mut state, Message::AutoLaunchResult {
        result: Err("Flutter SDK not found".to_string()),
    });

    assert!(state.loading_state.is_none());
    assert_eq!(state.ui_mode, UiMode::StartupDialog);
    assert!(state.startup_dialog_state.error.as_ref().unwrap().contains("Flutter SDK"));
}
```

##### Test 5: Progress Messages Don't Crash Without Loading State

```rust
#[test]
fn test_auto_launch_progress_without_loading_is_safe() {
    let mut state = AppState::new();
    // No loading state set

    // Should not panic
    let result = update(&mut state, Message::AutoLaunchProgress {
        message: "Testing...".to_string(),
    });

    assert!(result.action.is_none());
}
```

### Test Location

Add tests to `src/app/handler/tests.rs` in a new section:

```rust
// ============================================================================
// Auto-Launch Flow Tests
// ============================================================================

mod auto_launch_tests {
    use super::*;
    // ... tests here
}
```

### Acceptance Criteria

1. Test for successful auto-launch flow passes
2. Test for auto-launch with config passes
3. Test for no devices error passes
4. Test for discovery error passes
5. Test for progress without loading state passes
6. All existing tests still pass
7. `cargo test` passes

### Notes

- These tests verify the handler logic, not the spawn function
- The spawn function would need mocking to test properly
- Manual testing (Task 4) covers the full end-to-end flow
- Consider using `tempfile` for project_path if file system access is needed

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**

(pending)

**Testing Performed:**
- (pending)

**Notable Decisions:**
- (pending)

**Risks/Limitations:**
- (pending)
