//! iOS simulator discovery using xcrun simctl
//!
//! This module provides functionality to discover available iOS simulators
//! on macOS using the `xcrun simctl list devices -j` command.

use crate::common::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use tokio::process::Command;

/// A bootable iOS simulator
#[derive(Debug, Clone)]
pub struct IosSimulator {
    pub udid: String,
    pub name: String,
    pub runtime: String, // e.g., "iOS 17.2"
    pub state: SimulatorState,
    pub device_type: String, // e.g., "iPhone 15 Pro"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulatorState {
    Shutdown,
    Booted,
    Booting,
    Unknown,
}

impl From<&str> for SimulatorState {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "shutdown" => SimulatorState::Shutdown,
            "booted" => SimulatorState::Booted,
            "booting" => SimulatorState::Booting,
            _ => SimulatorState::Unknown,
        }
    }
}

/// JSON output from `xcrun simctl list devices -j`
#[derive(Debug, Deserialize)]
struct SimctlOutput {
    devices: HashMap<String, Vec<SimctlDevice>>,
}

#[derive(Debug, Deserialize)]
struct SimctlDevice {
    #[serde(rename = "udid")]
    udid: String,
    name: String,
    state: String,
    #[serde(rename = "deviceTypeIdentifier")]
    _device_type_identifier: Option<String>,
    #[serde(rename = "isAvailable")]
    is_available: Option<bool>,
}

/// List all available iOS simulators
///
/// Returns simulators grouped by runtime, filtered to only available ones.
pub async fn list_ios_simulators() -> Result<Vec<IosSimulator>> {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "-j"])
        .output()
        .await
        .map_err(|e| Error::process(format!("Failed to run xcrun simctl: {}", e)))?;

    if !output.status.success() {
        return Err(Error::process("xcrun simctl returned error"));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: SimctlOutput = serde_json::from_str(&json_str)
        .map_err(|e| Error::protocol(format!("Failed to parse simctl output: {}", e)))?;

    let mut simulators = Vec::new();

    for (runtime_key, devices) in parsed.devices {
        // Extract runtime name (e.g., "com.apple.CoreSimulator.SimRuntime.iOS-17-2" -> "iOS 17.2")
        let runtime = parse_runtime_name(&runtime_key);

        for device in devices {
            // Skip unavailable devices
            if device.is_available == Some(false) {
                continue;
            }

            simulators.push(IosSimulator {
                udid: device.udid,
                name: device.name.clone(),
                runtime: runtime.clone(),
                state: SimulatorState::from(device.state.as_str()),
                device_type: device.name, // simctl gives name as device type
            });
        }
    }

    // Sort by runtime (newest first), then by name
    simulators.sort_by(|a, b| b.runtime.cmp(&a.runtime).then_with(|| a.name.cmp(&b.name)));

    Ok(simulators)
}

/// Parse runtime identifier to friendly name
/// "com.apple.CoreSimulator.SimRuntime.iOS-17-2" -> "iOS 17.2"
fn parse_runtime_name(identifier: &str) -> String {
    if let Some(suffix) = identifier.strip_prefix("com.apple.CoreSimulator.SimRuntime.") {
        // iOS-17-2 -> iOS 17.2
        // watchOS-10-5 -> watchOS 10.5
        // tvOS-17-0 -> tvOS 17.0
        if let Some((os_name, version)) = suffix.split_once('-') {
            let version_formatted = version.replace('-', ".");
            format!("{} {}", os_name, version_formatted)
        } else {
            suffix.to_string()
        }
    } else {
        identifier.to_string()
    }
}

/// Group simulators by runtime for display
pub fn group_simulators_by_runtime(simulators: &[IosSimulator]) -> Vec<(&str, Vec<&IosSimulator>)> {
    let mut groups: BTreeMap<&str, Vec<&IosSimulator>> = BTreeMap::new();

    for sim in simulators {
        groups.entry(&sim.runtime).or_default().push(sim);
    }

    // Convert to vec, sorted by runtime (newest first)
    let mut result: Vec<_> = groups.into_iter().collect();
    result.sort_by(|a, b| b.0.cmp(a.0));
    result
}

/// Boot an iOS simulator by UDID
///
/// Returns Ok(()) when the simulator is booted and ready.
/// Returns error if boot fails or times out.
pub async fn boot_simulator(udid: &str) -> Result<()> {
    // First check if already booted
    if is_simulator_booted(udid).await? {
        return Ok(());
    }

    // Boot the simulator
    let output = Command::new("xcrun")
        .args(["simctl", "boot", udid])
        .output()
        .await
        .map_err(|e| Error::process(format!("Failed to boot simulator: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "Unable to boot device in current state: Booted" is not an error
        if !stderr.contains("Booted") {
            return Err(Error::process(format!(
                "Failed to boot simulator: {}",
                stderr
            )));
        }
    }

    // Wait for simulator to be fully booted
    wait_for_simulator_boot(udid, std::time::Duration::from_secs(60)).await?;

    // Open Simulator.app to show the UI
    let _ = Command::new("open")
        .args(["-a", "Simulator"])
        .output()
        .await;

    Ok(())
}

/// Check if a simulator is already booted
async fn is_simulator_booted(udid: &str) -> Result<bool> {
    let simulators = list_ios_simulators().await?;
    Ok(simulators
        .iter()
        .any(|s| s.udid == udid && s.state == SimulatorState::Booted))
}

/// Wait for simulator to finish booting
async fn wait_for_simulator_boot(udid: &str, max_wait: std::time::Duration) -> Result<()> {
    let poll_interval = tokio::time::Duration::from_millis(500);
    let start = std::time::Instant::now();

    while start.elapsed() < max_wait {
        if is_simulator_booted(udid).await? {
            return Ok(());
        }
        tokio::time::sleep(poll_interval).await;
    }

    Err(Error::process("Simulator boot timed out"))
}

/// Shutdown an iOS simulator
pub async fn shutdown_simulator(udid: &str) -> Result<()> {
    let output = Command::new("xcrun")
        .args(["simctl", "shutdown", udid])
        .output()
        .await
        .map_err(|e| Error::process(format!("Failed to shutdown simulator: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Ignore "Unable to shutdown device in current state: Shutdown"
        if !stderr.contains("Shutdown") {
            return Err(Error::process(format!(
                "Failed to shutdown simulator: {}",
                stderr
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_runtime_name() {
        assert_eq!(
            parse_runtime_name("com.apple.CoreSimulator.SimRuntime.iOS-17-2"),
            "iOS 17.2"
        );
        assert_eq!(
            parse_runtime_name("com.apple.CoreSimulator.SimRuntime.iOS-16-0"),
            "iOS 16.0"
        );
    }

    #[test]
    fn test_simulator_state_from_str() {
        assert_eq!(SimulatorState::from("Shutdown"), SimulatorState::Shutdown);
        assert_eq!(SimulatorState::from("Booted"), SimulatorState::Booted);
        assert_eq!(SimulatorState::from("Booting"), SimulatorState::Booting);
        assert_eq!(SimulatorState::from("shutdown"), SimulatorState::Shutdown);
        assert_eq!(SimulatorState::from("unknown"), SimulatorState::Unknown);
    }

    #[test]
    fn test_parse_simctl_json() {
        let json = r#"{
            "devices": {
                "com.apple.CoreSimulator.SimRuntime.iOS-17-2": [
                    {
                        "udid": "ABC-123",
                        "name": "iPhone 15 Pro",
                        "state": "Shutdown",
                        "isAvailable": true
                    }
                ]
            }
        }"#;

        let parsed: SimctlOutput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.devices.len(), 1);
    }

    #[test]
    fn test_group_simulators_by_runtime() {
        let simulators = vec![
            IosSimulator {
                udid: "1".to_string(),
                name: "iPhone 15".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 15".to_string(),
            },
            IosSimulator {
                udid: "2".to_string(),
                name: "iPhone 14".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 14".to_string(),
            },
            IosSimulator {
                udid: "3".to_string(),
                name: "iPhone 13".to_string(),
                runtime: "iOS 16.0".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 13".to_string(),
            },
        ];

        let groups = group_simulators_by_runtime(&simulators);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "iOS 17.2");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "iOS 16.0");
        assert_eq!(groups[1].1.len(), 1);
    }
}
