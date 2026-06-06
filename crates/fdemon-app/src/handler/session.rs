//! Session lifecycle handlers for multi-session mode
//!
//! Uses log batching to coalesce rapid log arrivals during high-volume
//! output (hot reload, verbose debugging, etc.).

use crate::handler::UpdateAction;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::{AppPhase, DaemonMessage, LogEntry, LogLevel, LogSource, ParsedStackTrace};
use fdemon_daemon::{parse_daemon_message, to_log_entry};

/// Handle stdout events for a specific session
///
/// Parses daemon JSON messages and queues log entries for batched processing.
pub fn handle_session_stdout(state: &mut AppState, session_id: SessionId, line: &str) {
    // Try to parse as JSON daemon message
    if let Some(msg) = parse_daemon_message(line) {
        // Handle responses separately (they don't create log entries)
        if matches!(msg, DaemonMessage::Response { .. }) {
            tracing::debug!("Session {} response: {}", session_id, msg.summary());
            return;
        }

        // Log exception-related events for diagnostics
        if let DaemonMessage::AppLog(ref log) = msg {
            if log.log.contains("EXCEPTION") || log.log.contains("══") {
                tracing::info!(
                    "Session {} EXCEPTION LINE: log={:?} error={} has_stack={}",
                    session_id,
                    &log.log[..log.log.len().min(100)],
                    log.error,
                    log.stack_trace.is_some(),
                );
            }
        }

        // Convert to log entry if applicable
        if let Some(entry_info) = to_log_entry(&msg) {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                if let Some(ref stack_trace) = entry_info.stack_trace {
                    // Has dedicated stack trace — use existing path
                    let parsed_trace = ParsedStackTrace::parse(stack_trace);
                    let log_entry = LogEntry::with_stack_trace(
                        entry_info.level,
                        entry_info.source,
                        entry_info.message,
                        parsed_trace,
                    );
                    if handle.session.queue_log(log_entry) {
                        handle.session.flush_batched_logs();
                    }
                } else {
                    // No stack trace — route through exception parser for
                    // multi-line exception block detection (app.log events)
                    let entries = handle.session.process_log_line_with_fallback(
                        &entry_info.message,
                        entry_info.level,
                        entry_info.source,
                        entry_info.message.clone(),
                    );
                    for entry in entries {
                        if handle.session.queue_log(entry) {
                            handle.session.flush_batched_logs();
                        }
                    }
                }
            }
        } else {
            // Unknown event type, log at debug level
            tracing::debug!(
                "Session {} unhandled daemon message: {}",
                session_id,
                msg.summary()
            );
        }

        // Update session state based on message type
        handle_session_message_state(state, session_id, &msg);
    } else if !line.trim().is_empty() {
        // Non-JSON output (build progress, device logcat, etc.)
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            // Process through exception detection and raw line handling
            let entries = handle.session.process_raw_line(line);
            for entry in entries {
                // Use batched logging for performance
                if handle.session.queue_log(entry) {
                    handle.session.flush_batched_logs();
                }
            }
            // Surface the on-device Dart VM service failure (Android only) with
            // actionable guidance. Without the VM service, Flutter never emits
            // `app.started`, so the session would otherwise stay in Launching
            // with no explanation. No-op unless the line is the failure marker.
            handle.session.detect_vm_service_failure(line);
        }
    }
}

/// Handle session exit events
pub fn handle_session_exited(state: &mut AppState, session_id: SessionId, code: Option<i32>) {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Guard: ignore duplicate exit events — the session is already stopped.
        if handle.session.phase == AppPhase::Stopped {
            tracing::debug!(
                "Session {} already stopped, ignoring duplicate exit event",
                session_id
            );
            return;
        }

        let (level, message) = match code {
            Some(0) => (
                LogLevel::Info,
                "Flutter process exited normally".to_string(),
            ),
            Some(c) => (
                LogLevel::Warning,
                format!("Flutter process exited with code {}", c),
            ),
            None => (LogLevel::Warning, "Flutter process exited".to_string()),
        };

        handle
            .session
            .add_log(LogEntry::new(level, LogSource::App, message));
        handle.session.phase = AppPhase::Stopped;
        handle.session.vm_connected = false;

        // Signal VM Service forwarding task to stop (if running)
        if let Some(shutdown_tx) = handle.vm_shutdown_tx.take() {
            let _ = shutdown_tx.send(true);
            tracing::info!(
                "Sent VM Service shutdown signal on process exit for session {}",
                session_id
            );
        }

        // Abort and signal the performance polling task to stop.
        if let Some(h) = handle.perf_task_handle.take() {
            h.abort();
        }
        if let Some(tx) = handle.perf_shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::info!(
                "Sent perf shutdown signal on process exit for session {}",
                session_id
            );
        }
        handle.session.performance.monitoring_active = false;

        // Abort and signal the network monitoring polling task to stop.
        if let Some(h) = handle.network_task_handle.take() {
            h.abort();
        }
        if let Some(tx) = handle.network_shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::info!(
                "Sent network shutdown signal on process exit for session {}",
                session_id
            );
        }
        // Abort and signal the timeline monitoring polling task to stop.
        if let Some(h) = handle.timeline_task_handle.take() {
            h.abort();
        }
        if let Some(tx) = handle.timeline_shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::info!(
                "Sent timeline shutdown signal on process exit for session {}",
                session_id
            );
        }

        // Shut down the native log capture task (if running).
        handle.shutdown_native_logs();

        // Reset native tag state — tags from the previous run should not
        // persist across a session stop/restart.
        handle.native_tag_state = crate::session::NativeTagState::default();

        // Clear DevTools endpoint — on next run the Flutter daemon may serve
        // DevTools on a different port (or not at all), so the stored URL
        // would point at a stale or non-listening server. Pressing `B` after
        // exit must NOT silently open a dead URL.
        handle.session.devtools_endpoint = None;
        handle.session.devtools_serve_pending = false;

        // Don't auto-quit - let user decide what to do with the session
        // The session tab remains visible showing the exit log
    }
}

/// Update session state based on daemon message type
pub fn handle_session_message_state(
    state: &mut AppState,
    session_id: SessionId,
    msg: &DaemonMessage,
) {
    // Handle app.start event - capture app_id in session
    if let DaemonMessage::AppStart(app_start) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.mark_started(app_start.app_id.clone());
            tracing::info!(
                "Session {} app started: app_id={}",
                session_id,
                app_start.app_id
            );
        }
    }

    // Handle app.stop event
    if let DaemonMessage::AppStop(app_stop) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if handle.session.app_id.as_ref() == Some(&app_stop.app_id) {
                handle.session.app_id = None;
                handle.session.ws_uri = None;
                handle.session.vm_connected = false;
                handle.session.phase = AppPhase::Initializing;
                tracing::info!(
                    "Session {} app stopped: app_id={}",
                    session_id,
                    app_stop.app_id
                );
                // Signal the VM Service forwarding task to disconnect
                if let Some(shutdown_tx) = handle.vm_shutdown_tx.take() {
                    let _ = shutdown_tx.send(true);
                    tracing::info!("Sent VM Service shutdown signal for session {}", session_id);
                }

                // Abort and signal the performance polling task to stop.
                if let Some(h) = handle.perf_task_handle.take() {
                    h.abort();
                }
                if let Some(tx) = handle.perf_shutdown_tx.take() {
                    let _ = tx.send(true);
                    tracing::info!("Sent perf shutdown signal for session {}", session_id);
                }
                handle.session.performance.monitoring_active = false;

                // Abort and signal the network monitoring polling task to stop.
                if let Some(h) = handle.network_task_handle.take() {
                    h.abort();
                }
                if let Some(tx) = handle.network_shutdown_tx.take() {
                    let _ = tx.send(true);
                    tracing::info!("Sent network shutdown signal for session {}", session_id);
                }
                // Abort and signal the timeline monitoring polling task to stop.
                if let Some(h) = handle.timeline_task_handle.take() {
                    h.abort();
                }
                if let Some(tx) = handle.timeline_shutdown_tx.take() {
                    let _ = tx.send(true);
                    tracing::info!("Sent timeline shutdown signal for session {}", session_id);
                }

                // Shut down the native log capture task (if running).
                handle.shutdown_native_logs();

                // Reset native tag state — tags from the previous run should
                // not persist when the app is restarted within the same session.
                handle.native_tag_state = crate::session::NativeTagState::default();

                // Clear DevTools endpoint — hot restart cycles the Flutter
                // app and likely cycles its DevTools server. The stored URL
                // may now point at a dead port; the next `app.devTools`
                // event will repopulate it (or the eager fallback fires on
                // the next VmServiceConnected).
                handle.session.devtools_endpoint = None;
                handle.session.devtools_serve_pending = false;
            }
        }
    }

    // Handle app.started event — the app is actually running now.
    if let DaemonMessage::AppStarted(app_started) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if handle.session.app_id.as_ref() == Some(&app_started.app_id) {
                handle.session.mark_running();
                tracing::info!(
                    "Session {} app is running: app_id={}",
                    session_id,
                    app_started.app_id
                );
            } else {
                // Observability: an app.started whose app_id does not match the
                // session's app_id (captured at app.start) is dropped here, so the
                // session stays in Launching. Log it instead of silently ignoring,
                // so a stuck-in-Launching report is diagnosable from the INFO log.
                tracing::warn!(
                    "Session {} received app.started for app_id={} but session app_id={:?}; \
                     phase NOT advanced to Running (app_id mismatch)",
                    session_id,
                    app_started.app_id,
                    handle.session.app_id,
                );
            }
        }
    }

    // Handle app.progress events — feed build/launch progress text while not running.
    if let DaemonMessage::AppProgress(progress) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if !handle.session.is_running() {
                match (&progress.message, progress.finished) {
                    (Some(m), false) => handle.session.set_progress(m.clone()),
                    (_, true) => handle.session.clear_progress(),
                    _ => {}
                }
            }
        }
    }

    // Handle app.debugPort event — capture VM Service URI
    if let DaemonMessage::AppDebugPort(debug_port) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if handle.session.app_id.as_ref() == Some(&debug_port.app_id) {
                handle.session.ws_uri = Some(debug_port.ws_uri.clone());
                tracing::info!(
                    "Session {} VM Service ready: ws_uri={}",
                    session_id,
                    debug_port.ws_uri
                );
            }
        }
    }
}

/// Check if a session should fire the `devtools.serve` fallback RPC now that
/// the VM Service WebSocket is connected.
///
/// The primary DevTools URL path is the `app.devTools` daemon event (parsed
/// into `DaemonMessage::DevToolsServed` and lifted to `Message::DevToolsServed`
/// by the daemon bridge). This helper provides a belt-and-suspenders fallback:
/// when the VM Service becomes connected and DevTools has not yet been served
/// via the primary path, fire a `devtools.serve` RPC.
///
/// Guards:
/// - Only fires when `devtools_endpoint.is_none()` (not already served by the
///   primary `app.devTools` event path that fires automatically on modern Flutter).
/// - Only fires when `!devtools_serve_pending` (idempotent — prevents double
///   dispatch when `VmServiceConnected` and `app.devTools` race).
///
/// The returned action carries `cmd_sender: None`; `process.rs` hydrates it
/// with the actual `CommandSender` before dispatching to `handle_action`.
///
/// Returns `Some(SendDaemonCommand)` when the command should be sent, else `None`.
pub fn maybe_serve_devtools(state: &mut AppState, session_id: SessionId) -> Option<UpdateAction> {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Guard: the session must have a command sender (process attached).
        // If not, we can't send the devtools.serve command yet; process.rs would
        // discard it anyway, but bailing early avoids setting devtools_serve_pending
        // prematurely (which would prevent a future attempt when the sender arrives).
        handle.cmd_sender.as_ref()?;
        // Idempotence guards
        if handle.session.devtools_endpoint.is_some() {
            return None; // already served via app.devTools primary path
        }
        if handle.session.devtools_serve_pending {
            return None; // already dispatched once; waiting for response
        }
        handle.session.devtools_serve_pending = true;
        tracing::info!(
            "Session {} VM Service connected; firing devtools.serve fallback",
            session_id
        );
        return Some(UpdateAction::SendDaemonCommand {
            session_id,
            command: fdemon_daemon::DaemonCommand::ServeDevTools {
                request_id: Some(format!(
                    "{}{}",
                    crate::process::DEVTOOLS_SERVE_REQUEST_PREFIX,
                    session_id
                )),
            },
            cmd_sender: None,
        });
    }
    None
}

/// Check if an AppDebugPort message should trigger a VM Service connection.
///
/// Returns `Some(ConnectVmService)` when the message is an AppDebugPort for the
/// session's current app_id, otherwise returns `None`.
pub fn maybe_connect_vm_service(
    state: &AppState,
    session_id: SessionId,
    msg: &DaemonMessage,
) -> Option<UpdateAction> {
    if let DaemonMessage::AppDebugPort(debug_port) = msg {
        if let Some(handle) = state.session_manager.get(session_id) {
            if handle.session.app_id.as_ref() == Some(&debug_port.app_id)
                && !handle.session.vm_connected
                && handle.vm_shutdown_tx.is_none()
            {
                return Some(UpdateAction::ConnectVmService {
                    session_id,
                    ws_uri: debug_port.ws_uri.clone(),
                    rebuilt_widgets_gate_rx: None, // hydrated by process.rs
                });
            }
        }
    }
    None
}

/// Check if an `AppStart` event should trigger native platform log capture.
///
/// Returns `Some(StartNativeLogCapture)` when the message is an `AppStart` and
/// the session's platform is `"android"`, `"macos"`, or `"ios"` (native log
/// capture is only needed on these platforms — Linux/Windows/Web already
/// surface native logs via Flutter's stdout pipe).
///
/// iOS capture is only attempted on macOS hosts (gated by `cfg!(target_os = "macos")`).
///
/// Returns `None` for non-`AppStart` messages, unsupported platforms, or when
/// native logs are disabled in settings.
pub fn maybe_start_native_log_capture(
    state: &AppState,
    session_id: SessionId,
    msg: &DaemonMessage,
) -> Option<UpdateAction> {
    if let DaemonMessage::AppStart(app_start) = msg {
        // Guard: only start if native logs are enabled.
        if !state.settings.native_logs.enabled {
            return None;
        }

        if let Some(handle) = state.session_manager.get(session_id) {
            let platform = &handle.session.platform;

            // Only Android, macOS, and iOS need a separate platform capture process.
            // Linux / Windows / Web already receive native logs via flutter's stdout pipe.
            // iOS capture requires a macOS host (xcrun simctl / idevicesyslog).
            // Computed early — needed by guard Branch B.
            let needs_platform_capture = platform == "android"
                || (cfg!(target_os = "macos") && platform == "macos")
                || (cfg!(target_os = "macos") && platform == "ios");

            // Guard: don't start a second capture if everything is already running
            // (prevents double-start on repeated AppStart, e.g. hot-restart).
            //
            // We must allow fall-through when pre-app custom sources are tracked in
            // `custom_source_handles` but post-app custom sources have not yet been
            // spawned. The fine-grained check:
            //
            // 1. Compute the set of post-app sources from config that are not yet
            //    running (i.e., their name is absent from `custom_source_handles`).
            //
            // 2. Guard on platform capture: if `native_log_shutdown_tx.is_some()`
            //    AND all post-app sources are running → nothing left to do.
            //
            // 3. Guard on custom-sources-only sessions (Linux/Windows/Web, where
            //    `native_log_shutdown_tx` is never set): if any custom sources are
            //    tracked AND all post-app sources are running → nothing left to do.
            //    Without this guard, hot-restart would spawn duplicate processes.
            //    Must NOT fire for platform-capture sessions (Android/macOS/iOS)
            //    where `native_log_shutdown_tx` being None means capture hasn't
            //    started yet — not that it's unneeded.
            {
                let mut running_names: std::collections::HashSet<&str> = handle
                    .custom_source_handles
                    .iter()
                    .map(|h| h.name.as_str())
                    .collect();

                // Include shared post-app sources that are already running globally.
                // These are stored on AppState, not on the per-session handle, so they
                // would otherwise always appear "unstarted", causing spurious
                // StartNativeLogCapture dispatches on hot-restart.
                for shared_handle in &state.shared_source_handles {
                    if !shared_handle.start_before_app {
                        running_names.insert(shared_handle.name.as_str());
                    }
                }
                let has_unstarted_post_app = state
                    .settings
                    .native_logs
                    .custom_sources
                    .iter()
                    .filter(|s| !s.start_before_app)
                    .any(|s| !running_names.contains(s.name.as_str()));

                // Branch A: platform capture running + all post-app sources running → stop.
                if handle.native_log_shutdown_tx.is_some() && !has_unstarted_post_app {
                    tracing::debug!(
                        "Native log capture already fully running for session {}",
                        session_id
                    );
                    return None;
                }
                // Branch B: custom-sources-only session (Linux/Windows/Web): some
                // sources tracked + all post-app sources running → stop (hot-restart
                // guard). The `!needs_platform_capture` condition prevents this branch
                // from firing on Android/macOS/iOS, where `native_log_shutdown_tx`
                // being None means platform capture hasn't started yet.
                if !handle.custom_source_handles.is_empty()
                    && !has_unstarted_post_app
                    && !needs_platform_capture
                {
                    tracing::debug!(
                        "All custom sources already running for session {} — skipping",
                        session_id
                    );
                    return None;
                }
            }

            let has_platform_tools = state.tool_availability.native_logs_available(platform);

            let has_custom_sources = !state.settings.native_logs.custom_sources.is_empty();

            tracing::debug!(
                "platform={}, needs_platform={}, has_tools={}, custom_sources={}, enabled={}",
                platform,
                needs_platform_capture,
                has_platform_tools,
                has_custom_sources,
                state.settings.native_logs.enabled
            );

            // Determine if we should emit the action:
            // - Platform capture is requested AND tools are available, OR
            // - Custom sources are configured (these work regardless of platform/tools)
            let should_start = (needs_platform_capture && has_platform_tools) || has_custom_sources;

            if !should_start {
                if needs_platform_capture && !has_platform_tools {
                    tracing::debug!(
                        "Native log capture skipped for {}: tools not available",
                        platform
                    );
                }
                return None;
            }

            // Collect names of custom sources already running so spawn_custom_sources()
            // can skip them (prevents double-spawning pre-app sources on AppStarted).
            let running_source_names: Vec<String> = handle
                .custom_source_handles
                .iter()
                .map(|h| h.name.clone())
                .collect();

            // Collect names of shared sources already running globally so
            // spawn_custom_sources() can skip them (prevents spawning a shared
            // source twice when multiple sessions come up simultaneously).
            let running_shared_names = state.running_shared_source_names();

            tracing::debug!("Emitting StartNativeLogCapture for session {}", session_id);
            return Some(UpdateAction::StartNativeLogCapture {
                session_id,
                platform: platform.clone(),
                device_id: handle.session.device_id.clone(),
                device_name: handle.session.device_name.clone(),
                app_id: Some(app_start.app_id.clone()),
                settings: state.settings.native_logs.clone(),
                project_path: state.project_path.clone(),
                running_source_names,
                running_shared_names,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use fdemon_core::{
        AppDebugPort, AppProgress, AppStart, AppStarted, AppStop, DaemonMessage, LogSource,
    };

    /// Helper to create a test Device
    fn test_device(id: &str) -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: id.to_string(),
            name: format!("Device {}", id),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: true,
            capabilities: None,
        }
    }

    /// Helper to create a state with a session that has a given app_id
    fn state_with_session(app_id: &str) -> (AppState, SessionId) {
        let mut state = AppState::new();
        let device = test_device("test-device");
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Mark session as started with given app_id
        let msg = DaemonMessage::AppStart(AppStart {
            app_id: app_id.to_string(),
            device_id: "test-device".to_string(),
            directory: "/tmp/app".to_string(),
            launch_mode: None,
            supports_restart: true,
        });
        handle_session_message_state(&mut state, session_id, &msg);

        (state, session_id)
    }

    #[test]
    fn test_handle_app_debug_port_stores_ws_uri() {
        let (mut state, session_id) = state_with_session("test-app");

        let msg = DaemonMessage::AppDebugPort(AppDebugPort {
            app_id: "test-app".to_string(),
            port: 8080,
            ws_uri: "ws://127.0.0.1:8080/ws".to_string(),
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.session.ws_uri,
            Some("ws://127.0.0.1:8080/ws".to_string())
        );
    }

    #[test]
    fn test_handle_app_debug_port_ignores_wrong_app_id() {
        let (mut state, session_id) = state_with_session("test-app");

        let msg = DaemonMessage::AppDebugPort(AppDebugPort {
            app_id: "other-app".to_string(),
            port: 8080,
            ws_uri: "ws://127.0.0.1:8080/ws".to_string(),
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.session.ws_uri, None);
    }

    #[test]
    fn test_ws_uri_cleared_on_app_stop() {
        let (mut state, session_id) = state_with_session("test-app");

        // First set the ws_uri
        let debug_port_msg = DaemonMessage::AppDebugPort(AppDebugPort {
            app_id: "test-app".to_string(),
            port: 8080,
            ws_uri: "ws://127.0.0.1:8080/ws".to_string(),
        });
        handle_session_message_state(&mut state, session_id, &debug_port_msg);

        {
            let handle = state.session_manager.get(session_id).unwrap();
            assert!(handle.session.ws_uri.is_some(), "ws_uri should be set");
        }

        // Now stop the app
        let stop_msg = DaemonMessage::AppStop(AppStop {
            app_id: "test-app".to_string(),
            error: None,
        });
        handle_session_message_state(&mut state, session_id, &stop_msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.session.ws_uri, None,
            "ws_uri should be cleared on stop"
        );
        assert_eq!(handle.session.app_id, None, "app_id should also be cleared");
    }

    #[test]
    fn test_log_source_vm_service_prefix() {
        assert_eq!(LogSource::VmService.prefix(), "vm");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // maybe_serve_devtools tests
    // ─────────────────────────────────────────────────────────────────────────

    /// Attach a test `CommandSender` to a session so `maybe_serve_devtools` will
    /// pass its `cmd_sender.is_some()` guard.
    fn attach_cmd_sender(state: &mut AppState, session_id: SessionId) {
        let sender = fdemon_daemon::CommandSender::new_for_test();
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.cmd_sender = Some(sender);
        }
    }

    /// `maybe_serve_devtools` returns `Some(SendDaemonCommand)` when neither the
    /// endpoint nor a pending request is present, and the session has a cmd_sender.
    #[test]
    fn vm_service_ready_triggers_serve_devtools() {
        let (mut state, session_id) = state_with_session("test-app");
        attach_cmd_sender(&mut state, session_id);

        let action = maybe_serve_devtools(&mut state, session_id);

        match action {
            Some(UpdateAction::SendDaemonCommand {
                session_id: sid,
                command: fdemon_daemon::DaemonCommand::ServeDevTools { request_id },
                cmd_sender: _,
            }) => {
                assert_eq!(sid, session_id);
                let rid = request_id.expect("request_id should be Some");
                assert!(
                    rid.starts_with("devtools-serve-"),
                    "request_id should start with devtools-serve-"
                );
            }
            other => panic!("expected SendDaemonCommand(ServeDevTools), got {:?}", other),
        }

        // devtools_serve_pending should now be true
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.session.devtools_serve_pending,
            "devtools_serve_pending should be true after dispatch"
        );
        assert!(
            handle.session.devtools_endpoint.is_none(),
            "devtools_endpoint should still be None"
        );
    }

    /// Without a `cmd_sender`, `maybe_serve_devtools` returns `None` and does
    /// NOT set `devtools_serve_pending` (guard prevents premature pending state).
    #[test]
    fn no_action_without_cmd_sender() {
        let (mut state, session_id) = state_with_session("test-app");
        // No cmd_sender attached

        let action = maybe_serve_devtools(&mut state, session_id);
        assert!(
            action.is_none(),
            "should return None when no cmd_sender attached"
        );
        // pending should NOT be set
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            !handle.session.devtools_serve_pending,
            "devtools_serve_pending must not be set when cmd_sender is absent"
        );
    }

    /// When `devtools_serve_pending` is already true, `maybe_serve_devtools`
    /// returns `None` (idempotent — no duplicate dispatch).
    #[test]
    fn idempotent_dispatch_when_serve_pending() {
        let (mut state, session_id) = state_with_session("test-app");
        attach_cmd_sender(&mut state, session_id);

        // First call sets pending flag
        let first = maybe_serve_devtools(&mut state, session_id);
        assert!(first.is_some(), "first call should return Some");

        // Second call should be a no-op
        let second = maybe_serve_devtools(&mut state, session_id);
        assert!(
            second.is_none(),
            "second call should return None (idempotent)"
        );
    }

    /// When `devtools_endpoint` is already populated (primary `app.devTools` path),
    /// `maybe_serve_devtools` returns `None` — no redundant RPC needed.
    #[test]
    fn idempotent_dispatch_when_endpoint_already_set() {
        let (mut state, session_id) = state_with_session("test-app");
        attach_cmd_sender(&mut state, session_id);

        // Simulate the primary app.devTools path having already populated the endpoint
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.session.devtools_endpoint = Some(crate::session::DevToolsEndpoint {
                base_url: "http://127.0.0.1:9100".to_string(),
            });
        }

        let action = maybe_serve_devtools(&mut state, session_id);
        assert!(
            action.is_none(),
            "should return None when endpoint already set"
        );
    }

    /// Multiple sessions get distinct request IDs in their `ServeDevTools` commands.
    #[test]
    fn multiple_sessions_get_distinct_request_ids() {
        let mut state = AppState::new();
        let device1 = test_device("device-1");
        let device2 = test_device("device-2");
        let sid1 = state.session_manager.create_session(&device1).unwrap();
        let sid2 = state.session_manager.create_session(&device2).unwrap();

        // Attach cmd_senders so the guard passes
        attach_cmd_sender(&mut state, sid1);
        attach_cmd_sender(&mut state, sid2);

        let action1 = maybe_serve_devtools(&mut state, sid1);
        let action2 = maybe_serve_devtools(&mut state, sid2);

        let rid1 = match action1 {
            Some(UpdateAction::SendDaemonCommand {
                command: fdemon_daemon::DaemonCommand::ServeDevTools { request_id },
                ..
            }) => request_id.unwrap(),
            other => panic!("expected SendDaemonCommand for sid1, got {:?}", other),
        };

        let rid2 = match action2 {
            Some(UpdateAction::SendDaemonCommand {
                command: fdemon_daemon::DaemonCommand::ServeDevTools { request_id },
                ..
            }) => request_id.unwrap(),
            other => panic!("expected SendDaemonCommand for sid2, got {:?}", other),
        };

        assert_ne!(
            rid1, rid2,
            "distinct sessions should produce distinct request IDs"
        );
        assert!(rid1.contains(&sid1.to_string()), "rid1 should embed sid1");
        assert!(rid2.contains(&sid2.to_string()), "rid2 should embed sid2");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // AppStarted / AppProgress lifecycle tests
    // ─────────────────────────────────────────────────────────────────────────

    /// `app.started` with matching app_id calls `mark_running()` → `Running` phase,
    /// and clears any in-flight progress text.
    #[test]
    fn app_started_event_sets_running() {
        let (mut state, session_id) = state_with_session("my-app");

        // Confirm we start in Launching (mark_started sets Launching)
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.session.phase, fdemon_core::AppPhase::Launching);

        // Simulate in-flight progress text
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.session.set_progress("Building…");
        }

        // Feed AppStarted for the same app_id
        let msg = DaemonMessage::AppStarted(AppStarted {
            app_id: "my-app".to_string(),
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.session.phase,
            fdemon_core::AppPhase::Running,
            "phase should be Running after AppStarted"
        );
        assert!(
            handle.session.current_progress.is_none(),
            "mark_running should clear current_progress"
        );
    }

    /// `app.started` with a different app_id is ignored — phase stays Launching.
    #[test]
    fn app_started_event_ignores_wrong_app_id() {
        let (mut state, session_id) = state_with_session("my-app");

        let msg = DaemonMessage::AppStarted(AppStarted {
            app_id: "other-app".to_string(),
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.session.phase,
            fdemon_core::AppPhase::Launching,
            "wrong app_id should leave phase unchanged"
        );
    }

    /// `app.progress` with `finished:false` and a message sets `current_progress`
    /// while the session is not yet running (Launching).
    #[test]
    fn app_progress_sets_progress_while_launching() {
        let (mut state, session_id) = state_with_session("my-app");

        let msg = DaemonMessage::AppProgress(AppProgress {
            app_id: "my-app".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Building debug APK…".to_string()),
            finished: false,
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.session.current_progress.as_deref(),
            Some("Building debug APK…"),
            "current_progress should be set from AppProgress"
        );
    }

    /// `app.progress` with `finished:true` clears `current_progress`.
    #[test]
    fn app_progress_finished_clears_progress() {
        let (mut state, session_id) = state_with_session("my-app");

        // First set progress
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.session.set_progress("Building…");
        }

        let msg = DaemonMessage::AppProgress(AppProgress {
            app_id: "my-app".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: None,
            finished: true,
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.session.current_progress.is_none(),
            "finished:true should clear current_progress"
        );
    }

    /// `app.progress` is ignored once the session is Running.
    #[test]
    fn app_progress_ignored_when_running() {
        let (mut state, session_id) = state_with_session("my-app");

        // Move to Running
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.session.mark_running();
        }

        let msg = DaemonMessage::AppProgress(AppProgress {
            app_id: "my-app".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Should be ignored".to_string()),
            finished: false,
        });
        handle_session_message_state(&mut state, session_id, &msg);

        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.session.current_progress.is_none(),
            "AppProgress should be ignored once running"
        );
    }

    #[test]
    fn vm_service_failure_line_surfaces_guidance_and_keeps_launching() {
        // Android session in Launching (state_with_session marks app.start).
        let (mut state, session_id) = state_with_session("my-app");
        {
            let handle = state.session_manager.get(session_id).unwrap();
            assert_eq!(handle.session.phase, AppPhase::Launching);
            assert!(!handle.session.vm_service_unavailable);
        }

        // The raw on-device error line (non-JSON) flows through stdout handling.
        handle_session_stdout(
            &mut state,
            session_id,
            "I/flutter (25963): Could not start Dart VM service HTTP server:",
        );

        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.session.vm_service_unavailable,
            "failure line must flag the session"
        );
        // The phase machine is untouched — only app.started promotes to Running.
        assert_eq!(handle.session.phase, AppPhase::Launching);
        // Guidance reached the log buffer.
        assert!(handle
            .session
            .logs
            .iter()
            .any(|e| e.message.contains("android.permission.INTERNET")));
    }
}
