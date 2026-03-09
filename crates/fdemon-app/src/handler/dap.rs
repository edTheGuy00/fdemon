//! DAP server lifecycle message handler.
//!
//! Processes DAP server state transitions through the TEA update cycle.
//! The actual server start/stop is performed by UpdateAction handlers in
//! the TUI/headless event loops — this module only manages AppState.

use std::collections::HashSet;

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::Message;
use crate::state::{AppState, DapConfigStatus, DapStatus};

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
        Message::DapConfigGenerated {
            ide_name,
            path,
            action,
        } => handle_config_generated(state, ide_name, path, action),
        _ => UpdateResult::none(),
    }
}

fn handle_start(state: &mut AppState) -> UpdateResult {
    if state.dap_status.is_running() || state.dap_status == DapStatus::Starting {
        return UpdateResult::none(); // Already running or starting, no-op
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
    match state.dap_status {
        DapStatus::Running { .. } => handle_stop(state),
        DapStatus::Off => handle_start(state),
        DapStatus::Starting | DapStatus::Stopping => UpdateResult::none(),
    }
}

fn handle_started(state: &mut AppState, port: u16) -> UpdateResult {
    state.dap_status = DapStatus::Running {
        port,
        clients: HashSet::new(),
    };
    tracing::info!(
        "DAP server listening on {}:{}",
        state.settings.dap.bind_address,
        port
    );

    // Trigger IDE config auto-generation if the setting is enabled.
    if state.settings.dap.auto_configure_ide {
        UpdateResult::action(UpdateAction::GenerateIdeConfig {
            port,
            ide_override: None,
        })
    } else {
        UpdateResult::none()
    }
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
    if let DapStatus::Running { clients, .. } = &mut state.dap_status {
        clients.insert(client_id.to_string());
        tracing::info!("DAP client connected: {}", client_id);
    }
    UpdateResult::none()
}

fn handle_client_disconnected(state: &mut AppState, client_id: &str) -> UpdateResult {
    if let DapStatus::Running { clients, .. } = &mut state.dap_status {
        clients.remove(client_id);
        tracing::info!("DAP client disconnected: {}", client_id);
    }

    // Phase 4, Task 03: if the watcher was suspended (debugger was paused when
    // the client disconnected) resume it so auto-reload is not stuck disabled.
    // Emit ResumeFileWatcher as a follow-up message so the existing resume
    // handler handles the pending-changes flush in a single place.
    if state.file_watcher_suspended {
        tracing::info!(
            "DAP client '{}' disconnected while watcher was suspended — resuming",
            client_id
        );
        return UpdateResult::message(crate::message::Message::ResumeFileWatcher);
    }

    UpdateResult::none()
}

fn handle_config_generated(
    state: &mut AppState,
    ide_name: &str,
    path: &std::path::Path,
    action: &str,
) -> UpdateResult {
    state.dap_config_status = Some(DapConfigStatus {
        ide_name: ide_name.to_string(),
        path: path.to_path_buf(),
        action: action.to_string(),
    });
    tracing::info!(
        "DAP config for {}: {} at {}",
        ide_name,
        action,
        path.display()
    );
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
                clients: HashSet::new(),
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
            clients: HashSet::new(),
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
                clients: ["a".to_string(), "b".to_string(), "c".to_string()]
                    .into_iter()
                    .collect(),
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
    fn test_start_when_starting_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Starting;
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_none(), "Should not spawn a second server");
    }

    #[test]
    fn test_start_when_running_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            clients: HashSet::new(),
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
            clients: HashSet::new(),
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
            clients: HashSet::new(),
        };
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Stopping);
        assert!(result.action.is_some());
    }

    #[test]
    fn test_toggle_when_starting_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Starting;
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_toggle_when_stopping_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Stopping;
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Stopping);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dap_server_started_transitions_to_running() {
        let mut state = test_state();
        state.dap_status = DapStatus::Starting;
        // auto_configure_ide defaults to true, so the action will be GenerateIdeConfig.
        state.settings.dap.auto_configure_ide = false;
        let result = handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
        assert_eq!(
            state.dap_status,
            DapStatus::Running {
                port: 4711,
                clients: HashSet::new(),
            }
        );
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dap_server_started_triggers_ide_config_when_enabled() {
        let mut state = test_state();
        state.settings.dap.auto_configure_ide = true;
        let result = handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
        assert!(matches!(
            result.action,
            Some(UpdateAction::GenerateIdeConfig {
                port: 4711,
                ide_override: None
            })
        ));
    }

    #[test]
    fn test_dap_server_started_skips_ide_config_when_disabled() {
        let mut state = test_state();
        state.settings.dap.auto_configure_ide = false;
        let result = handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dap_server_started_still_sets_status() {
        let mut state = test_state();
        state.settings.dap.auto_configure_ide = true;
        handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
        assert!(matches!(
            state.dap_status,
            DapStatus::Running { port: 4711, .. }
        ));
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
            clients: HashSet::new(),
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
    fn test_client_connected_duplicate_is_idempotent() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            clients: ["c1".to_string()].into_iter().collect(),
        };
        handle_dap_message(
            &mut state,
            &Message::DapClientConnected {
                client_id: "c1".into(),
            },
        );
        // Duplicate insert must not increase the count
        assert_eq!(state.dap_status.client_count(), 1);
    }

    #[test]
    fn test_client_connected_multiple_distinct_clients() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            clients: ["c1".to_string()].into_iter().collect(),
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
            clients: ["c1".to_string(), "c2".to_string()].into_iter().collect(),
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
    fn test_client_disconnected_unknown_id_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running {
            port: 4711,
            clients: HashSet::new(),
        };
        // Disconnecting an ID that was never connected is a silent no-op
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
            clients: HashSet::new(),
        };
        let result = handle_dap_message(&mut state, &Message::StopDapServer);
        assert!(matches!(result.action, Some(UpdateAction::StopDapServer)));
    }

    // --- DapConfigGenerated tests (Phase 5, Task 03) ---

    #[test]
    fn test_handle_dap_config_generated_stores_status() {
        let mut state = test_state();
        let msg = Message::DapConfigGenerated {
            ide_name: "VS Code".into(),
            path: std::path::PathBuf::from(".vscode/launch.json"),
            action: "Created".into(),
        };
        let result = handle_dap_message(&mut state, &msg);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
        assert!(state.dap_config_status.is_some());
        let status = state.dap_config_status.unwrap();
        assert_eq!(status.ide_name, "VS Code");
        assert_eq!(status.path, std::path::PathBuf::from(".vscode/launch.json"));
        assert_eq!(status.action, "Created");
    }

    #[test]
    fn test_handle_dap_config_generated_overwrites_previous_status() {
        let mut state = test_state();
        // First generation
        handle_dap_message(
            &mut state,
            &Message::DapConfigGenerated {
                ide_name: "Neovim".into(),
                path: std::path::PathBuf::from(".nvim/dap.lua"),
                action: "Created".into(),
            },
        );
        // Second generation overwrites
        handle_dap_message(
            &mut state,
            &Message::DapConfigGenerated {
                ide_name: "VS Code".into(),
                path: std::path::PathBuf::from(".vscode/launch.json"),
                action: "Updated".into(),
            },
        );
        let status = state.dap_config_status.unwrap();
        assert_eq!(status.ide_name, "VS Code");
        assert_eq!(status.action, "Updated");
    }

    #[test]
    fn test_handle_dap_config_generated_skipped_action() {
        let mut state = test_state();
        let msg = Message::DapConfigGenerated {
            ide_name: "Helix".into(),
            path: std::path::PathBuf::from(".config/helix/languages.toml"),
            action: "Skipped: file already up to date".into(),
        };
        handle_dap_message(&mut state, &msg);
        let status = state.dap_config_status.unwrap();
        assert_eq!(status.action, "Skipped: file already up to date");
    }
}
