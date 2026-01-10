//! Background task spawning for async operations
//!
//! Contains functions that spawn background tokio tasks for:
//! - Device discovery
//! - Emulator discovery and launch
//! - iOS Simulator launch
//! - Auto-launch device discovery and selection

use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

use crate::app::message::{AutoLaunchSuccess, Message};
use crate::config::{
    get_first_auto_start, get_first_config, load_last_selection, validate_last_selection,
    LoadedConfigs,
};
use crate::daemon::{devices, emulators, Device};

/// Spawn device discovery in background
pub fn spawn_device_discovery(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match devices::discover_devices().await {
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
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator discovery in background
pub fn spawn_emulator_discovery(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match emulators::discover_emulators().await {
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
pub fn spawn_emulator_launch(msg_tx: mpsc::Sender<Message>, emulator_id: String) {
    tokio::spawn(async move {
        match emulators::launch_emulator(&emulator_id).await {
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
) {
    tokio::spawn(async move {
        // Step 1: Update progress
        let _ = msg_tx
            .send(Message::AutoLaunchProgress {
                message: "Detecting devices...".to_string(),
            })
            .await;

        // Step 2: Discover devices
        let discovery_result = devices::discover_devices().await;

        let devices = match discovery_result {
            Ok(result) => result.devices,
            Err(e) => {
                // Send error result
                let _ = msg_tx
                    .send(Message::AutoLaunchResult {
                        result: Err(e.to_string()),
                    })
                    .await;
                return;
            }
        };

        if devices.is_empty() {
            let _ = msg_tx
                .send(Message::AutoLaunchResult {
                    result: Err("No devices found".to_string()),
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
            devices::find_device(devices, &sourced.config.device).or_else(|| devices.first())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_auto_launch_target_uses_first_device() {
        let configs = LoadedConfigs::default();
        let devices = vec![Device {
            id: "device1".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            emulator_id: None,
            ephemeral: false,
            category: None,
            platform_type: None,
        }];
        let project_path = Path::new("/tmp/test");

        let result = find_auto_launch_target(&configs, &devices, project_path);

        assert_eq!(result.device.id, "device1");
        assert!(result.config.is_none()); // No configs = bare run
    }
}
