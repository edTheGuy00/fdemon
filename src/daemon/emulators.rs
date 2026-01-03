//! Emulator discovery and launch using flutter emulators command
//!
//! Based on Flutter daemon protocol v3.38.5 (protocol version 0.6.1)
//! See: https://github.com/flutter/flutter/blob/main/packages/flutter_tools/doc/daemon.md

use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::common::prelude::*;

/// Default timeout for emulator list command
const EMULATORS_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for emulator launch (longer as it may need to boot)
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(120);

/// An available emulator/simulator
///
/// Fields `category` and `platformType` added in protocol v0.5.2
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Emulator {
    /// Unique emulator identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Category: "mobile", "web", "desktop", or null
    /// Added in protocol v0.5.2
    #[serde(default)]
    pub category: Option<String>,

    /// Platform type: "android", "ios", etc.
    /// Added in protocol v0.5.2
    #[serde(default)]
    pub platform_type: Option<String>,
}

impl Emulator {
    /// Get the platform display name
    pub fn platform_display(&self) -> &str {
        match self.platform_type.as_deref() {
            Some("android") => "Android",
            Some("ios") => "iOS",
            Some(other) => other,
            None => "Unknown",
        }
    }

    /// Check if this is an Android emulator
    pub fn is_android(&self) -> bool {
        self.platform_type.as_deref() == Some("android")
    }

    /// Check if this is an iOS simulator
    pub fn is_ios(&self) -> bool {
        self.platform_type.as_deref() == Some("ios")
    }

    /// Get a display string for the emulator
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.name, self.platform_display())
    }
}

/// Result of emulator discovery
#[derive(Debug, Clone)]
pub struct EmulatorDiscoveryResult {
    /// List of discovered emulators
    pub emulators: Vec<Emulator>,

    /// Any warning message from Flutter
    pub warning: Option<String>,

    /// Time taken to discover emulators
    pub elapsed: Duration,
}

/// Discover available emulators using flutter emulators --machine
pub async fn discover_emulators() -> Result<EmulatorDiscoveryResult> {
    discover_emulators_with_timeout(EMULATORS_TIMEOUT).await
}

/// Discover emulators with a custom timeout
pub async fn discover_emulators_with_timeout(
    timeout_duration: Duration,
) -> Result<EmulatorDiscoveryResult> {
    let start = std::time::Instant::now();

    info!("Discovering emulators...");

    let output = timeout(timeout_duration, run_flutter_emulators())
        .await
        .map_err(|_| Error::process("Emulator discovery timed out"))??;

    let elapsed = start.elapsed();

    // Parse the JSON output
    let emulators = parse_emulators_output(&output.stdout)?;

    // Check for warnings in stderr
    let warning = if output.stderr.is_empty() {
        None
    } else {
        Some(output.stderr.clone())
    };

    info!("Discovered {} emulators in {:?}", emulators.len(), elapsed);

    Ok(EmulatorDiscoveryResult {
        emulators,
        warning,
        elapsed,
    })
}

/// Run flutter emulators --machine command
async fn run_flutter_emulators() -> Result<FlutterOutput> {
    let output = Command::new("flutter")
        .args(["emulators", "--machine"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::FlutterNotFound
            } else {
                Error::process(format!("Failed to run flutter emulators: {}", e))
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    debug!("flutter emulators stdout: {}", stdout);
    if !stderr.is_empty() {
        debug!("flutter emulators stderr: {}", stderr);
    }

    if !output.status.success() {
        return Err(Error::process(format!(
            "flutter emulators failed with exit code {:?}: {}",
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

/// Parse the JSON output from flutter emulators --machine
fn parse_emulators_output(output: &str) -> Result<Vec<Emulator>> {
    // The output might have non-JSON lines before the array
    let json_start = output.find('[');
    let json_end = output.rfind(']');

    let json_str = match (json_start, json_end) {
        (Some(start), Some(end)) if end > start => &output[start..=end],
        _ => {
            warn!("No JSON array found in flutter emulators output");
            return Ok(Vec::new());
        }
    };

    let emulators: Vec<Emulator> = serde_json::from_str(json_str)
        .map_err(|e| Error::protocol(format!("Failed to parse emulator list: {}", e)))?;

    Ok(emulators)
}

/// Options for launching an emulator
#[derive(Debug, Clone, Default)]
pub struct EmulatorLaunchOptions {
    /// Whether to cold boot the emulator (Android only)
    /// When true, the emulator will boot from a clean state
    /// Added in protocol v0.6.1
    pub cold_boot: bool,
}

/// Result of emulator launch
#[derive(Debug, Clone)]
pub struct EmulatorLaunchResult {
    /// Whether launch was successful
    pub success: bool,

    /// The emulator ID that was launched
    pub emulator_id: String,

    /// Optional message (success info or error details)
    pub message: Option<String>,

    /// Time taken to launch
    pub elapsed: Duration,
}

/// Launch an emulator by ID
pub async fn launch_emulator(emulator_id: &str) -> Result<EmulatorLaunchResult> {
    launch_emulator_with_options(
        emulator_id,
        EmulatorLaunchOptions::default(),
        LAUNCH_TIMEOUT,
    )
    .await
}

/// Launch an emulator with cold boot option (Android only)
///
/// Cold boot starts the emulator from a clean state instead of using a snapshot.
/// This option is silently ignored for non-Android emulators (iOS simulators).
/// Added in Flutter daemon protocol v0.6.1
pub async fn launch_emulator_cold(emulator_id: &str) -> Result<EmulatorLaunchResult> {
    launch_emulator_with_options(
        emulator_id,
        EmulatorLaunchOptions { cold_boot: true },
        LAUNCH_TIMEOUT,
    )
    .await
}

/// Launch an emulator with custom options and timeout
pub async fn launch_emulator_with_options(
    emulator_id: &str,
    options: EmulatorLaunchOptions,
    timeout_duration: Duration,
) -> Result<EmulatorLaunchResult> {
    let start = std::time::Instant::now();

    info!(
        "Launching emulator: {} (cold_boot: {})",
        emulator_id, options.cold_boot
    );

    let result = timeout(
        timeout_duration,
        run_flutter_emulator_launch(emulator_id, options.cold_boot),
    )
    .await
    .map_err(|_| Error::process("Emulator launch timed out"))?;

    let elapsed = start.elapsed();

    match result {
        Ok(output) => {
            let message = if output.stdout.is_empty() && output.stderr.is_empty() {
                None
            } else if !output.stderr.is_empty() {
                Some(output.stderr)
            } else {
                Some(output.stdout)
            };

            Ok(EmulatorLaunchResult {
                success: true,
                emulator_id: emulator_id.to_string(),
                message,
                elapsed,
            })
        }
        Err(e) => Ok(EmulatorLaunchResult {
            success: false,
            emulator_id: emulator_id.to_string(),
            message: Some(e.to_string()),
            elapsed,
        }),
    }
}

/// Run flutter emulators --launch command
///
/// The `cold_boot` parameter is only supported for Android emulators (v0.6.1+)
/// and is silently ignored for iOS simulators.
async fn run_flutter_emulator_launch(emulator_id: &str, cold_boot: bool) -> Result<FlutterOutput> {
    let mut args = vec!["emulators", "--launch", emulator_id];
    if cold_boot {
        args.push("--cold");
    }

    let output = Command::new("flutter")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::FlutterNotFound
            } else {
                Error::process(format!("Failed to launch emulator: {}", e))
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    debug!("flutter emulators --launch stdout: {}", stdout);
    if !stderr.is_empty() {
        debug!("flutter emulators --launch stderr: {}", stderr);
    }

    // Note: flutter emulators --launch may return success even if launch fails
    // The emulator starts asynchronously

    Ok(FlutterOutput { stdout, stderr })
}

/// Launch iOS Simulator (macOS only)
pub async fn launch_ios_simulator() -> Result<EmulatorLaunchResult> {
    let start = std::time::Instant::now();

    info!("Launching iOS Simulator...");

    // On macOS, we can use 'open' to launch Simulator.app
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("open")
            .args(["-a", "Simulator"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::process(format!("Failed to launch iOS Simulator: {}", e)))?;

        let success = output.status.success();
        let elapsed = start.elapsed();

        Ok(EmulatorLaunchResult {
            success,
            emulator_id: "apple_ios_simulator".to_string(),
            message: if success {
                Some("iOS Simulator launched".to_string())
            } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            elapsed,
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(EmulatorLaunchResult {
            success: false,
            emulator_id: "apple_ios_simulator".to_string(),
            message: Some("iOS Simulator is only available on macOS".to_string()),
            elapsed: start.elapsed(),
        })
    }
}

/// Filter emulators by platform
pub fn filter_by_platform<'a>(emulators: &'a [Emulator], platform: &str) -> Vec<&'a Emulator> {
    let platform_lower = platform.to_lowercase();
    emulators
        .iter()
        .filter(|e| {
            e.platform_type
                .as_ref()
                .map(|p| p.to_lowercase() == platform_lower)
                .unwrap_or(false)
        })
        .collect()
}

/// Get Android emulators only
pub fn android_emulators(emulators: &[Emulator]) -> Vec<&Emulator> {
    filter_by_platform(emulators, "android")
}

/// Get iOS simulators only
pub fn ios_simulators(emulators: &[Emulator]) -> Vec<&Emulator> {
    filter_by_platform(emulators, "ios")
}

/// Check if any emulators are available
pub fn has_emulators(emulators: &[Emulator]) -> bool {
    !emulators.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_emulators_output() {
        let output = r#"[
            {
                "id": "Pixel_8_API_34",
                "name": "Pixel 8 API 34",
                "category": "mobile",
                "platformType": "android"
            },
            {
                "id": "apple_ios_simulator",
                "name": "iOS Simulator",
                "category": "mobile",
                "platformType": "ios"
            }
        ]"#;

        let emulators = parse_emulators_output(output).unwrap();

        assert_eq!(emulators.len(), 2);
        assert_eq!(emulators[0].id, "Pixel_8_API_34");
        assert_eq!(emulators[0].name, "Pixel 8 API 34");
        assert!(emulators[0].is_android());
        assert!(!emulators[0].is_ios());

        assert_eq!(emulators[1].id, "apple_ios_simulator");
        assert!(emulators[1].is_ios());
        assert!(!emulators[1].is_android());
    }

    #[test]
    fn test_parse_emulators_with_extra_output() {
        let output = r#"Checking for updates...
[
    {"id": "test_emu", "name": "Test Emulator", "platformType": "android"}
]
Done."#;

        let emulators = parse_emulators_output(output).unwrap();

        assert_eq!(emulators.len(), 1);
        assert_eq!(emulators[0].name, "Test Emulator");
    }

    #[test]
    fn test_parse_emulators_empty() {
        let output = "[]";
        let emulators = parse_emulators_output(output).unwrap();
        assert!(emulators.is_empty());
    }

    #[test]
    fn test_parse_emulators_no_json() {
        let output = "No emulators available";
        let emulators = parse_emulators_output(output).unwrap();
        assert!(emulators.is_empty());
    }

    #[test]
    fn test_emulator_display_name() {
        let android = Emulator {
            id: "test".to_string(),
            name: "Pixel 8".to_string(),
            category: Some("mobile".to_string()),
            platform_type: Some("android".to_string()),
        };

        assert_eq!(android.display_name(), "Pixel 8 (Android)");
        assert_eq!(android.platform_display(), "Android");
    }

    #[test]
    fn test_filter_by_platform() {
        let emulators = vec![
            Emulator {
                id: "pixel".to_string(),
                name: "Pixel".to_string(),
                category: None,
                platform_type: Some("android".to_string()),
            },
            Emulator {
                id: "ios_sim".to_string(),
                name: "iOS Sim".to_string(),
                category: None,
                platform_type: Some("ios".to_string()),
            },
            Emulator {
                id: "nexus".to_string(),
                name: "Nexus".to_string(),
                category: None,
                platform_type: Some("android".to_string()),
            },
        ];

        let android = android_emulators(&emulators);
        assert_eq!(android.len(), 2);

        let ios = ios_simulators(&emulators);
        assert_eq!(ios.len(), 1);
    }

    #[test]
    fn test_emulator_platform_checks() {
        let android = Emulator {
            id: "a".to_string(),
            name: "A".to_string(),
            category: None,
            platform_type: Some("android".to_string()),
        };

        let ios = Emulator {
            id: "i".to_string(),
            name: "I".to_string(),
            category: None,
            platform_type: Some("ios".to_string()),
        };

        let unknown = Emulator {
            id: "u".to_string(),
            name: "U".to_string(),
            category: None,
            platform_type: None,
        };

        assert!(android.is_android());
        assert!(!android.is_ios());

        assert!(ios.is_ios());
        assert!(!ios.is_android());

        assert!(!unknown.is_android());
        assert!(!unknown.is_ios());
        assert_eq!(unknown.platform_display(), "Unknown");
    }

    #[test]
    fn test_has_emulators() {
        let empty: Vec<Emulator> = vec![];
        assert!(!has_emulators(&empty));

        let emulators = vec![Emulator {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: None,
            platform_type: Some("android".to_string()),
        }];
        assert!(has_emulators(&emulators));
    }

    #[test]
    fn test_emulator_launch_options_default() {
        let opts = EmulatorLaunchOptions::default();
        assert!(!opts.cold_boot);
    }

    #[test]
    fn test_emulator_launch_result() {
        let result = EmulatorLaunchResult {
            success: true,
            emulator_id: "test".to_string(),
            message: Some("Launched".to_string()),
            elapsed: Duration::from_secs(1),
        };

        assert!(result.success);
        assert_eq!(result.emulator_id, "test");
    }

    #[tokio::test]
    #[ignore] // Requires Flutter SDK
    async fn test_discover_emulators_integration() {
        let result = discover_emulators().await;

        match result {
            Ok(discovery) => {
                println!(
                    "Found {} emulators in {:?}",
                    discovery.emulators.len(),
                    discovery.elapsed
                );
                for emu in &discovery.emulators {
                    println!("  - {} [{}] ({})", emu.name, emu.id, emu.platform_display());
                }
            }
            Err(Error::FlutterNotFound) => {
                println!("Flutter SDK not found - skipping integration test");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // Actually launches an emulator
    async fn test_launch_emulator_integration() {
        // First discover emulators
        let discovery = discover_emulators().await.unwrap();

        if let Some(android_emu) = android_emulators(&discovery.emulators).first() {
            println!("Launching: {}", android_emu.display_name());

            let result = launch_emulator(&android_emu.id).await.unwrap();

            println!(
                "Launch result: success={}, elapsed={:?}",
                result.success, result.elapsed
            );
            if let Some(msg) = result.message {
                println!("Message: {}", msg);
            }
        } else {
            println!("No Android emulators available for testing");
        }
    }
}
