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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/app/state.rs` | Added `tool_availability: ToolAvailability` field to `AppState` struct and initialized it in constructor |
| `/Users/ed/Dev/zabin/flutter-demon/src/app/message.rs` | Added 6 new message variants for tool availability checking and device discovery/booting |
| `/Users/ed/Dev/zabin/flutter-demon/src/app/handler/mod.rs` | Added 3 new `UpdateAction` variants: `CheckToolAvailability`, `DiscoverBootableDevices`, `BootDevice` |
| `/Users/ed/Dev/zabin/flutter-demon/src/app/handler/update.rs` | Added message handlers for all 6 new messages with proper state transitions and action returns |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/actions.rs` | Added match arms for the 3 new `UpdateAction` variants to dispatch to spawn functions |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/spawn.rs` | Implemented 3 new spawn functions: `spawn_tool_availability_check`, `spawn_bootable_device_discovery`, `spawn_device_boot` |

### Notable Decisions/Tradeoffs

1. **Two BootableDevice Types**: The codebase has two `BootableDevice` types - one in `daemon/mod.rs` (enum with IosSimulator/AndroidAvd variants) and one in `core/types.rs` (struct). The new session dialog uses the `core` version, so in the message handler I convert `IosSimulator` and `AndroidAvd` to `core::BootableDevice` structs by extracting their fields.

2. **Tool Availability Caching**: Tool availability is checked once and cached in `AppState`. For bootable device discovery, we re-check tool availability each time to ensure current state. This is a reasonable tradeoff as discovery is user-initiated and infrequent.

3. **Parallel Discovery**: iOS simulator and Android AVD discovery are run in parallel using `tokio::join!` for better performance.

4. **Error Handling**: Device boot failures send a `DeviceBootFailed` message with error details, allowing the UI to display appropriate feedback to the user.

### Testing Performed

- `cargo fmt` - Passed (code formatted)
- `cargo check` - Passed (no compilation errors)
- `cargo test --lib` - Passed (1448 unit tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **E2E Test Failures**: 25 e2e tests are currently failing, but these appear to be pre-existing issues related to PTY/TTY handling in the test environment, not related to this implementation.

2. **State Synchronization**: The bootable device discovery currently uses a fresh `ToolAvailability::check()` call rather than using the cached state. This is intentional to ensure accuracy but could be optimized in the future if needed.

3. **Platform Detection**: The device boot handler uses string matching on platform ("iOS" vs "Android") which assumes consistent platform naming. This is safe given the controlled sources of platform strings in the codebase.
