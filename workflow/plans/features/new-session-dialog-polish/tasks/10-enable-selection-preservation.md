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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | Uncommented and enabled selection preservation logic in `Message::DevicesDiscovered` handler (lines 291-316) |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Added public `reset_selection_to_first()` method to reset selection to first selectable device (lines 315-320) |
| `src/app/handler/tests.rs` | Added two tests: `test_selection_preserved_on_background_refresh()` and `test_selection_resets_when_device_removed()` (lines 2752-2845) |

### Implementation Details

1. **Uncommented selection preservation code**: The code that was commented out with "TODO: missing methods" has been uncommented and updated with proper comment referencing Task 10.

2. **Added reset helper method**: Added `reset_selection_to_first()` public method to `TargetSelectorState` to properly reset selection when device is not found. This was necessary because `first_selectable_index()` is private and cannot be accessed from the handler.

3. **Selection restoration flow**:
   - Before updating devices, capture the currently selected device ID
   - Update the device list with `set_connected_devices()`
   - Attempt to restore selection by ID using `select_device_by_id()`
   - If restoration fails (device no longer available), reset to first selectable device using `reset_selection_to_first()`

4. **Fixed race condition**: The implementation now preserves user's device selection by ID rather than by index position, preventing the bug where device list reordering would cause wrong device to be selected.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib test_selection_preserved_on_background_refresh` - Passed
- `cargo test --lib test_selection_resets_when_device_removed` - Passed
- `cargo fmt` - Passed (no formatting changes needed)
- `cargo clippy --lib` - Passed (no warnings in modified code)

### Notable Decisions/Tradeoffs

1. **Added helper method**: While the task focused on uncommenting existing code, I added `reset_selection_to_first()` as a public helper method because `first_selectable_index()` is private. This ensures proper selection reset when device grouping headers are present (e.g., "Android Devices", "iOS Devices" headers precede actual devices in the flattened list).

2. **Selection reset behavior**: When the previously selected device is no longer available, the selection resets to the first selectable device (not index 0, which might be a header). This matches user expectations and prevents selecting non-device items.

### Risks/Limitations

1. **Pre-existing test failure**: The test `test_background_discovery_error_is_silent` was found to be failing, but this is unrelated to this task. The test incorrectly calls `state.show_new_session_dialog()` directly instead of using `handle_open_new_session_dialog()`, which would properly load cached devices into the dialog.

2. **Device grouping dependency**: The implementation relies on device grouping logic (devices grouped by platform with headers). If grouping logic changes significantly, the reset behavior would need to be retested.

3. **No validation of selected_device_id persistence**: The implementation assumes device IDs remain stable across discovery calls. If device IDs change between discoveries (unlikely but possible), the selection might reset unnecessarily.
