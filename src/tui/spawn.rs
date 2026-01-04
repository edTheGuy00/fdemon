//! Background task spawning for async operations
//!
//! Contains functions that spawn background tokio tasks for:
//! - Device discovery
//! - Emulator discovery and launch
//! - iOS Simulator launch

use tokio::sync::mpsc;

use crate::app::message::Message;
use crate::daemon::{devices, emulators};

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
