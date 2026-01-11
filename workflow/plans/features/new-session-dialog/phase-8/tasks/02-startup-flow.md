# Task: Startup Flow

## Summary

Wire up the app startup flow to use NewSessionDialog. Trigger tool availability check at startup, then show the dialog.

## Files

| File | Action |
|------|--------|
| `src/main.rs` | Modify (startup sequence) |
| `src/app/handler/update.rs` | Modify (add handlers) |

## Implementation

### 1. Update startup sequence

```rust
// src/main.rs or src/app/mod.rs

async fn run_app(project_path: PathBuf) -> Result<()> {
    // Initialize state
    let mut state = AppState::new(project_path.clone());

    // Load configs
    let configs = load_configs(&project_path).await?;
    state.loaded_configs = configs.clone();

    // Start tool availability check (async, don't wait)
    let (tx, rx) = mpsc::channel(100);
    spawn_tool_availability_check(tx.clone());

    // Open NewSessionDialog for startup
    state.new_session_dialog = Some(NewSessionDialogState::new(configs));
    state.ui_mode = UiMode::Startup;

    // Start device discovery (async)
    spawn_device_discovery(tx.clone());

    // Main event loop
    loop {
        // Render
        terminal.draw(|f| render(f, &state))?;

        // Handle events
        select! {
            Some(msg) = rx.recv() => {
                if let Some(action) = handle_message(&mut state, msg) {
                    execute_action(action, tx.clone()).await;
                }
            }
            Ok(event) = event_reader.next() => {
                if let Some(msg) = handle_event(event, &state) {
                    if let Some(action) = handle_message(&mut state, msg) {
                        execute_action(action, tx.clone()).await;
                    }
                }
            }
        }

        // Check for exit
        if state.should_exit {
            break;
        }
    }

    Ok(())
}
```

### 2. Spawn tool availability check

```rust
// src/app/handler/update.rs

use crate::daemon::ToolAvailability;

fn spawn_tool_availability_check(tx: Sender<Message>) {
    tokio::spawn(async move {
        let availability = ToolAvailability::check().await;
        let _ = tx.send(Message::ToolAvailabilityChecked(availability)).await;
    });
}

fn handle_tool_availability_checked(
    state: &mut AppState,
    availability: ToolAvailability,
) -> Option<UpdateAction> {
    state.tool_availability = availability;

    tracing::info!(
        "Tool availability: xcrun_simctl={}, android_emulator={}",
        state.tool_availability.xcrun_simctl,
        state.tool_availability.android_emulator
    );

    None
}
```

### 3. Spawn device discovery

```rust
fn spawn_device_discovery(tx: Sender<Message>) {
    tokio::spawn(async move {
        match discover_flutter_devices().await {
            Ok(devices) => {
                let _ = tx.send(Message::NewSessionDialogConnectedDevicesReceived(devices)).await;
            }
            Err(e) => {
                let _ = tx.send(Message::NewSessionDialogDeviceDiscoveryFailed(e.to_string())).await;
            }
        }
    });
}

async fn discover_flutter_devices() -> Result<Vec<Device>, Error> {
    use crate::daemon::DeviceDiscovery;

    let discovery = DeviceDiscovery::new();
    discovery.discover().await
}
```

### 4. Handle launch success

```rust
fn handle_new_session_dialog_launch_success(
    state: &mut AppState,
    session_id: Uuid,
) -> Option<UpdateAction> {
    // Close dialog
    state.new_session_dialog = None;

    // Switch to normal mode
    state.ui_mode = UiMode::Normal;

    // Select the new session
    state.session_manager.select_session(session_id);

    None
}
```

### 5. Handle 'd' key in normal mode

```rust
// src/app/handler/keys.rs

fn handle_normal_mode_key(key: KeyEvent, state: &AppState) -> Option<Message> {
    match key.code {
        // 'd' opens NewSessionDialog to add device
        KeyCode::Char('d') => Some(Message::OpenNewSessionDialog),

        // ... other keys
        _ => None,
    }
}
```

### 6. Handle auto-launch from config

```rust
// If a config has auto_launch=true, launch immediately after device discovery

fn handle_connected_devices_received_with_auto_launch(
    state: &mut AppState,
    devices: Vec<Device>,
) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.target_selector.set_connected_devices(devices.clone());

        // Check for auto-launch config
        if let Some(ref config) = dialog.launch_context.selected_config() {
            if config.config.auto_launch {
                // Find matching device
                if let Some(device_id) = &config.config.device_id {
                    if let Some(device) = devices.iter().find(|d| &d.id == device_id) {
                        // Auto-launch with this device
                        let params = LaunchParams {
                            device_id: device.id.clone(),
                            mode: dialog.launch_context.mode,
                            flavor: dialog.launch_context.flavor.clone(),
                            dart_defines: dialog.launch_context.dart_defines
                                .iter()
                                .map(|d| d.to_arg())
                                .collect(),
                            config_name: Some(config.display_name.clone()),
                        };
                        return Some(UpdateAction::LaunchFlutterSession(params));
                    }
                }
            }
        }
    }
    None
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_creates_dialog() {
        let configs = LoadedConfigs::default();
        let mut state = AppState::new(PathBuf::from("/test"));

        state.loaded_configs = configs.clone();
        state.new_session_dialog = Some(NewSessionDialogState::new(configs));
        state.ui_mode = UiMode::Startup;

        assert!(state.new_session_dialog.is_some());
        assert_eq!(state.ui_mode, UiMode::Startup);
    }

    #[test]
    fn test_d_key_opens_dialog() {
        let state = create_test_state_with_sessions();
        state.ui_mode = UiMode::Normal;

        let msg = handle_normal_mode_key(
            KeyEvent::from(KeyCode::Char('d')),
            &state,
        );

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_launch_success_closes_dialog() {
        let mut state = create_test_state();
        state.new_session_dialog = Some(NewSessionDialogState::new(LoadedConfigs::default()));
        state.ui_mode = UiMode::NewSessionDialog;

        handle_new_session_dialog_launch_success(&mut state, Uuid::new_v4());

        assert!(state.new_session_dialog.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test startup_flow && cargo clippy -- -D warnings
```

## Manual Testing

1. Start app with no Flutter project → should show NewSessionDialog
2. Start app with project → should show NewSessionDialog with devices
3. Launch session → dialog closes, log view appears
4. Press 'd' in normal mode → NewSessionDialog opens as overlay
5. Launch second session → dialog closes, session tabs update

## Notes

- Tool availability check runs async at startup
- Device discovery runs async at startup
- Auto-launch respects config settings
- 'd' key works in normal mode to add devices
