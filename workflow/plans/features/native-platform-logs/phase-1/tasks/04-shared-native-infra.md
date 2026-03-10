## Task: Shared Native Log Capture Infrastructure

**Objective**: Create the `native_logs` module in `fdemon-daemon` with the shared types (`NativeLogEvent`), the `NativeLogCapture` trait, and platform dispatch function. This provides the common interface that Android and macOS capture backends implement.

**Depends on**: 01-core-types (for `NativeLogPriority`, `LogLevel`)

### Scope

- `crates/fdemon-daemon/src/native_logs/mod.rs`: **NEW** — shared types, trait, platform dispatch
- `crates/fdemon-daemon/src/lib.rs`: Declare and re-export the new module

### Details

#### 1. Create module structure

```
crates/fdemon-daemon/src/native_logs/
├── mod.rs       ← shared types, trait, dispatch (this task)
├── android.rs   ← task 05
└── macos.rs     ← task 06
```

#### 2. Define `NativeLogEvent`

The shared event type emitted by all platform capture backends:

```rust
use fdemon_core::LogLevel;

/// A single log line captured from a native platform log source.
#[derive(Debug, Clone)]
pub struct NativeLogEvent {
    /// The native log tag (e.g., "GoLog", "OkHttp", "com.example.plugin").
    pub tag: String,
    /// The log level, already mapped from platform-specific priority.
    pub level: LogLevel,
    /// The log message content.
    pub message: String,
    /// Raw timestamp string from the platform log (format varies by platform).
    pub timestamp: Option<String>,
}
```

#### 3. Define `NativeLogHandle`

The return type from spawning a native log capture process — provides the channel to receive events and a way to shut down the capture:

```rust
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

/// Handle to a running native log capture process.
pub struct NativeLogHandle {
    /// Receive native log events from the capture process.
    pub event_rx: mpsc::Receiver<NativeLogEvent>,
    /// Send `true` to signal the capture process to stop.
    pub shutdown_tx: watch::Sender<bool>,
    /// The background task handle — can be aborted as a fallback.
    pub task_handle: JoinHandle<()>,
}
```

This follows the existing pattern from performance polling (`perf_shutdown_tx` + `perf_task_handle` on `SessionHandle`).

#### 4. Define `NativeLogCapture` trait

```rust
/// Trait for platform-specific native log capture backends.
///
/// Each platform implements this to spawn and manage a native log process
/// (e.g., `adb logcat` for Android, `log stream` for macOS).
pub trait NativeLogCapture: Send + Sync {
    /// Spawn the native log capture process.
    ///
    /// Returns a `NativeLogHandle` with:
    /// - An `mpsc::Receiver` for receiving parsed log events
    /// - A `watch::Sender` for signaling shutdown
    /// - A `JoinHandle` for the background task
    ///
    /// Returns `None` if the capture cannot be started (e.g., missing tool,
    /// unknown PID, etc.). The caller should log a warning and continue.
    fn spawn(&self) -> Option<NativeLogHandle>;
}
```

#### 5. Define platform-specific config structs

These carry the per-platform parameters needed to start capture:

```rust
/// Configuration for Android logcat capture.
pub struct AndroidLogConfig {
    /// The ADB device serial (e.g., "emulator-5554", "R5CT200QFLJ").
    /// Passed as `adb -s <serial>`.
    pub device_serial: String,
    /// The app's process ID for `--pid` filtering.
    /// If `None`, falls back to unfiltered capture.
    pub pid: Option<u32>,
    /// Tags to exclude from output (e.g., ["flutter"]).
    pub exclude_tags: Vec<String>,
    /// If non-empty, only show these tags (overrides exclude_tags).
    pub include_tags: Vec<String>,
    /// Minimum priority level string (e.g., "info").
    pub min_level: String,
}

/// Configuration for macOS `log stream` capture.
#[cfg(target_os = "macos")]
pub struct MacOsLogConfig {
    /// Process name to filter by (e.g., "my_flutter_app").
    pub process_name: String,
    /// Tags/subsystems to exclude from output.
    pub exclude_tags: Vec<String>,
    /// If non-empty, only show these tags.
    pub include_tags: Vec<String>,
    /// Minimum log level for `log stream --level` (e.g., "debug", "info").
    pub min_level: String,
}
```

#### 6. Define platform dispatch function

```rust
/// Create the appropriate native log capture backend for the given platform.
///
/// Returns `None` for platforms that don't need native log capture
/// (Linux, Windows, Web — already covered by stdout/stderr pipe).
pub fn create_native_log_capture(
    platform: &str,
    android_config: Option<AndroidLogConfig>,
    #[cfg(target_os = "macos")]
    macos_config: Option<MacOsLogConfig>,
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
        _ => None, // Linux, Windows, Web — no native capture needed
    }
}
```

Note: This function will initially not compile until tasks 05 and 06 create the `AndroidLogCapture` and `MacOsLogCapture` structs. To allow incremental development, you can either:
- Stub out the android/macos modules with empty implementations
- Comment out the match arms and add them in tasks 05/06
- Use `#[cfg(feature = "...")]` gating (not recommended)

**Recommended approach**: Create stub files for `android.rs` and `macos.rs` with minimal compilable structs:

```rust
// android.rs (stub — filled in by task 05)
use super::{AndroidLogConfig, NativeLogCapture, NativeLogHandle};

pub struct AndroidLogCapture {
    config: AndroidLogConfig,
}

impl AndroidLogCapture {
    pub fn new(config: AndroidLogConfig) -> Self {
        Self { config }
    }
}

impl NativeLogCapture for AndroidLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        todo!("Implemented in task 05")
    }
}
```

#### 7. Register module in `lib.rs`

Add to `crates/fdemon-daemon/src/lib.rs`:

```rust
pub mod native_logs;
```

And re-export key types:

```rust
pub use native_logs::{NativeLogEvent, NativeLogHandle, NativeLogCapture, AndroidLogConfig};
#[cfg(target_os = "macos")]
pub use native_logs::MacOsLogConfig;
```

### Acceptance Criteria

1. `crates/fdemon-daemon/src/native_logs/mod.rs` exists with `NativeLogEvent`, `NativeLogHandle`, `NativeLogCapture` trait, config structs
2. `NativeLogEvent` can be constructed with tag, level, message, optional timestamp
3. `NativeLogHandle` contains `event_rx`, `shutdown_tx`, `task_handle`
4. `NativeLogCapture` trait has `spawn()` method returning `Option<NativeLogHandle>`
5. `create_native_log_capture("linux", ...)` returns `None`
6. `create_native_log_capture("android", Some(config), ...)` returns `Some(...)` (with stub)
7. Module is declared and key types re-exported from `fdemon-daemon` crate root
8. Stub `android.rs` and `macos.rs` files compile
9. Workspace compiles: `cargo check --workspace`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::LogLevel;

    #[test]
    fn test_native_log_event_construction() {
        let event = NativeLogEvent {
            tag: "GoLog".to_string(),
            level: LogLevel::Info,
            message: "test message".to_string(),
            timestamp: Some("03-10 14:30:00.123".to_string()),
        };
        assert_eq!(event.tag, "GoLog");
        assert_eq!(event.level, LogLevel::Info);
    }

    #[test]
    fn test_dispatch_unsupported_platform_returns_none() {
        let result = create_native_log_capture(
            "linux",
            None,
            #[cfg(target_os = "macos")]
            None,
        );
        assert!(result.is_none());
    }
}
```

### Notes

- The `NativeLogCapture` trait uses `fn spawn(&self)` (not `async fn`) because `tokio::spawn` itself is sync — the async work runs inside the spawned task. This matches the pattern in `FlutterProcess::spawn_internal()`.
- `NativeLogEvent` carries `LogLevel` (already mapped) rather than `NativeLogPriority` to keep the app layer simple — the daemon layer does the platform-specific priority mapping.
- The `watch::channel<bool>` + `JoinHandle` pattern for shutdown is identical to `perf_shutdown_tx`/`perf_task_handle` on `SessionHandle`. The app layer (task 07) will store these on the handle.
- The stub approach for `android.rs`/`macos.rs` allows this task to complete independently while tasks 05 and 06 fill in the real implementations.

---

## Completion Summary

**Status:** Not Started
