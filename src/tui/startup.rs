//! Startup and cleanup functions for the TUI runner
//!
//! Contains initialization logic and graceful shutdown handling:
//! - `startup_flutter`: Auto-start or device selector setup
//! - `cleanup_sessions`: Session shutdown and process cleanup

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};
use tracing::{error, info, warn};

use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::{AppState, UiMode};
use crate::config;
use crate::core::{AppPhase, DaemonEvent, LogSource};
use crate::daemon::{devices, CommandSender, FlutterProcess, RequestTracker};

use super::actions::SessionTaskMap;
use super::{render, spawn};

/// Handle auto-start or show device selector
pub async fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
    daemon_tx: mpsc::Sender<DaemonEvent>,
    msg_tx: mpsc::Sender<Message>,
) -> (Option<FlutterProcess>, Option<CommandSender>) {
    if settings.behavior.auto_start {
        // Auto-start mode: try to start with auto_start configs
        state.ui_mode = UiMode::Loading;
        state.log_info(LogSource::App, "Auto-start mode enabled");

        // Load launch configs and start auto-start sessions
        let fdemon_configs = config::load_launch_configs(project_path);
        let vscode_configs = config::load_vscode_configs(project_path);

        let all_configs: Vec<_> = fdemon_configs.into_iter().chain(vscode_configs).collect();
        let auto_configs = config::get_auto_start_configs(&all_configs);

        if auto_configs.is_empty() {
            // No auto-start configs, fall back to showing device selector
            state.log_info(
                LogSource::App,
                "No auto-start configs found, showing device selector",
            );
            state.ui_mode = UiMode::DeviceSelector;
            state.device_selector.show_loading();
            spawn::spawn_device_discovery(msg_tx);
            (None, None)
        } else {
            // Start first auto-start config (for now, single-session backward compatibility)
            let first_config = &auto_configs[0].config;
            state.log_info(
                LogSource::App,
                format!("Starting auto-start config: {}", first_config.name),
            );

            // Discover devices first
            match devices::discover_devices().await {
                Ok(result) => {
                    let device = if first_config.device == "auto" {
                        result.devices.first().cloned()
                    } else {
                        devices::find_device(&result.devices, &first_config.device).cloned()
                    };

                    if let Some(device) = device {
                        match FlutterProcess::spawn_with_config(
                            project_path,
                            &device.id,
                            first_config,
                            daemon_tx,
                        )
                        .await
                        {
                            Ok(p) => {
                                state.log_info(
                                    LogSource::App,
                                    format!(
                                        "Flutter process started on {} (PID: {:?})",
                                        device.name,
                                        p.id()
                                    ),
                                );
                                state.device_name = Some(device.name.clone());
                                state.platform = Some(device.platform.clone());
                                state.phase = AppPhase::Running;
                                state.ui_mode = UiMode::Normal;
                                let request_tracker = Arc::new(RequestTracker::default());
                                let sender = p.command_sender(request_tracker);
                                (Some(p), Some(sender))
                            }
                            Err(e) => {
                                state.log_error(
                                    LogSource::App,
                                    format!("Failed to start Flutter: {}", e),
                                );
                                state.ui_mode = UiMode::DeviceSelector;
                                state.device_selector.show_loading();
                                spawn::spawn_device_discovery(msg_tx);
                                (None, None)
                            }
                        }
                    } else {
                        state.log_error(
                            LogSource::App,
                            format!("No device matches specifier: {}", first_config.device),
                        );
                        state.ui_mode = UiMode::DeviceSelector;
                        state.device_selector.set_devices(result.devices);
                        (None, None)
                    }
                }
                Err(e) => {
                    state.log_error(LogSource::App, format!("Device discovery failed: {}", e));
                    state.ui_mode = UiMode::DeviceSelector;
                    state.device_selector.set_error(e.to_string());
                    (None, None)
                }
            }
        }
    } else {
        // Manual start mode: show device selector first
        state.log_info(LogSource::App, "Manual start mode - select a device");
        state.ui_mode = UiMode::DeviceSelector;
        state.device_selector.show_loading();
        spawn::spawn_device_discovery(msg_tx);
        (None, None)
    }
}

/// Cleanup sessions on shutdown
pub async fn cleanup_sessions(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    flutter: Option<FlutterProcess>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_tasks: SessionTaskMap,
    shutdown_tx: watch::Sender<bool>,
) {
    if let Some(mut p) = flutter {
        // Auto-start mode: we own the process directly
        state.log_info(LogSource::App, "Shutting down Flutter process...");

        // Draw one more frame to show shutdown message
        let _ = term.draw(|frame| render::view(frame, state));

        // Get the command sender for shutdown
        let sender_guard = cmd_sender.lock().await;
        if let Err(e) = p
            .shutdown(state.current_app_id.as_deref(), sender_guard.as_ref())
            .await
        {
            error!("Error during Flutter shutdown: {}", e);
        } else {
            info!("Flutter process shut down cleanly");
        }
    } else {
        // SpawnSession mode: processes are owned by background tasks
        // Collect all session tasks and wait for them
        let tasks: Vec<(SessionId, tokio::task::JoinHandle<()>)> = {
            let mut guard = session_tasks.lock().await;
            guard.drain().collect()
        };

        if !tasks.is_empty() {
            state.log_info(
                LogSource::App,
                format!("Shutting down {} Flutter session(s)...", tasks.len()),
            );

            // Draw one more frame to show shutdown message
            let _ = term.draw(|frame| render::view(frame, state));

            // Signal all background tasks to shut down
            info!(
                "Sending shutdown signal to {} session task(s)...",
                tasks.len()
            );
            let _ = shutdown_tx.send(true);

            // Wait for all tasks with timeout
            for (session_id, handle) in tasks {
                info!("Waiting for session {} to complete shutdown...", session_id);
                match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
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
}
