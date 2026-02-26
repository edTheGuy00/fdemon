//! Flutter session lifecycle: process spawning, task execution, and watchdog.
//!
//! This module contains the two primary async helpers called by the action
//! dispatcher in `mod.rs`:
//!
//! - [`spawn_session`] — spawns a `FlutterProcess`, forwards daemon events to
//!   the TEA message loop, and manages a process watchdog.
//! - [`execute_task`] — sends a single daemon command (reload / restart / stop)
//!   and returns a completion message.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tracing::{error, info, warn};

use crate::config::LaunchConfig;
use crate::handler::Task;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_core::{DaemonEvent, DaemonMessage};
use fdemon_daemon::{CommandSender, DaemonCommand, Device, FlutterProcess, RequestTracker};

use super::SessionTaskMap;

/// Watchdog interval for detecting externally-killed Flutter processes (e.g. SIGKILL, OOM).
/// When stdout EOF does not occur, this ensures the session is marked exited within 5 seconds.
pub(super) const PROCESS_WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);

/// Spawn a Flutter session for a device (multi-session mode)
pub(super) fn spawn_session(
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
) {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
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
            // Build flutter args from config (conversion happens here in app layer)
            let args = cfg.build_flutter_args(&device_id);
            FlutterProcess::spawn_with_args(&project_path, args, daemon_tx).await
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

                // Set up watchdog to detect processes killed without stdout EOF (e.g. SIGKILL).
                // The first tick fires immediately; consume it so the first real check happens
                // after PROCESS_WATCHDOG_INTERVAL seconds, not at loop entry.
                let mut watchdog = tokio::time::interval(PROCESS_WATCHDOG_INTERVAL);
                watchdog.tick().await; // consume the immediate first tick

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
                                        if let Some(DaemonMessage::AppStart(app_start)) =
                                            fdemon_daemon::parse_daemon_message(line)
                                        {
                                            app_id = Some(app_start.app_id.clone());
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
                        _ = watchdog.tick() => {
                            // Periodically poll for process death that does not produce a
                            // stdout EOF (e.g. SIGKILL, OOM kill, frozen pipe).
                            // Guard: skip synthesis if daemon_rx already delivered the real
                            // Exited event to avoid a duplicate (and code: None overwrite).
                            if !process_exited && process.has_exited() {
                                info!(
                                    "Watchdog detected process exit for session {}",
                                    session_id
                                );
                                // Synthesize an exit event so the TEA layer transitions the
                                // session to Stopped, just as a normal EOF exit would.
                                let _ = msg_tx_clone
                                    .send(Message::SessionDaemon {
                                        session_id,
                                        event: DaemonEvent::Exited { code: None },
                                    })
                                    .await;
                                process_exited = true;
                                break;
                            }
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
        if let Ok(mut guard) = session_tasks_clone.lock() {
            guard.remove(&session_id);
            info!("Session {} task removed from tracking", session_id);
        } else {
            warn!(
                "Session {} task could not be removed from tracking (poisoned lock)",
                session_id
            );
        }
    });

    // Store the handle with session_id as key (allows multiple concurrent sessions)
    match session_tasks.lock() {
        Ok(mut guard) => {
            guard.insert(session_id, handle);
            info!(
                "Session {} task added to tracking (total: {})",
                session_id,
                guard.len()
            );
        }
        Err(e) => {
            warn!(
                "Session {} task handle could not be tracked (poisoned lock): {}",
                session_id, e
            );
        }
    }
}

/// Execute a task and send completion message
pub(super) async fn execute_task(
    task: Task,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) {
    let Some(sender) = cmd_sender else {
        // No command sender available - send session-specific failure
        let msg = match task {
            Task::Reload { session_id, .. } => Message::SessionReloadFailed {
                session_id,
                reason: "Flutter not running".to_string(),
            },
            Task::Restart { session_id, .. } => Message::SessionRestartFailed {
                session_id,
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
                        let _ = msg_tx
                            .send(Message::SessionReloadCompleted {
                                session_id,
                                time_ms,
                            })
                            .await;
                    } else {
                        let reason = response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string());
                        let _ = msg_tx
                            .send(Message::SessionReloadFailed { session_id, reason })
                            .await;
                    }
                }
                Err(e) => {
                    let reason = e.to_string();
                    let _ = msg_tx
                        .send(Message::SessionReloadFailed { session_id, reason })
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
                        let _ = msg_tx
                            .send(Message::SessionRestartCompleted { session_id })
                            .await;
                    } else {
                        let reason = response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string());
                        let _ = msg_tx
                            .send(Message::SessionRestartFailed { session_id, reason })
                            .await;
                    }
                }
                Err(e) => {
                    let reason = e.to_string();
                    let _ = msg_tx
                        .send(Message::SessionRestartFailed { session_id, reason })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchdog_interval_is_reasonable() {
        assert_eq!(
            PROCESS_WATCHDOG_INTERVAL,
            Duration::from_secs(5),
            "watchdog interval should be 5 seconds"
        );
    }
}
