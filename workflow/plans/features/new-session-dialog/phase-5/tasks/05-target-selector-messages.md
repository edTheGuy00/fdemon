# Task: Target Selector Messages

## Summary

Add message types and handlers for Target Selector navigation and actions (tab switching, device selection, boot commands).

## Files

| File | Action |
|------|--------|
| `src/app/message.rs` | Modify (add messages) |
| `src/app/handler/update.rs` | Modify (add handlers) |

## Implementation

### 1. Add Target Selector messages

```rust
// src/app/message.rs

use crate::tui::widgets::new_session_dialog::tab_bar::TargetTab;

#[derive(Debug, Clone)]
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // Target Selector Messages
    // ─────────────────────────────────────────────────────────

    /// Switch target selector tab
    NewSessionDialogSwitchTab(TargetTab),

    /// Toggle between Connected and Bootable tabs
    NewSessionDialogToggleTab,

    /// Navigate up in device list
    NewSessionDialogDeviceUp,

    /// Navigate down in device list
    NewSessionDialogDeviceDown,

    /// Select current device (Connected tab) or boot device (Bootable tab)
    NewSessionDialogDeviceSelect,

    /// Refresh device list for current tab
    NewSessionDialogRefreshDevices,

    /// Connected devices discovered
    NewSessionDialogConnectedDevicesReceived(Vec<Device>),

    /// Bootable devices discovered
    NewSessionDialogBootableDevicesReceived {
        ios_simulators: Vec<IosSimulator>,
        android_avds: Vec<AndroidAvd>,
    },

    /// Device discovery failed
    NewSessionDialogDeviceDiscoveryFailed(String),

    /// Device boot started
    NewSessionDialogBootStarted { device_id: String },

    /// Device boot completed - switch to Connected tab
    NewSessionDialogBootCompleted { device_id: String },

    /// Device boot failed
    NewSessionDialogBootFailed { device_id: String, error: String },
}
```

### 2. Handle tab switching

```rust
// src/app/handler/update.rs

fn handle_new_session_dialog_switch_tab(
    state: &mut AppState,
    tab: TargetTab,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.set_tab(tab);

        // Trigger bootable device discovery if switching to Bootable tab
        if tab == TargetTab::Bootable && !dialog.target_selector.bootable_loading {
            dialog.target_selector.bootable_loading = true;
            return Some(UpdateAction::DiscoverBootableDevices);
        }
    }
    None
}

fn handle_new_session_dialog_toggle_tab(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref dialog) = state.new_session_dialog {
        let new_tab = dialog.target_selector.active_tab.toggle();
        return handle_new_session_dialog_switch_tab(state, new_tab);
    }
    None
}
```

### 3. Handle navigation

```rust
fn handle_new_session_dialog_device_up(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.select_previous();
    }
    None
}

fn handle_new_session_dialog_device_down(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.select_next();
    }
    None
}
```

### 4. Handle device selection/boot

```rust
fn handle_new_session_dialog_device_select(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref dialog) = state.new_session_dialog {
        match dialog.target_selector.active_tab {
            TargetTab::Connected => {
                // Select device for launch - handled in Launch Context
                if let Some(device) = dialog.target_selector.selected_connected_device() {
                    // Store selected device ID for launch
                    // The actual launch happens when user confirms in Launch Context
                    return None;
                }
            }
            TargetTab::Bootable => {
                // Boot the selected device
                if let Some(device) = dialog.target_selector.selected_bootable_device() {
                    return Some(UpdateAction::BootDevice {
                        device_id: device.id().to_string(),
                        platform: device.platform().to_string(),
                    });
                }
            }
        }
    }
    None
}
```

### 5. Handle refresh

```rust
fn handle_new_session_dialog_refresh_devices(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        match dialog.target_selector.active_tab {
            TargetTab::Connected => {
                dialog.target_selector.loading = true;
                return Some(UpdateAction::DiscoverConnectedDevices);
            }
            TargetTab::Bootable => {
                dialog.target_selector.bootable_loading = true;
                return Some(UpdateAction::DiscoverBootableDevices);
            }
        }
    }
    None
}
```

### 6. Handle discovery results

```rust
fn handle_new_session_dialog_connected_devices_received(
    state: &mut AppState,
    devices: Vec<Device>,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.set_connected_devices(devices);
    }
    None
}

fn handle_new_session_dialog_bootable_devices_received(
    state: &mut AppState,
    ios_simulators: Vec<IosSimulator>,
    android_avds: Vec<AndroidAvd>,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.set_bootable_devices(ios_simulators, android_avds);
    }
    None
}

fn handle_new_session_dialog_device_discovery_failed(
    state: &mut AppState,
    error: String,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.set_error(error);
    }
    None
}
```

### 7. Handle boot completion

```rust
fn handle_new_session_dialog_boot_completed(
    state: &mut AppState,
    device_id: String,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        // Switch to Connected tab
        dialog.target_selector.set_tab(TargetTab::Connected);

        // Trigger connected device discovery to see the now-booted device
        dialog.target_selector.loading = true;
        return Some(UpdateAction::DiscoverConnectedDevices);
    }
    None
}

fn handle_new_session_dialog_boot_failed(
    state: &mut AppState,
    device_id: String,
    error: String,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.set_error(format!("Boot failed: {}", error));
    }
    None
}
```

### 8. Key handler integration

```rust
// src/app/handler/keys.rs

fn handle_new_session_dialog_keys(
    key: KeyEvent,
    state: &AppState,
) -> Option<Message> {
    // Check if any modal is open first
    if let Some(ref dialog) = state.new_session_dialog {
        if dialog.fuzzy_modal.is_some() || dialog.dart_defines_modal.is_some() {
            return None; // Modal handles its own keys
        }
    }

    match key.code {
        // Tab switching
        KeyCode::Char('1') => Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
        KeyCode::Char('2') => Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),

        // Navigation (when Target Selector is focused)
        KeyCode::Up => Some(Message::NewSessionDialogDeviceUp),
        KeyCode::Down => Some(Message::NewSessionDialogDeviceDown),

        // Selection
        KeyCode::Enter => Some(Message::NewSessionDialogDeviceSelect),

        // Refresh
        KeyCode::Char('r') => Some(Message::NewSessionDialogRefreshDevices),

        // Tab key switches pane focus (handled at dialog level)
        KeyCode::Tab => Some(Message::NewSessionDialogSwitchPane),

        _ => None,
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_switch_tab() {
        let mut state = create_test_state_with_dialog();

        let action = handle_new_session_dialog_switch_tab(
            &mut state,
            TargetTab::Bootable,
        );

        assert!(state.new_session_dialog.is_some());
        let dialog = state.new_session_dialog.as_ref().unwrap();
        assert_eq!(dialog.target_selector.active_tab, TargetTab::Bootable);

        // Should trigger bootable device discovery
        assert!(matches!(action, Some(UpdateAction::DiscoverBootableDevices)));
    }

    #[test]
    fn test_handle_device_navigation() {
        let mut state = create_test_state_with_dialog();
        state.new_session_dialog.as_mut().unwrap()
            .target_selector
            .set_connected_devices(vec![
                test_device_full("1", "Device 1", "ios", false),
                test_device_full("2", "Device 2", "ios", false),
            ]);

        handle_new_session_dialog_device_down(&mut state);

        let dialog = state.new_session_dialog.as_ref().unwrap();
        // Selection should have moved
        assert!(dialog.target_selector.selected_index > 0);
    }

    #[test]
    fn test_handle_boot_completed_switches_tab() {
        let mut state = create_test_state_with_dialog();
        state.new_session_dialog.as_mut().unwrap()
            .target_selector.active_tab = TargetTab::Bootable;

        let action = handle_new_session_dialog_boot_completed(
            &mut state,
            "device-123".to_string(),
        );

        let dialog = state.new_session_dialog.as_ref().unwrap();
        assert_eq!(dialog.target_selector.active_tab, TargetTab::Connected);
        assert!(matches!(action, Some(UpdateAction::DiscoverConnectedDevices)));
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test new_session_dialog && cargo clippy -- -D warnings
```

## Notes

- Boot completion triggers switch to Connected tab + device refresh
- Tab switching to Bootable triggers discovery if not already loaded
- Key handlers respect modal state (modals handle their own keys)
- Navigation wraps around device list
