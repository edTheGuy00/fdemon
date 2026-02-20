//! Multi-session daemon event handling

use crate::handler::UpdateResult;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::{DaemonEvent, DaemonMessage, LogEntry, LogSource};
use fdemon_daemon::parse_daemon_message;

use super::session::{
    handle_session_exited, handle_session_message_state, handle_session_stdout,
    maybe_connect_vm_service,
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
            // Check for AppDebugPort before handle_session_stdout mutates state,
            // so we can capture the ws_uri for VM Service connection.
            let vm_action =
                if let Some(msg @ DaemonMessage::AppDebugPort(_)) = parse_daemon_message(&line) {
                    maybe_connect_vm_service(state, session_id, &msg)
                } else {
                    None
                };

            handle_session_stdout(state, session_id, &line);

            match vm_action {
                Some(action) => UpdateResult::action(action),
                None => UpdateResult::none(),
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

            // Return ConnectVmService action if AppDebugPort was received
            match vm_action {
                Some(action) => UpdateResult::action(action),
                None => UpdateResult::none(),
            }
        }
    }
}
