//! DAP server lifecycle message handler.
//!
//! Processes DAP server state transitions through the TEA update cycle.
//! The actual server start/stop is performed by UpdateAction handlers in
//! the TUI/headless event loops — this module only manages AppState.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::Message;
use crate::state::{AppState, DapStatus};

/// Handle a DAP server lifecycle message.
pub fn handle_dap_message(state: &mut AppState, message: &Message) -> UpdateResult {
    match message {
        Message::StartDapServer => handle_start(state),
        Message::StopDapServer => handle_stop(state),
        Message::ToggleDap => handle_toggle(state),
        Message::DapServerStarted { port } => handle_started(state, *port),
        Message::DapServerStopped => handle_stopped(state),
        Message::DapServerFailed { reason } => handle_failed(state, reason),
        Message::DapClientConnected { client_id } => handle_client_connected(state, client_id),
        Message::DapClientDisconnected { client_id } => {
            handle_client_disconnected(state, client_id)
        }
        _ => UpdateResult::none(),
    }
}

fn handle_start(state: &mut AppState) -> UpdateResult {
    if state.dap_status.is_running() {
        return UpdateResult::none(); // Already running, no-op
    }
    state.dap_status = DapStatus::Starting;
    let port = state.settings.dap.port;
    let bind_addr = state.settings.dap.bind_address.clone();
    UpdateResult::action(UpdateAction::SpawnDapServer { port, bind_addr })
}

fn handle_stop(state: &mut AppState) -> UpdateResult {
    if !state.dap_status.is_running() {
        return UpdateResult::none(); // Not running, no-op
    }
    state.dap_status = DapStatus::Stopping;
    UpdateResult::action(UpdateAction::StopDapServer)
}

fn handle_toggle(state: &mut AppState) -> UpdateResult {
    if state.dap_status.is_running() {
        handle_stop(state)
    } else {
        handle_start(state)
    }
}

fn handle_started(state: &mut AppState, port: u16) -> UpdateResult {
    state.dap_status = DapStatus::Running {
        port,
        client_count: 0,
    };
    tracing::info!(
        "DAP server listening on {}:{}",
        state.settings.dap.bind_address,
        port
    );
    UpdateResult::none()
}

fn handle_stopped(state: &mut AppState) -> UpdateResult {
    state.dap_status = DapStatus::Off;
    tracing::info!("DAP server stopped");
    UpdateResult::none()
}

fn handle_failed(state: &mut AppState, reason: &str) -> UpdateResult {
    state.dap_status = DapStatus::Off;
    tracing::error!("DAP server failed to start: {}", reason);
    UpdateResult::none()
}

fn handle_client_connected(state: &mut AppState, client_id: &str) -> UpdateResult {
    if let DapStatus::Running { client_count, .. } = &mut state.dap_status {
        *client_count += 1;
        tracing::info!("DAP client connected: {}", client_id);
    }
    UpdateResult::none()
}

fn handle_client_disconnected(state: &mut AppState, client_id: &str) -> UpdateResult {
    if let DapStatus::Running { client_count, .. } = &mut state.dap_status {
        *client_count = client_count.saturating_sub(1);
        tracing::info!("DAP client disconnected: {}", client_id);
    }
    UpdateResult::none()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn test_state() -> AppState {
        AppState::new()
    }

    // --- DapStatus helper method tests ---

    #[test]
    fn test_dap_status_default_is_off() {
        assert_eq!(DapStatus::default(), DapStatus::Off);
    }

    #[test]
    fn test_dap_status_port() {
        assert_eq!(DapStatus::Off.port(), None);
        assert_eq!(DapStatus::Starting.port(), None);
        assert_eq!(
            DapStatus::Running {
                port: 4711,
                client_count: 0
            }
            .port(),
            Some(4711)
        );
        assert_eq!(DapStatus::Stopping.port(), None);
    }

    #[test]
    fn test_dap_status_is_running() {
        assert!(!DapStatus::Off.is_running());
        assert!(!DapStatus::Starting.is_running());
        assert!(DapStatus::Running {
            port: 4711,
            client_count: 0
        }
        .is_running());
        assert!(!DapStatus::Stopping.is_running());
    }

    #[test]
    fn test_dap_status_client_count() {
        assert_eq!(DapStatus::Off.client_count(), 0);
        assert_eq!(DapStatus::Starting.client_count(), 0);
        assert_eq!(
            DapStatus::Running {
                port: 4711,
                client_count: 3
            }
            .client_count(),
            3
        );
        assert_eq!(DapStatus::Stopping.client_count(), 0);
    }

    // --- handle_dap_message state transition tests ---

    #[test]
    fn test_start_when_off_transitions_to_starting() {
        let mut state = test_state();
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_some()); // SpawnDapServer
    }

    #[test]
    fn test_start_when_starting_transitions_to_starting_with_action() {
        let mut state = test_state();
        // Starting is not "running", so StartDapServer should still emit action
        state.dap_status = DapStatus::Starting;
        // Actually per spec: StartDapServer when Starting is not explicitly
        // covered, but since is_running() returns false for Starting,
        // it should re-emit the action. Let's verify it does not panic.
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_some());
    }

    #[test]
    fn test_start_when_running_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 0,
        };
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        assert!(state.dap_status.is_running());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_stop_when_running_transitions_to_stopping() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 0,
        };
        let result = handle_dap_message(&mut state, &Message::StopDapServer);
        assert_eq!(state.dap_status, DapStatus::Stopping);
        assert!(result.action.is_some()); // StopDapServer action
    }

    #[test]
    fn test_stop_when_off_is_noop() {
        let mut state = test_state();
        assert_eq!(state.dap_status, DapStatus::Off);
        let result = handle_dap_message(&mut state, &Message::StopDapServer);
        assert_eq!(state.dap_status, DapStatus::Off);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_toggle_when_off_starts() {
        let mut state = test_state();
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_some());
    }

    #[test]
    fn test_toggle_when_running_stops() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 0,
        };
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Stopping);
        assert!(result.action.is_some());
    }

    #[test]
    fn test_dap_server_started_transitions_to_running() {
        let mut state = test_state();
        state.dap_status = DapStatus::Starting;
        let result = handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
        assert_eq!(
            state.dap_status,
            DapStatus::Running {
                port: 4711,
                client_count: 0
            }
        );
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dap_server_stopped_transitions_to_off() {
        let mut state = test_state();
        state.dap_status = DapStatus::Stopping;
        let result = handle_dap_message(&mut state, &Message::DapServerStopped);
        assert_eq!(state.dap_status, DapStatus::Off);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_server_failed_resets_to_off() {
        let mut state = test_state();
        state.dap_status = DapStatus::Starting;
        let result = handle_dap_message(
            &mut state,
            &Message::DapServerFailed {
                reason: "port in use".into(),
            },
        );
        assert_eq!(state.dap_status, DapStatus::Off);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_client_connected_increments_count() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 0,
        };
        handle_dap_message(
            &mut state,
            &Message::DapClientConnected {
                client_id: "c1".into(),
            },
        );
        assert_eq!(state.dap_status.client_count(), 1);
    }

    #[test]
    fn test_client_connected_multiple_times_increments_correctly() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 1,
        };
        handle_dap_message(
            &mut state,
            &Message::DapClientConnected {
                client_id: "c2".into(),
            },
        );
        assert_eq!(state.dap_status.client_count(), 2);
    }

    #[test]
    fn test_client_connected_when_not_running_is_noop() {
        let mut state = test_state();
        // Server is Off, client connected should be silently ignored
        handle_dap_message(
            &mut state,
            &Message::DapClientConnected {
                client_id: "c1".into(),
            },
        );
        assert_eq!(state.dap_status, DapStatus::Off);
    }

    #[test]
    fn test_client_disconnected_decrements_count() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 2,
        };
        handle_dap_message(
            &mut state,
            &Message::DapClientDisconnected {
                client_id: "c1".into(),
            },
        );
        assert_eq!(state.dap_status.client_count(), 1);
    }

    #[test]
    fn test_client_disconnected_saturates_at_zero() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 0,
        };
        handle_dap_message(
            &mut state,
            &Message::DapClientDisconnected {
                client_id: "c1".into(),
            },
        );
        assert_eq!(state.dap_status.client_count(), 0);
    }

    #[test]
    fn test_client_disconnected_when_not_running_is_noop() {
        let mut state = test_state();
        // Server is Off, client disconnected should be silently ignored
        handle_dap_message(
            &mut state,
            &Message::DapClientDisconnected {
                client_id: "c1".into(),
            },
        );
        assert_eq!(state.dap_status, DapStatus::Off);
    }

    #[test]
    fn test_unknown_message_returns_none() {
        let mut state = test_state();
        // Pass a non-DAP message to the handler; it should return UpdateResult::none()
        let result = handle_dap_message(&mut state, &Message::Tick);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    // --- Verify SpawnDapServer action carries correct port from settings ---

    #[test]
    fn test_start_emits_spawn_action_with_settings_port() {
        let mut state = test_state();
        // Default DapSettings port is 4711
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        match result.action {
            Some(UpdateAction::SpawnDapServer { port, .. }) => {
                // The port should come from state.settings.dap.port
                assert_eq!(port, state.settings.dap.port);
            }
            other => panic!("Expected SpawnDapServer action, got: {:?}", other),
        }
    }

    #[test]
    fn test_stop_emits_stop_dap_server_action() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            client_count: 0,
        };
        let result = handle_dap_message(&mut state, &Message::StopDapServer);
        assert!(matches!(result.action, Some(UpdateAction::StopDapServer)));
    }
}
