//! Session lifecycle handlers
//!
//! Handles session creation, switching, and closing.

use crate::session::SessionId;
use crate::state::{AppState, DevToolsPanel, UiMode};
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

        // Flush any watcher errors that arrived before this session existed
        for err in state.pending_watcher_errors.drain(..) {
            handle.session.log_error(LogSource::Watcher, err);
        }
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
        return maybe_start_monitoring_for_selected_session(state);
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
        return maybe_start_monitoring_for_selected_session(state);
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
        return maybe_start_monitoring_for_selected_session(state);
    }
    UpdateResult::none()
}

/// Start performance monitoring for the newly selected session if DevTools is
/// active, the VM is connected, and no polling task is already running.
///
/// This handles the edge case where the user switches sessions while in
/// DevTools — the new session may never have had monitoring started (it was
/// connected before the user first opened DevTools) so we must start it now.
///
/// Uses `session.vm_connected` (the session's own connection flag) rather than
/// `devtools_view_state.connection_status` because the view state is reset to
/// `Disconnected` by `DevToolsViewState::reset()` during session switching.
fn maybe_start_monitoring_for_selected_session(state: &mut AppState) -> UpdateResult {
    if state.ui_mode != UiMode::DevTools {
        return UpdateResult::none();
    }

    let needs_start = if let Some(handle) = state.session_manager.selected() {
        handle.perf_shutdown_tx.is_none() && handle.session.vm_connected
    } else {
        false
    };

    if !needs_start {
        // Task already running — unpause it for the newly selected session.
        // Without this, switching back to a session whose perf task was paused
        // (by a prior DevTools exit) would leave polling paused until the user
        // exits and re-enters DevTools.
        if let Some(handle) = state.session_manager.selected() {
            if let Some(ref tx) = handle.perf_pause_tx {
                let _ = tx.send(false); // unpause
            }
            // Also unpause allocation polling if the Performance panel is active.
            if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
                if let Some(ref tx) = handle.alloc_pause_tx {
                    let _ = tx.send(false); // unpause
                }
            }
            // Unpause network polling if the Network panel is active.
            if state.devtools_view_state.active_panel == DevToolsPanel::Network {
                if let Some(ref tx) = handle.network_pause_tx {
                    let _ = tx.send(false); // unpause
                }
            }
        }
        return UpdateResult::none();
    }

    let session_id = state.session_manager.selected_id().unwrap();
    let performance_refresh_ms = state.settings.devtools.performance_refresh_ms;
    let allocation_profile_interval_ms = state.settings.devtools.allocation_profile_interval_ms;
    let mode = state
        .session_manager
        .selected()
        .and_then(|h| h.session.launch_config.as_ref())
        .map(|c| c.mode)
        .unwrap_or(crate::config::FlutterMode::Debug);

    UpdateResult::action(UpdateAction::StartPerformanceMonitoring {
        session_id,
        handle: None, // hydrated by process.rs
        performance_refresh_ms,
        allocation_profile_interval_ms,
        mode,
    })
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

            // Shut down the native log capture task (if running).
            handle.shutdown_native_logs();
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
            // Trigger device discovery (only if SDK is available)
            if let Some(flutter) = state.flutter_executable() {
                return UpdateResult::action(UpdateAction::DiscoverDevices { flutter });
            }
        }
    }
    UpdateResult::none()
}
