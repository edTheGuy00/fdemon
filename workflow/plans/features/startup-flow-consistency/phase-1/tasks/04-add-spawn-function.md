## Task: Add Auto-Launch Spawn Function

**Objective**: Create the spawn function that handles `UpdateAction::DiscoverDevicesAndAutoLaunch` by running device discovery and sending result messages.

**Depends on**: 03-add-handler-scaffolding

**Estimated Time**: 2-3 hours

### Scope

- `src/tui/spawn.rs`: Add `spawn_auto_launch()` function
- `src/tui/actions.rs`: Handle the new action in `handle_action()`

### Details

#### 1. Add spawn function (`src/tui/spawn.rs`)

```rust
/// Spawn auto-launch task for device discovery and session launch
///
/// Discovers devices, validates last selection or finds auto-start config,
/// and sends result back via message channel.
pub fn spawn_auto_launch(
    msg_tx: mpsc::Sender<Message>,
    configs: LoadedConfigs,
    project_path: PathBuf,
) {
    tokio::spawn(async move {
        // Step 1: Update progress
        let _ = msg_tx.send(Message::AutoLaunchProgress {
            message: "Detecting devices...".to_string(),
        }).await;

        // Step 2: Discover devices
        let discovery_result = devices::discover_devices().await;

        let devices = match discovery_result {
            Ok(result) => result.devices,
            Err(e) => {
                // Send error result
                let _ = msg_tx.send(Message::AutoLaunchResult {
                    result: Err(e.to_string()),
                }).await;
                return;
            }
        };

        if devices.is_empty() {
            let _ = msg_tx.send(Message::AutoLaunchResult {
                result: Err("No devices found".to_string()),
            }).await;
            return;
        }

        // Step 3: Update progress
        let _ = msg_tx.send(Message::AutoLaunchProgress {
            message: "Preparing launch...".to_string(),
        }).await;

        // Step 4: Try to find best device/config combination
        let success = find_auto_launch_target(&configs, &devices, &project_path);

        // Step 5: Send result
        let _ = msg_tx.send(Message::AutoLaunchResult {
            result: Ok(success),
        }).await;
    });
}

/// Find the best device/config combination for auto-launch
fn find_auto_launch_target(
    configs: &LoadedConfigs,
    devices: &[Device],
    project_path: &Path,
) -> AutoLaunchSuccess {
    // Priority 1: Check settings.local.toml for saved selection
    if let Some(selection) = load_last_selection(project_path) {
        if let Some(validated) = validate_last_selection(&selection, configs, devices) {
            let config = validated.config_idx.and_then(|i| configs.configs.get(i));
            if let Some(device) = validated.device_idx.and_then(|i| devices.get(i)) {
                return AutoLaunchSuccess {
                    device: device.clone(),
                    config: config.map(|c| c.config.clone()),
                };
            }
        }
    }

    // Priority 2: Find auto_start config or first config
    let config = get_first_auto_start(configs).or_else(|| get_first_config(configs));

    if let Some(sourced) = config {
        // Find matching device
        let device = if sourced.config.device == "auto" {
            devices.first()
        } else {
            devices::find_device(devices, &sourced.config.device)
                .or_else(|| devices.first())
        };

        if let Some(device) = device {
            return AutoLaunchSuccess {
                device: device.clone(),
                config: Some(sourced.config.clone()),
            };
        }
    }

    // Priority 3: Bare run with first device
    AutoLaunchSuccess {
        device: devices.first().unwrap().clone(), // Safe: we checked devices.is_empty() above
        config: None,
    }
}
```

#### 2. Handle action in `handle_action()` (`src/tui/actions.rs`)

Add a match arm for the new action:

```rust
UpdateAction::DiscoverDevicesAndAutoLaunch { configs } => {
    spawn::spawn_auto_launch(
        msg_tx,
        configs,
        project_path.to_path_buf(),
    );
}
```

### Import Requirements

In `spawn.rs`:
```rust
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

use crate::app::message::{AutoLaunchSuccess, Message};
use crate::config::{
    get_first_auto_start, get_first_config, load_last_selection,
    validate_last_selection, LoadedConfigs,
};
use crate::daemon::{devices, Device};
```

In `actions.rs`:
```rust
use crate::config::LoadedConfigs;
```

### Acceptance Criteria

1. `spawn_auto_launch()` function exists and compiles
2. Function discovers devices asynchronously
3. Function sends `AutoLaunchProgress` messages during discovery
4. Function sends `AutoLaunchResult` with success or error
5. `find_auto_launch_target()` implements correct priority logic:
   - Settings.local.toml saved selection
   - Auto-start config from launch.toml
   - First config
   - Bare run with first device
6. `handle_action()` dispatches to spawn function
7. `cargo check` passes
8. `cargo clippy -- -D warnings` passes

### Testing

The spawn function is async and integration-style, so testing options:

1. **Mock testing** (preferred): Create a test that mocks `discover_devices()` and verifies message flow
2. **Integration test**: Test with real device discovery (requires Flutter SDK)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_find_auto_launch_target_uses_first_device() {
        let configs = LoadedConfigs::default();
        let devices = vec![
            Device {
                id: "device1".to_string(),
                name: "Test Device".to_string(),
                platform: "android".to_string(),
                emulator: false,
                sdk: "30".to_string(),
                category: DeviceCategory::Mobile,
            },
        ];
        let project_path = Path::new("/tmp/test");

        let result = find_auto_launch_target(&configs, &devices, project_path);

        assert_eq!(result.device.id, "device1");
        assert!(result.config.is_none()); // No configs = bare run
    }
}
```

### Notes

- The logic mirrors `try_auto_start_config()` from `startup.rs` - that function will be removed in Phase 4
- `find_auto_launch_target()` is sync because it only reads in-memory data
- Error handling sends message rather than returning Result, since this runs in spawned task
- The `devices.first().unwrap()` is safe because we check `is_empty()` earlier

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/spawn.rs` | Added `spawn_auto_launch()` function and `find_auto_launch_target()` helper, including unit test |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/actions.rs` | Replaced stub handler for `DiscoverDevicesAndAutoLaunch` with call to `spawn_auto_launch()` |

### Notable Decisions/Tradeoffs

1. **Priority Logic Implementation**: The `find_auto_launch_target()` function implements the three-tier priority system exactly as specified:
   - Priority 1: Saved selection from settings.local.toml
   - Priority 2: Auto-start config or first config from launch.toml
   - Priority 3: Bare run with first device

2. **Error Handling**: Device discovery errors and empty device lists send `AutoLaunchResult` with `Err(String)` message instead of panicking, allowing the UI to display the error gracefully.

3. **Device Matching**: Used existing `devices::find_device()` function for config device matching with fallback to first device if specified device not found.

4. **Progress Messages**: Added two progress messages ("Detecting devices..." and "Preparing launch...") to provide user feedback during async operation.

### Testing Performed

- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test --lib` - Passed (1333 tests, including new `test_find_auto_launch_target_uses_first_device`)
- `cargo test --lib spawn` - Passed (8 tests including the new spawn test)

### Risks/Limitations

1. **Async Task Error Handling**: If the message channel is closed while sending progress/result messages, the task silently returns. This is acceptable as it means the app is shutting down.

2. **Empty Devices Safety**: The `devices.first().unwrap()` in Priority 3 is safe because we check `devices.is_empty()` earlier and return an error if true. Added a comment to clarify this safety invariant.

3. **Device Selection Validation**: The `validate_last_selection()` function requires a valid device to return a result, which aligns with the requirement that we can't auto-launch without a device.
