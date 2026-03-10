## Task: iOS Log Config + Dispatch

**Objective**: Add `IosLogConfig` struct and `"ios"` dispatch arm to `create_native_log_capture()` in the shared native log infrastructure module. Also declare the `ios` submodule.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/native_logs/mod.rs`: Add `IosLogConfig`, declare `pub mod ios`, add `"ios"` arm to `create_native_log_capture()`
- `crates/fdemon-daemon/src/native_logs/ios.rs`: Create stub file with `IosLogCapture` struct (full implementation in tasks 03/04)
- `crates/fdemon-daemon/src/lib.rs`: Export `IosLogConfig` (cfg-gated)

### Details

#### 1. Add `IosLogConfig` to `mod.rs`

```rust
/// Configuration for iOS native log capture.
///
/// iOS has two capture backends depending on whether the target is a simulator
/// or a physical device:
/// - Simulator: `xcrun simctl spawn <udid> log stream --style syslog`
/// - Physical: `idevicesyslog -u <udid> -p <process>`
#[cfg(target_os = "macos")]
#[derive(Clone)]
pub struct IosLogConfig {
    /// The device UDID (simulator or physical).
    /// Used as `xcrun simctl spawn <udid>` or `idevicesyslog -u <udid>`.
    pub device_udid: String,
    /// Whether this is a simulator device.
    /// Determines which capture tool to use.
    pub is_simulator: bool,
    /// Process name to filter by (e.g., "Runner").
    /// Used as `--predicate 'process == "<name>"'` (simulator)
    /// or `-p <name>` (physical).
    pub process_name: String,
    /// Tags/subsystems to exclude from output.
    pub exclude_tags: Vec<String>,
    /// If non-empty, only show these tags.
    pub include_tags: Vec<String>,
    /// Minimum log level (e.g., "debug", "info").
    pub min_level: String,
}
```

#### 2. Declare `ios` submodule

Add to `mod.rs` alongside the existing `android` and `macos` declarations:

```rust
pub mod android;
#[cfg(target_os = "macos")]
pub mod ios;
#[cfg(target_os = "macos")]
pub mod macos;
```

#### 3. Create stub `ios.rs`

Create `crates/fdemon-daemon/src/native_logs/ios.rs` with a minimal stub so the module compiles:

```rust
//! # iOS Native Log Capture
//!
//! Captures native platform logs from iOS devices (physical and simulator).
//!
//! ## Simulator
//!
//! Uses `xcrun simctl spawn <udid> log stream --predicate 'process == "Runner"' --style syslog`
//! to capture the unified logging stream. Output format matches macOS `log stream` syslog format.
//!
//! ## Physical Device
//!
//! Uses `idevicesyslog -u <udid> -p Runner` to relay the device's syslog stream.
//! Output format: `Mon DD HH:MM:SS DeviceName Process(Framework)[PID] <Level>: message`
//!
//! ## Tool Availability
//!
//! - Simulator: requires `xcrun simctl` (checked via `ToolAvailability::xcrun_simctl`)
//! - Physical: requires `idevicesyslog` (checked via `ToolAvailability::idevicesyslog`)
//! - Both tools only available on macOS (this entire module is `#[cfg(target_os = "macos")]`)

use super::{IosLogConfig, NativeLogCapture, NativeLogHandle};

/// iOS native log capture backend.
///
/// Spawns either `xcrun simctl spawn log stream` (simulator) or
/// `idevicesyslog` (physical device) based on `config.is_simulator`.
pub struct IosLogCapture {
    config: IosLogConfig,
}

impl IosLogCapture {
    pub fn new(config: IosLogConfig) -> Self {
        Self { config }
    }
}

impl NativeLogCapture for IosLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        // Stub — implemented in tasks 03 (simulator) and 04 (physical).
        tracing::warn!("iOS native log capture not yet implemented");
        None
    }
}
```

#### 4. Add `"ios"` dispatch arm to `create_native_log_capture()`

Extend the existing function signature to accept `ios_config`:

```rust
pub fn create_native_log_capture(
    platform: &str,
    android_config: Option<AndroidLogConfig>,
    #[cfg(target_os = "macos")] macos_config: Option<MacOsLogConfig>,
    #[cfg(target_os = "macos")] ios_config: Option<IosLogConfig>,
) -> Option<Box<dyn NativeLogCapture>> {
    match platform {
        "android" => {
            let config = android_config?;
            Some(Box::new(android::AndroidLogCapture::new(config)))
        }
        #[cfg(target_os = "macos")]
        "macos" => {
            let config = macos_config?;
            Some(Box::new(macos::MacOsLogCapture::new(config)))
        }
        #[cfg(target_os = "macos")]
        "ios" => {
            let config = ios_config?;
            Some(Box::new(ios::IosLogCapture::new(config)))
        }
        _ => None,
    }
}
```

**Important:** This changes the signature of `create_native_log_capture()`, so call sites in `fdemon-app/src/actions/native_logs.rs` must be updated to pass the new `ios_config` parameter (as `None` for now — wired in task 05).

#### 5. Export from `lib.rs`

Add `IosLogConfig` to the existing exports in `crates/fdemon-daemon/src/lib.rs`:

```rust
#[cfg(target_os = "macos")]
pub use native_logs::IosLogConfig;
```

#### 6. Update call sites for new parameter

The `create_native_log_capture()` call in `fdemon-app/src/actions/native_logs.rs` must add the new `ios_config` parameter (pass `None` for now):

```rust
let capture = create_native_log_capture(
    &platform,
    android_config,
    #[cfg(target_os = "macos")]
    macos_config,
    #[cfg(target_os = "macos")]
    None, // ios_config — wired in task 05
);
```

### Acceptance Criteria

1. `IosLogConfig` struct exists with fields: `device_udid`, `is_simulator`, `process_name`, `exclude_tags`, `include_tags`, `min_level`
2. `IosLogConfig` derives `Clone` and is gated `#[cfg(target_os = "macos")]`
3. `pub mod ios;` is declared in `mod.rs` behind `#[cfg(target_os = "macos")]`
4. `ios.rs` exists with `IosLogCapture` struct implementing `NativeLogCapture` (stub `spawn()`)
5. `create_native_log_capture("ios", ...)` returns `Some` when given a config
6. `create_native_log_capture("ios", ..., None)` returns `None` (no config)
7. Existing `"android"` and `"macos"` dispatch arms unchanged
8. `IosLogConfig` is exported from `fdemon-daemon/src/lib.rs`
9. Existing call sites updated to pass new parameter (as `None`)
10. `cargo check --workspace` compiles

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn test_dispatch_ios_with_config_returns_some() {
        let config = IosLogConfig {
            device_udid: "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE".to_string(),
            is_simulator: true,
            process_name: "Runner".to_string(),
            exclude_tags: vec!["flutter".to_string()],
            include_tags: vec![],
            min_level: "info".to_string(),
        };
        let result = create_native_log_capture(
            "ios",
            None,
            None,
            Some(config),
        );
        assert!(result.is_some());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_dispatch_ios_without_config_returns_none() {
        let result = create_native_log_capture(
            "ios",
            None,
            None,
            None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_dispatch_android_still_works() {
        let config = AndroidLogConfig {
            device_serial: "emulator-5554".to_string(),
            pid: Some(1234),
            exclude_tags: vec![],
            include_tags: vec![],
            min_level: "info".to_string(),
        };
        let result = create_native_log_capture(
            "android",
            Some(config),
            #[cfg(target_os = "macos")]
            None,
            #[cfg(target_os = "macos")]
            None,
        );
        assert!(result.is_some());
    }
}
```

### Notes

- The `ios_config` parameter uses `#[cfg(target_os = "macos")]` attribute on the parameter itself, matching the existing pattern for `macos_config`. This means on non-macOS builds the parameter is omitted entirely.
- The stub `ios.rs` returns `None` from `spawn()` — tasks 03 and 04 replace this with real implementations.
- The signature change to `create_native_log_capture()` is a breaking change within the workspace. The call site in `actions/native_logs.rs` must be updated in this task to pass `None` for `ios_config`. The actual iOS config building is wired in task 05.
