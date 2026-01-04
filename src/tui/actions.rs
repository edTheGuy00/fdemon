//! Action handlers: UpdateAction dispatch and background task spawning

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};
use tracing::{error, info, warn};

use crate::app::handler::Task;
use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::UpdateAction;
use crate::config::LaunchConfig;
use crate::core::DaemonEvent;
use crate::daemon::{
    protocol, CommandSender, DaemonCommand, DaemonMessage, Device, FlutterProcess, RequestTracker,
};

use super::spawn;

/// Convenience type alias for session task tracking
pub type SessionTaskMap = Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;

/// Execute an action by spawning a background task
pub fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_cmd_sender: Option<CommandSender>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) {
    match action {
        UpdateAction::SpawnTask(task) => {
            // Spawn async task for command execution
            // Prefer session-specific cmd_sender, fall back to global
            if let Some(sender) = session_cmd_sender {
                tokio::spawn(async move {
                    execute_task(task, msg_tx, Some(sender)).await;
                });
            } else {
                // Fall back to global cmd_sender (legacy mode)
                let cmd_sender_clone = cmd_sender.clone();
                tokio::spawn(async move {
                    let sender = cmd_sender_clone.lock().await.clone();
                    execute_task(task, msg_tx, sender).await;
                });
            }
        }

        UpdateAction::DiscoverDevices => {
            spawn::spawn_device_discovery(msg_tx);
        }

        UpdateAction::SpawnSession {
            session_id,
            device,
            config,
        } => {
            spawn_session(
                session_id,
                device,
                config,
                project_path,
                msg_tx,
                cmd_sender,
                session_tasks,
                shutdown_rx,
            );
        }

        UpdateAction::DiscoverEmulators => {
            spawn::spawn_emulator_discovery(msg_tx);
        }

        UpdateAction::LaunchEmulator { emulator_id } => {
            spawn::spawn_emulator_launch(msg_tx, emulator_id);
        }

        UpdateAction::LaunchIOSSimulator => {
            spawn::spawn_ios_simulator_launch(msg_tx);
        }
    }
}

/// Spawn a Flutter session for a device (multi-session mode)
#[allow(clippy::too_many_arguments)]
fn spawn_session(
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
) {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
    let cmd_sender_clone = cmd_sender.clone();
    let session_tasks_clone = session_tasks.clone();
    let mut shutdown_rx_clone = shutdown_rx.clone();
    let device_id = device.id.clone();
    let device_name = device.name.clone();
    let device_platform = device.platform.clone();

    let handle = tokio::spawn(async move {
        info!(
            "Spawning Flutter session {} on device: {} ({})",
            session_id, device_name, device_id
        );

        // Create event channel for this session
        let (daemon_tx, mut daemon_rx) = mpsc::channel::<DaemonEvent>(256);

        // Spawn the Flutter process
        let spawn_result = if let Some(cfg) = config {
            FlutterProcess::spawn_with_config(&project_path, &device_id, &cfg, daemon_tx).await
        } else {
            FlutterProcess::spawn_with_device(&project_path, &device_id, daemon_tx).await
        };

        match spawn_result {
            Ok(mut process) => {
                info!(
                    "Flutter process started for session {} (PID: {:?})",
                    session_id,
                    process.id()
                );

                // Create command sender for this session
                let request_tracker = Arc::new(RequestTracker::default());
                let session_sender = process.command_sender(request_tracker);

                // Send SessionProcessAttached to store cmd_sender in SessionHandle
                let _ = msg_tx_clone
                    .send(Message::SessionProcessAttached {
                        session_id,
                        cmd_sender: session_sender.clone(),
                    })
                    .await;

                // Also update legacy global cmd_sender for backward compatibility
                *cmd_sender_clone.lock().await = Some(session_sender.clone());

                // Send session started message
                let _ = msg_tx_clone
                    .send(Message::SessionStarted {
                        session_id,
                        device_id: device_id.clone(),
                        device_name: device_name.clone(),
                        platform: device_platform.clone(),
                        pid: process.id(),
                    })
                    .await;

                // Track app_id from events for shutdown
                let mut app_id: Option<String> = None;

                // Track if process has already exited (for fast shutdown path)
                let mut process_exited = false;

                // Forward daemon events to the main message channel
                // This runs until the process exits, main loop closes, or shutdown signal
                loop {
                    tokio::select! {
                        event = daemon_rx.recv() => {
                            match event {
                                Some(event) => {
                                    // Track exit events for fast shutdown
                                    if matches!(event, DaemonEvent::Exited { .. }) {
                                        process_exited = true;
                                    }

                                    // Capture app_id from stdout events
                                    if let DaemonEvent::Stdout(ref line) = event {
                                        if let Some(json) = protocol::strip_brackets(line) {
                                            if let Some(DaemonMessage::AppStart(app_start)) =
                                                DaemonMessage::parse(json)
                                            {
                                                app_id = Some(app_start.app_id.clone());
                                            }
                                        }
                                    }

                                    // Send event with session context for multi-session routing
                                    if msg_tx_clone
                                        .send(Message::SessionDaemon {
                                            session_id,
                                            event,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        // Main loop closed, need to shutdown
                                        break;
                                    }
                                }
                                None => {
                                    // Channel closed, process likely ended
                                    process_exited = true;
                                    break;
                                }
                            }
                        }
                        _ = shutdown_rx_clone.changed() => {
                            // Shutdown signal received
                            info!(
                                "Shutdown signal received, stopping session {}...",
                                session_id
                            );
                            break;
                        }
                    }
                }

                // Fast shutdown path: skip shutdown commands if we know process already exited
                if process_exited {
                    info!(
                        "Session {} process already exited, skipping shutdown commands",
                        session_id
                    );
                } else {
                    // Graceful shutdown when loop ends - use session's own sender
                    info!("Session {} ending, initiating shutdown...", session_id);
                    if let Err(e) = process
                        .shutdown(app_id.as_deref(), Some(&session_sender))
                        .await
                    {
                        warn!(
                            "Shutdown error for session {} (process may already be gone): {}",
                            session_id, e
                        );
                    }
                }

                // Clear the global command sender if it was ours
                // (only matters for legacy single-session compatibility)
                let mut guard = cmd_sender_clone.lock().await;
                *guard = None;
                drop(guard);
            }
            Err(e) => {
                error!(
                    "Failed to spawn Flutter process for session {}: {}",
                    session_id, e
                );
                let _ = msg_tx_clone
                    .send(Message::SessionSpawnFailed {
                        session_id,
                        device_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }

        // Remove this session's task from the tracking map
        session_tasks_clone.lock().await.remove(&session_id);
        info!("Session {} task removed from tracking", session_id);
    });

    // Store the handle with session_id as key (allows multiple concurrent sessions)
    if let Ok(mut guard) = session_tasks.try_lock() {
        guard.insert(session_id, handle);
        info!(
            "Session {} task added to tracking (total: {})",
            session_id,
            guard.len()
        );
    }
}

/// Execute a task and send completion message
pub async fn execute_task(
    task: Task,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) {
    let Some(sender) = cmd_sender else {
        // No command sender available
        let msg = match task {
            Task::Reload { .. } => Message::ReloadFailed {
                reason: "Flutter not running".to_string(),
            },
            Task::Restart { .. } => Message::RestartFailed {
                reason: "Flutter not running".to_string(),
            },
            Task::Stop { .. } => return, // Nothing to do
        };
        let _ = msg_tx.send(msg).await;
        return;
    };

    match task {
        Task::Reload { session_id, app_id } => {
            let start = std::time::Instant::now();
            info!(
                "Executing reload for session {} (app_id: {})",
                session_id, app_id
            );
            match sender.send(DaemonCommand::Reload { app_id }).await {
                Ok(response) => {
                    if response.success {
                        let time_ms = start.elapsed().as_millis() as u64;
                        let _ = msg_tx.send(Message::ReloadCompleted { time_ms }).await;
                    } else {
                        let _ = msg_tx
                            .send(Message::ReloadFailed {
                                reason: response
                                    .error
                                    .unwrap_or_else(|| "Unknown error".to_string()),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    let _ = msg_tx
                        .send(Message::ReloadFailed {
                            reason: e.to_string(),
                        })
                        .await;
                }
            }
        }
        Task::Restart { session_id, app_id } => {
            info!(
                "Executing restart for session {} (app_id: {})",
                session_id, app_id
            );
            match sender.send(DaemonCommand::Restart { app_id }).await {
                Ok(response) => {
                    if response.success {
                        let _ = msg_tx.send(Message::RestartCompleted).await;
                    } else {
                        let _ = msg_tx
                            .send(Message::RestartFailed {
                                reason: response
                                    .error
                                    .unwrap_or_else(|| "Unknown error".to_string()),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    let _ = msg_tx
                        .send(Message::RestartFailed {
                            reason: e.to_string(),
                        })
                        .await;
                }
            }
        }
        Task::Stop { session_id, app_id } => {
            info!(
                "Executing stop for session {} (app_id: {})",
                session_id, app_id
            );
            if let Err(e) = sender.send(DaemonCommand::Stop { app_id }).await {
                error!("Failed to stop app: {}", e);
            }
        }
    }
}
