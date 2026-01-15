## Task: Standardize Error Clearing Logic

**Objective**: Make error clearing behavior consistent across all state methods and handlers.

**Depends on**: 05-target-selector-messages

**Priority**: Major

**Source**: Logic Reasoning Checker, Code Quality Inspector - Review Issue #4

### Scope

- `src/tui/widgets/new_session_dialog/state.rs:752`: `set_connected_devices()` method
- `src/tui/widgets/new_session_dialog/target_selector.rs:183-184`: `set_connected_devices()` method
- `src/app/handler/update.rs:1799-1809`: `DeviceDiscoveryFailed` handler

### Problem

1. **Inconsistent error clearing on success:**
   - `NewSessionDialogState.set_connected_devices()` does NOT clear error
   - `TargetSelectorState.set_connected_devices()` DOES clear error

2. **Wrong flags cleared on failure:**
   - `DeviceDiscoveryFailed` clears BOTH loading flags regardless of which discovery failed

### Details

**Fix 1: Add error clearing to NewSessionDialogState.set_connected_devices()**

```rust
// state.rs - BEFORE
pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
    self.devices = devices;
    self.loading_connected = false;
}

// state.rs - AFTER
pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
    self.devices = devices;
    self.loading_connected = false;
    self.error = None; // Clear error on successful load
}
```

**Fix 2: Add context to DeviceDiscoveryFailed message**

```rust
// message.rs
pub enum Message {
    // BEFORE
    NewSessionDialogDeviceDiscoveryFailed(String),

    // AFTER - Add context about which discovery failed
    NewSessionDialogDeviceDiscoveryFailed {
        error: String,
        discovery_type: DiscoveryType, // Connected or Bootable
    },
}

pub enum DiscoveryType {
    Connected,
    Bootable,
}
```

**Fix 3: Update handler to clear only relevant flag**

```rust
// update.rs - BEFORE
Message::NewSessionDialogDeviceDiscoveryFailed(error) => {
    state.new_session_dialog_state.loading_connected = false;
    state.new_session_dialog_state.loading_bootable = false;
    state.new_session_dialog_state.set_error(error);
}

// update.rs - AFTER
Message::NewSessionDialogDeviceDiscoveryFailed { error, discovery_type } => {
    match discovery_type {
        DiscoveryType::Connected => {
            state.new_session_dialog_state.loading_connected = false;
        }
        DiscoveryType::Bootable => {
            state.new_session_dialog_state.loading_bootable = false;
        }
    }
    state.new_session_dialog_state.set_error(error);
}
```

### Acceptance Criteria

1. `set_connected_devices()` clears error in both state structs
2. `DeviceDiscoveryFailed` message includes context about which discovery failed
3. Only the relevant loading flag is cleared on failure
4. Error clearing behavior is consistent across all state methods
5. All existing tests pass (update tests for new message format)

### Testing

```rust
#[test]
fn test_set_connected_devices_clears_error() {
    let mut state = create_state();
    state.set_error("Previous error".to_string());
    state.set_connected_devices(vec![device()]);
    assert!(state.error.is_none());
}

#[test]
fn test_discovery_failed_clears_only_relevant_flag() {
    let mut state = AppState::new();
    state.new_session_dialog_state.loading_connected = true;
    state.new_session_dialog_state.loading_bootable = true;

    // Fail connected discovery
    handle_message(&mut state, Message::NewSessionDialogDeviceDiscoveryFailed {
        error: "Network error".into(),
        discovery_type: DiscoveryType::Connected,
    });

    assert!(!state.new_session_dialog_state.loading_connected);
    assert!(state.new_session_dialog_state.loading_bootable); // Still true!
}
```

### Notes

- Consider if `DiscoveryType` enum should be defined in message.rs or reuse existing types
- Update all callers of `DeviceDiscoveryFailed` to include discovery type
- May need to update async discovery code to propagate context

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Added `self.error = None;` to `set_connected_devices()` method to clear error on successful device load (line 778) |
| `src/app/message.rs` | Added `DiscoveryType` enum (lines 13-20) and converted `NewSessionDialogDeviceDiscoveryFailed` from tuple variant to struct variant with `error` and `discovery_type` fields (lines 517-521) |
| `src/app/handler/update.rs` | Updated `NewSessionDialogDeviceDiscoveryFailed` handler to only clear the loading flag for the discovery type that failed (lines 1845-1862) |
| `src/app/handler/tests.rs` | Updated `test_device_discovery_failed` to test both Connected and Bootable discovery failures separately, verifying that only the relevant flag is cleared (lines 2860-2895) |
| `src/tui/widgets/new_session_dialog/state.rs` | Added `test_set_connected_devices_clears_error()` test to verify error clearing on successful device load (lines 1054-1075) |

### Notable Decisions/Tradeoffs

1. **DiscoveryType enum location**: Defined in `message.rs` rather than creating a shared types module, keeping it close to where it's used in the Message enum. This is consistent with other enum types like `FlutterMode` that are used in messages.

2. **Error clearing pattern**: Matched the pattern from `TargetSelectorState.set_connected_devices()` (line 184 of target_selector.rs) which already clears error on successful load. This ensures consistency between the two state implementations.

3. **Test coverage**: Added both unit test for state method (`test_set_connected_devices_clears_error`) and integration test for handler (`test_device_discovery_failed`), ensuring behavior is tested at multiple levels.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1537 tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **No callers yet**: Currently, no code path actually sends `NewSessionDialogDeviceDiscoveryFailed` with the discovery type context. The message infrastructure is ready, but actual error reporting from device discovery tasks would need to be added in future tasks (likely when implementing the device discovery spawn tasks for new session dialog).

2. **Bootable discovery errors swallowed**: The current `spawn_bootable_device_discovery` function in `src/tui/spawn.rs` uses `unwrap_or_default()` on errors, so bootable discovery failures are silently ignored. Future tasks may want to add proper error reporting for bootable device discovery.