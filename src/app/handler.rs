//! Update function - handles state transitions (TEA pattern)

use super::message::Message;
use super::state::AppState;
use crate::core::{AppPhase, DaemonEvent, LogEntry, LogLevel, LogSource};
use crate::daemon::protocol;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Process a message and update state
/// Returns an optional follow-up message
pub fn update(state: &mut AppState, message: Message) -> Option<Message> {
    match message {
        Message::Quit => {
            state.phase = AppPhase::Quitting;
            None
        }

        Message::Key(key) => handle_key(state, key),

        Message::Daemon(event) => {
            handle_daemon_event(state, event);
            None
        }

        Message::ScrollUp => {
            state.log_view_state.scroll_up(1);
            None
        }

        Message::ScrollDown => {
            state.log_view_state.scroll_down(1);
            None
        }

        Message::ScrollToTop => {
            state.log_view_state.scroll_to_top();
            None
        }

        Message::ScrollToBottom => {
            state.log_view_state.scroll_to_bottom();
            None
        }

        Message::PageUp => {
            state.log_view_state.page_up();
            None
        }

        Message::PageDown => {
            state.log_view_state.page_down();
            None
        }

        Message::Tick => None,
    }
}

/// Handle daemon events - convert to log entries
fn handle_daemon_event(state: &mut AppState, event: DaemonEvent) {
    match event {
        DaemonEvent::Stdout(line) => {
            // Try to strip brackets and parse
            if let Some(json) = protocol::strip_brackets(&line) {
                // For now, log the raw message (Phase 2 will parse properly)
                if let Some(msg) = protocol::RawMessage::parse(json) {
                    state.add_log(LogEntry::new(
                        LogLevel::Info,
                        LogSource::Flutter,
                        msg.summary(),
                    ));
                } else {
                    // Unparseable JSON
                    state.add_log(LogEntry::new(LogLevel::Debug, LogSource::Flutter, line));
                }
            } else if !line.trim().is_empty() {
                // Non-JSON output (e.g., build progress)
                state.add_log(LogEntry::new(LogLevel::Info, LogSource::Flutter, line));
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
            let message = match code {
                Some(0) => "Flutter process exited normally".to_string(),
                Some(c) => format!("Flutter process exited with code {}", c),
                None => "Flutter process exited".to_string(),
            };
            state.add_log(LogEntry::new(LogLevel::Warning, LogSource::App, message));
            state.phase = AppPhase::Initializing;
        }

        DaemonEvent::SpawnFailed { reason } => {
            state.add_log(LogEntry::error(
                LogSource::App,
                format!("Failed to start Flutter: {}", reason),
            ));
        }
    }
}

/// Convert key events to messages
fn handle_key(_state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Scrolling
        KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollUp),
        KeyCode::Char('g') => Some(Message::ScrollToTop),
        KeyCode::Char('G') => Some(Message::ScrollToBottom),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown => Some(Message::PageDown),
        KeyCode::Home => Some(Message::ScrollToTop),
        KeyCode::End => Some(Message::ScrollToBottom),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::AppState;

    #[test]
    fn test_quit_message_sets_quitting_phase() {
        let mut state = AppState::new();
        assert_ne!(state.phase, AppPhase::Quitting);

        update(&mut state, Message::Quit);

        assert_eq!(state.phase, AppPhase::Quitting);
        assert!(state.should_quit());
    }

    #[test]
    fn test_should_quit_returns_true_when_quitting() {
        let mut state = AppState::new();
        state.phase = AppPhase::Quitting;
        assert!(state.should_quit());
    }

    #[test]
    fn test_should_quit_returns_false_when_running() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        assert!(!state.should_quit());
    }

    #[test]
    fn test_q_key_produces_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::Quit)));
    }

    #[test]
    fn test_escape_key_produces_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::Quit)));
    }

    #[test]
    fn test_ctrl_c_produces_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::Quit)));
    }

    #[test]
    fn test_daemon_exited_event_logs_message() {
        let mut state = AppState::new();
        let initial_logs = state.logs.len();

        update(
            &mut state,
            Message::Daemon(DaemonEvent::Exited { code: Some(0) }),
        );

        assert!(state.logs.len() > initial_logs);
    }

    #[test]
    fn test_scroll_messages_update_log_view_state() {
        let mut state = AppState::new();
        state.log_view_state.total_lines = 100;
        state.log_view_state.visible_lines = 20;
        state.log_view_state.offset = 50;

        update(&mut state, Message::ScrollUp);
        assert_eq!(state.log_view_state.offset, 49);

        update(&mut state, Message::ScrollDown);
        assert_eq!(state.log_view_state.offset, 50);

        update(&mut state, Message::ScrollToTop);
        assert_eq!(state.log_view_state.offset, 0);

        update(&mut state, Message::ScrollToBottom);
        assert_eq!(state.log_view_state.offset, 80);
    }
}
