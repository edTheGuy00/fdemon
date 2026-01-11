# Task: Tool Availability Check

## Summary

Create a module to check for availability of `xcrun simctl` (iOS) and `emulator` (Android) commands at app startup and cache the results.

## Files

| File | Action |
|------|--------|
| `src/daemon/tool_availability.rs` | Create |
| `src/daemon/mod.rs` | Modify (add export) |

## Implementation

### 1. Create ToolAvailability struct

```rust
// src/daemon/tool_availability.rs

/// Cached availability of external tools for device discovery
#[derive(Debug, Clone, Default)]
pub struct ToolAvailability {
    /// Whether `xcrun simctl` is available (macOS with Xcode)
    pub xcrun_simctl: bool,

    /// Whether `emulator` command is available (Android SDK)
    pub android_emulator: bool,

    /// Path to emulator command if found
    pub emulator_path: Option<String>,
}
```

### 2. Implement availability check

```rust
impl ToolAvailability {
    /// Check tool availability (run once at startup)
    pub async fn check() -> Self {
        let xcrun_simctl = Self::check_xcrun_simctl().await;
        let (android_emulator, emulator_path) = Self::check_android_emulator().await;

        Self {
            xcrun_simctl,
            android_emulator,
            emulator_path,
        }
    }

    /// Check if xcrun simctl is available
    async fn check_xcrun_simctl() -> bool {
        // Only available on macOS
        #[cfg(not(target_os = "macos"))]
        return false;

        #[cfg(target_os = "macos")]
        {
            Command::new("xcrun")
                .args(["simctl", "help"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }

    /// Check if Android emulator is available
    async fn check_android_emulator() -> (bool, Option<String>) {
        // Try common paths and PATH
        let paths_to_try = Self::get_emulator_paths();

        for path in paths_to_try {
            if Command::new(&path)
                .arg("-list-avds")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .map(|s| s.success())
                .unwrap_or(false)
            {
                return (true, Some(path));
            }
        }

        (false, None)
    }

    /// Get list of paths to try for emulator command
    fn get_emulator_paths() -> Vec<String> {
        let mut paths = vec!["emulator".to_string()];

        // Check ANDROID_HOME/emulator/emulator
        if let Ok(android_home) = std::env::var("ANDROID_HOME") {
            paths.push(format!("{}/emulator/emulator", android_home));
        }

        // Check ANDROID_SDK_ROOT/emulator/emulator
        if let Ok(sdk_root) = std::env::var("ANDROID_SDK_ROOT") {
            paths.push(format!("{}/emulator/emulator", sdk_root));
        }

        paths
    }
}
```

### 3. Add helper methods for UI messages

```rust
impl ToolAvailability {
    /// Get user-friendly message for unavailable iOS tools
    pub fn ios_unavailable_message(&self) -> Option<&'static str> {
        if self.xcrun_simctl {
            None
        } else {
            #[cfg(target_os = "macos")]
            { Some("Xcode not installed. Install Xcode to manage iOS simulators.") }

            #[cfg(not(target_os = "macos"))]
            { Some("iOS simulators are only available on macOS.") }
        }
    }

    /// Get user-friendly message for unavailable Android tools
    pub fn android_unavailable_message(&self) -> Option<&'static str> {
        if self.android_emulator {
            None
        } else {
            Some("Android SDK not found. Set ANDROID_HOME or install Android Studio.")
        }
    }
}
```

### 4. Export from daemon module

```rust
// src/daemon/mod.rs
mod tool_availability;
pub use tool_availability::ToolAvailability;
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_availability_default() {
        let availability = ToolAvailability::default();
        assert!(!availability.xcrun_simctl);
        assert!(!availability.android_emulator);
    }

    #[test]
    fn test_ios_unavailable_message() {
        let availability = ToolAvailability::default();
        assert!(availability.ios_unavailable_message().is_some());
    }

    #[test]
    fn test_android_unavailable_message() {
        let availability = ToolAvailability::default();
        assert!(availability.android_unavailable_message().is_some());
    }

    #[test]
    fn test_emulator_paths_includes_env_vars() {
        // Set test env var
        std::env::set_var("ANDROID_HOME", "/test/android");
        let paths = ToolAvailability::get_emulator_paths();
        assert!(paths.iter().any(|p| p.contains("/test/android")));
        std::env::remove_var("ANDROID_HOME");
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test tool_availability && cargo clippy -- -D warnings
```

## Notes

- Use `tokio::process::Command` for async execution
- Avoid blocking the UI during startup checks
- Cache results in `AppState` after check completes
