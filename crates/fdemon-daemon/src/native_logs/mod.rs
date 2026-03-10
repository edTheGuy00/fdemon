//! # Native Log Capture Infrastructure
//!
//! Shared types, trait, and platform dispatch for native platform log capture.
//!
//! Each platform backend (Android logcat, macOS unified logging) implements the
//! [`NativeLogCapture`] trait and is selected at runtime via [`create_native_log_capture`].
//!
//! ## Platform Support
//!
//! | Platform | Mechanism          | Module        |
//! |----------|--------------------|---------------|
//! | Android  | `adb logcat`       | `android.rs`  |
//! | macOS    | `log stream`       | `macos.rs`    |
//! | Others   | Not needed (pipe)  | —             |

pub mod android;
#[cfg(target_os = "macos")]
pub mod macos;

use fdemon_core::LogLevel;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

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

/// Handle to a running native log capture process.
///
/// Follows the same pattern as `perf_shutdown_tx`/`perf_task_handle` on `SessionHandle`:
/// send `true` on `shutdown_tx` to signal graceful stop, or abort `task_handle` as fallback.
pub struct NativeLogHandle {
    /// Receive native log events from the capture process.
    pub event_rx: mpsc::Receiver<NativeLogEvent>,
    /// Send `true` to signal the capture process to stop.
    pub shutdown_tx: watch::Sender<bool>,
    /// The background task handle — can be aborted as a fallback.
    pub task_handle: JoinHandle<()>,
}

/// Trait for platform-specific native log capture backends.
///
/// Each platform implements this to spawn and manage a native log process
/// (e.g., `adb logcat` for Android, `log stream` for macOS).
pub trait NativeLogCapture: Send + Sync {
    /// Spawn the native log capture process.
    ///
    /// Returns a [`NativeLogHandle`] with:
    /// - An `mpsc::Receiver` for receiving parsed log events
    /// - A `watch::Sender` for signaling shutdown
    /// - A `JoinHandle` for the background task
    ///
    /// Returns `None` if the capture cannot be started (e.g., missing tool,
    /// unknown PID, etc.). The caller should log a warning and continue.
    fn spawn(&self) -> Option<NativeLogHandle>;
}

/// Configuration for Android logcat capture.
pub struct AndroidLogConfig {
    /// The ADB device serial (e.g., "emulator-5554", "R5CT200QFLJ").
    /// Passed as `adb -s <serial>`.
    pub device_serial: String,
    /// The app's process ID for `--pid` filtering.
    /// If `None`, falls back to unfiltered capture.
    pub pid: Option<u32>,
    /// Tags to exclude from output (e.g., `["flutter"]`).
    pub exclude_tags: Vec<String>,
    /// If non-empty, only show these tags (overrides `exclude_tags`).
    pub include_tags: Vec<String>,
    /// Minimum priority level string (e.g., `"info"`).
    pub min_level: String,
}

/// Configuration for macOS `log stream` capture.
#[cfg(target_os = "macos")]
pub struct MacOsLogConfig {
    /// Process name to filter by (e.g., `"my_flutter_app"`).
    pub process_name: String,
    /// Tags/subsystems to exclude from output.
    pub exclude_tags: Vec<String>,
    /// If non-empty, only show these tags.
    pub include_tags: Vec<String>,
    /// Minimum log level for `log stream --level` (e.g., `"debug"`, `"info"`).
    pub min_level: String,
}

/// Create the appropriate native log capture backend for the given platform.
///
/// Returns `None` for platforms that don't need native log capture
/// (Linux, Windows, Web — already covered by stdout/stderr pipe).
pub fn create_native_log_capture(
    platform: &str,
    android_config: Option<AndroidLogConfig>,
    #[cfg(target_os = "macos")] macos_config: Option<MacOsLogConfig>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(event.message, "test message");
        assert_eq!(event.timestamp, Some("03-10 14:30:00.123".to_string()));
    }

    #[test]
    fn test_native_log_event_no_timestamp() {
        let event = NativeLogEvent {
            tag: "OkHttp".to_string(),
            level: LogLevel::Debug,
            message: "network call".to_string(),
            timestamp: None,
        };
        assert!(event.timestamp.is_none());
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

    #[test]
    fn test_dispatch_windows_returns_none() {
        let result = create_native_log_capture(
            "windows",
            None,
            #[cfg(target_os = "macos")]
            None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_dispatch_web_returns_none() {
        let result = create_native_log_capture(
            "web",
            None,
            #[cfg(target_os = "macos")]
            None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_dispatch_android_with_config_returns_some() {
        let config = AndroidLogConfig {
            device_serial: "emulator-5554".to_string(),
            pid: Some(1234),
            exclude_tags: vec!["flutter".to_string()],
            include_tags: vec![],
            min_level: "info".to_string(),
        };
        let result = create_native_log_capture(
            "android",
            Some(config),
            #[cfg(target_os = "macos")]
            None,
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_dispatch_android_without_config_returns_none() {
        let result = create_native_log_capture(
            "android",
            None,
            #[cfg(target_os = "macos")]
            None,
        );
        assert!(result.is_none());
    }
}
