## Task: Enable Selection Preservation on Device Refresh

**Objective**: Uncomment and enable the selection preservation logic that keeps the user's selected device after background refresh completes.

**Depends on**: None

**Estimated Time**: 15m

**Priority**: Critical

**Source**: Code Review - Logic Reasoning Checker, Code Quality Inspector

### Scope

- `src/app/handler/update.rs`: Uncomment selection preservation code in `Message::DevicesDiscovered` handler

### Details

The selection preservation code is commented out with a TODO note saying "missing methods", but the required methods (`selected_device_id()` and `select_device_by_id()`) already exist in `target_selector.rs:272-313`.

**Current code (lines 290-309):**
```rust
// TODO: Preserve selection if possible (Task 04 - Device Cache Usage)
// Temporarily commented out due to missing methods - WIP code
// let previous_selection = state
//     .new_session_dialog_state
//     .target_selector
//     .selected_device_id();

state
    .new_session_dialog_state
    .target_selector
    .set_connected_devices(devices);

// Restore selection if device still exists
// if let Some(device_id) = previous_selection {
//     state
//         .new_session_dialog_state
//         .target_selector
//         .select_device_by_id(&device_id);
// }
```

**Required fix:**
```rust
// Preserve selection if possible (Task 04 - Device Cache Usage)
let previous_selection = state
    .new_session_dialog_state
    .target_selector
    .selected_device_id();

state
    .new_session_dialog_state
    .target_selector
    .set_connected_devices(devices);

// Restore selection if device still exists
if let Some(device_id) = previous_selection {
    state
        .new_session_dialog_state
        .target_selector
        .select_device_by_id(&device_id);
}
```

### Race Condition Being Fixed

Without this fix, the following race condition exists:

1. User opens dialog with cached devices: [iPhone, Pixel]
2. User selects Pixel (index 1)
3. Background refresh returns: [Pixel, iPhone, iPad] (order changed)
4. Selection index stays 1 → Now selecting iPhone instead of Pixel
5. User launches on wrong device!

With the fix, the selection is preserved by device ID, not index position.

### Acceptance Criteria

1. Selection preservation code is uncommented and functional
2. TODO comment is removed or updated
3. Test `test_selection_preserved_on_background_refresh()` passes
4. User's selected device remains selected after background refresh completes
5. Selection resets to first device if selected device is no longer available

### Testing

Add a handler test to verify selection preservation:

```rust
#[test]
fn test_selection_preserved_on_background_refresh() {
    let mut state = create_test_state();

    // Initial devices
    let initial_devices = vec![
        create_test_device("device-a", "iPhone"),
        create_test_device("device-b", "Pixel"),
    ];
    state.new_session_dialog_state.target_selector.set_connected_devices(initial_devices);

    // Select second device
    state.new_session_dialog_state.target_selector.move_down();
    assert_eq!(
        state.new_session_dialog_state.target_selector.selected_device_id(),
        Some("device-b".to_string())
    );

    // Background refresh returns devices in different order with new device
    let refreshed_devices = vec![
        create_test_device("device-c", "iPad"),
        create_test_device("device-b", "Pixel"),  // Same device, different position
        create_test_device("device-a", "iPhone"),
    ];

    // Simulate DevicesDiscovered message
    let (new_state, _) = handler::update(
        state,
        Message::DevicesDiscovered { devices: refreshed_devices },
    );

    // Selection should still be device-b (Pixel), not device-c (iPad)
    assert_eq!(
        new_state.new_session_dialog_state.target_selector.selected_device_id(),
        Some("device-b".to_string())
    );
}

#[test]
fn test_selection_resets_when_device_removed() {
    let mut state = create_test_state();

    // Initial devices
    let initial_devices = vec![
        create_test_device("device-a", "iPhone"),
        create_test_device("device-b", "Pixel"),
    ];
    state.new_session_dialog_state.target_selector.set_connected_devices(initial_devices);

    // Select second device
    state.new_session_dialog_state.target_selector.move_down();

    // Refresh without the selected device
    let refreshed_devices = vec![
        create_test_device("device-a", "iPhone"),
        create_test_device("device-c", "iPad"),
    ];

    let (new_state, _) = handler::update(
        state,
        Message::DevicesDiscovered { devices: refreshed_devices },
    );

    // Selection should fall back to first device since device-b is gone
    assert_eq!(
        new_state.new_session_dialog_state.target_selector.selected_device_id(),
        Some("device-a".to_string())
    );
}
```

### Notes

- This is a **critical fix** - users could accidentally launch on the wrong device
- The methods `selected_device_id()` and `select_device_by_id()` are already tested
- `select_device_by_id()` returns `false` if device not found, which means selection falls back to first item

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
