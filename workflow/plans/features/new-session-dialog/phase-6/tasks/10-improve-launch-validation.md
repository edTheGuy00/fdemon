# Task: Improve Launch Tab Validation Error Message

## Summary

Fix the misleading error message when user attempts to launch from the Bootable tab, providing clearer guidance or auto-switching behavior.

## Files

| File | Action |
|------|--------|
| `src/app/handler/update.rs` | Modify (improve validation) |

## Background

The code review identified that if a user is on the Bootable tab and tries to launch, they get the error "Please select a device first" even if connected devices exist. This is misleading because the real issue is that they're on the wrong tab.

**Current behavior:**
- User is on Bootable tab
- User presses Enter to launch
- Gets error: "Please select a device first"
- Confusing because devices may be connected

## Implementation

### Option A: Clearer Error Message (Recommended)

```rust
fn handle_new_session_dialog_launch(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref dialog) = state.new_session_dialog_state {
        // Check which tab is active
        let active_tab = dialog.target_selector_state.active_tab;

        // Get selected device based on active tab
        let selected_device = match active_tab {
            DeviceTab::Connected => dialog.target_selector_state.selected_connected_device(),
            DeviceTab::Bootable => None, // Cannot launch bootable devices directly
        };

        if selected_device.is_none() {
            // Provide context-specific error message
            let error_msg = match active_tab {
                DeviceTab::Bootable => {
                    if dialog.target_selector_state.connected_devices.is_empty() {
                        "No connected devices. Boot a device first, or switch to Connected tab."
                    } else {
                        "Switch to Connected tab to select a running device for launch."
                    }
                }
                DeviceTab::Connected => {
                    if dialog.target_selector_state.connected_devices.is_empty() {
                        "No connected devices. Connect a device or start an emulator."
                    } else {
                        "Please select a device from the list."
                    }
                }
            };

            state.notifications.push(Notification::warning(error_msg.to_string()));
            return None;
        }

        // ... proceed with launch
    }
    None
}
```

### Option B: Auto-Switch to Connected Tab

```rust
fn handle_new_session_dialog_launch(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog_state {
        // If on Bootable tab but connected devices exist, auto-switch
        if dialog.target_selector_state.active_tab == DeviceTab::Bootable {
            if !dialog.target_selector_state.connected_devices.is_empty() {
                // Auto-switch to Connected tab
                dialog.target_selector_state.active_tab = DeviceTab::Connected;
                dialog.target_selector_state.selected_index = 0;

                state.notifications.push(Notification::info(
                    "Switched to Connected tab. Select a device and press Enter to launch."
                ));
                return None;
            }
        }

        // ... rest of launch logic
    }
    None
}
```

## Acceptance Criteria

1. Error message clearly indicates when user is on wrong tab
2. Users understand what action to take to launch
3. No confusing "select a device" error when devices exist but wrong tab is active
4. `cargo test launch` passes

## Verification

```bash
cargo fmt && cargo check && cargo test handler && cargo clippy -- -D warnings
```

## Manual Testing

1. Open NewSessionDialog with connected devices
2. Switch to Bootable tab
3. Try to launch
4. Verify error message is clear (Option A) or tab switches (Option B)
5. Switch to Connected tab
6. Select device and launch
7. Verify launch works

## Notes

- Option A is simpler and doesn't change state unexpectedly
- Option B is more "helpful" but may surprise users
- Consider UX preference when choosing approach
- Could also add visual indicator that launch requires Connected tab
