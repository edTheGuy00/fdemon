//! Multi-session daemon event handling

use crate::app::session::SessionId;
use crate::app::state::AppState;
use crate::core::{DaemonEvent, LogEntry, LogLevel, LogSource};

use super::session::{handle_session_exited, handle_session_message_state, handle_session_stdout};

/// Handle daemon events for a specific session (multi-session mode)
pub fn handle_session_daemon_event(
    state: &mut AppState,
    session_id: SessionId,
    event: DaemonEvent,
) {
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
        return;
    }

    match event {
        DaemonEvent::Stdout(line) => {
            handle_session_stdout(state, session_id, &line);
        }
        DaemonEvent::Stderr(line) => {
            if !line.trim().is_empty() {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    handle.session.add_log(LogEntry::new(
                        LogLevel::Error,
                        LogSource::FlutterError,
                        line,
                    ));
                }
            }
        }
        DaemonEvent::Exited { code } => {
            handle_session_exited(state, session_id, code);
        }
        DaemonEvent::SpawnFailed { reason } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.add_log(LogEntry::error(
                    LogSource::App,
                    format!("Failed to start Flutter: {}", reason),
                ));
            }
        }
        DaemonEvent::Message(msg) => {
            // Legacy path - convert typed message
            if let Some(entry_info) = msg.to_log_entry() {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    handle.session.add_log(LogEntry::new(
                        entry_info.level,
                        entry_info.source,
                        entry_info.message,
                    ));
                }
            }
            // Update session state based on message type
            handle_session_message_state(state, session_id, &msg);
        }
    }
}
