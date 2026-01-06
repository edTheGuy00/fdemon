# Task: Device Discovery Integration

**Objective**: Fix device discovery for the startup dialog so devices are loaded and displayed correctly.

**Depends on**: Task 03 (Startup Dialog Widget), Task 06 (Startup Flow)

## Problem

The device section shows "Discovering devices..." indefinitely because:

1. **`ShowStartupDialog` message handler is a stub** (`update.rs:1185-1188`):
   ```rust
   Message::ShowStartupDialog => {
       // TODO: Load configs and show dialog
       UpdateResult::none()  // Does nothing!
   }
   ```

2. **`StartupDialogRefreshDevices` is also a stub** (`update.rs:1277-1280`):
   ```rust
   Message::StartupDialogRefreshDevices => {
       // TODO: Trigger device discovery
       UpdateResult::none()  // Does nothing!
   }
   ```

3. **`DevicesDiscovered` only updates `device_selector`, not `startup_dialog_state`** (`update.rs:426-441`):
   ```rust
   Message::DevicesDiscovered { devices } => {
       state.device_selector.set_devices(devices);  // Only device_selector!
       // startup_dialog_state is never updated
   }
   ```

## Scope

- `src/app/handler/update.rs` - Implement message handlers
- `src/app/message.rs` - Add startup-dialog specific device messages (optional)
- `src/tui/spawn.rs` - May need spawn function for startup dialog device discovery

## Implementation

### 1. Fix `ShowStartupDialog` Handler (`src/app/handler/update.rs`)

```rust
Message::ShowStartupDialog => {
    // Load all configs
    let configs = crate::config::load_all_configs(&state.project_path);

    // Show the dialog with configs
    state.show_startup_dialog(configs);

    // Trigger device discovery
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

### 2. Fix `StartupDialogRefreshDevices` Handler (`src/app/handler/update.rs`)

```rust
Message::StartupDialogRefreshDevices => {
    // Mark as refreshing (shows loading indicator but keeps existing devices)
    state.startup_dialog_state.refreshing = true;

    // Trigger device discovery
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

### 3. Fix `DevicesDiscovered` Handler (`src/app/handler/update.rs`)

```rust
Message::DevicesDiscovered { devices } => {
    let device_count = devices.len();

    // Update device_selector (for add-session use case)
    state.device_selector.set_devices(devices.clone());

    // ALSO update startup_dialog_state (for initial startup)
    if state.ui_mode == UiMode::StartupDialog {
        state.startup_dialog_state.set_devices(devices);
    }

    // If we were in Loading mode, transition appropriately
    if state.ui_mode == UiMode::Loading {
        state.ui_mode = UiMode::DeviceSelector;
    }

    if device_count > 0 {
        tracing::info!("Discovered {} device(s)", device_count);
    } else {
        tracing::info!("No devices found");
    }

    UpdateResult::none()
}
```

### 4. Fix `DeviceDiscoveryFailed` Handler (`src/app/handler/update.rs`)

```rust
Message::DeviceDiscoveryFailed { error } => {
    // Update device_selector
    state.device_selector.set_error(error.clone());

    // ALSO update startup_dialog_state
    if state.ui_mode == UiMode::StartupDialog {
        state.startup_dialog_state.set_error(error.clone());
    }

    // If we were in Loading mode, transition to DeviceSelector to show error
    if state.ui_mode == UiMode::Loading {
        state.ui_mode = UiMode::DeviceSelector;
    }

    tracing::error!("Device discovery failed: {}", error);
    UpdateResult::none()
}
```

### 5. (Optional) Add Tick Handler for Animation

Ensure animation frame updates for loading spinner:

```rust
Message::Tick => {
    // ... existing device_selector tick ...

    // Also tick startup dialog when visible and loading
    if state.ui_mode == UiMode::StartupDialog
        && (state.startup_dialog_state.loading || state.startup_dialog_state.refreshing)
    {
        state.startup_dialog_state.tick();
    }

    UpdateResult::none()
}
```

## Acceptance Criteria

1. Opening startup dialog triggers device discovery
2. Devices appear in the device section (not stuck on "Discovering...")
3. Refresh button ('r' key) re-discovers devices
4. Error states are displayed in the dialog
5. Animation plays during discovery
6. Unit tests for message handlers

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_startup_dialog_triggers_discovery() {
        let mut state = AppState::new();

        let result = update(&mut state, Message::ShowStartupDialog);

        assert_eq!(state.ui_mode, UiMode::StartupDialog);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_devices_discovered_updates_startup_dialog() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;
        state.startup_dialog_state.loading = true;

        let devices = vec![test_device("dev1", "Device 1")];
        update(&mut state, Message::DevicesDiscovered { devices: devices.clone() });

        assert!(!state.startup_dialog_state.loading);
        assert_eq!(state.startup_dialog_state.devices.len(), 1);
        assert_eq!(state.startup_dialog_state.selected_device, Some(0));
    }

    #[test]
    fn test_device_discovery_failed_shows_error() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;

        update(&mut state, Message::DeviceDiscoveryFailed {
            error: "No Flutter SDK found".to_string()
        });

        assert_eq!(state.startup_dialog_state.error, Some("No Flutter SDK found".to_string()));
        assert!(!state.startup_dialog_state.loading);
    }

    #[test]
    fn test_refresh_devices_triggers_discovery() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;

        let result = update(&mut state, Message::StartupDialogRefreshDevices);

        assert!(state.startup_dialog_state.refreshing);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }
}
```

## Notes

- Device discovery is async via `UpdateAction::DiscoverDevices`
- The existing spawn infrastructure handles the async call
- Both `device_selector` and `startup_dialog_state` need updating since either might be active
- The `devices.clone()` is acceptable since `Device` is small and discovery is infrequent

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | Implemented 5 message handlers: ShowStartupDialog (loads configs + triggers discovery), StartupDialogRefreshDevices (sets refreshing + triggers discovery), DevicesDiscovered (updates both device_selector and startup_dialog_state), DeviceDiscoveryFailed (updates both selectors with error), Tick (advances startup dialog animation) |
| `src/app/handler/tests.rs` | Added 10 unit tests covering all acceptance criteria: dialog triggers discovery, devices update both selectors, error handling, refresh functionality, animation ticking |

### Notable Decisions/Tradeoffs

1. **Dual state updates**: Both `device_selector` and `startup_dialog_state` are updated when devices are discovered. This is necessary because either UI mode might be active. We use conditional logic (`if state.ui_mode == UiMode::StartupDialog`) to only update startup dialog when appropriate.

2. **Config loading**: `ShowStartupDialog` uses `load_all_configs()` which loads from both `.fdemon/launch.toml` and `.vscode/launch.json` with proper priority ordering (Task 01 implementation).

3. **Device cloning**: `devices.clone()` is used when updating both selectors. This is acceptable since `Device` is a small struct and discovery is infrequent (only on startup and explicit refresh).

4. **Animation ticking**: The `Tick` message handler now updates both `device_selector` and `startup_dialog_state` animations. This ensures loading spinners work correctly in both UI modes.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1153 tests, 0 failures)
- `cargo clippy` - Passed (no warnings)

**Specific tests added:**
- `test_show_startup_dialog_triggers_discovery` - Confirms dialog opens and discovery action is returned
- `test_devices_discovered_updates_startup_dialog` - Verifies device list is updated and loading cleared
- `test_devices_discovered_updates_both_selectors` - Ensures both selectors get device list
- `test_device_discovery_failed_shows_error` - Error state is set correctly
- `test_device_discovery_failed_updates_both_selectors` - Both selectors get error message
- `test_refresh_devices_triggers_discovery` - Refresh button triggers discovery
- `test_tick_advances_startup_dialog_animation` - Animation frame advances when loading
- `test_tick_does_not_advance_startup_dialog_when_not_loading` - Animation stops when not loading
- `test_tick_advances_startup_dialog_when_refreshing` - Animation works during refresh
- `test_devices_discovered_only_updates_startup_dialog_in_startup_mode` - Conditional update logic works

### Risks/Limitations

1. **None identified**: The implementation follows the existing patterns for device discovery used by `device_selector`. All edge cases are covered by tests.
