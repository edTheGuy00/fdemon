# Task: iOS Simulator Discovery

## Summary

Implement iOS simulator discovery using `xcrun simctl list devices -j` to parse and return available simulators.

## Files

| File | Action |
|------|--------|
| `src/daemon/simulators.rs` | Create |
| `src/daemon/mod.rs` | Modify (add export) |

## Implementation

### 1. Define simulator types

```rust
// src/daemon/simulators.rs

use serde::Deserialize;

/// A bootable iOS simulator
#[derive(Debug, Clone)]
pub struct IosSimulator {
    pub udid: String,
    pub name: String,
    pub runtime: String,        // e.g., "iOS 17.2"
    pub state: SimulatorState,
    pub device_type: String,    // e.g., "iPhone 15 Pro"
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
```

### 2. Define JSON parsing structures

```rust
/// JSON output from `xcrun simctl list devices -j`
#[derive(Debug, Deserialize)]
struct SimctlOutput {
    devices: std::collections::HashMap<String, Vec<SimctlDevice>>,
}

#[derive(Debug, Deserialize)]
struct SimctlDevice {
    #[serde(rename = "udid")]
    udid: String,
    name: String,
    state: String,
    #[serde(rename = "deviceTypeIdentifier")]
    device_type_identifier: Option<String>,
    #[serde(rename = "isAvailable")]
    is_available: Option<bool>,
}
```

### 3. Implement discovery function

```rust
use tokio::process::Command;
use crate::common::Error;

/// List all available iOS simulators
///
/// Returns simulators grouped by runtime, filtered to only available ones.
pub async fn list_ios_simulators() -> Result<Vec<IosSimulator>, Error> {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "-j"])
        .output()
        .await
        .map_err(|e| Error::recoverable(format!("Failed to run xcrun simctl: {}", e)))?;

    if !output.status.success() {
        return Err(Error::recoverable("xcrun simctl returned error"));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: SimctlOutput = serde_json::from_str(&json_str)
        .map_err(|e| Error::recoverable(format!("Failed to parse simctl output: {}", e)))?;

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
    simulators.sort_by(|a, b| {
        b.runtime.cmp(&a.runtime)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(simulators)
}

/// Parse runtime identifier to friendly name
/// "com.apple.CoreSimulator.SimRuntime.iOS-17-2" -> "iOS 17.2"
fn parse_runtime_name(identifier: &str) -> String {
    if let Some(suffix) = identifier.strip_prefix("com.apple.CoreSimulator.SimRuntime.") {
        // iOS-17-2 -> iOS 17.2
        suffix.replace('-', " ").replace(" ", ".").replace("iOS.", "iOS ")
            .replace("watchOS.", "watchOS ")
            .replace("tvOS.", "tvOS ")
    } else {
        identifier.to_string()
    }
}
```

### 4. Add grouping helper

```rust
/// Group simulators by runtime for display
pub fn group_simulators_by_runtime(
    simulators: &[IosSimulator]
) -> Vec<(&str, Vec<&IosSimulator>)> {
    let mut groups: std::collections::BTreeMap<&str, Vec<&IosSimulator>> =
        std::collections::BTreeMap::new();

    for sim in simulators {
        groups.entry(&sim.runtime).or_default().push(sim);
    }

    // Convert to vec, sorted by runtime (newest first)
    let mut result: Vec<_> = groups.into_iter().collect();
    result.sort_by(|a, b| b.0.cmp(a.0));
    result
}
```

## Tests

```rust
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
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test simulators && cargo clippy -- -D warnings
```

## Notes

- Only runs on macOS (guard with cfg attribute if needed)
- Filter out unavailable simulators (isAvailable: false)
- Handle simulators that are already booted (state: Booted)
