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
use crate::config::{
    self, get_first_auto_start, get_first_config, load_all_configs, load_last_selection,
    validate_last_selection, LaunchConfig, LoadedConfigs, ValidatedSelection,
};
use crate::core::LogSource;
use crate::daemon::{devices, Device};

use super::actions::SessionTaskMap;
use super::render;
use super::spawn;

/// Helper to animate loading screen during an async operation
///
/// Uses `tokio::select!` to tick the loading animation at ~10fps (100ms intervals)
/// while waiting for the future to complete.
async fn animate_during_async<T, F>(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    future: F,
) -> T
where
    F: std::future::Future<Output = T>,
{
    use tokio::time::{interval, Duration};

    tokio::pin!(future);
    let mut tick_interval = interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            biased;  // Prioritize completion over animation
            result = &mut future => {
                return result;
            }
            _ = tick_interval.tick() => {
                state.tick_loading_animation();
                let _ = term.draw(|frame| render::view(frame, state));
            }
        }
    }
}

/// Handle auto-start or show startup dialog
///
/// Returns `Some(UpdateAction)` if a session should be spawned immediately,
/// or `None` if the device selector/startup dialog is being shown.
pub async fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    term: &mut ratatui::DefaultTerminal,
) -> Option<UpdateAction> {
    // Load all configs upfront (needed for both paths)
    let configs = load_all_configs(project_path);

    if settings.behavior.auto_start {
        auto_start_session(state, &configs, project_path, msg_tx, term).await
    } else {
        show_startup_dialog(state, configs, msg_tx)
    }
}

/// Auto-start mode: try to launch immediately based on preferences
async fn auto_start_session(
    state: &mut AppState,
    configs: &LoadedConfigs,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    term: &mut ratatui::DefaultTerminal,
) -> Option<UpdateAction> {
    // Loading state is already set in runner.rs, just ensure UI mode is Loading
    state.ui_mode = UiMode::Loading;

    // Step 1: Check settings.local.toml for saved selection
    if let Some(selection) = load_last_selection(project_path) {
        // Update loading message (Task 08d)
        state.update_loading_message("Detecting devices...");

        // Discover devices with animation (Task 09c)
        let discovery = devices::discover_devices();
        let result = animate_during_async(state, term, discovery).await;

        match result {
            Ok(discovery_result) => {
                // Cache devices globally (Task 08e)
                state.set_device_cache(discovery_result.devices.clone());

                if let Some(validated) =
                    validate_last_selection(&selection, configs, &discovery_result.devices)
                {
                    return launch_with_validated_selection(
                        state,
                        configs,
                        &discovery_result.devices,
                        validated,
                        project_path,
                    );
                }
                // Selection invalid, fall through to auto_start config
                return try_auto_start_config(
                    state,
                    configs,
                    discovery_result.devices,
                    project_path,
                    msg_tx,
                );
            }
            Err(e) => {
                // Device discovery failed, show startup dialog with error
                state.show_startup_dialog(configs.clone());
                state.startup_dialog_state.set_error(e.to_string());
                return None;
            }
        }
    }

    // Step 2: No saved selection, discover devices and find config
    // Update loading message (Task 08d)
    state.update_loading_message("Detecting devices...");

    // Discover devices with animation (Task 09c)
    let discovery = devices::discover_devices();
    let result = animate_during_async(state, term, discovery).await;

    match result {
        Ok(discovery_result) => {
            // Cache devices globally (Task 08e)
            state.set_device_cache(discovery_result.devices.clone());

            state.update_loading_message("Preparing launch...");
            try_auto_start_config(
                state,
                configs,
                discovery_result.devices,
                project_path,
                msg_tx,
            )
        }
        Err(e) => {
            // Device discovery failed, show startup dialog with error
            state.clear_loading();
            state.show_startup_dialog(configs.clone());
            state.startup_dialog_state.set_error(e.to_string());
            None
        }
    }
}

/// Try to find and use an auto_start config
fn try_auto_start_config(
    state: &mut AppState,
    configs: &LoadedConfigs,
    devices: Vec<Device>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    // Find config: auto_start > first config > bare run
    let config = get_first_auto_start(configs).or_else(|| get_first_config(configs));

    if let Some(config) = config {
        // Find matching device
        let device = if config.config.device == "auto" {
            devices.first().cloned()
        } else {
            devices::find_device(&devices, &config.config.device)
                .cloned()
                .or_else(|| devices.first().cloned())
        };

        if let Some(device) = device {
            return launch_session(state, Some(&config.config), &device, project_path);
        }
    }

    // No config with matching device, try bare run with first device
    if let Some(device) = devices.first() {
        return launch_session(state, None, device, project_path);
    }

    // No devices at all, show startup dialog
    state.show_startup_dialog(configs.clone());
    spawn::spawn_device_discovery(msg_tx);
    None
}

/// Launch with validated selection from settings.local.toml
fn launch_with_validated_selection(
    state: &mut AppState,
    configs: &LoadedConfigs,
    devices: &[Device],
    validated: ValidatedSelection,
    project_path: &Path,
) -> Option<UpdateAction> {
    let config = validated.config_idx.and_then(|i| configs.configs.get(i));
    let device = validated.device_idx.and_then(|i| devices.get(i))?;

    launch_session(state, config.map(|c| &c.config), device, project_path)
}

/// Launch a session with optional config
fn launch_session(
    state: &mut AppState,
    config: Option<&LaunchConfig>,
    device: &Device,
    project_path: &Path,
) -> Option<UpdateAction> {
    // Create session via SessionManager
    let result = if let Some(cfg) = config {
        state
            .session_manager
            .create_session_with_config(device, cfg.clone())
    } else {
        state.session_manager.create_session(device)
    };

    match result {
        Ok(session_id) => {
            // Clear loading state and return to normal UI (Task 08d)
            state.clear_loading();
            state.ui_mode = UiMode::Normal;

            // Save selection for next time
            let _ = config::save_last_selection(
                project_path,
                config.map(|c| c.name.as_str()),
                Some(&device.id),
            );

            Some(UpdateAction::SpawnSession {
                session_id,
                device: device.clone(),
                config: config.map(|c| Box::new(c.clone())),
            })
        }
        Err(e) => {
            if let Some(session) = state.session_manager.selected_mut() {
                session
                    .session
                    .log_error(LogSource::App, format!("Failed to create session: {}", e));
            }
            None
        }
    }
}

/// Show startup dialog (manual mode)
fn show_startup_dialog(
    state: &mut AppState,
    configs: LoadedConfigs,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    state.show_startup_dialog(configs);
    spawn::spawn_device_discovery(msg_tx);
    None
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
