## Task: Device Discovery

**Objective**: Implement device discovery by running `flutter devices --machine` to get the list of connected devices, parse the JSON output into typed structs, and provide an API for querying available devices.

**Depends on**: [01-config-module](01-config-module.md)

---

### Scope

- `src/daemon/devices.rs`: **NEW** - Device discovery and parsing
- `src/daemon/mod.rs`: Add `pub mod devices;` and re-exports
- `src/daemon/events.rs`: Ensure `DeviceInfo` struct is reusable

---

### Implementation Details

#### Device Discovery Strategy

We use `flutter devices --machine` for device discovery because:
1. **Simpler**: One-shot command vs. long-running daemon
2. **Reliable**: No connection management needed
3. **Sufficient**: Device list only needs refresh on user request or startup

Future enhancement could upgrade to `flutter daemon` with `device.enable` for live updates.

#### Protocol Reference

Based on Flutter daemon protocol v3.38.5 (protocol version 0.6.1).
See: https://github.com/flutter/flutter/blob/main/packages/flutter_tools/doc/daemon.md

**Changelog relevant to device fields:**
- v0.5.3: Added `emulatorId` field to device
- v0.5.2: Added `platformType` and `category` fields to emulator
- v0.5.1: Added `platformType`, `ephemeral`, and `category` fields to device

#### Device Info Structure

The existing `DeviceInfo` in `src/daemon/events.rs` can be reused:

```rust
// Already exists in src/daemon/events.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    #[serde(default)]
    pub emulator: bool,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub platform_type: Option<String>,
    #[serde(default)]
    pub ephemeral: bool,
}
```

Additional fields from `flutter devices --machine`:

```rust
/// Extended device info from flutter devices command
/// 
/// Based on Flutter daemon protocol v3.38.5 (protocol version 0.6.1)
/// See: https://github.com/flutter/flutter/blob/main/packages/flutter_tools/doc/daemon.md
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Unique device identifier
    pub id: String,
    
    /// Human-readable device name (e.g., "iPhone 15 Pro")
    pub name: String,
    
    /// Platform identifier (e.g., "ios", "android", "macos", "chrome")
    pub platform: String,
    
    /// Whether this is an emulator/simulator
    #[serde(default)]
    pub emulator: bool,
    
    /// Device category: "mobile", "web", "desktop", or null
    /// Added in protocol v0.5.1
    #[serde(default)]
    pub category: Option<String>,
    
    /// Platform type: "android", "ios", "linux", "macos", "fuchsia", "windows", "web"
    /// Added in protocol v0.5.1
    #[serde(default)]
    pub platform_type: Option<String>,
    
    /// Whether device is ephemeral (needs manual connection, e.g., physical Android device)
    /// Non-ephemeral devices are always present (e.g., web device)
    /// Added in protocol v0.5.1
    #[serde(default)]
    pub ephemeral: bool,
    
    /// Emulator ID that matches the ID from `emulator.getEmulators`
    /// Allows matching running devices to the emulators that started them
    /// Note: May be null even for emulators if connection failed
    /// Added in protocol v0.5.3
    #[serde(default)]
    pub emulator_id: Option<String>,
}
```

#### Device Discovery Module (`src/daemon/devices.rs`)

```rust
//! Device discovery using flutter devices command

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::common::prelude::*;

/// Default timeout for flutter devices command
const DEVICES_TIMEOUT: Duration = Duration::from_secs(30);

/// A connected Flutter device
/// 
/// Based on Flutter daemon protocol v3.38.5 (protocol version 0.6.1)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Unique device identifier
    pub id: String,
    
    /// Human-readable device name
    pub name: String,
    
    /// Platform identifier (e.g., "ios", "android", "macos")
    pub platform: String,
    
    /// Whether this is an emulator/simulator
    #[serde(default)]
    pub emulator: bool,
    
    /// Device category: "mobile", "web", "desktop", or null
    #[serde(default)]
    pub category: Option<String>,
    
    /// Platform type: "android", "ios", "linux", "macos", "fuchsia", "windows", "web"
    #[serde(default)]
    pub platform_type: Option<String>,
    
    /// Whether device is ephemeral (needs manual connection)
    #[serde(default)]
    pub ephemeral: bool,
    
    /// Emulator ID matching `emulator.getEmulators` result
    /// Useful for hiding emulators that are already running as devices
    /// Added in protocol v0.5.3
    #[serde(default)]
    pub emulator_id: Option<String>,
}

impl Device {
    /// Get a display string for the device
    pub fn display_name(&self) -> String {
        if self.emulator {
            format!("{} ({})", self.name, self.emulator_type())
        } else {
            self.name.clone()
        }
    }
    
    /// Get emulator type string
    pub fn emulator_type(&self) -> &'static str {
        match self.platform.as_str() {
            "ios" | "ios_x64" | "ios_arm64" => "simulator",
            "android" | "android-arm" | "android-arm64" | "android-x64" | "android-x86" => "emulator",
            _ => "virtual",
        }
    }
    
    /// Get a short platform name for display
    pub fn platform_short(&self) -> &str {
        match self.platform.as_str() {
            p if p.starts_with("ios") => "iOS",
            p if p.starts_with("android") => "Android",
            "macos" => "macOS",
            "windows" => "Windows",
            "linux" => "Linux",
            "chrome" | "web-javascript" => "Web",
            _ => &self.platform,
        }
    }
    
    /// Check if device matches a device specifier
    /// 
    /// The specifier can be:
    /// - Exact device ID
    /// - Device name (case-insensitive partial match)
    /// - Platform prefix (e.g., "ios", "android")
    pub fn matches(&self, specifier: &str) -> bool {
        let spec_lower = specifier.to_lowercase();
        
        // Exact ID match
        if self.id.to_lowercase() == spec_lower {
            return true;
        }
        
        // Name contains (case-insensitive)
        if self.name.to_lowercase().contains(&spec_lower) {
            return true;
        }
        
        // Platform prefix match
        if self.platform.to_lowercase().starts_with(&spec_lower) {
            return true;
        }
        
        // Platform type match
        if let Some(ref pt) = self.platform_type {
            if pt.to_lowercase() == spec_lower {
                return true;
            }
        }
        
        false
    }
}

/// Result of device discovery
#[derive(Debug, Clone)]
pub struct DeviceDiscoveryResult {
    /// List of discovered devices
    pub devices: Vec<Device>,
    
    /// Any warning message from Flutter
    pub warning: Option<String>,
    
    /// Time taken to discover devices
    pub elapsed: Duration,
}

/// Discover connected devices using flutter devices --machine
/// 
/// This runs the flutter command and parses the JSON output.
pub async fn discover_devices() -> Result<DeviceDiscoveryResult> {
    discover_devices_with_timeout(DEVICES_TIMEOUT).await
}

/// Discover devices with a custom timeout
pub async fn discover_devices_with_timeout(timeout_duration: Duration) -> Result<DeviceDiscoveryResult> {
    let start = std::time::Instant::now();
    
    info!("Discovering Flutter devices...");
    
    let output = timeout(timeout_duration, run_flutter_devices()).await
        .map_err(|_| Error::process("Device discovery timed out"))??;
    
    let elapsed = start.elapsed();
    
    // Parse the JSON output
    let devices = parse_devices_output(&output.stdout)?;
    
    // Check for warnings in stderr
    let warning = if output.stderr.is_empty() {
        None
    } else {
        Some(output.stderr.clone())
    };
    
    info!("Discovered {} devices in {:?}", devices.len(), elapsed);
    
    Ok(DeviceDiscoveryResult {
        devices,
        warning,
        elapsed,
    })
}

/// Run flutter devices command
async fn run_flutter_devices() -> Result<FlutterOutput> {
    let output = Command::new("flutter")
        .args(["devices", "--machine"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::FlutterNotFound
            } else {
                Error::process(format!("Failed to run flutter devices: {}", e))
            }
        })?;
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    debug!("flutter devices stdout: {}", stdout);
    if !stderr.is_empty() {
        debug!("flutter devices stderr: {}", stderr);
    }
    
    if !output.status.success() {
        return Err(Error::process(format!(
            "flutter devices failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        )));
    }
    
    Ok(FlutterOutput { stdout, stderr })
}

struct FlutterOutput {
    stdout: String,
    stderr: String,
}

/// Parse the JSON output from flutter devices --machine
fn parse_devices_output(output: &str) -> Result<Vec<Device>> {
    // The output might have non-JSON lines (like "Downloading..." messages)
    // Find the JSON array in the output
    let json_start = output.find('[');
    let json_end = output.rfind(']');
    
    let json_str = match (json_start, json_end) {
        (Some(start), Some(end)) if end > start => &output[start..=end],
        _ => {
            warn!("No JSON array found in flutter devices output");
            return Ok(Vec::new());
        }
    };
    
    let devices: Vec<Device> = serde_json::from_str(json_str)
        .map_err(|e| Error::protocol(format!("Failed to parse device list: {}", e)))?;
    
    Ok(devices)
}

/// Find a device matching the given specifier
/// 
/// Returns the first matching device, or None if no match.
pub fn find_device<'a>(devices: &'a [Device], specifier: &str) -> Option<&'a Device> {
    // Special case: "auto" returns first available device
    if specifier.to_lowercase() == "auto" {
        return devices.first();
    }
    
    devices.iter().find(|d| d.matches(specifier))
}

/// Filter devices by platform
pub fn filter_by_platform<'a>(devices: &'a [Device], platform: &str) -> Vec<&'a Device> {
    let platform_lower = platform.to_lowercase();
    devices.iter()
        .filter(|d| d.platform.to_lowercase().starts_with(&platform_lower))
        .collect()
}

/// Check if any devices are available
pub fn has_devices(devices: &[Device]) -> bool {
    !devices.is_empty()
}

/// Get devices grouped by platform
pub fn group_by_platform(devices: &[Device]) -> std::collections::HashMap<String, Vec<&Device>> {
    let mut groups: std::collections::HashMap<String, Vec<&Device>> = std::collections::HashMap::new();
    
    for device in devices {
        let platform = device.platform_short().to_string();
        groups.entry(platform).or_default().push(device);
    }
    
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn sample_device(id: &str, name: &str, platform: &str, emulator: bool) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: platform.to_string(),
            emulator,
            category: None,
            platform_type: None,
            ephemeral: false,
            sdk: None,
            is_supported: true,
        }
    }
    
    #[test]
    fn test_parse_devices_output() {
        let output = r#"[
            {
                "id": "00008101-000123456789001E",
                "name": "iPhone 15 Pro",
                "platform": "ios",
                "emulator": false
            },
            {
                "id": "emulator-5554",
                "name": "Pixel 8 API 34",
                "platform": "android-arm64",
                "emulator": true
            }
        ]"#;
        
        let devices = parse_devices_output(output).unwrap();
        
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "iPhone 15 Pro");
        assert!(!devices[0].emulator);
        assert_eq!(devices[1].name, "Pixel 8 API 34");
        assert!(devices[1].emulator);
    }
    
    #[test]
    fn test_parse_devices_with_extra_output() {
        let output = r#"Downloading iOS tools...
[
    {"id": "chrome", "name": "Chrome", "platform": "web-javascript", "emulator": false}
]
Some trailing message"#;
        
        let devices = parse_devices_output(output).unwrap();
        
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Chrome");
    }
    
    #[test]
    fn test_parse_devices_empty() {
        let output = "[]";
        let devices = parse_devices_output(output).unwrap();
        assert!(devices.is_empty());
    }
    
    #[test]
    fn test_parse_devices_no_json() {
        let output = "Some error message without JSON";
        let devices = parse_devices_output(output).unwrap();
        assert!(devices.is_empty());
    }
    
    #[test]
    fn test_device_display_name() {
        let physical = sample_device("id1", "iPhone 15", "ios", false);
        assert_eq!(physical.display_name(), "iPhone 15");
        
        let simulator = sample_device("id2", "iPhone 15", "ios", true);
        assert_eq!(simulator.display_name(), "iPhone 15 (simulator)");
        
        let emulator = sample_device("id3", "Pixel 8", "android-arm64", true);
        assert_eq!(emulator.display_name(), "Pixel 8 (emulator)");
    }
    
    #[test]
    fn test_device_platform_short() {
        assert_eq!(sample_device("", "", "ios", false).platform_short(), "iOS");
        assert_eq!(sample_device("", "", "ios_x64", false).platform_short(), "iOS");
        assert_eq!(sample_device("", "", "android-arm64", false).platform_short(), "Android");
        assert_eq!(sample_device("", "", "macos", false).platform_short(), "macOS");
        assert_eq!(sample_device("", "", "chrome", false).platform_short(), "Web");
        assert_eq!(sample_device("", "", "linux", false).platform_short(), "Linux");
    }
    
    #[test]
    fn test_device_matches() {
        let device = Device {
            id: "00008101-ABC123".to_string(),
            name: "iPhone 15 Pro Max".to_string(),
            platform: "ios".to_string(),
            platform_type: Some("ios".to_string()),
            ..sample_device("", "", "", false)
        };
        
        // Exact ID
        assert!(device.matches("00008101-ABC123"));
        
        // Case-insensitive ID
        assert!(device.matches("00008101-abc123"));
        
        // Name contains
        assert!(device.matches("iPhone"));
        assert!(device.matches("iphone 15"));
        assert!(device.matches("Pro Max"));
        
        // Platform
        assert!(device.matches("ios"));
        
        // No match
        assert!(!device.matches("android"));
        assert!(!device.matches("Pixel"));
    }
    
    #[test]
    fn test_find_device() {
        let devices = vec![
            sample_device("id1", "iPhone 15", "ios", false),
            sample_device("id2", "Pixel 8", "android-arm64", true),
            sample_device("chrome", "Chrome", "web-javascript", false),
        ];
        
        // By ID
        assert_eq!(find_device(&devices, "id1").unwrap().name, "iPhone 15");
        
        // By name
        assert_eq!(find_device(&devices, "Pixel").unwrap().name, "Pixel 8");
        
        // By platform
        assert_eq!(find_device(&devices, "ios").unwrap().name, "iPhone 15");
        
        // Auto selects first
        assert_eq!(find_device(&devices, "auto").unwrap().name, "iPhone 15");
        
        // Not found
        assert!(find_device(&devices, "windows").is_none());
    }
    
    #[test]
    fn test_filter_by_platform() {
        let devices = vec![
            sample_device("id1", "iPhone 15", "ios", false),
            sample_device("id2", "iPhone 14", "ios", true),
            sample_device("id3", "Pixel 8", "android-arm64", true),
        ];
        
        let ios = filter_by_platform(&devices, "ios");
        assert_eq!(ios.len(), 2);
        
        let android = filter_by_platform(&devices, "android");
        assert_eq!(android.len(), 1);
        
        let windows = filter_by_platform(&devices, "windows");
        assert!(windows.is_empty());
    }
    
    #[test]
    fn test_group_by_platform() {
        let devices = vec![
            sample_device("id1", "iPhone 15", "ios", false),
            sample_device("id2", "iPhone 14", "ios_x64", true),
            sample_device("id3", "Pixel 8", "android-arm64", true),
            sample_device("chrome", "Chrome", "chrome", false),
        ];
        
        let groups = group_by_platform(&devices);
        
        assert_eq!(groups.get("iOS").map(|v| v.len()), Some(2));
        assert_eq!(groups.get("Android").map(|v| v.len()), Some(1));
        assert_eq!(groups.get("Web").map(|v| v.len()), Some(1));
    }
}
```

---

### Acceptance Criteria

1. [ ] `src/daemon/devices.rs` created with device discovery implementation
2. [ ] `Device` struct deserializes from `flutter devices --machine` output
3. [ ] `discover_devices()` runs flutter command and parses JSON output
4. [ ] Discovery handles timeout (default 30 seconds)
5. [ ] Non-JSON output before/after the array is tolerated
6. [ ] `Device::matches()` supports ID, name, and platform matching
7. [ ] `find_device()` with "auto" returns first device
8. [ ] `Device::display_name()` shows emulator/simulator type
9. [ ] `Device::platform_short()` returns user-friendly platform names
10. [ ] Error handling for flutter not found
11. [ ] All new code has unit tests
12. [ ] `cargo test` passes
13. [ ] `cargo clippy` has no warnings

---

### Testing

Unit tests are included in the implementation above. Integration test:

```rust
#[tokio::test]
#[ignore] // Requires Flutter SDK
async fn test_discover_devices_integration() {
    let result = discover_devices().await;
    
    // This test requires Flutter SDK to be installed
    match result {
        Ok(discovery) => {
            println!("Found {} devices in {:?}", discovery.devices.len(), discovery.elapsed);
            for device in &discovery.devices {
                println!("  - {} ({}) [{}]", 
                    device.display_name(), 
                    device.platform_short(),
                    device.id
                );
            }
        }
        Err(Error::FlutterNotFound) => {
            println!("Flutter SDK not found - skipping integration test");
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
```

---

### Notes

- The `flutter devices --machine` output may include downloading messages before the JSON
- Some devices may have `is_supported: false` if missing requirements
- Web browsers (chrome) are ephemeral devices
- Physical iOS devices require valid development certificates
- The command may take longer if Flutter needs to check for updates

---

### Files to Create/Modify

| File | Action |
|------|--------|
| `src/daemon/devices.rs` | Create with device discovery implementation |
| `src/daemon/mod.rs` | Add `pub mod devices;` and re-export `Device`, `discover_devices` |