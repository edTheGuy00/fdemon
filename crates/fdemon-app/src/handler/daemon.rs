//! Multi-session daemon event handling

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::Message;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::{DaemonEvent, DaemonMessage, LogEntry, LogSource};
use fdemon_daemon::parse_daemon_message;
use fdemon_dap::adapter::DebugEvent as DapDebugEvent;

use super::session::{
    handle_session_exited, handle_session_message_state, handle_session_stdout,
    maybe_connect_vm_service, maybe_start_native_log_capture,
};

/// Handle daemon events for a specific session (multi-session mode)
///
/// Uses log batching to coalesce rapid log arrivals during high-volume
/// output (hot reload, verbose debugging, etc.). Logs are queued and
/// flushed based on time (16ms) or size (100 entries) thresholds.
///
/// Returns an UpdateResult which may contain a ConnectVmService action
/// when an AppDebugPort event is received.
pub fn handle_session_daemon_event(
    state: &mut AppState,
    session_id: SessionId,
    event: DaemonEvent,
) -> UpdateResult {
    // Check if session still exists (may have been closed)
    if state.session_manager.get(session_id).is_none() {
        tracing::debug!(
            "Discarding event for closed session {}: {:?}",
            session_id,
            match &event {
                DaemonEvent::Stdout(_) => "Stdout",
                DaemonEvent::Stderr(_) => "Stderr",
                DaemonEvent::Exited { .. } => "Exited",
                DaemonEvent::SpawnFailed { .. } => "SpawnFailed",
                DaemonEvent::Message(_) => "Message",
            }
        );
        return UpdateResult::none();
    }

    match event {
        DaemonEvent::Stdout(line) => {
            // Parse once — used for VM connection, native log capture, and state mutation.
            let parsed = parse_daemon_message(&line);

            // Check for AppDebugPort before handle_session_stdout mutates state,
            // so we can capture the ws_uri for VM Service connection.
            let vm_action = match &parsed {
                Some(msg @ DaemonMessage::AppDebugPort(_)) => {
                    maybe_connect_vm_service(state, session_id, msg)
                }
                _ => None,
            };

            // Bridge app.devTools event → Message::DevToolsServed.
            // The `app.devTools` event fires automatically during `flutter run --machine`
            // startup (Flutter ≥ 1.22.0) and provides the base DevTools server URL.
            // This is the primary (preferred) path; the devtools.serve RPC fallback
            // fires later on VmServiceConnected.
            let devtools_msg: Option<Message> = match &parsed {
                Some(DaemonMessage::DevToolsServed { app_id, base_url }) => {
                    // Map app_id → session_id via the session manager.
                    // Use the session_id passed to this handler when app_id is empty
                    // (empty means the message came from the devtools.serve RPC response
                    // which doesn't carry an app_id; in that case the session is already
                    // known from the request routing).
                    let resolved_id = if app_id.is_empty() {
                        Some(session_id)
                    } else {
                        state.session_manager.find_by_app_id(app_id)
                    };
                    resolved_id.map(|sid| Message::DevToolsServed {
                        session_id: sid,
                        base_url: base_url.clone(),
                    })
                }
                Some(DaemonMessage::DevToolsServeFailed { reason }) => {
                    Some(Message::DevToolsServeFailed {
                        session_id,
                        reason: reason.clone(),
                    })
                }
                _ => None,
            };

            // Mutate state (logs the line, updates session phase, etc.).
            handle_session_stdout(state, session_id, &line);

            // Check for AppStart → native log capture.
            // This runs after handle_session_stdout so session.app_id is set.
            let native_log_action = match &parsed {
                Some(msg @ DaemonMessage::AppStart(_)) => {
                    maybe_start_native_log_capture(state, session_id, msg)
                }
                _ => None,
            };

            // Check for AppStarted → forward to DAP adapter so VS Code's Dart
            // extension clears its "Starting debug session..." indicator.
            let app_started_action = match &parsed {
                Some(DaemonMessage::AppStarted(_)) => {
                    tracing::debug!("Session {} app.started → forwarding to DAP", session_id);
                    Some(UpdateAction::ForwardDapDebugEvents(vec![
                        DapDebugEvent::AppStarted,
                    ]))
                }
                _ => None,
            };

            // Priority: VM service connection > native log capture > app started.
            // DevTools events are separate from these (different event names), so
            // devtools_msg and the action are mutually exclusive in practice.
            // When a DevTools message is present, return it as a follow-up message;
            // action handling (VM connect, native log, DAP) is returned as the action.
            let action = vm_action.or(native_log_action).or(app_started_action);
            match (action, devtools_msg) {
                (Some(a), Some(m)) => {
                    // Both present (unlikely in practice — distinct events).
                    // Return the action; the devtools follow-up is deferred.
                    // The devtools state will be populated when the next
                    // VmServiceConnected message triggers the fallback path.
                    tracing::debug!(
                        "Session {}: action and devtools_msg both present; prioritizing action",
                        session_id
                    );
                    let _ = m; // devtools_msg is deferred; fallback path covers it
                    UpdateResult::action(a)
                }
                (Some(a), None) => UpdateResult::action(a),
                (None, Some(m)) => UpdateResult::message(m),
                (None, None) => UpdateResult::none(),
            }
        }
        DaemonEvent::Stderr(line) => {
            if !line.trim().is_empty() {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    // Process through exception detection and raw line handling
                    let entries = handle.session.process_raw_line(&line);
                    for entry in entries {
                        // Use batched logging for performance
                        if handle.session.queue_log(entry) {
                            handle.session.flush_batched_logs();
                        }
                    }
                }
            }
            UpdateResult::none()
        }
        DaemonEvent::Exited { code } => {
            // Flush pending exception buffer before handling exit
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                if let Some(entry) = handle.session.flush_exception_buffer() {
                    handle.session.add_log(entry);
                }
            }
            handle_session_exited(state, session_id, code);
            UpdateResult::none()
        }
        DaemonEvent::SpawnFailed { reason } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                // Spawn failures should be shown immediately (not batched)
                handle.session.add_log(LogEntry::error(
                    LogSource::App,
                    format!("Failed to start Flutter: {}", reason),
                ));
            }
            UpdateResult::none()
        }
        DaemonEvent::Message(msg) => {
            // Check for AppDebugPort before state mutation so we can capture ws_uri
            let vm_action = if let DaemonMessage::AppDebugPort(_) = &msg {
                maybe_connect_vm_service(state, session_id, &msg)
            } else {
                None
            };

            // Bridge DevToolsServed / DevToolsServeFailed → Message::DevTools*.
            let devtools_msg: Option<Message> = match &msg {
                DaemonMessage::DevToolsServed { app_id, base_url } => {
                    let resolved_id = if app_id.is_empty() {
                        Some(session_id)
                    } else {
                        state.session_manager.find_by_app_id(app_id)
                    };
                    resolved_id.map(|sid| Message::DevToolsServed {
                        session_id: sid,
                        base_url: base_url.clone(),
                    })
                }
                DaemonMessage::DevToolsServeFailed { reason } => {
                    Some(Message::DevToolsServeFailed {
                        session_id,
                        reason: reason.clone(),
                    })
                }
                _ => None,
            };

            // Legacy path - convert typed message
            if let Some(entry_info) = fdemon_daemon::to_log_entry(&msg) {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    let entry =
                        LogEntry::new(entry_info.level, entry_info.source, entry_info.message);
                    // Use batched logging for performance
                    if handle.session.queue_log(entry) {
                        handle.session.flush_batched_logs();
                    }
                }
            }
            // Update session state based on message type
            handle_session_message_state(state, session_id, &msg);

            // After handle_session_message_state, mark_started() has been called
            // for AppStart events, so session.app_id is now set.
            let native_log_action = if let DaemonMessage::AppStart(_) = &msg {
                maybe_start_native_log_capture(state, session_id, &msg)
            } else {
                None
            };

            // Priority: VM service connection (AppDebugPort) > native log capture (AppStart).
            // DevTools events are separate and returned as follow-up messages.
            let action = vm_action.or(native_log_action);
            match (action, devtools_msg) {
                (Some(a), Some(m)) => {
                    // Both present — action takes priority; devtools deferred to fallback.
                    tracing::debug!(
                        "Session {}: action and devtools_msg both present in Message arm; \
                         prioritizing action",
                        session_id
                    );
                    let _ = m;
                    UpdateResult::action(a)
                }
                (Some(a), None) => UpdateResult::action(a),
                (None, Some(m)) => UpdateResult::message(m),
                (None, None) => UpdateResult::none(),
            }
        }
    }
}
