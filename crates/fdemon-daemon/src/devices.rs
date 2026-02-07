//! Device discovery using flutter devices command

use fdemon_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Default timeout for flutter devices command
const DEVICES_TIMEOUT: Duration = Duration::from_secs(30);

/// A connected Flutter device
///
/// Based on Flutter daemon protocol v3.38.5 (protocol version 0.6.1)
/// Note: `flutter devices --machine` uses `targetPlatform` while daemon uses `platform`
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Unique device identifier
    pub id: String,

    /// Human-readable device name
    pub name: String,

    /// Platform identifier (e.g., "ios", "android", "darwin", "web-javascript")
    /// Note: `flutter devices --machine` uses `targetPlatform`, daemon uses `platform`
    #[serde(alias = "targetPlatform")]
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
            "android" | "android-arm" | "android-arm64" | "android-x64" | "android-x86" => {
                "emulator"
            }
            _ => "virtual",
        }
    }

    /// Get a short platform name for display
    pub fn platform_short(&self) -> &str {
        match self.platform.as_str() {
            p if p.starts_with("ios") => "iOS",
            p if p.starts_with("android") => "Android",
            "macos" | "darwin" => "macOS",
            "windows" => "Windows",
            "linux" => "Linux",
            "chrome" | "web-javascript" => "Web",
            "fuchsia" => "Fuchsia",
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
pub async fn discover_devices_with_timeout(
    timeout_duration: Duration,
) -> Result<DeviceDiscoveryResult> {
    let start = std::time::Instant::now();

    info!("Discovering Flutter devices...");

    let output = timeout(timeout_duration, run_flutter_devices())
        .await
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

    // Be lenient with exit codes - flutter devices may fail for non-critical reasons
    // (e.g., adb not found) but still output valid Linux/web devices
    if !output.status.success() {
        // Check if there's still usable JSON output before failing
        if stdout.contains('[') && stdout.contains(']') {
            warn!(
                "flutter devices exited with code {:?} but has JSON output, parsing anyway",
                output.status.code()
            );
        } else {
            return Err(Error::process(format!(
                "flutter devices failed with exit code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }
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
    devices
        .iter()
        .filter(|d| d.platform.to_lowercase().starts_with(&platform_lower))
        .collect()
}

/// Check if any devices are available
pub fn has_devices(devices: &[Device]) -> bool {
    !devices.is_empty()
}

/// Get devices grouped by platform
pub fn group_by_platform(devices: &[Device]) -> HashMap<String, Vec<&Device>> {
    let mut groups: HashMap<String, Vec<&Device>> = HashMap::new();

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
            emulator_id: None,
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
    fn test_parse_devices_with_all_fields() {
        let output = r#"[{
            "id": "702ABC1F-5EA5-4F83-84AB-6380CA91D39A",
            "name": "iPhone 15 Pro",
            "platform": "ios",
            "category": "mobile",
            "platformType": "ios",
            "ephemeral": false,
            "emulator": true,
            "emulatorId": "apple_ios_simulator"
        }]"#;

        let devices = parse_devices_output(output).unwrap();

        assert_eq!(devices.len(), 1);
        let device = &devices[0];
        assert_eq!(device.id, "702ABC1F-5EA5-4F83-84AB-6380CA91D39A");
        assert_eq!(device.name, "iPhone 15 Pro");
        assert_eq!(device.platform, "ios");
        assert_eq!(device.category, Some("mobile".to_string()));
        assert_eq!(device.platform_type, Some("ios".to_string()));
        assert!(!device.ephemeral);
        assert!(device.emulator);
        assert_eq!(device.emulator_id, Some("apple_ios_simulator".to_string()));
    }

    #[test]
    fn test_parse_devices_with_target_platform() {
        // Test the actual format from `flutter devices --machine`
        // which uses `targetPlatform` instead of `platform`
        let output = r#"[
          {
            "name": "Tacos al pastor",
            "id": "00008110-000455042605801E",
            "isSupported": true,
            "targetPlatform": "ios",
            "emulator": false,
            "sdk": "iOS 26.3 23D5089e",
            "capabilities": {
              "hotReload": true,
              "hotRestart": true
            }
          },
          {
            "name": "macOS",
            "id": "macos",
            "isSupported": true,
            "targetPlatform": "darwin",
            "emulator": false,
            "sdk": "macOS 26.1"
          },
          {
            "name": "Chrome",
            "id": "chrome",
            "isSupported": true,
            "targetPlatform": "web-javascript",
            "emulator": false,
            "sdk": "Google Chrome 143"
          }
        ]"#;

        let devices = parse_devices_output(output).unwrap();

        assert_eq!(devices.len(), 3);

        // iOS device
        assert_eq!(devices[0].name, "Tacos al pastor");
        assert_eq!(devices[0].platform, "ios");
        assert_eq!(devices[0].platform_short(), "iOS");

        // macOS uses "darwin" in targetPlatform
        assert_eq!(devices[1].name, "macOS");
        assert_eq!(devices[1].platform, "darwin");
        assert_eq!(devices[1].platform_short(), "macOS");

        // Chrome uses "web-javascript"
        assert_eq!(devices[2].name, "Chrome");
        assert_eq!(devices[2].platform, "web-javascript");
        assert_eq!(devices[2].platform_short(), "Web");
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
    fn test_device_emulator_type() {
        assert_eq!(
            sample_device("", "", "ios", true).emulator_type(),
            "simulator"
        );
        assert_eq!(
            sample_device("", "", "ios_x64", true).emulator_type(),
            "simulator"
        );
        assert_eq!(
            sample_device("", "", "ios_arm64", true).emulator_type(),
            "simulator"
        );
        assert_eq!(
            sample_device("", "", "android", true).emulator_type(),
            "emulator"
        );
        assert_eq!(
            sample_device("", "", "android-arm64", true).emulator_type(),
            "emulator"
        );
        assert_eq!(
            sample_device("", "", "chrome", true).emulator_type(),
            "virtual"
        );
    }

    #[test]
    fn test_device_platform_short() {
        assert_eq!(sample_device("", "", "ios", false).platform_short(), "iOS");
        assert_eq!(
            sample_device("", "", "ios_x64", false).platform_short(),
            "iOS"
        );
        assert_eq!(
            sample_device("", "", "android-arm64", false).platform_short(),
            "Android"
        );
        assert_eq!(
            sample_device("", "", "macos", false).platform_short(),
            "macOS"
        );
        assert_eq!(
            sample_device("", "", "chrome", false).platform_short(),
            "Web"
        );
        assert_eq!(
            sample_device("", "", "web-javascript", false).platform_short(),
            "Web"
        );
        assert_eq!(
            sample_device("", "", "linux", false).platform_short(),
            "Linux"
        );
        assert_eq!(
            sample_device("", "", "windows", false).platform_short(),
            "Windows"
        );
    }

    #[test]
    fn test_device_matches_by_id() {
        let device = sample_device("00008101-ABC123", "iPhone 15 Pro", "ios", false);

        // Exact ID
        assert!(device.matches("00008101-ABC123"));

        // Case-insensitive ID
        assert!(device.matches("00008101-abc123"));
    }

    #[test]
    fn test_device_matches_by_name() {
        let device = sample_device("id", "iPhone 15 Pro Max", "ios", false);

        // Name contains
        assert!(device.matches("iPhone"));
        assert!(device.matches("iphone 15"));
        assert!(device.matches("Pro Max"));

        // No match
        assert!(!device.matches("Pixel"));
    }

    #[test]
    fn test_device_matches_by_platform() {
        let device = Device {
            id: "id".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            platform_type: Some("ios".to_string()),
            ..sample_device("", "", "", false)
        };

        // Platform prefix
        assert!(device.matches("ios"));

        // Platform type
        assert!(device.matches("ios"));

        // No match
        assert!(!device.matches("android"));
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
        assert_eq!(find_device(&devices, "AUTO").unwrap().name, "iPhone 15");

        // Not found
        assert!(find_device(&devices, "windows").is_none());
    }

    #[test]
    fn test_find_device_empty() {
        let devices: Vec<Device> = vec![];
        assert!(find_device(&devices, "auto").is_none());
        assert!(find_device(&devices, "ios").is_none());
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
    fn test_has_devices() {
        let empty: Vec<Device> = vec![];
        assert!(!has_devices(&empty));

        let devices = vec![sample_device("id", "Device", "ios", false)];
        assert!(has_devices(&devices));
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
        assert!(groups.get("Windows").is_none());
    }

    #[tokio::test]
    #[ignore] // Requires Flutter SDK
    async fn test_discover_devices_integration() {
        let result = discover_devices().await;

        // This test requires Flutter SDK to be installed
        match result {
            Ok(discovery) => {
                println!(
                    "Found {} devices in {:?}",
                    discovery.devices.len(),
                    discovery.elapsed
                );
                for device in &discovery.devices {
                    println!(
                        "  - {} ({}) [{}]",
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
}
