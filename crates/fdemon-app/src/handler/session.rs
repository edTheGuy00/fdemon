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
                if entry_info.stack_trace.is_some() {
                    // Has dedicated stack trace — use existing path
                    let parsed_trace =
                        ParsedStackTrace::parse(entry_info.stack_trace.as_ref().unwrap());
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
        // Non-JSON output (build progress, etc.)
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            // Process through exception detection and raw line handling
            let entries = handle.session.process_raw_line(line);
            for entry in entries {
                // Use batched logging for performance
                if handle.session.queue_log(entry) {
                    handle.session.flush_batched_logs();
                }
            }
        }
    }
}

/// Handle session exit events
pub fn handle_session_exited(state: &mut AppState, session_id: SessionId, code: Option<i32>) {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
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
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use fdemon_core::{AppDebugPort, AppStart, AppStop, DaemonMessage, LogSource};

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
}
