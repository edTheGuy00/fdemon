//! Session lifecycle handlers
//!
//! Handles session creation, switching, and closing.

use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::{AppPhase, LogSource};
use fdemon_daemon::CommandSender;

use super::{UpdateAction, UpdateResult};

/// Handle session started message
pub fn handle_session_started(
    state: &mut AppState,
    session_id: SessionId,
    device_name: String,
    pid: Option<u32>,
) -> UpdateResult {
    // Update session-specific state
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Running;
        handle.session.started_at = Some(chrono::Local::now());

        // Log to session-specific logs
        handle.session.log_info(
            LogSource::App,
            format!(
                "Flutter process started on {} (PID: {})",
                device_name,
                pid.map_or("unknown".to_string(), |p| p.to_string())
            ),
        );
    }

    UpdateResult::none()
}

/// Handle session spawn failed message
pub fn handle_session_spawn_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    // Update session-specific state before removal
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Stopped;
        handle.session.log_error(
            LogSource::App,
            format!("Failed to start session: {}", error),
        );
    }

    tracing::error!("Failed to start session {}: {}", session_id, error);

    // Remove the failed session from manager
    state.session_manager.remove_session(session_id);

    // Show new session dialog again so user can retry
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_new_session_dialog(configs);
    UpdateResult::none()
}

/// Handle session process attached message
pub fn handle_session_process_attached(
    state: &mut AppState,
    session_id: SessionId,
    cmd_sender: CommandSender,
) -> UpdateResult {
    // Attach the command sender to the session
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.cmd_sender = Some(cmd_sender);
        tracing::debug!("Command sender attached to session {}", session_id);
    } else {
        tracing::error!("Cannot attach cmd_sender: session {} not found", session_id);
    }
    UpdateResult::none()
}

/// Handle select session by index message
pub fn handle_select_session_by_index(state: &mut AppState, index: usize) -> UpdateResult {
    let old_index = state.session_manager.selected_index();
    // Silently ignore if index is out of range
    state.session_manager.select_by_index(index);
    if state.session_manager.selected_index() != old_index {
        state.devtools_view_state.reset();
    }
    UpdateResult::none()
}

/// Handle next session message
pub fn handle_next_session(state: &mut AppState) -> UpdateResult {
    let old_id = state.session_manager.selected_id();
    state.session_manager.select_next();
    let new_id = state.session_manager.selected_id();
    if old_id != new_id {
        state.devtools_view_state.reset();
    }
    UpdateResult::none()
}

/// Handle previous session message
pub fn handle_previous_session(state: &mut AppState) -> UpdateResult {
    let old_id = state.session_manager.selected_id();
    state.session_manager.select_previous();
    let new_id = state.session_manager.selected_id();
    if old_id != new_id {
        state.devtools_view_state.reset();
    }
    UpdateResult::none()
}

/// Handle close current session message
pub fn handle_close_current_session(state: &mut AppState) -> UpdateResult {
    // If there's only one session (or none), treat 'x' as quit request
    if state.session_manager.len() <= 1 {
        state.request_quit();
        return UpdateResult::none();
    }

    if let Some(current_session_id) = state.session_manager.selected_id() {
        // Check if session has a running app and cmd_sender
        let session_info = state.session_manager.get(current_session_id).and_then(|h| {
            h.session
                .app_id
                .clone()
                .map(|app_id| (app_id, h.cmd_sender.clone()))
        });

        // Signal VM Service and performance monitoring shutdown BEFORE removing
        // the session, mirroring the pattern in VmServiceDisconnected handler.
        if let Some(handle) = state.session_manager.get_mut(current_session_id) {
            if let Some(shutdown_tx) = handle.vm_shutdown_tx.take() {
                let _ = shutdown_tx.send(true);
                tracing::info!(
                    "Sent VM Service shutdown signal on session close for session {}",
                    current_session_id
                );
            }
            // Abort and signal the performance polling task to stop.
            if let Some(h) = handle.perf_task_handle.take() {
                h.abort();
            }
            if let Some(tx) = handle.perf_shutdown_tx.take() {
                let _ = tx.send(true);
                tracing::info!(
                    "Sent perf shutdown signal on session close for session {}",
                    current_session_id
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
                    "Sent network shutdown signal on session close for session {}",
                    current_session_id
                );
            }
        }

        if let Some((app_id, cmd_sender_opt)) = session_info {
            tracing::info!(
                "Closing session {} (app: {})...",
                current_session_id,
                app_id
            );

            // Send stop command if we have a cmd_sender
            if let Some(cmd_sender) = cmd_sender_opt {
                // Spawn async task to stop the app
                let app_id_clone = app_id.clone();
                tokio::spawn(async move {
                    let _ = cmd_sender
                        .send(fdemon_daemon::DaemonCommand::Stop {
                            app_id: app_id_clone,
                        })
                        .await;
                });
            }

            // Remove the session from the manager
            state.session_manager.remove_session(current_session_id);
        } else {
            // No running app, just remove the session
            state.session_manager.remove_session(current_session_id);
        }

        // If no sessions left after removal, show new session dialog
        if state.session_manager.is_empty() {
            let configs = crate::config::load_all_configs(&state.project_path);
            state.show_new_session_dialog(configs);
            // Trigger device discovery
            return UpdateResult::action(UpdateAction::DiscoverDevices);
        }
    }
    UpdateResult::none()
}
