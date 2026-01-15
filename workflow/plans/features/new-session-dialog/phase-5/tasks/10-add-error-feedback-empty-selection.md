## Task: Add Error Feedback for Empty Device Selection

**Objective**: Provide user feedback when device selection fails due to no device being selected.

**Depends on**: 05-target-selector-messages

**Priority**: Major

**Source**: Code Quality Inspector - Review Issue #5

### Scope

- `src/app/handler/update.rs:1768-1785`: `NewSessionDialogDeviceSelect` handler

### Problem

When no device is selected on the Bootable tab, the handler silently returns `None`:

```rust
Message::NewSessionDialogDeviceSelect => {
    match state.new_session_dialog_state.target_tab {
        TargetTab::Bootable => {
            if let Some(device) = state.new_session_dialog_state.selected_bootable_device() {
                // ... boot logic
            }
            // SILENT FAILURE: No else branch, just falls through to None
        }
        // ...
    }
    None
}
```

**Impact:** User presses Enter with no device selected, nothing happens, no feedback.

### Details

Add logging and optional error state:

```rust
// update.rs - AFTER
Message::NewSessionDialogDeviceSelect => {
    match state.new_session_dialog_state.target_tab {
        TargetTab::Bootable => {
            if let Some(device) = state.new_session_dialog_state.selected_bootable_device() {
                info!("Booting device: {:?}", device.name());
                // ... existing boot logic
            } else {
                warn!("Cannot boot device: no device selected");
                // Optionally show error in dialog:
                // state.new_session_dialog_state.set_error("No device selected".to_string());
            }
        }
        TargetTab::Connected => {
            if let Some(device) = state.new_session_dialog_state.selected_device() {
                info!("Selecting device: {:?}", device.name);
                // ... existing select logic
            } else {
                warn!("Cannot select device: no device selected");
                // Optionally show error in dialog
            }
        }
    }
    None
}
```

### Acceptance Criteria

1. Silent failures are logged at `warn` level at minimum
2. Optional: Error message displayed in dialog when selection fails
3. Logging includes context about which operation failed
4. Existing behavior unchanged (still returns `None`)
5. All existing tests pass

### Testing

```rust
#[test]
fn test_select_with_no_device_logs_warning() {
    let mut state = AppState::new();
    // Empty device list
    state.new_session_dialog_state.set_connected_devices(vec![]);

    let action = handle_message(&mut state, Message::NewSessionDialogDeviceSelect);

    assert!(action.is_none());
    // Verify warning was logged (may need test log capture)
}
```

### Notes

- Decide if error should be shown in dialog UI or just logged
- If showing in dialog, consider a temporary toast-style message vs persistent error
- Could also play a sound or visual feedback for invalid action
- Keep it minimal - don't over-engineer the error feedback
