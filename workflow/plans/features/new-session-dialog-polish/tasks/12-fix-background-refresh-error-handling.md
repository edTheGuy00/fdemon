## Task: Fix Background Refresh Error Handling Mismatch

**Objective**: Align error handling behavior with documentation - background device refresh errors should be logged but not shown to the user.

**Depends on**: None

**Estimated Time**: 25m

**Priority**: Major

**Source**: Code Review - Risks & Tradeoffs Analyzer

### Scope

- `src/tui/actions.rs`: Update comment or implementation
- `src/app/message.rs`: Possibly add field to message
- `src/app/handler/update.rs`: Handle background errors differently

### Details

The comment says "errors are logged but not shown to user" but the implementation uses the same handler as foreground discovery, which sends `DeviceDiscoveryFailed` message that may show a UI error.

**Current code (lines 61-65):**
```rust
UpdateAction::RefreshDevicesBackground => {
    // Same as DiscoverDevices but errors are logged only (no UI feedback)
    // This runs when we already have cached devices displayed
    spawn::spawn_device_discovery(msg_tx);
}
```

**Approach options:**

**Option A: Add `is_background` flag to error message (Recommended)**
```rust
// In message.rs
DeviceDiscoveryFailed { error: String, is_background: bool }

// In actions.rs - create separate spawn function
UpdateAction::RefreshDevicesBackground => {
    spawn::spawn_device_discovery_background(msg_tx);
}

// In handler - suppress UI for background errors
Message::DeviceDiscoveryFailed { error, is_background } => {
    if is_background {
        tracing::warn!("Background device refresh failed: {}", error);
        // Keep showing cached devices, no error UI
    } else {
        // Show error to user as before
    }
}
```

**Option B: Update comment to match actual behavior**
```rust
UpdateAction::RefreshDevicesBackground => {
    // Same as DiscoverDevices - errors are shown to user
    // User will see error briefly but cached devices remain
    spawn::spawn_device_discovery(msg_tx);
}
```

### Recommendation

Use **Option A** because:
1. Background errors during cached display shouldn't interrupt the user
2. The user already has working cached devices to select from
3. Silent logging allows debugging without UX disruption

### Acceptance Criteria

1. Background refresh errors are logged but don't show error UI
2. Foreground discovery errors still show error UI
3. User can still select from cached devices if background refresh fails
4. Code behavior matches documentation comments
5. Test verifies background errors don't trigger error state

### Testing

```rust
#[test]
fn test_background_discovery_error_is_silent() {
    let mut state = create_test_state_with_cached_devices();

    // Simulate background discovery failure
    let (new_state, action) = handler::update(
        state,
        Message::DeviceDiscoveryFailed {
            error: "Network error".to_string(),
            is_background: true,
        },
    );

    // Cached devices should still be available
    assert!(!new_state.new_session_dialog_state.target_selector.connected_devices().is_empty());

    // No error should be shown (check error state field)
    assert!(new_state.new_session_dialog_state.error.is_none());
}

#[test]
fn test_foreground_discovery_error_shows_ui() {
    let mut state = create_test_state();

    // Simulate foreground discovery failure
    let (new_state, action) = handler::update(
        state,
        Message::DeviceDiscoveryFailed {
            error: "Network error".to_string(),
            is_background: false,
        },
    );

    // Error should be shown to user
    assert!(new_state.new_session_dialog_state.error.is_some());
}
```

### Notes

- If the team prefers Option B (simpler), just update the comment
- Option A requires changes to 3 files but provides better UX
- The `is_background` field is the minimal change; alternatively could use separate message types

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**
(pending)

**Testing Performed:**
(pending)

**Notable Decisions:**
(pending)

**Risks/Limitations:**
(pending)
