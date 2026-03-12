//! Native platform log capture spawning.
//!
//! Spawns platform-specific log capture processes (`adb logcat` for Android,
//! `log stream` for macOS) and forwards their output as [`Message::NativeLog`]
//! events into the TEA message loop.
//!
//! The public-to-module entry point is [`spawn_native_log_capture`], called
//! from `actions/mod.rs` when a `StartNativeLogCapture` action is dispatched.

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::config::NativeLogsSettings;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::native_logs::{create_native_log_capture, AndroidLogConfig};
#[cfg(target_os = "macos")]
use fdemon_daemon::native_logs::{IosLogConfig, MacOsLogConfig};

/// Spawn native log capture for a session.
///
/// For Android: resolves the app PID via `adb shell pidof -s <package>`,
/// then spawns `adb logcat --pid=<pid>`.
/// For macOS: spawns `log stream --predicate 'process == "<app_name>"'`.
/// For Linux / Windows / Web: returns immediately (no native capture needed;
/// these platforms already surface native logs via stdout/stderr pipe).
///
/// When `settings.enabled` is `false` the function returns immediately
/// without spawning anything.
///
/// The spawned task sends:
/// 1. [`Message::NativeLogCaptureStarted`] with shutdown + task handles.
/// 2. One [`Message::NativeLog`] per captured line.
/// 3. [`Message::NativeLogCaptureStopped`] when the capture process exits.
pub(super) fn spawn_native_log_capture(
    session_id: SessionId,
    platform: String,
    device_id: String,
    device_name: String,
    app_id: Option<String>,
    settings: &NativeLogsSettings,
    msg_tx: mpsc::Sender<Message>,
) {
    if !settings.enabled {
        tracing::debug!(
            "Native log capture disabled by config — skipping for session {}",
            session_id
        );
        return;
    }

    // Only Android, macOS, and iOS need a separate capture process.
    // Linux / Windows / Web already receive native logs via flutter's stdout pipe.
    // iOS capture requires a macOS host (xcrun simctl / idevicesyslog).
    if platform != "android" {
        #[cfg(not(target_os = "macos"))]
        {
            tracing::debug!(
                "Native log capture not supported on platform '{}' — skipping for session {}",
                platform,
                session_id
            );
            return;
        }
        #[cfg(target_os = "macos")]
        if platform != "macos" && platform != "ios" {
            tracing::debug!(
                "Native log capture not supported on platform '{}' — skipping for session {}",
                platform,
                session_id
            );
            return;
        }
    }

    let exclude_tags = settings.exclude_tags.clone();
    let include_tags = settings.include_tags.clone();
    let min_level = settings.min_level.clone();

    tokio::spawn(async move {
        // ── Build platform config ──────────────────────────────────────────

        let android_config = if platform == "android" {
            // Attempt to resolve the PID; unfiltered capture on failure.
            let pid = resolve_android_pid(&device_id, &app_id).await;
            if pid.is_none() {
                tracing::info!(
                    "Could not resolve Android app PID for session {} — logcat will run unfiltered",
                    session_id
                );
            }
            Some(AndroidLogConfig {
                device_serial: device_id.clone(),
                pid,
                exclude_tags: exclude_tags.clone(),
                include_tags: include_tags.clone(),
                min_level: min_level.clone(),
            })
        } else {
            None
        };

        #[cfg(target_os = "macos")]
        let macos_config = if platform == "macos" {
            let process_name = derive_macos_process_name(&app_id);
            Some(MacOsLogConfig {
                process_name,
                exclude_tags: exclude_tags.clone(),
                include_tags: include_tags.clone(),
                min_level: min_level.clone(),
            })
        } else {
            None
        };

        #[cfg(target_os = "macos")]
        let ios_config = if platform == "ios" {
            let process_name = derive_ios_process_name(&app_id);
            let is_simulator = is_ios_simulator(&device_name, &device_id);

            tracing::info!(
                "Starting iOS native log capture for session {} ({}, process={})",
                session_id,
                if is_simulator {
                    "simulator"
                } else {
                    "physical"
                },
                process_name,
            );

            Some(IosLogConfig {
                device_udid: device_id.clone(),
                is_simulator,
                process_name,
                exclude_tags: exclude_tags.clone(),
                include_tags: include_tags.clone(),
                min_level: min_level.clone(),
            })
        } else {
            None
        };

        // ── Create the platform capture backend ───────────────────────────

        let capture = create_native_log_capture(
            &platform,
            android_config,
            #[cfg(target_os = "macos")]
            macos_config,
            #[cfg(target_os = "macos")]
            ios_config,
        );

        let capture = match capture {
            Some(c) => c,
            None => {
                tracing::debug!(
                    "No native log capture backend for platform '{}' (session {})",
                    platform,
                    session_id
                );
                return;
            }
        };

        // ── Spawn the capture process ─────────────────────────────────────

        let native_handle = match capture.spawn() {
            Some(h) => h,
            None => {
                tracing::warn!(
                    "Failed to spawn native log capture for platform '{}' (session {})",
                    platform,
                    session_id
                );
                return;
            }
        };

        // ── Transfer ownership of shutdown handles to the TEA state ────────
        // Wrap the shutdown_tx in Arc so Message::NativeLogCaptureStarted can
        // derive Clone.
        let shutdown_tx = Arc::new(native_handle.shutdown_tx);
        // Wrap the task_handle in Arc<Mutex<Option>> to satisfy Clone on Message.
        // The TEA handler takes it out of the Option when storing on SessionHandle.
        let task_handle_slot: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> =
            Arc::new(Mutex::new(Some(native_handle.task_handle)));
        let mut event_rx = native_handle.event_rx;

        if msg_tx
            .send(Message::NativeLogCaptureStarted {
                session_id,
                shutdown_tx,
                task_handle: task_handle_slot,
            })
            .await
            .is_err()
        {
            // Engine channel closed — engine is shutting down.
            return;
        }

        // ── Forward events to the TEA message loop ────────────────────────
        while let Some(event) = event_rx.recv().await {
            if msg_tx
                .send(Message::NativeLog { session_id, event })
                .await
                .is_err()
            {
                // Engine channel closed.
                break;
            }
        }

        // Notify TEA that the capture process has ended.
        let _ = msg_tx
            .send(Message::NativeLogCaptureStopped { session_id })
            .await;
    });
}

/// Resolve the Android app's process ID via `adb shell pidof -s <package>`.
///
/// Returns `None` if `app_id` is not set, if `adb` is unavailable, or if
/// the process has not started yet (PID not found).
async fn resolve_android_pid(device_serial: &str, app_id: &Option<String>) -> Option<u32> {
    let app_id = app_id.as_ref()?;
    // The app_id from Flutter's app.start event is the package name
    // (e.g., "com.example.app").
    let output = tokio::process::Command::new("adb")
        .args(["-s", device_serial, "shell", "pidof", "-s", app_id])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let pid_str = String::from_utf8_lossy(&output.stdout);
    pid_str.trim().parse::<u32>().ok()
}

/// Derive the macOS process name from the Flutter app ID.
///
/// For macOS Flutter apps the process name is typically the last component of
/// the bundle identifier (e.g., `"com.example.myApp"` → `"myApp"`).
/// Falls back to `"Runner"` (Flutter's default macOS app name) when no
/// `app_id` is available.
fn derive_macos_process_name(app_id: &Option<String>) -> String {
    if let Some(id) = app_id {
        if let Some(name) = id.rsplit('.').next() {
            if !name.is_empty() {
                return name.to_string();
            }
        }
        return id.clone();
    }
    // Flutter's default macOS app name when the project hasn't been renamed.
    "Runner".to_string()
}

/// Derive the iOS process name for native log filtering.
///
/// iOS Flutter apps always use `"Runner"` as the Xcode target/process name.
/// Unlike macOS, the process name does not correspond to the bundle ID.
/// The `_app_id` parameter is kept for API consistency with
/// [`derive_macos_process_name`] and `derive_android_process_name`.
fn derive_ios_process_name(_app_id: &Option<String>) -> String {
    "Runner".to_string()
}

/// Detect whether an iOS device is a simulator based on its metadata.
///
/// Uses two heuristics in order:
/// 1. **Device name**: Flutter's device discovery names simulators with the
///    suffix `" Simulator"` (e.g., `"iPhone 15 Simulator"`). Physical device
///    names are user-set (e.g., `"Ed's iPhone"`).
/// 2. **UDID format**: Simulator UDIDs use standard UUID format with hyphens
///    (`XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX`, 36 chars, 4 hyphens). Physical
///    device UDIDs are 40-char hex strings without hyphens (or 24-char for
///    newer Apple Silicon devices).
///
/// Falls back to `false` (physical device) if detection is ambiguous.
fn is_ios_simulator(device_name: &str, device_id: &str) -> bool {
    // Heuristic 1: device name contains "simulator" (case-insensitive)
    if device_name.to_lowercase().contains("simulator") {
        return true;
    }
    // Heuristic 2: UDID matches standard UUID format (8-4-4-4-12, 36 chars, 4 hyphens)
    if device_id.len() == 36 && device_id.chars().filter(|c| *c == '-').count() == 4 {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_macos_process_name_from_bundle_id() {
        assert_eq!(
            derive_macos_process_name(&Some("com.example.myApp".to_string())),
            "myApp"
        );
    }

    #[test]
    fn test_derive_macos_process_name_single_component() {
        assert_eq!(
            derive_macos_process_name(&Some("Runner".to_string())),
            "Runner"
        );
    }

    #[test]
    fn test_derive_macos_process_name_fallback_when_none() {
        assert_eq!(derive_macos_process_name(&None), "Runner");
    }

    #[test]
    fn test_derive_macos_process_name_empty_last_component() {
        // Edge case: trailing dot produces an empty last component — fall back to full id.
        assert_eq!(
            derive_macos_process_name(&Some("com.example.".to_string())),
            "com.example."
        );
    }

    #[test]
    fn test_native_log_event_creates_native_source() {
        use fdemon_core::{LogEntry, LogLevel, LogSource};
        use fdemon_daemon::NativeLogEvent;

        let event = NativeLogEvent {
            tag: "GoLog".to_string(),
            level: LogLevel::Info,
            message: "Hello from Go".to_string(),
            timestamp: None,
        };
        // Inline conversion (same logic as update.rs handler)
        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag },
            event.message,
        );
        assert!(matches!(
            entry.source,
            LogSource::Native { ref tag } if tag == "GoLog"
        ));
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "Hello from Go");
    }

    #[test]
    fn test_native_log_event_warning_level() {
        use fdemon_core::{LogEntry, LogLevel, LogSource};
        use fdemon_daemon::NativeLogEvent;

        let event = NativeLogEvent {
            tag: "OkHttp".to_string(),
            level: LogLevel::Warning,
            message: "connection timeout".to_string(),
            timestamp: Some("03-10 14:30:00.123".to_string()),
        };
        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag },
            event.message,
        );
        assert!(matches!(
            entry.source,
            LogSource::Native { ref tag } if tag == "OkHttp"
        ));
        assert_eq!(entry.level, LogLevel::Warning);
    }

    // ── is_ios_simulator tests ─────────────────────────────────────────────

    #[test]
    fn test_is_ios_simulator_by_name() {
        assert!(is_ios_simulator("iPhone 15 Simulator", "some-id"));
        assert!(is_ios_simulator(
            "iPad Air (5th generation) Simulator",
            "some-id"
        ));
        assert!(!is_ios_simulator("Ed's iPhone", "some-id"));
    }

    #[test]
    fn test_is_ios_simulator_by_name_case_insensitive() {
        // "simulator" is checked case-insensitively
        assert!(is_ios_simulator("iPhone 15 SIMULATOR", "some-id"));
    }

    #[test]
    fn test_is_ios_simulator_by_udid_format() {
        // Simulator UDID: standard UUID format (36 chars, 4 hyphens)
        assert!(is_ios_simulator(
            "iPhone 15",
            "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE"
        ));
        // Physical UDID: 40-char hex without hyphens
        assert!(!is_ios_simulator(
            "iPhone 15",
            "00008030000011ABC000DEF1234567890abcdef0"
        ));
    }

    #[test]
    fn test_is_ios_simulator_physical_device_not_simulator() {
        // Real device name without "Simulator" and non-UUID UDID
        assert!(!is_ios_simulator(
            "Ed's iPhone",
            "00008030000011ABC000DEF1234567890abcdef0"
        ));
    }

    // ── derive_ios_process_name tests ──────────────────────────────────────

    #[test]
    fn test_derive_ios_process_name_from_bundle_id() {
        // iOS always returns "Runner" regardless of bundle ID
        assert_eq!(
            derive_ios_process_name(&Some("com.example.myApp".to_string())),
            "Runner"
        );
    }

    #[test]
    fn test_derive_ios_process_name_fallback() {
        // iOS unconditionally returns "Runner" — no app_id required
        assert_eq!(derive_ios_process_name(&None), "Runner");
    }

    #[test]
    fn test_derive_ios_process_name_single_component() {
        assert_eq!(
            derive_ios_process_name(&Some("Runner".to_string())),
            "Runner"
        );
    }

    #[test]
    fn test_derive_ios_process_name_always_runner() {
        // iOS Flutter apps always use "Runner" regardless of bundle ID
        assert_eq!(
            derive_ios_process_name(&Some("com.example.myApp".to_string())),
            "Runner"
        );
        assert_eq!(
            derive_ios_process_name(&Some("org.flutter.app".to_string())),
            "Runner"
        );
        assert_eq!(derive_ios_process_name(&None), "Runner");
    }
}
