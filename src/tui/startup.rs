//! Startup and cleanup functions for the TUI runner
//!
//! Contains initialization logic and graceful shutdown handling:
//! - `startup_flutter`: Auto-start or device selector setup
//! - `cleanup_sessions`: Session shutdown and process cleanup

use std::path::Path;

use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::{AppState, UiMode};
use crate::app::UpdateAction;
use crate::config;
use crate::core::LogSource;
use crate::daemon::devices;

use super::actions::SessionTaskMap;
use super::render;
use super::spawn;

/// Handle auto-start or show device selector
///
/// Returns `Some(UpdateAction)` if a session should be spawned immediately,
/// or `None` if the device selector is being shown.
pub async fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    if settings.behavior.auto_start {
        // Auto-start mode: try to start with auto_start configs
        state.ui_mode = UiMode::Loading;

        // Load launch configs and start auto-start sessions
        let fdemon_configs = config::load_launch_configs(project_path);
        let vscode_configs = config::load_vscode_configs(project_path);

        let all_configs: Vec<_> = fdemon_configs.into_iter().chain(vscode_configs).collect();
        let auto_configs = config::get_auto_start_configs(&all_configs);

        if auto_configs.is_empty() {
            // No auto-start configs, fall back to showing device selector
            state.ui_mode = UiMode::DeviceSelector;
            state.device_selector.show_loading();
            spawn::spawn_device_discovery(msg_tx);
            None
        } else {
            // Start first auto-start config
            let first_config = auto_configs[0].config.clone();

            // Discover devices first
            match devices::discover_devices().await {
                Ok(result) => {
                    let device = if first_config.device == "auto" {
                        result.devices.first().cloned()
                    } else {
                        devices::find_device(&result.devices, &first_config.device).cloned()
                    };

                    if let Some(device) = device {
                        // Create session via SessionManager (like normal device selection)
                        match state
                            .session_manager
                            .create_session_with_config(&device, first_config.clone())
                        {
                            Ok(session_id) => {
                                state.ui_mode = UiMode::Normal;

                                // Return action to spawn the session
                                Some(UpdateAction::SpawnSession {
                                    session_id,
                                    device,
                                    config: Some(Box::new(first_config)),
                                })
                            }
                            Err(e) => {
                                // Log error to session manager's selected session if available
                                if let Some(session) = state.session_manager.selected_mut() {
                                    session.session.log_error(
                                        LogSource::App,
                                        format!("Failed to create session: {}", e),
                                    );
                                }
                                state.ui_mode = UiMode::DeviceSelector;
                                state.device_selector.show_loading();
                                spawn::spawn_device_discovery(msg_tx);
                                None
                            }
                        }
                    } else {
                        state.ui_mode = UiMode::DeviceSelector;
                        state.device_selector.set_devices(result.devices);
                        None
                    }
                }
                Err(e) => {
                    state.ui_mode = UiMode::DeviceSelector;
                    state.device_selector.set_error(e.to_string());
                    None
                }
            }
        }
    } else {
        // Manual start mode: show device selector first
        state.ui_mode = UiMode::DeviceSelector;
        state.device_selector.show_loading();
        spawn::spawn_device_discovery(msg_tx);
        None
    }
}

/// Cleanup sessions on shutdown
///
/// All sessions are managed through the session task system.
/// This function signals all background tasks to shut down and waits for them.
pub async fn cleanup_sessions(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    session_tasks: SessionTaskMap,
    shutdown_tx: watch::Sender<bool>,
) {
    // Collect all session tasks and wait for them
    let tasks: Vec<(SessionId, tokio::task::JoinHandle<()>)> = {
        let mut guard = session_tasks.lock().await;
        guard.drain().collect()
    };

    if !tasks.is_empty() {
        // Log to selected session if available
        if let Some(session) = state.session_manager.selected_mut() {
            session.session.log_info(
                LogSource::App,
                format!("Shutting down {} Flutter session(s)...", tasks.len()),
            );
        }

        // Draw one more frame to show shutdown message
        let _ = term.draw(|frame| render::view(frame, state));

        // Signal all background tasks to shut down
        info!(
            "Sending shutdown signal to {} session task(s)...",
            tasks.len()
        );
        let _ = shutdown_tx.send(true);

        // Wait for all tasks with reduced timeout (2s instead of 5s for faster shutdown)
        for (session_id, handle) in tasks {
            info!("Waiting for session {} to complete shutdown...", session_id);
            match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
                Ok(Ok(())) => info!("Session {} completed cleanly", session_id),
                Ok(Err(e)) => warn!("Session {} task panicked: {}", session_id, e),
                Err(_) => warn!(
                    "Timeout waiting for session {}, may be orphaned",
                    session_id
                ),
            }
        }
    }
}
