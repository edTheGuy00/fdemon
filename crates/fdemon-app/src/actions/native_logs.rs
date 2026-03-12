//! Native platform log capture spawning.
//!
//! Spawns platform-specific log capture processes (`adb logcat` for Android,
//! `log stream` for macOS) and forwards their output as [`Message::NativeLog`]
//! events into the TEA message loop.
//!
//! Also spawns user-defined custom log source processes configured via
//! `[[native_logs.custom_sources]]` in `.fdemon/config.toml`.
//!
//! The public-to-module entry point is [`spawn_native_log_capture`], called
//! from `actions/mod.rs` when a `StartNativeLogCapture` action is dispatched.

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::config::NativeLogsSettings;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::native_logs::{
    create_native_log_capture, custom::create_custom_log_capture,
    custom::CustomSourceConfig as DaemonCustomSourceConfig, AndroidLogConfig,
};
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
    project_path: std::path::PathBuf,
    msg_tx: mpsc::Sender<Message>,
) {
    if !settings.enabled {
        tracing::debug!(
            "Native log capture disabled by config — skipping for session {}",
            session_id
        );
        return;
    }

    // Spawn custom sources regardless of platform (they are always user-defined).
    // Custom sources share the same master toggle as platform capture.
    tracing::info!(
        "[native-logs-debug] spawn_native_log_capture called, {} custom sources configured, project_path={}",
        settings.custom_sources.len(),
        project_path.display()
    );
    spawn_custom_sources(session_id, settings, &project_path, &msg_tx);

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

/// Spawn all custom log source processes configured for this session.
///
/// Iterates over `settings.custom_sources` and, for each valid entry:
/// 1. Builds a daemon-layer `CustomSourceConfig` from the app-layer config.
/// 2. Spawns the custom capture backend.
/// 3. Sends `Message::CustomSourceStarted` so the TEA handler can store
///    the handles on `SessionHandle::custom_source_handles`.
/// 4. Spawns a forwarding task that sends `Message::NativeLog` for each
///    captured event and `Message::CustomSourceStopped` when the process exits.
///
/// Invalid configurations (empty name or command) are skipped with a warning.
/// This function is synchronous; each capture is spawned as a Tokio task internally.
fn spawn_custom_sources(
    session_id: SessionId,
    settings: &NativeLogsSettings,
    project_path: &std::path::Path,
    msg_tx: &mpsc::Sender<Message>,
) {
    for source_config in &settings.custom_sources {
        // Validate config — skip silently on empty name/command.
        if source_config.name.trim().is_empty() || source_config.command.trim().is_empty() {
            tracing::warn!(
                "Skipping custom log source with empty name or command for session {}",
                session_id
            );
            continue;
        }

        // Build the daemon-layer config from the app-layer config.
        // Default working_dir to the Flutter project directory so relative
        // paths in command/args resolve correctly.
        let working_dir = source_config
            .working_dir
            .clone()
            .or_else(|| project_path.to_str().map(|s| s.to_string()));

        let daemon_config = DaemonCustomSourceConfig {
            name: source_config.name.clone(),
            command: source_config.command.clone(),
            args: source_config.args.clone(),
            format: source_config.format,
            working_dir,
            env: source_config.env.clone(),
            exclude_tags: settings.exclude_tags.clone(),
            include_tags: settings.include_tags.clone(),
        };

        let capture = create_custom_log_capture(daemon_config);

        let native_handle = match capture.spawn() {
            Some(h) => h,
            None => {
                // spawn() on CustomLogCapture always returns Some — the real
                // failure surfaces asynchronously when the background task
                // cannot exec the command. This branch is a safety net.
                tracing::warn!(
                    "Failed to get handle for custom log source '{}' (session {})",
                    source_config.name,
                    session_id
                );
                continue;
            }
        };

        let shutdown_tx = Arc::new(native_handle.shutdown_tx);
        let task_handle_slot: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> =
            Arc::new(Mutex::new(Some(native_handle.task_handle)));
        let source_name = source_config.name.clone();
        let msg_tx_clone = msg_tx.clone();

        // Notify TEA that this custom source started (stores handles on SessionHandle).
        let startup_msg = Message::CustomSourceStarted {
            session_id,
            name: source_name.clone(),
            shutdown_tx: shutdown_tx.clone(),
            task_handle: task_handle_slot,
        };

        // Spawn a task to send the startup message and then forward events.
        let mut event_rx = native_handle.event_rx;
        tokio::spawn(async move {
            // Send the lifecycle message first.
            if msg_tx_clone.send(startup_msg).await.is_err() {
                // Engine channel closed — nothing to do.
                return;
            }

            tracing::debug!(
                "Custom log source '{}' started for session {}",
                source_name,
                session_id
            );

            // Forward events through Message::NativeLog (same path as platform capture).
            while let Some(event) = event_rx.recv().await {
                if msg_tx_clone
                    .send(Message::NativeLog { session_id, event })
                    .await
                    .is_err()
                {
                    // Engine channel closed.
                    break;
                }
            }

            // Notify TEA that the custom source has stopped.
            let _ = msg_tx_clone
                .send(Message::CustomSourceStopped {
                    session_id,
                    name: source_name.clone(),
                })
                .await;

            tracing::debug!(
                "Custom log source '{}' stopped for session {}",
                source_name,
                session_id
            );
        });
    }
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
