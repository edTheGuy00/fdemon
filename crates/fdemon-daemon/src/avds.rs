//! Android AVD (Android Virtual Device) discovery
//!
//! This module provides functionality to list available Android AVDs using the
//! `emulator -list-avds` command from the Android SDK.

use crate::ToolAvailability;
use fdemon_core::prelude::*;
use regex::Regex;
use std::sync::LazyLock;
use tokio::process::Command;
use tokio::time::Duration;

/// Delay to wait after starting emulator for initialization
const AVD_INIT_DELAY: Duration = Duration::from_secs(2);

/// Static regex pattern for extracting API level from AVD names
static API_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"_API_(\d+)$").expect("Invalid API pattern regex"));

/// An Android Virtual Device (AVD)
#[derive(Debug, Clone)]
pub struct AndroidAvd {
    /// AVD name (used for boot command)
    pub name: String,
    /// Friendly display name
    pub display_name: String,
    /// API level (e.g., 33 for Android 13)
    pub api_level: Option<u32>,
    /// Target (e.g., "android-33" or "google_apis")
    pub target: Option<String>,
}

/// List all available Android AVDs
///
/// Uses the emulator path from ToolAvailability if available.
pub async fn list_android_avds(tool_availability: &ToolAvailability) -> Result<Vec<AndroidAvd>> {
    let emulator_cmd = tool_availability
        .emulator_path
        .as_deref()
        .unwrap_or("emulator");

    let output = Command::new(emulator_cmd)
        .arg("-list-avds")
        .output()
        .await
        .map_err(|e| Error::process(format!("Failed to run emulator: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::process(format!(
            "emulator -list-avds failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let avds = parse_avd_list(&stdout);

    Ok(avds)
}

/// Parse the output of `emulator -list-avds`
///
/// Output format is one AVD name per line.
fn parse_avd_list(output: &str) -> Vec<AndroidAvd> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|name| {
            let name = name.trim().to_string();
            let (display_name, api_level) = parse_avd_name(&name);

            AndroidAvd {
                name: name.clone(),
                display_name,
                api_level,
                target: None, // Would need to parse AVD config for this
            }
        })
        .collect()
}

/// Parse AVD name to extract display name and API level
///
/// Common naming patterns:
/// - "Pixel_6_API_33" -> ("Pixel 6", Some(33))
/// - "Nexus_5X_API_29" -> ("Nexus 5X", Some(29))
/// - "My_Custom_AVD" -> ("My Custom AVD", None)
fn parse_avd_name(name: &str) -> (String, Option<u32>) {
    // Try to extract API level from name using static regex
    if let Some(caps) = API_PATTERN.captures(name) {
        let api_level = caps.get(1).and_then(|m| m.as_str().parse().ok());
        let display = API_PATTERN.replace(name, "").replace('_', " ");
        return (display.trim().to_string(), api_level);
    }

    // No API pattern found, just replace underscores
    (name.replace('_', " "), None)
}

/// Boot an Android AVD by name
///
/// Launches the emulator in the background and returns immediately.
/// The emulator process continues running independently.
pub async fn boot_avd(avd_name: &str, tool_availability: &ToolAvailability) -> Result<()> {
    let emulator_cmd = tool_availability
        .emulator_path
        .as_deref()
        .ok_or_else(|| Error::process("Android emulator not available"))?;

    // Start emulator in background
    // Using spawn() instead of output() so it doesn't wait
    let mut child = tokio::process::Command::new(emulator_cmd)
        .args([
            "-avd",
            avd_name,
            "-no-snapshot-load", // Start fresh
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| Error::process(format!("Failed to start emulator: {}", e)))?;

    // Detach the child process so it continues running
    // We don't wait for it to complete
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    // Wait a moment for the emulator to start initializing
    tokio::time::sleep(AVD_INIT_DELAY).await;

    Ok(())
}

/// Check if any Android emulator is currently running
///
/// Uses `adb devices` to check for running emulators.
///
/// # Returns
/// - `Ok(true)` if at least one emulator is detected
/// - `Ok(false)` if no emulators are running or adb fails
pub async fn is_any_emulator_running() -> Result<bool> {
    let output = Command::new("adb")
        .args(["devices", "-l"])
        .output()
        .await
        .map_err(|e| Error::process(format!("Failed to run adb: {}", e)))?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Look for emulator entries
    // Format: "emulator-5554    device product:sdk_gphone64_x86_64 model:sdk_gphone64_x86_64 device:emu64x transport_id:1"
    Ok(stdout.lines().any(|line| line.starts_with("emulator-")))
}

/// Kill all running emulators
pub async fn kill_all_emulators() -> Result<()> {
    let _ = Command::new("adb").args(["emu", "kill"]).output().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_avd_list() {
        let output = "Pixel_6_API_33\nNexus_5X_API_29\nMy_Custom_AVD\n";
        let avds = parse_avd_list(output);

        assert_eq!(avds.len(), 3);
        assert_eq!(avds[0].name, "Pixel_6_API_33");
        assert_eq!(avds[1].name, "Nexus_5X_API_29");
        assert_eq!(avds[2].name, "My_Custom_AVD");
    }

    #[test]
    fn test_parse_avd_name_with_api() {
        let (display, api) = parse_avd_name("Pixel_6_API_33");
        assert_eq!(display, "Pixel 6");
        assert_eq!(api, Some(33));
    }

    #[test]
    fn test_parse_avd_name_without_api() {
        let (display, api) = parse_avd_name("My_Custom_AVD");
        assert_eq!(display, "My Custom AVD");
        assert_eq!(api, None);
    }

    #[test]
    fn test_parse_avd_list_empty() {
        let output = "";
        let avds = parse_avd_list(output);
        assert!(avds.is_empty());
    }

    #[test]
    fn test_parse_avd_list_with_whitespace() {
        let output = "  Pixel_6_API_33  \n\n  Nexus_5X_API_29\n";
        let avds = parse_avd_list(output);

        assert_eq!(avds.len(), 2);
        assert_eq!(avds[0].name, "Pixel_6_API_33");
        assert_eq!(avds[1].name, "Nexus_5X_API_29");
    }

    #[test]
    fn test_parse_avd_name_with_multiple_underscores() {
        let (display, api) = parse_avd_name("Pixel_6_Pro_API_34");
        assert_eq!(display, "Pixel 6 Pro");
        assert_eq!(api, Some(34));
    }

    #[test]
    fn test_parse_avd_name_single_word() {
        let (display, api) = parse_avd_name("MyAVD");
        assert_eq!(display, "MyAVD");
        assert_eq!(api, None);
    }

    #[test]
    fn test_android_avd_structure() {
        let avd = AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: Some("android-33".to_string()),
        };

        assert_eq!(avd.name, "Pixel_6_API_33");
        assert_eq!(avd.display_name, "Pixel 6");
        assert_eq!(avd.api_level, Some(33));
        assert_eq!(avd.target, Some("android-33".to_string()));
    }

    #[tokio::test]
    async fn test_is_any_emulator_running() {
        // This test will check that the function runs without panicking.
        // Actual emulator testing requires Android SDK to be installed.
        // The function should return Ok(false) if adb is not available or no emulators running.
        let result = is_any_emulator_running().await;

        // We accept either Ok(true) or Ok(false) depending on system state
        // An error is also acceptable if adb is not installed
        match result {
            Ok(_) => {
                // Function executed successfully
            }
            Err(e) => {
                // Expected if adb is not available
                assert!(e.to_string().contains("adb"));
            }
        }
    }
}
