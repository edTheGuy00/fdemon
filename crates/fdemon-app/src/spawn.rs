//! Background task spawning for async operations
//!
//! Contains functions that spawn background tokio tasks for:
//! - Device discovery
//! - Emulator discovery and launch
//! - iOS Simulator launch
//! - Auto-launch device discovery and selection

use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

use crate::config::{
    get_first_auto_start, get_first_config, load_last_selection, validate_last_selection,
    LoadedConfigs,
};
use crate::message::{AutoLaunchSuccess, Message};
use fdemon_daemon::{devices, emulators, Device, FlutterExecutable, ToolAvailability};

/// Spawn device discovery in background (foreground mode - shows errors to user)
pub fn spawn_device_discovery(msg_tx: mpsc::Sender<Message>, flutter: FlutterExecutable) {
    tokio::spawn(async move {
        match devices::discover_devices(&flutter).await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::DevicesDiscovered {
                        devices: result.devices,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::DeviceDiscoveryFailed {
                        error: e.to_string(),
                        is_background: false,
                    })
                    .await;
            }
        }
    });
}

/// Spawn device discovery in background (background mode - errors logged only)
/// Used when refreshing device cache while user already has cached devices to select from
pub fn spawn_device_discovery_background(
    msg_tx: mpsc::Sender<Message>,
    flutter: FlutterExecutable,
) {
    tokio::spawn(async move {
        match devices::discover_devices(&flutter).await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::DevicesDiscovered {
                        devices: result.devices,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::DeviceDiscoveryFailed {
                        error: e.to_string(),
                        is_background: true,
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator discovery in background
pub fn spawn_emulator_discovery(msg_tx: mpsc::Sender<Message>, flutter: FlutterExecutable) {
    tokio::spawn(async move {
        match emulators::discover_emulators(&flutter).await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::EmulatorsDiscovered {
                        emulators: result.emulators,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::EmulatorDiscoveryFailed {
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator launch in background
pub fn spawn_emulator_launch(
    msg_tx: mpsc::Sender<Message>,
    emulator_id: String,
    flutter: FlutterExecutable,
) {
    tokio::spawn(async move {
        match emulators::launch_emulator(&flutter, &emulator_id).await {
            Ok(result) => {
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
            Err(e) => {
                // Create a failed result
                let result = emulators::EmulatorLaunchResult {
                    success: false,
                    emulator_id,
                    message: Some(e.to_string()),
                    elapsed: std::time::Duration::from_secs(0),
                };
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
        }
    });
}

/// Spawn iOS Simulator launch in background (macOS only)
pub fn spawn_ios_simulator_launch(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match emulators::launch_ios_simulator().await {
            Ok(result) => {
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
            Err(e) => {
                let result = emulators::EmulatorLaunchResult {
                    success: false,
                    emulator_id: "apple_ios_simulator".to_string(),
                    message: Some(e.to_string()),
                    elapsed: std::time::Duration::from_secs(0),
                };
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
        }
    });
}

/// Spawn auto-launch task for device discovery and session launch
///
/// Discovers devices, validates last selection or finds auto-start config,
/// and sends result back via message channel.
pub fn spawn_auto_launch(
    msg_tx: mpsc::Sender<Message>,
    configs: LoadedConfigs,
    project_path: PathBuf,
    flutter: FlutterExecutable,
) {
    tokio::spawn(async move {
        // Step 1: Update progress
        let _ = msg_tx
            .send(Message::AutoLaunchProgress {
                message: "Detecting devices...".to_string(),
            })
            .await;

        // Step 2: Discover devices
        let discovery_result = devices::discover_devices(&flutter).await;

        let devices = match discovery_result {
            Ok(result) => {
                // Update device cache for future dialogs (Phase 3, Task 01)
                let _ = msg_tx
                    .send(Message::DevicesDiscovered {
                        devices: result.devices.clone(),
                    })
                    .await;

                result.devices
            }
            Err(e) => {
                // Send error result with helpful context
                let error_msg = format!(
                    "Device discovery failed: {}. Check Flutter SDK installation.",
                    e
                );
                let _ = msg_tx
                    .send(Message::AutoLaunchResult {
                        result: Err(error_msg),
                    })
                    .await;
                return;
            }
        };

        if devices.is_empty() {
            let _ = msg_tx
                .send(Message::AutoLaunchResult {
                    result: Err(
                        "No devices found. Connect a device or start an emulator.".to_string()
                    ),
                })
                .await;
            return;
        }

        // Step 3: Update progress
        let _ = msg_tx
            .send(Message::AutoLaunchProgress {
                message: "Preparing launch...".to_string(),
            })
            .await;

        // Step 4: Try to find best device/config combination
        let success = find_auto_launch_target(&configs, &devices, &project_path);

        // Step 5: Send result
        let _ = msg_tx
            .send(Message::AutoLaunchResult {
                result: Ok(success),
            })
            .await;
    });
}

/// Find the best device/config combination for auto-launch
///
/// Priority order:
/// 1. `launch.toml` config with `auto_start = true` — always wins over cached selection
/// 2. `settings.local.toml` cached `last_device` / `last_config` — used when no auto_start config
/// 3. First launch config + first device (fallback when cache is stale or missing)
/// 4. Bare flutter run with first device (no configs at all)
pub fn find_auto_launch_target(
    configs: &LoadedConfigs,
    devices: &[Device],
    project_path: &Path,
) -> AutoLaunchSuccess {
    // Priority 1: launch.toml config with auto_start = true
    if let Some(result) = try_auto_start_config(configs, devices) {
        return result;
    }

    // Priority 2: settings.local.toml cached selection (only when no auto_start config)
    if let Some(result) = try_cached_selection(configs, devices, project_path) {
        return result;
    }

    // Priority 3: first launch config + first device
    if let Some(result) = try_first_config(configs, devices) {
        return result;
    }

    // Priority 4: bare flutter run with first device
    bare_flutter_run(devices)
}

/// Priority 1: try to find a config with `auto_start = true` and resolve its device.
fn try_auto_start_config(configs: &LoadedConfigs, devices: &[Device]) -> Option<AutoLaunchSuccess> {
    let sourced = get_first_auto_start(configs)?;

    let device = if sourced.config.device == "auto" {
        devices.first()
    } else {
        let found = devices::find_device(devices, &sourced.config.device);
        if found.is_none() {
            tracing::warn!(
                "Configured device '{}' not found, falling back to first available device",
                sourced.config.device
            );
        }
        found.or_else(|| devices.first())
    };

    device.map(|d| AutoLaunchSuccess {
        device: d.clone(),
        config: Some(sourced.config.clone()),
    })
}

/// Priority 2: try the cached `last_device` / `last_config` from `settings.local.toml`.
fn try_cached_selection(
    configs: &LoadedConfigs,
    devices: &[Device],
    project_path: &Path,
) -> Option<AutoLaunchSuccess> {
    let selection = load_last_selection(project_path)?;
    let Some(validated) = validate_last_selection(&selection, configs, devices) else {
        tracing::warn!(
            "Cached selection in settings.local.toml is no longer valid \
             (saved device disconnected or config removed); \
             falling back to first available config + device"
        );
        return None;
    };

    let config = validated.config_idx.and_then(|i| configs.configs.get(i));
    // Defense-in-depth: device_idx is guaranteed Some by validate_last_selection's contract,
    // but we use `?` to guard against future contract drift.
    let device = validated.device_idx.and_then(|i| devices.get(i))?;

    Some(AutoLaunchSuccess {
        device: device.clone(),
        config: config.map(|c| c.config.clone()),
    })
}

/// Priority 3: use the first launch config with the first available device.
fn try_first_config(configs: &LoadedConfigs, devices: &[Device]) -> Option<AutoLaunchSuccess> {
    let sourced = get_first_config(configs)?;

    let device = if sourced.config.device == "auto" {
        devices.first()
    } else {
        let found = devices::find_device(devices, &sourced.config.device);
        if found.is_none() {
            tracing::warn!(
                "Configured device '{}' not found, falling back to first available device",
                sourced.config.device
            );
        }
        found.or_else(|| devices.first())
    };

    device.map(|d| AutoLaunchSuccess {
        device: d.clone(),
        config: Some(sourced.config.clone()),
    })
}

/// Priority 4: bare `flutter run` — no config, just the first device.
fn bare_flutter_run(devices: &[Device]) -> AutoLaunchSuccess {
    AutoLaunchSuccess {
        device: devices
            .first()
            .expect("devices non-empty; checked at spawn_auto_launch line 137")
            .clone(),
        config: None,
    }
}

/// Timeout for tool availability checks
const TOOL_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Spawn tool availability check in background (Phase 4, Task 05)
pub fn spawn_tool_availability_check(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        let availability = match timeout(TOOL_CHECK_TIMEOUT, ToolAvailability::check()).await {
            Ok(result) => result,
            Err(_elapsed) => {
                tracing::warn!(
                    "Tool availability check timed out after {:?}, assuming no tools available",
                    TOOL_CHECK_TIMEOUT
                );
                ToolAvailability::default()
            }
        };

        let _ = msg_tx
            .send(Message::ToolAvailabilityChecked { availability })
            .await;
    });
}

/// Spawn bootable device discovery in background (Phase 4, Task 05)
pub fn spawn_bootable_device_discovery(
    msg_tx: mpsc::Sender<Message>,
    tool_availability: ToolAvailability,
) {
    tokio::spawn(async move {
        // Discover iOS simulators and Android AVDs in parallel
        let (ios_result, android_result) = tokio::join!(
            fdemon_daemon::list_ios_simulators(),
            fdemon_daemon::list_android_avds(&tool_availability)
        );

        let ios_simulators = ios_result.unwrap_or_default();
        let android_avds = android_result.unwrap_or_default();

        let _ = msg_tx
            .send(Message::BootableDevicesDiscovered {
                ios_simulators,
                android_avds,
            })
            .await;
    });
}

/// Spawn device boot in background (Phase 4, Task 05)
pub fn spawn_device_boot(
    msg_tx: mpsc::Sender<Message>,
    device_id: String,
    platform: fdemon_core::Platform,
    tool_availability: ToolAvailability,
) {
    tokio::spawn(async move {
        use fdemon_core::Platform;
        let result = match platform {
            Platform::IOS => fdemon_daemon::boot_simulator(&device_id).await,
            Platform::Android => fdemon_daemon::boot_avd(&device_id, &tool_availability).await,
        };

        match result {
            Ok(()) => {
                let _ = msg_tx
                    .send(Message::DeviceBootCompleted {
                        device_id: device_id.clone(),
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::DeviceBootFailed {
                        device_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn entry point discovery in background (Phase 3, Task 09)
pub fn spawn_entry_point_discovery(msg_tx: mpsc::Sender<Message>, project_path: PathBuf) {
    tokio::spawn(async move {
        // Use spawn_blocking since discover_entry_points is sync I/O
        let entry_points = tokio::task::spawn_blocking(move || {
            fdemon_core::discovery::discover_entry_points(&project_path)
        })
        .await
        .unwrap_or_default();

        let _ = msg_tx
            .send(Message::EntryPointsDiscovered { entry_points })
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{priority::SourcedConfig, types::ConfigSource};
    use tempfile::tempdir;

    fn make_device(id: &str, platform: &str) -> Device {
        Device {
            id: id.to_string(),
            name: id.to_string(),
            platform: platform.to_string(),
            emulator: false,
            emulator_id: None,
            ephemeral: false,
            category: None,
            platform_type: None,
        }
    }

    fn make_sourced_config(name: &str, device: &str, auto_start: bool) -> SourcedConfig {
        use crate::config::types::LaunchConfig;
        SourcedConfig {
            config: LaunchConfig {
                name: name.to_string(),
                device: device.to_string(),
                auto_start,
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: name.to_string(),
        }
    }

    #[test]
    fn test_find_auto_launch_target_uses_first_device() {
        let configs = LoadedConfigs::default();
        let devices = vec![make_device("device1", "android")];
        let project_path = Path::new("/tmp/test");

        let result = find_auto_launch_target(&configs, &devices, project_path);

        assert_eq!(result.device.id, "device1");
        assert!(result.config.is_none()); // No configs = bare run
    }

    #[test]
    fn test_tool_check_timeout_is_reasonable() {
        // Verify timeout is set to a reasonable value (10 seconds)
        assert_eq!(TOOL_CHECK_TIMEOUT.as_secs(), 10);
    }

    /// T1: auto_start config beats cached selection
    ///
    /// launch.toml has auto_start=true with device="android"
    /// settings.local.toml has last_device="macos-device"
    /// Expected: returns android device + auto_start config (cache is ignored)
    #[test]
    fn test_auto_start_config_beats_cached_selection() {
        let temp = tempdir().unwrap();
        let project_path = temp.path();

        // Write a cached selection pointing to a macOS device
        crate::config::save_last_selection(project_path, None, Some("macos-device")).unwrap();

        // Build LoadedConfigs with one auto_start config targeting android
        let mut configs = LoadedConfigs::default();
        configs
            .configs
            .push(make_sourced_config("Dev", "android", true));
        configs.is_empty = false;

        let devices = vec![
            make_device("android-device-1", "android"),
            make_device("macos-device", "macos"),
        ];

        let result = find_auto_launch_target(&configs, &devices, project_path);

        // Should resolve via auto_start, not cache
        assert_eq!(result.device.id, "android-device-1");
        assert_eq!(result.config.as_ref().unwrap().name, "Dev");
    }

    /// T2: no auto_start config → cached selection is used
    ///
    /// launch.toml has no auto_start=true
    /// settings.local.toml has a valid last_device
    /// Expected: returns the cached device selection
    #[test]
    fn test_no_auto_start_uses_cached_selection() {
        let temp = tempdir().unwrap();
        let project_path = temp.path();

        // Write a cached selection pointing to android device
        crate::config::save_last_selection(project_path, None, Some("android-device-1")).unwrap();

        // Build LoadedConfigs with one non-auto_start config
        let mut configs = LoadedConfigs::default();
        configs
            .configs
            .push(make_sourced_config("Dev", "auto", false));
        configs.is_empty = false;

        let devices = vec![
            make_device("android-device-1", "android"),
            make_device("ios-device-1", "ios"),
        ];

        let result = find_auto_launch_target(&configs, &devices, project_path);

        // Should use cached device
        assert_eq!(result.device.id, "android-device-1");
    }

    /// T3: no auto_start + stale cached device → falls through to first config + first device
    ///
    /// launch.toml has no auto_start=true
    /// settings.local.toml has last_device pointing to a device that's no longer available
    /// Expected: falls through to first config + first available device
    #[test]
    fn test_stale_cached_device_falls_through_to_first_config() {
        let temp = tempdir().unwrap();
        let project_path = temp.path();

        // Cache a device that won't be in the discovered list
        crate::config::save_last_selection(project_path, None, Some("disconnected-device"))
            .unwrap();

        // Build LoadedConfigs with one non-auto_start config
        let mut configs = LoadedConfigs::default();
        configs
            .configs
            .push(make_sourced_config("Dev", "auto", false));
        configs.is_empty = false;

        // Only one device available, not the cached one
        let devices = vec![make_device("ios-device-1", "ios")];

        let result = find_auto_launch_target(&configs, &devices, project_path);

        // Should fall through to first config + first device
        assert_eq!(result.device.id, "ios-device-1");
        assert_eq!(result.config.as_ref().unwrap().name, "Dev");
    }

    /// T4 (regression): auto_start=true with device="auto" and no cache → first config + first device
    ///
    /// launch.toml has auto_start=true, device="auto"
    /// No settings.local.toml cache
    /// Expected: returns first config + first device
    #[test]
    fn test_auto_start_with_device_auto_uses_first_device() {
        let temp = tempdir().unwrap();
        let project_path = temp.path();

        // No cache file written

        // Build LoadedConfigs with one auto_start config using device="auto"
        let mut configs = LoadedConfigs::default();
        configs
            .configs
            .push(make_sourced_config("Dev", "auto", true));
        configs.is_empty = false;

        let devices = vec![
            make_device("ios-device-1", "ios"),
            make_device("android-device-1", "android"),
        ];

        let result = find_auto_launch_target(&configs, &devices, project_path);

        // auto_start=true + device="auto" → first device
        assert_eq!(result.device.id, "ios-device-1");
        assert_eq!(result.config.as_ref().unwrap().name, "Dev");
    }

    /// T5: stale cache (cached device disconnected) falls through to Tier 3 (first config + first device)
    ///
    /// settings.local.toml has last_device="disconnected" pointing to a device no longer in the
    /// discovered list.  No auto_start config.
    ///
    /// Expected: `find_auto_launch_target` returns the Tier 3 result (first config + first device).
    /// `try_cached_selection` returns None and warns to the log file via tracing (not asserted here —
    /// tracing::warn! is hard to capture in unit tests; a comment is sufficient per task spec).
    #[test]
    fn test_disconnected_cache_falls_through_to_first_config() {
        let temp = tempdir().unwrap();
        let project_path = temp.path();

        // Cache a device that is NOT in the discovered list
        crate::config::save_last_selection(project_path, None, Some("disconnected")).unwrap();

        // One non-auto_start config; no auto_start wins
        let mut configs = LoadedConfigs::default();
        configs
            .configs
            .push(make_sourced_config("MyConfig", "auto", false));
        configs.is_empty = false;

        let devices = vec![make_device("ios-1", "ios")];

        // Full cascade: Tier 1 skipped (no auto_start), Tier 2 skipped (cache invalid, warns to
        // log file via tracing), Tier 3 resolves to first config + first device.
        let result = find_auto_launch_target(&configs, &devices, project_path);

        assert_eq!(result.device.id, "ios-1");
        assert_eq!(result.config.as_ref().unwrap().name, "MyConfig");

        // Verify try_cached_selection directly returns None for the disconnected device.
        let cached = try_cached_selection(&configs, &devices, project_path);
        assert!(
            cached.is_none(),
            "try_cached_selection should return None when cached device is not in device list"
        );
    }
}
