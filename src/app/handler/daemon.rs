//! Daemon event handlers for legacy and multi-session modes

use crate::app::session::SessionId;
use crate::app::state::AppState;
use crate::core::{AppPhase, DaemonEvent, LogEntry, LogLevel, LogSource};
use crate::daemon::{protocol, DaemonMessage};

use super::helpers::detect_raw_line_level;
use super::session::{handle_session_exited, handle_session_message_state, handle_session_stdout};

/// Handle daemon events - convert to log entries (legacy single-session mode)
pub fn handle_daemon_event(state: &mut AppState, event: DaemonEvent) {
    match event {
        DaemonEvent::Stdout(line) => {
            // Try to strip brackets and parse
            if let Some(json) = protocol::strip_brackets(&line) {
                if let Some(msg) = DaemonMessage::parse(json) {
                    // Handle responses separately (they don't create log entries)
                    if matches!(msg, DaemonMessage::Response { .. }) {
                        tracing::debug!("Response received: {}", msg.summary());
                        return;
                    }

                    // Convert to log entry if applicable
                    if let Some(entry_info) = msg.to_log_entry() {
                        // Add main log entry
                        state.add_log(LogEntry::new(
                            entry_info.level,
                            entry_info.source,
                            entry_info.message,
                        ));

                        // Add stack trace as separate entries if present
                        if let Some(trace) = entry_info.stack_trace {
                            for line in trace.lines().take(10) {
                                // Limit stack trace
                                state.add_log(LogEntry::new(
                                    LogLevel::Debug,
                                    LogSource::FlutterError,
                                    format!("    {}", line),
                                ));
                            }
                        }

                        // Update app state based on message type
                        handle_daemon_message_state(state, &msg);
                    } else {
                        // Unknown event type, log at debug level
                        tracing::debug!("Unhandled daemon message: {}", msg.summary());
                    }
                } else {
                    // Unparseable JSON - show raw
                    tracing::debug!("Unparseable daemon JSON: {}", json);
                }
            } else if !line.trim().is_empty() {
                // Non-JSON output (build progress, etc.)
                // Detect if it's an error or warning
                let (level, message) = detect_raw_line_level(&line);
                state.add_log(LogEntry::new(level, LogSource::Flutter, message));
            }
        }

        DaemonEvent::Stderr(line) => {
            if !line.trim().is_empty() {
                state.add_log(LogEntry::new(
                    LogLevel::Error,
                    LogSource::FlutterError,
                    line,
                ));
            }
        }

        DaemonEvent::Exited { code } => {
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
            state.add_log(LogEntry::new(level, LogSource::App, message));
            state.add_log(LogEntry::info(LogSource::App, "Exiting Flutter Demon..."));
            state.phase = AppPhase::Quitting;
        }

        DaemonEvent::SpawnFailed { reason } => {
            state.add_log(LogEntry::error(
                LogSource::App,
                format!("Failed to start Flutter: {}", reason),
            ));
        }

        DaemonEvent::Message(msg) => {
            // Legacy path - convert typed message
            if let Some(entry_info) = msg.to_log_entry() {
                state.add_log(LogEntry::new(
                    entry_info.level,
                    entry_info.source,
                    entry_info.message,
                ));
            }
            handle_daemon_message_state(state, &msg);
        }
    }
}

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

/// Handle typed daemon messages - update app state (not logging)
pub fn handle_daemon_message_state(state: &mut AppState, msg: &DaemonMessage) {
    // Capture app_id from AppStart event
    if let DaemonMessage::AppStart(app_start) = msg {
        state.current_app_id = Some(app_start.app_id.clone());
        state.phase = AppPhase::Running;
    }

    // Clear app_id on AppStop
    if let DaemonMessage::AppStop(app_stop) = msg {
        if state.current_app_id.as_ref() == Some(&app_stop.app_id) {
            state.current_app_id = None;
            state.phase = AppPhase::Initializing;
        }
    }
}
