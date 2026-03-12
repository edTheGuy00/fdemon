## Task: App Layer Integration

**Objective**: Wire native log capture into the TEA architecture — add `Message::NativeLog` variant, `UpdateAction::StartNativeLogCapture`, store shutdown handles on `SessionHandle`, spawn native log capture after `AppStarted`, route events into the session log buffer, and clean up on session end.

**Depends on**: 01-core-types, 02-native-log-config, 03-tool-availability, 04-shared-native-infra, 05-android-logcat, 06-macos-log-stream

### Scope

- `crates/fdemon-app/src/message.rs`: Add `NativeLog` message variant
- `crates/fdemon-app/src/session/handle.rs`: Add native log shutdown/task fields
- `crates/fdemon-app/src/handler/mod.rs`: Add `UpdateAction::StartNativeLogCapture`
- `crates/fdemon-app/src/handler/session.rs`: Trigger native log capture after `AppStarted`
- `crates/fdemon-app/src/handler/session_lifecycle.rs`: Cleanup on session close
- `crates/fdemon-app/src/handler/daemon.rs`: Route `NativeLog` messages
- `crates/fdemon-app/src/actions/native_logs.rs`: **NEW** — spawn native log capture
- `crates/fdemon-app/src/actions/mod.rs`: Dispatch `StartNativeLogCapture` action

### Details

#### 1. Add `Message::NativeLog` variant

In `crates/fdemon-app/src/message.rs` (currently ~150 variants), add:

```rust
use fdemon_daemon::NativeLogEvent;

pub enum Message {
    // ... existing variants ...

    /// A native platform log line was captured (from adb logcat, log stream, etc.)
    NativeLog {
        session_id: SessionId,
        event: NativeLogEvent,
    },

    /// Native log capture process started successfully for a session.
    NativeLogCaptureStarted {
        session_id: SessionId,
        shutdown_tx: watch::Sender<bool>,
        task_handle: JoinHandle<()>,
    },

    /// Native log capture process ended (exited or failed to start).
    NativeLogCaptureStopped {
        session_id: SessionId,
    },
}
```

#### 2. Add fields to `SessionHandle`

In `crates/fdemon-app/src/session/handle.rs` (line 11–80), add:

```rust
pub struct SessionHandle {
    // ... existing fields ...

    /// Shutdown signal for the native log capture task.
    pub native_log_shutdown_tx: Option<Arc<watch::Sender<bool>>>,
    /// Handle to the native log event forwarding task.
    pub native_log_task_handle: Option<JoinHandle<()>>,
}
```

Initialize both to `None` in `SessionHandle::new()` (line 102).

Add cleanup to existing shutdown sequences. In `handle.rs` or wherever the shutdown helper lives:

```rust
pub fn shutdown_native_logs(&mut self) {
    if let Some(tx) = self.native_log_shutdown_tx.take() {
        let _ = tx.send(true);
    }
    if let Some(handle) = self.native_log_task_handle.take() {
        handle.abort();
    }
}
```

#### 3. Add `UpdateAction::StartNativeLogCapture`

In `crates/fdemon-app/src/handler/mod.rs`, add to the `UpdateAction` enum:

```rust
pub enum UpdateAction {
    // ... existing variants ...

    /// Start native platform log capture for a session (after AppStarted).
    StartNativeLogCapture {
        session_id: SessionId,
        platform: String,
        device_id: String,
        app_id: Option<String>,
    },
}
```

#### 4. Trigger after `AppStarted`

In `crates/fdemon-app/src/handler/session.rs`, the `AppStart` daemon message handler (lines 164–179) calls `handle.session.mark_started(app_id)`. After this, return an `UpdateAction::StartNativeLogCapture`:

```rust
// After handle.session.mark_started(app_start.app_id.clone()):
let platform = handle.session.platform.clone();
let device_id = handle.session.device_id.clone();
let app_id = handle.session.app_id.clone();

// Return the action to start native log capture
UpdateResult::with_action(UpdateAction::StartNativeLogCapture {
    session_id,
    platform,
    device_id,
    app_id,
})
```

**Note**: If the current handler already returns an `UpdateResult` with an action, you may need to return a follow-up `Message` instead that triggers the action on the next tick. Check the existing pattern — `AppStart` → `mark_started()` may not currently return an action, making direct action return possible.

#### 5. Create `actions/native_logs.rs`

This is the core spawning logic. Follow the pattern from `actions/performance.rs`:

```rust
//! Native platform log capture spawning.
//!
//! Spawns platform-specific log capture processes (adb logcat, log stream)
//! and forwards their output as Message::NativeLog events.

use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::native_logs::{
    AndroidLogConfig, NativeLogCapture, NativeLogEvent,
    create_native_log_capture,
};
#[cfg(target_os = "macos")]
use fdemon_daemon::native_logs::MacOsLogConfig;
use tokio::sync::{mpsc, watch};

/// Spawn native log capture for a session.
///
/// For Android: resolves PID via `adb shell pidof`, then spawns `adb logcat --pid=<pid>`.
/// For macOS: spawns `log stream --predicate 'process == "<app_name>"'`.
/// For Linux/Windows/Web: no-op (native logs already captured via stdout/stderr).
pub(super) fn spawn_native_log_capture(
    session_id: SessionId,
    platform: String,
    device_id: String,
    app_id: Option<String>,
    settings: &crate::config::NativeLogsSettings,
    msg_tx: mpsc::Sender<Message>,
) {
    if !settings.enabled {
        tracing::debug!("Native log capture disabled by config");
        return;
    }

    let exclude_tags = settings.exclude_tags.clone();
    let include_tags = settings.include_tags.clone();
    let min_level = settings.min_level.clone();

    tokio::spawn(async move {
        let android_config = if platform == "android" {
            // Resolve PID via adb shell pidof
            let pid = resolve_android_pid(&device_id, &app_id).await;
            if pid.is_none() {
                tracing::info!(
                    "Could not resolve Android app PID; logcat will run unfiltered"
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
            // Derive process name from app_id or device info
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

        let capture = create_native_log_capture(
            &platform,
            android_config,
            #[cfg(target_os = "macos")]
            macos_config,
        );

        let capture = match capture {
            Some(c) => c,
            None => {
                tracing::debug!(
                    "No native log capture needed for platform '{}'",
                    platform
                );
                return;
            }
        };

        let handle = match capture.spawn() {
            Some(h) => h,
            None => {
                tracing::warn!("Failed to spawn native log capture for {}", platform);
                return;
            }
        };

        // Send the shutdown handles to the TEA state
        let _ = msg_tx.send(Message::NativeLogCaptureStarted {
            session_id,
            shutdown_tx: handle.shutdown_tx,
            task_handle: handle.task_handle,
        }).await;

        // Forward events from the capture process to the TEA message loop
        let mut event_rx = handle.event_rx;
        while let Some(event) = event_rx.recv().await {
            if msg_tx.send(Message::NativeLog { session_id, event }).await.is_err() {
                break;
            }
        }

        let _ = msg_tx.send(Message::NativeLogCaptureStopped { session_id }).await;
    });
}

/// Resolve the Android app's PID via `adb shell pidof -s <package>`.
async fn resolve_android_pid(device_serial: &str, app_id: &Option<String>) -> Option<u32> {
    let app_id = app_id.as_ref()?;
    // The app_id from Flutter's app.start is the package name (e.g., "com.example.app")
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

/// Derive the macOS process name from the app ID or project info.
fn derive_macos_process_name(app_id: &Option<String>) -> String {
    // For macOS Flutter apps, the process name is typically the app name
    // derived from the Xcode project. The app_id from Flutter may be
    // a bundle identifier (e.g., "com.example.myApp") — extract the last component.
    if let Some(id) = app_id {
        if let Some(name) = id.rsplit('.').next() {
            return name.to_string();
        }
        return id.clone();
    }
    "Runner".to_string() // Flutter's default macOS app name
}
```

#### 6. Wire into action dispatcher

In `crates/fdemon-app/src/actions/mod.rs`, add the module declaration and match arm:

```rust
pub(super) mod native_logs;

// In handle_action():
UpdateAction::StartNativeLogCapture { session_id, platform, device_id, app_id } => {
    native_logs::spawn_native_log_capture(
        session_id,
        platform,
        device_id,
        app_id,
        &state.settings.native_logs,
        msg_tx.clone(),
    );
}
```

#### 7. Handle `NativeLog` messages in update

In `crates/fdemon-app/src/handler/update.rs` or `daemon.rs`, add handlers:

```rust
// Message::NativeLog — convert event to LogEntry and add to session
Message::NativeLog { session_id, event } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag },
            event.message,
        );
        handle.session.queue_log(entry);
    }
    UpdateResult::default()
}

// Message::NativeLogCaptureStarted — store handles on SessionHandle
Message::NativeLogCaptureStarted { session_id, shutdown_tx, task_handle } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.native_log_shutdown_tx = Some(Arc::new(shutdown_tx));
        handle.native_log_task_handle = Some(task_handle);
    }
    UpdateResult::default()
}

// Message::NativeLogCaptureStopped — clear handles
Message::NativeLogCaptureStopped { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.native_log_shutdown_tx = None;
        handle.native_log_task_handle = None;
    }
    UpdateResult::default()
}
```

#### 8. Cleanup on session stop / close

Add native log shutdown to the existing shutdown sequences:

**In `handler/session.rs`** — `handle_session_exited()` and `handle_session_app_stopped()`:
```rust
// Add alongside existing vm_shutdown_tx, perf, network shutdown:
handle.shutdown_native_logs();
```

**In `handler/session_lifecycle.rs`** — `handle_close_current_session()`:
```rust
// Add alongside existing shutdown calls:
handle.shutdown_native_logs();
```

### Acceptance Criteria

1. `Message::NativeLog` variant exists and carries `session_id` + `NativeLogEvent`
2. `SessionHandle` has `native_log_shutdown_tx` and `native_log_task_handle` fields
3. `UpdateAction::StartNativeLogCapture` exists and is dispatched from `handle_action()`
4. Native log capture is triggered after `AppStart` / `mark_started()` for Android/macOS sessions
5. Native log capture is NOT triggered for Linux/Windows/Web sessions
6. `NativeLogEvent` is converted to `LogEntry { source: Native { tag }, level, message }` and queued
7. `flutter` tag is excluded by default (configurable via settings)
8. PID resolution works for Android (`adb shell pidof -s <package>`)
9. Process name derivation works for macOS (extracts from app_id)
10. Native log capture shuts down when session stops (`AppStop`, `Exited`, `CloseCurrentSession`)
11. `settings.native_logs.enabled = false` prevents native log capture from starting
12. All existing tests pass — no regressions
13. `cargo check --workspace` compiles

### Testing

```rust
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
    fn test_derive_macos_process_name_fallback() {
        assert_eq!(derive_macos_process_name(&None), "Runner");
    }

    #[test]
    fn test_native_log_event_to_log_entry() {
        let event = NativeLogEvent {
            tag: "GoLog".to_string(),
            level: LogLevel::Info,
            message: "Hello from Go".to_string(),
            timestamp: None,
        };
        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag },
            event.message,
        );
        assert!(matches!(entry.source, LogSource::Native { ref tag } if tag == "GoLog"));
        assert_eq!(entry.level, LogLevel::Info);
    }

    // Integration-style tests for the message handling:

    #[test]
    fn test_native_log_message_creates_log_entry_with_native_source() {
        // Setup minimal AppState with a session
        let mut state = test_state_with_android_session();
        let session_id = state.session_manager.active_session_id().unwrap();

        let event = NativeLogEvent {
            tag: "GoLog".to_string(),
            level: LogLevel::Warning,
            message: "test warning".to_string(),
            timestamp: None,
        };

        // Simulate handling Message::NativeLog
        let msg = Message::NativeLog { session_id, event };
        let result = update(&mut state, msg);

        // Verify a log entry was added with Native source
        let handle = state.session_manager.get(session_id).unwrap();
        let last_log = handle.session.logs.back().unwrap();
        assert!(matches!(last_log.source, LogSource::Native { ref tag } if tag == "GoLog"));
        assert_eq!(last_log.level, LogLevel::Warning);
    }

    #[test]
    fn test_disabled_native_logs_does_not_spawn() {
        // Verify that spawn_native_log_capture with enabled=false returns immediately
        // (This is a unit test of the early return, not an integration test)
        let settings = NativeLogsSettings {
            enabled: false,
            ..Default::default()
        };
        // spawn_native_log_capture should return without spawning a task
        // Testing this precisely requires mocking tokio::spawn or checking side effects
    }
}
```

### Notes

- **PID changes on hot restart**: When the Flutter app is restarted (not just reloaded), the PID changes. The current implementation starts native log capture once after `AppStart` and doesn't re-resolve PID. For Phase 1, this is acceptable — the logcat process will continue running (it monitors the stale PID and gets EOF) and a new `AppStart` event will trigger a new capture with the new PID. Consider tracking this as a known limitation.
- **Multiple `UpdateAction`s**: If `handle_session_message_state()` already returns an `UpdateAction` for `AppStart`, you may need to chain the native log start as a follow-up `Message` rather than a direct action. Check if the existing code returns `UpdateResult::default()` or something with an action.
- **The `NativeLogCaptureStarted` message pattern** (sending handles back via Message) follows the same approach used by `VmServiceConnected` and `VmServiceHandleReady` — the spawned task sends its own handles back to the TEA state via the message channel.
- **`queue_log` vs `add_log`**: Use `queue_log` (batched) for native log events to leverage the existing `LogBatcher` (16ms / 100-entry batching). This prevents high-volume native logs from overwhelming the render loop.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `NativeLog`, `NativeLogCaptureStarted`, `NativeLogCaptureStopped` variants; imported `NativeLogEvent` from `fdemon_daemon` |
| `crates/fdemon-app/src/session/handle.rs` | Added `native_log_shutdown_tx`, `native_log_task_handle` fields; added `shutdown_native_logs()` method; updated `new()` and `Debug` impl |
| `crates/fdemon-app/src/handler/mod.rs` | Added `UpdateAction::StartNativeLogCapture` variant with `settings: NativeLogsSettings` payload |
| `crates/fdemon-app/src/handler/session.rs` | Added `maybe_start_native_log_capture()` function; added `shutdown_native_logs()` call on process exit and app stop |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | Added `shutdown_native_logs()` call in `handle_close_current_session` |
| `crates/fdemon-app/src/handler/daemon.rs` | Wired `maybe_start_native_log_capture` into `Stdout` and `Message` event paths after state mutation |
| `crates/fdemon-app/src/handler/update.rs` | Added handlers for `NativeLog`, `NativeLogCaptureStarted`, `NativeLogCaptureStopped` messages |
| `crates/fdemon-app/src/actions/native_logs.rs` | NEW — `spawn_native_log_capture()`, `resolve_android_pid()`, `derive_macos_process_name()`, tests |
| `crates/fdemon-app/src/actions/mod.rs` | Registered `native_logs` module; added `StartNativeLogCapture` dispatch arm |

### Notable Decisions/Tradeoffs

1. **Settings in action payload**: Rather than threading `AppState` through `handle_action`, the `NativeLogsSettings` snapshot is embedded in `UpdateAction::StartNativeLogCapture`. This keeps the action dispatcher stateless and matches the existing pattern for `performance_refresh_ms` etc.

2. **`SharedTaskHandle` pattern for `NativeLogCaptureStarted`**: `JoinHandle<()>` doesn't implement `Clone`, so `task_handle` is wrapped in `Arc<Mutex<Option<>>>` (same as `VmServicePerformanceMonitoringStarted` / `SharedTaskHandle` type alias). The handler takes the handle out of the `Option`.

3. **AppStart action trigger via `maybe_start_native_log_capture`**: Follows the same pattern as `maybe_connect_vm_service` — called after `handle_session_stdout`/`handle_session_message_state` mutates state, so `session.app_id` is already set when we build the action.

4. **Platform guard in two places**: `maybe_start_native_log_capture` guards on Android/macOS, AND `spawn_native_log_capture` repeats the guard as defense-in-depth. This prevents accidental captures even if the action is dispatched for unexpected platforms.

5. **cfg-gated macOS check in `maybe_start_native_log_capture`**: Uses `cfg!(target_os = "macos") && platform == "macos"` so that on Linux/Windows, the check compiles to `false && ...` and no dead code is generated.

6. **Shutdown in three places**: Process exited, app stopped (AppStop event), and session closed all call `shutdown_native_logs()`, matching the existing pattern for VM/perf/network shutdown handles.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (0 errors, 0 warnings)
- `cargo test -p fdemon-app --lib` - Passed (1464 tests, 0 failed)
- `cargo test -p fdemon-app native_logs` - Passed (11 tests, 0 failed)
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)
- Pre-existing snapshot failures in `fdemon-tui` (version string mismatch `v0.1.0` vs `v0.2.1`) confirmed unrelated to this task

### Risks/Limitations

1. **PID changes on hot restart**: Native log capture starts once after `AppStart` with the initial PID. On hot restart, the PID changes and the old logcat process gets EOF. A new `AppStart` event will trigger a new capture. This is documented in the task notes as an accepted Phase 1 limitation.

2. **Double parse for AppStart in Stdout path**: The `DaemonEvent::Stdout` arm calls `parse_daemon_message` twice (once to check for AppDebugPort, once to check for AppStart). This is a minor inefficiency but consistent with the existing pattern and not on a hot path.
