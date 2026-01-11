# Task: Discovery Integration

## Summary

Integrate tool availability checking and device discovery into the app startup flow and state management.

## Files

| File | Action |
|------|--------|
| `src/daemon/mod.rs` | Modify (add exports) |
| `src/app/state.rs` | Modify (add ToolAvailability to AppState) |
| `src/app/handler/update.rs` | Modify (add startup check) |
| `src/app/message.rs` | Modify (add discovery messages) |

## Implementation

### 1. Export new modules from daemon

```rust
// src/daemon/mod.rs

mod tool_availability;
mod simulators;
mod avds;

pub use tool_availability::ToolAvailability;
pub use simulators::{IosSimulator, SimulatorState, list_ios_simulators, boot_simulator};
pub use avds::{AndroidAvd, list_android_avds, boot_avd};
```

### 2. Add ToolAvailability to AppState

```rust
// src/app/state.rs

use crate::daemon::ToolAvailability;

pub struct AppState {
    // ... existing fields ...

    /// Cached tool availability (checked at startup)
    pub tool_availability: ToolAvailability,
}

impl AppState {
    pub fn new(/* ... */) -> Self {
        Self {
            // ... existing fields ...
            tool_availability: ToolAvailability::default(),
        }
    }
}
```

### 3. Add discovery messages

```rust
// src/app/message.rs

#[derive(Debug, Clone)]
pub enum Message {
    // ... existing variants ...

    /// Tool availability check completed
    ToolAvailabilityChecked(ToolAvailability),

    /// Request to discover bootable devices
    DiscoverBootableDevices,

    /// Bootable devices discovered
    BootableDevicesDiscovered {
        ios_simulators: Vec<IosSimulator>,
        android_avds: Vec<AndroidAvd>,
    },

    /// Boot a device (simulator or AVD)
    BootDevice { device_id: String, platform: String },

    /// Device boot completed
    DeviceBootCompleted { device_id: String },

    /// Device boot failed
    DeviceBootFailed { device_id: String, error: String },
}
```

### 4. Add startup check trigger

```rust
// src/app/handler/update.rs

use crate::daemon::ToolAvailability;

impl Handler {
    /// Initialize tool availability check at startup
    pub fn init_tool_availability_check(&self) -> UpdateAction {
        UpdateAction::CheckToolAvailability
    }
}

// Add to UpdateAction enum
pub enum UpdateAction {
    // ... existing variants ...

    /// Check tool availability (runs at startup)
    CheckToolAvailability,

    /// Discover bootable devices
    DiscoverBootableDevices,

    /// Boot a specific device
    BootDevice { device_id: String, platform: String },
}
```

### 5. Handle tool availability result

```rust
// src/app/handler/update.rs

fn handle_tool_availability_checked(
    state: &mut AppState,
    availability: ToolAvailability,
) -> Option<UpdateAction> {
    state.tool_availability = availability;

    // Log availability for debugging
    tracing::info!(
        "Tool availability: xcrun_simctl={}, android_emulator={}",
        state.tool_availability.xcrun_simctl,
        state.tool_availability.android_emulator
    );

    None
}
```

### 6. Implement action executor

```rust
// src/app/mod.rs or wherever actions are executed

async fn execute_action(action: UpdateAction, tx: Sender<Message>) {
    match action {
        UpdateAction::CheckToolAvailability => {
            let availability = ToolAvailability::check().await;
            let _ = tx.send(Message::ToolAvailabilityChecked(availability));
        }

        UpdateAction::DiscoverBootableDevices => {
            // Discover in parallel
            let (ios_result, android_result) = tokio::join!(
                list_ios_simulators(),
                list_android_avds(&ToolAvailability::default()) // TODO: get from state
            );

            let ios_simulators = ios_result.unwrap_or_default();
            let android_avds = android_result.unwrap_or_default();

            let _ = tx.send(Message::BootableDevicesDiscovered {
                ios_simulators,
                android_avds,
            });
        }

        UpdateAction::BootDevice { device_id, platform } => {
            let result = match platform.as_str() {
                "iOS" => boot_simulator(&device_id).await,
                "Android" => boot_avd(&device_id, &ToolAvailability::default()).await,
                _ => Err(Error::recoverable("Unknown platform")),
            };

            match result {
                Ok(()) => {
                    let _ = tx.send(Message::DeviceBootCompleted { device_id });
                }
                Err(e) => {
                    let _ = tx.send(Message::DeviceBootFailed {
                        device_id,
                        error: e.to_string(),
                    });
                }
            }
        }

        // ... other actions ...
    }
}
```

### 7. Add to startup sequence

```rust
// In app initialization (e.g., main.rs or app/mod.rs)

// After creating app state, trigger tool availability check
let action = UpdateAction::CheckToolAvailability;
action_tx.send(action).await?;
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_has_tool_availability() {
        let state = AppState::new(/* ... */);
        // Default should have both unavailable
        assert!(!state.tool_availability.xcrun_simctl);
        assert!(!state.tool_availability.android_emulator);
    }

    #[test]
    fn test_handle_tool_availability_checked() {
        let mut state = AppState::new(/* ... */);
        let availability = ToolAvailability {
            xcrun_simctl: true,
            android_emulator: true,
            emulator_path: Some("/path/to/emulator".to_string()),
        };

        handle_tool_availability_checked(&mut state, availability);

        assert!(state.tool_availability.xcrun_simctl);
        assert!(state.tool_availability.android_emulator);
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test discovery && cargo clippy -- -D warnings
```

## Notes

- Tool availability check runs once at app startup
- Results are cached in `AppState.tool_availability`
- Bootable device discovery is triggered when user opens the Bootable tab
- Boot commands are async and send completion/failure messages
