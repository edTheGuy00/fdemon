# Task: Android AVD Discovery

## Summary

Implement Android AVD (Android Virtual Device) discovery using `emulator -list-avds` to list available emulators.

## Files

| File | Action |
|------|--------|
| `src/daemon/avds.rs` | Create |
| `src/daemon/mod.rs` | Modify (add export) |

## Implementation

### 1. Define AVD types

```rust
// src/daemon/avds.rs

/// An Android Virtual Device (AVD)
#[derive(Debug, Clone)]
pub struct AndroidAvd {
    pub name: String,           // AVD name (used for boot command)
    pub display_name: String,   // Friendly display name
    pub api_level: Option<u32>, // e.g., 33 for Android 13
    pub target: Option<String>, // e.g., "android-33" or "google_apis"
}
```

### 2. Implement discovery function

```rust
use tokio::process::Command;
use crate::common::Error;
use crate::daemon::ToolAvailability;

/// List all available Android AVDs
///
/// Uses the emulator path from ToolAvailability if available.
pub async fn list_android_avds(
    tool_availability: &ToolAvailability,
) -> Result<Vec<AndroidAvd>, Error> {
    let emulator_cmd = tool_availability
        .emulator_path
        .as_deref()
        .unwrap_or("emulator");

    let output = Command::new(emulator_cmd)
        .arg("-list-avds")
        .output()
        .await
        .map_err(|e| Error::recoverable(format!("Failed to run emulator: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::recoverable(format!(
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
    // Try to extract API level from name
    let api_pattern = regex::Regex::new(r"_API_(\d+)$").ok();

    if let Some(re) = api_pattern {
        if let Some(caps) = re.captures(name) {
            let api_level = caps.get(1).and_then(|m| m.as_str().parse().ok());
            let display = re.replace(name, "").replace('_', " ");
            return (display.trim().to_string(), api_level);
        }
    }

    // No API pattern found, just replace underscores
    (name.replace('_', " "), None)
}
```

### 3. Add AVD config reading (optional enhancement)

```rust
use std::path::PathBuf;

/// Get path to AVD config file
fn avd_config_path(avd_name: &str) -> Option<PathBuf> {
    // AVD configs are stored in ~/.android/avd/<name>.avd/config.ini

    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home)
        .join(".android")
        .join("avd")
        .join(format!("{}.avd", avd_name))
        .join("config.ini");

    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Read API level from AVD config file
pub fn read_avd_api_level(avd_name: &str) -> Option<u32> {
    let config_path = avd_config_path(avd_name)?;
    let content = std::fs::read_to_string(config_path).ok()?;

    // Look for "image.sysdir.1=system-images/android-33/..."
    for line in content.lines() {
        if line.starts_with("image.sysdir.1=") {
            // Extract API level from path
            if let Some(api) = extract_api_from_sysdir(line) {
                return Some(api);
            }
        }
    }

    None
}

fn extract_api_from_sysdir(line: &str) -> Option<u32> {
    // image.sysdir.1=system-images/android-33/google_apis/x86_64/
    let re = regex::Regex::new(r"android-(\d+)").ok()?;
    re.captures(line)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
```

## Tests

```rust
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
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test avds && cargo clippy -- -D warnings
```

## Notes

- `emulator -list-avds` is a simple command that just lists AVD names
- For richer metadata, we could parse AVD config files (~/.android/avd/*.avd/config.ini)
- The regex crate is already a dependency for stack trace parsing
