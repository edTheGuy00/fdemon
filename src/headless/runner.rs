//! Headless mode runner - main event loop without TUI
//!
//! This module implements the headless (non-TUI) event loop for fdemon.
//! It processes daemon events and emits JSON events to stdout for E2E testing.

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};
use tracing::{error, info, warn};

use crate::app::actions::SessionTaskMap;
use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::AppState;
use crate::app::UpdateAction;
use crate::common::prelude::*;
use crate::config::{self, LaunchConfig};
use crate::core::DaemonEvent;
use crate::daemon::{devices, protocol, DaemonMessage, Device, FlutterProcess, RequestTracker};
use crate::watcher::{FileWatcher, WatcherConfig, WatcherEvent};

use super::HeadlessEvent;

/// Run in headless mode - output JSON events instead of TUI
pub async fn run_headless(project_path: &Path) -> Result<()> {
    // Initialize error handling
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;

    // Initialize logging
    crate::common::logging::init()?;

    info!("═══════════════════════════════════════════════════════");
    info!("Flutter Demon starting in HEADLESS mode");
    info!("Project: {}", project_path.display());
    info!("═══════════════════════════════════════════════════════");

    // Initialize .fdemon directory and gitignore
    if let Err(e) = config::init_fdemon_directory(project_path) {
        warn!("Failed to initialize .fdemon directory: {}", e);
        // Non-fatal - continue with defaults
    }

    // Load configuration
    let settings = config::load_settings(project_path);
    info!(
        "Loaded settings: auto_start={}",
        settings.behavior.auto_start
    );

    // Create initial state with settings
    let mut state = AppState::with_settings(project_path.to_path_buf(), settings.clone());

    // Create unified message channel
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);

    // Per-session task handles (SessionId is u64)
    let session_tasks: SessionTaskMap =
        std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));

    // Shutdown signal for background tasks
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Spawn stdin reader for commands (r = reload, q = quit)
    let stdin_tx = msg_tx.clone();
    std::thread::spawn(move || {
        spawn_stdin_reader_blocking(stdin_tx);
    });

    // Spawn signal handler for SIGINT/SIGTERM
    spawn_signal_handler(msg_tx.clone());

    // Auto-start: discover devices and spawn session
    // In headless mode, always auto-start regardless of config setting
    info!("Discovering devices for headless auto-start...");
    let startup_action = headless_auto_start(&mut state, project_path, msg_tx.clone()).await;

    // If we got an action, spawn the session
    if let Some(action) = startup_action {
        handle_headless_action(
            action,
            msg_tx.clone(),
            session_tasks.clone(),
            shutdown_rx.clone(),
            project_path,
        );
    }

    // Start file watcher for auto-reload
    let mut file_watcher = FileWatcher::new(
        project_path.to_path_buf(),
        WatcherConfig::new()
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload),
    );

    // Create watcher-specific channel
    let (watcher_tx, mut watcher_rx) = mpsc::channel::<WatcherEvent>(32);

    if let Err(e) = file_watcher.start(watcher_tx) {
        warn!("Failed to start file watcher: {}", e);
        HeadlessEvent::error(format!("Watcher failed: {}", e), false).emit();
    }

    // Bridge watcher events to app messages
    let watcher_msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = watcher_rx.recv().await {
            let msg = match event {
                WatcherEvent::AutoReloadTriggered => Message::AutoReloadTriggered,
                WatcherEvent::FilesChanged { count } => Message::FilesChanged { count },
                WatcherEvent::Error { message } => Message::WatcherError { message },
            };
            let _ = watcher_msg_tx.send(msg).await;
        }
    });

    // Main event loop
    let result = headless_event_loop(
        &mut state,
        &mut msg_rx,
        &msg_tx,
        &session_tasks,
        &shutdown_rx,
        project_path,
    )
    .await;

    // Stop file watcher
    file_watcher.stop();

    // Cleanup sessions
    info!("Shutting down sessions...");
    let _ = shutdown_tx.send(true);

    // Wait for tasks to finish
    let tasks = session_tasks.lock().await;
    for (session_id, _handle) in tasks.iter() {
        info!("Waiting for session {} to finish", session_id);
        // Note: we can't await here because we hold the lock
        // In production, we'd need to collect handles first
    }
    drop(tasks);

    info!("Flutter Demon headless mode exiting");
    result
}

/// Main headless event loop
async fn headless_event_loop(
    state: &mut AppState,
    msg_rx: &mut mpsc::Receiver<Message>,
    msg_tx: &mpsc::Sender<Message>,
    session_tasks: &SessionTaskMap,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) -> Result<()> {
    loop {
        // Check for shutdown
        if state.should_quit() {
            info!("Quit requested");
            break;
        }

        // Wait for next message
        match msg_rx.recv().await {
            Some(msg) => {
                // Process message and emit events
                process_headless_message(
                    state,
                    msg,
                    msg_tx,
                    session_tasks,
                    shutdown_rx,
                    project_path,
                )
                .await;
            }
            None => {
                // Channel closed
                warn!("Message channel closed");
                break;
            }
        }
    }

    Ok(())
}

/// Process a message in headless mode and emit appropriate events
async fn process_headless_message(
    state: &mut AppState,
    msg: Message,
    msg_tx: &mpsc::Sender<Message>,
    session_tasks: &SessionTaskMap,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) {
    use crate::app::process;

    // Log the message for debugging
    info!("Processing message: {:?}", msg);

    // Emit events based on message type before processing
    emit_pre_message_events(state, &msg);

    // Use the existing message processor (it's state management, no rendering)
    process::process_message(state, msg, msg_tx, session_tasks, shutdown_rx, project_path);

    // Flush pending logs
    state.session_manager.flush_all_pending_logs();

    // Emit events based on state changes after processing
    emit_post_message_events(state);
}

/// Emit events before message processing
fn emit_pre_message_events(_state: &AppState, msg: &Message) {
    if let Message::HotReload = msg {
        if let Some(session_id) = get_current_session_id(_state) {
            HeadlessEvent::hot_reload_started(&session_id).emit();
        }
    }
}

/// Emit events after message processing based on state changes
fn emit_post_message_events(state: &AppState) {
    // Emit log events for new logs
    // Note: This is a simplified version. In a full implementation,
    // we'd track which logs have been emitted already.
    if let Some(session) = state.session_manager.selected() {
        // Get the last few logs (we'd ideally track the last emitted index)
        for log in session.session.logs.iter().rev().take(1) {
            // Convert LogLevel to string
            let level_str = match log.level {
                crate::core::LogLevel::Debug => "debug",
                crate::core::LogLevel::Info => "info",
                crate::core::LogLevel::Warning => "warning",
                crate::core::LogLevel::Error => "error",
            };
            HeadlessEvent::log(
                level_str,
                log.message.clone(),
                Some(session.session.id.to_string()),
            )
            .emit();
        }
    }
}

/// Get current session ID if available
fn get_current_session_id(state: &AppState) -> Option<String> {
    state
        .session_manager
        .selected()
        .map(|s| s.session.id.to_string())
}

/// Spawn stdin reader task that sends commands to message channel (blocking version)
fn spawn_stdin_reader_blocking(msg_tx: mpsc::Sender<Message>) {
    use std::io::BufRead;

    let stdin = std::io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        match line {
            Ok(line) => {
                let trimmed = line.trim();
                match trimmed {
                    "r" | "reload" => {
                        info!("Stdin: hot reload requested");
                        let _ = msg_tx.blocking_send(Message::HotReload);
                    }
                    "R" | "restart" => {
                        info!("Stdin: hot restart requested");
                        let _ = msg_tx.blocking_send(Message::HotRestart);
                    }
                    "q" | "quit" => {
                        info!("Stdin: quit requested");
                        let _ = msg_tx.blocking_send(Message::Quit);
                        break;
                    }
                    "" => {
                        // Ignore empty lines
                    }
                    _ => {
                        warn!("Unknown stdin command: {}", trimmed);
                    }
                }
            }
            Err(e) => {
                error!("Failed to read stdin: {}", e);
                break;
            }
        }
    }

    info!("Stdin reader exiting");
}

/// Spawn signal handler for SIGINT/SIGTERM
fn spawn_signal_handler(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        use tokio::signal;

        #[cfg(unix)]
        {
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                .expect("Failed to create SIGINT handler");
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to create SIGTERM handler");

            tokio::select! {
                _ = sigint.recv() => {
                    info!("Received SIGINT");
                    HeadlessEvent::error("Received SIGINT".to_string(), false).emit();
                    let _ = msg_tx.send(Message::Quit).await;
                }
                _ = sigterm.recv() => {
                    info!("Received SIGTERM");
                    HeadlessEvent::error("Received SIGTERM".to_string(), false).emit();
                    let _ = msg_tx.send(Message::Quit).await;
                }
            }
        }

        #[cfg(not(unix))]
        {
            // Windows: just handle Ctrl+C
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            info!("Received Ctrl+C");
            HeadlessEvent::error("Received Ctrl+C".to_string(), false).emit();
            let _ = msg_tx.send(Message::Quit).await;
        }
    });
}

/// Auto-start in headless mode: discover devices and create session
async fn headless_auto_start(
    state: &mut AppState,
    _project_path: &Path,
    _msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    // Discover devices
    info!("Discovering devices...");
    match devices::discover_devices().await {
        Ok(result) => {
            info!("Found {} device(s)", result.devices.len());

            // Emit device_detected events for each device
            for device in &result.devices {
                HeadlessEvent::device_detected(&device.id, &device.name, &device.platform).emit();
            }

            // Cache devices in state
            state.set_device_cache(result.devices.clone());

            // Pick first device for auto-start
            if let Some(device) = result.devices.first() {
                info!("Auto-starting with device: {} ({})", device.name, device.id);

                // Create session via SessionManager
                match state.session_manager.create_session(device) {
                    Ok(session_id) => {
                        info!("Created session {}", session_id);

                        // Emit session_created event
                        HeadlessEvent::session_created(&session_id.to_string(), &device.name)
                            .emit();

                        Some(UpdateAction::SpawnSession {
                            session_id,
                            device: device.clone(),
                            config: None,
                        })
                    }
                    Err(e) => {
                        error!("Failed to create session: {}", e);
                        HeadlessEvent::error(format!("Failed to create session: {}", e), true)
                            .emit();
                        None
                    }
                }
            } else {
                error!("No devices found");
                HeadlessEvent::error("No devices found".to_string(), true).emit();
                None
            }
        }
        Err(e) => {
            error!("Device discovery failed: {}", e);
            HeadlessEvent::error(format!("Device discovery failed: {}", e), true).emit();
            None
        }
    }
}

/// Handle an UpdateAction in headless mode
fn handle_headless_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) {
    match action {
        UpdateAction::SpawnSession {
            session_id,
            device,
            config,
        } => {
            spawn_headless_session(
                session_id,
                device,
                config,
                project_path,
                msg_tx,
                session_tasks,
                shutdown_rx,
            );
        }
        _ => {
            // Other actions not needed for headless auto-start
            info!("Ignoring action in headless mode: {:?}", action);
        }
    }
}

/// Spawn a Flutter session in headless mode
fn spawn_headless_session(
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

                // Emit daemon_connected event
                HeadlessEvent::daemon_connected(&device_name).emit();

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

                // Track app_id from events
                let mut app_id: Option<String> = None;
                let mut process_exited = false;
                let session_id_str = session_id.to_string();

                // Forward daemon events and emit headless events
                loop {
                    tokio::select! {
                        event = daemon_rx.recv() => {
                            match event {
                                Some(event) => {
                                    // Track exit events
                                    if matches!(event, DaemonEvent::Exited { .. }) {
                                        process_exited = true;
                                        HeadlessEvent::app_stopped(
                                            &session_id_str,
                                            Some("Process exited".to_string()),
                                        )
                                        .emit();
                                    }

                                    // Parse and emit events from stdout
                                    if let DaemonEvent::Stdout(ref line) = event {
                                        if let Some(json) = protocol::strip_brackets(line) {
                                            if let Some(msg) = DaemonMessage::parse(json) {
                                                emit_daemon_message_event(&msg, &session_id_str, &mut app_id);
                                            }
                                        }
                                    }

                                    // Forward to main message channel
                                    if msg_tx_clone
                                        .send(Message::SessionDaemon {
                                            session_id,
                                            event,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                None => {
                                    process_exited = true;
                                    break;
                                }
                            }
                        }
                        _ = shutdown_rx_clone.changed() => {
                            info!("Shutdown signal received for session {}", session_id);
                            break;
                        }
                    }
                }

                // Shutdown
                if !process_exited {
                    info!("Session {} ending, initiating shutdown...", session_id);
                    if let Err(e) = process
                        .shutdown(app_id.as_deref(), Some(&session_sender))
                        .await
                    {
                        warn!("Shutdown error for session {}: {}", session_id, e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to spawn Flutter process: {}", e);
                HeadlessEvent::error(format!("Failed to spawn Flutter process: {}", e), true)
                    .emit();
                let _ = msg_tx_clone
                    .send(Message::SessionSpawnFailed {
                        session_id,
                        device_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }

        // Remove task from tracker
        let mut tasks = session_tasks_clone.lock().await;
        tasks.remove(&session_id);
    });

    // Track the task
    let session_tasks_for_insert = session_tasks.clone();
    tokio::spawn(async move {
        let mut tasks = session_tasks_for_insert.lock().await;
        tasks.insert(session_id, handle);
    });
}

/// Emit HeadlessEvent based on DaemonMessage
fn emit_daemon_message_event(msg: &DaemonMessage, session_id: &str, app_id: &mut Option<String>) {
    match msg {
        DaemonMessage::AppStart(app_start) => {
            *app_id = Some(app_start.app_id.clone());
            HeadlessEvent::app_started(session_id, &app_start.device_id).emit();
        }
        DaemonMessage::AppProgress(progress) => {
            if progress.finished {
                // Check if this is a reload completion
                if progress.progress_id.as_deref() == Some("hot.reload") {
                    // Extract duration if available
                    let duration_ms = 0; // Would need timing tracking
                    HeadlessEvent::hot_reload_completed(session_id, duration_ms).emit();
                } else if progress.progress_id.as_deref() == Some("hot.restart") {
                    HeadlessEvent::hot_reload_completed(session_id, 0).emit();
                }
            }
        }
        DaemonMessage::AppLog(log) => {
            let level = if log.error { "error" } else { "info" };
            HeadlessEvent::log(level, log.log.clone(), Some(session_id.to_string())).emit();
        }
        DaemonMessage::AppStop(_) => {
            HeadlessEvent::app_stopped(session_id, None).emit();
        }
        _ => {
            // Other messages don't need headless events
        }
    }
}
