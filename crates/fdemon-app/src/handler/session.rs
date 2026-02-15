//! Session lifecycle handlers for multi-session mode
//!
//! Uses log batching to coalesce rapid log arrivals during high-volume
//! output (hot reload, verbose debugging, etc.).

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
                handle.session.phase = AppPhase::Initializing;
                tracing::info!(
                    "Session {} app stopped: app_id={}",
                    session_id,
                    app_stop.app_id
                );
            }
        }
    }
}
