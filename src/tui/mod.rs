//! TUI presentation layer with signal handling

pub mod event;
pub mod layout;
pub mod render;
pub mod selector;
pub mod terminal;
pub mod widgets;

pub use selector::{select_project, SelectionResult};

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};

use crate::app::state::UiMode;
use crate::app::{handler, message::Message, state::AppState, Task, UpdateAction};
use crate::common::{prelude::*, signals};
use crate::config;
use crate::core::{AppPhase, DaemonEvent, LogSource};
use crate::daemon::{
    devices, emulators, protocol, CommandSender, DaemonCommand, DaemonMessage, FlutterProcess,
    RequestTracker,
};
use crate::watcher::{FileWatcher, WatcherConfig};

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Load configuration
    let settings = config::load_settings(project_path);
    info!(
        "Loaded settings: auto_start={}",
        settings.behavior.auto_start
    );

    // Initialize terminal
    let mut term = ratatui::init();

    // Create initial state with settings
    let mut state = AppState::with_settings(project_path.to_path_buf(), settings.clone());
    state.log_info(LogSource::App, "Flutter Demon starting...");

    // Create unified message channel (for signal handler, etc.)
    let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

    // Create channel for daemon events (used for legacy single-session mode)
    let (daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(256);

    // Spawn signal handler (sends Message::Quit on SIGINT/SIGTERM)
    signals::spawn_signal_handler(msg_tx.clone());

    // Shared command sender - can be updated when sessions are spawned
    let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));

    // Shared session task handle - for cleanup
    let session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(None));

    // Shutdown signal for background tasks
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Determine startup behavior based on settings
    let (flutter, initial_cmd_sender) = if settings.behavior.auto_start {
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
            spawn_device_discovery(msg_tx.clone());
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
                                spawn_device_discovery(msg_tx.clone());
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
        spawn_device_discovery(msg_tx.clone());
        (None, None)
    };

    // If we auto-started, set the initial command sender
    if let Some(sender) = initial_cmd_sender {
        *cmd_sender.lock().await = Some(sender);
    }

    // Start file watcher for auto-reload
    let mut file_watcher = FileWatcher::new(
        project_path.to_path_buf(),
        WatcherConfig::new()
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload),
    );

    if let Err(e) = file_watcher.start(msg_tx.clone()) {
        warn!("Failed to start file watcher: {}", e);
        state.log_error(
            LogSource::Watcher,
            format!("Failed to start file watcher: {}", e),
        );
    } else {
        state.log_info(LogSource::Watcher, "File watcher started (watching lib/)");
    }

    // Run the main loop
    let result = run_loop(
        &mut term,
        &mut state,
        msg_rx,
        daemon_rx,
        msg_tx,
        cmd_sender.clone(),
        session_task.clone(),
        shutdown_rx,
        project_path,
    );

    // Stop file watcher
    file_watcher.stop();

    // Cleanup Flutter process gracefully
    if let Some(mut p) = flutter {
        // Auto-start mode: we own the process directly
        state.log_info(LogSource::App, "Shutting down Flutter process...");

        // Draw one more frame to show shutdown message
        let _ = term.draw(|frame| render::view(frame, &mut state));

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
        // SpawnSession mode: process is owned by background task
        // Check if there's a session task to wait for
        let task_handle = session_task.lock().await.take();

        if let Some(handle) = task_handle {
            state.log_info(LogSource::App, "Shutting down Flutter session...");

            // Draw one more frame to show shutdown message
            let _ = term.draw(|frame| render::view(frame, &mut state));

            // Signal the background task to shut down
            info!("Sending shutdown signal to session task...");
            let _ = shutdown_tx.send(true);

            // Wait for the background task to complete its shutdown
            info!("Waiting for session task to complete shutdown...");
            match tokio::time::timeout(std::time::Duration::from_secs(10), handle).await {
                Ok(Ok(())) => info!("Session task completed cleanly"),
                Ok(Err(e)) => warn!("Session task panicked: {}", e),
                Err(_) => warn!("Timeout waiting for session task, process may be orphaned"),
            }
        }
    }

    // Restore terminal
    ratatui::restore();

    result
}

/// Spawn device discovery in background
fn spawn_device_discovery(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match devices::discover_devices().await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::DevicesDiscovered {
                        devices: result.devices,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::DeviceDiscoveryFailed {
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator discovery in background
fn spawn_emulator_discovery(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match emulators::discover_emulators().await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::EmulatorsDiscovered {
                        emulators: result.emulators,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::EmulatorDiscoveryFailed {
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator launch in background
fn spawn_emulator_launch(msg_tx: mpsc::Sender<Message>, emulator_id: String) {
    tokio::spawn(async move {
        match emulators::launch_emulator(&emulator_id).await {
            Ok(result) => {
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
            Err(e) => {
                // Create a failed result
                let result = emulators::EmulatorLaunchResult {
                    success: false,
                    emulator_id,
                    message: Some(e.to_string()),
                    elapsed: std::time::Duration::from_secs(0),
                };
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
        }
    });
}

/// Spawn iOS Simulator launch in background (macOS only)
fn spawn_ios_simulator_launch(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match emulators::launch_ios_simulator().await {
            Ok(result) => {
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
            Err(e) => {
                let result = emulators::EmulatorLaunchResult {
                    success: false,
                    emulator_id: "apple_ios_simulator".to_string(),
                    message: Some(e.to_string()),
                    elapsed: std::time::Duration::from_secs(0),
                };
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
        }
    });
}

/// Run TUI without Flutter (for testing/demo)
pub async fn run() -> Result<()> {
    terminal::install_panic_hook();
    let mut term = ratatui::init();
    let mut state = AppState::new();

    let (msg_tx, msg_rx) = mpsc::channel::<Message>(1);
    let (_daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(1);
    let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));
    let session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(None));
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let dummy_path = Path::new(".");
    let result = run_loop(
        &mut term, &mut state, msg_rx, daemon_rx, msg_tx, cmd_sender, session_task, shutdown_rx, dummy_path,
    );
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut msg_rx: mpsc::Receiver<Message>,
    mut daemon_rx: mpsc::Receiver<DaemonEvent>,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) -> Result<()> {
    while !state.should_quit() {
        // Process external messages (from signal handler, etc.)
        while let Ok(msg) = msg_rx.try_recv() {
            process_message(state, msg, &msg_tx, &cmd_sender, &session_task, &shutdown_rx, project_path);
        }

        // Process daemon events (non-blocking)
        while let Ok(event) = daemon_rx.try_recv() {
            // Route JSON-RPC responses to RequestTracker before processing
            if let DaemonEvent::Stdout(ref line) = event {
                if let Some(json) = protocol::strip_brackets(line) {
                    if let Some(DaemonMessage::Response { id, result, error }) =
                        DaemonMessage::parse(json)
                    {
                        // Try to get the command sender for response routing
                        if let Ok(guard) = cmd_sender.try_lock() {
                            if let Some(ref sender) = *guard {
                                if let Some(id_num) = id.as_u64() {
                                    let tracker = sender.tracker().clone();
                                    tokio::spawn(async move {
                                        tracker.handle_response(id_num, result, error).await;
                                    });
                                }
                            }
                        }
                    }
                }
            }
            // Still pass to handler for logging/other processing
            process_message(
                state,
                Message::Daemon(event),
                &msg_tx,
                &cmd_sender,
                &session_task,
                &shutdown_rx,
                project_path,
            );
        }

        // Render
        terminal.draw(|frame| render::view(frame, state))?;

        // Handle terminal events
        if let Some(message) = event::poll()? {
            process_message(state, message, &msg_tx, &cmd_sender, &session_task, &shutdown_rx, project_path);
        }
    }

    Ok(())
}

/// Process a message through the TEA update function
fn process_message(
    state: &mut AppState,
    message: Message,
    msg_tx: &mpsc::Sender<Message>,
    cmd_sender: &Arc<Mutex<Option<CommandSender>>>,
    session_task: &Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) {
    // Route responses from Message::Daemon events (from SpawnSession-spawned processes)
    if let Message::Daemon(DaemonEvent::Stdout(ref line)) = message {
        if let Some(json) = protocol::strip_brackets(line) {
            if let Some(DaemonMessage::Response { id, result, error }) = DaemonMessage::parse(json)
            {
                // Try to get the command sender for response routing
                if let Ok(guard) = cmd_sender.try_lock() {
                    if let Some(ref sender) = *guard {
                        if let Some(id_num) = id.as_u64() {
                            let tracker = sender.tracker().clone();
                            tokio::spawn(async move {
                                tracker.handle_response(id_num, result, error).await;
                            });
                        }
                    }
                }
            }
        }
    }

    let mut msg = Some(message);
    while let Some(m) = msg {
        let result = handler::update(state, m);

        // Handle any action
        if let Some(action) = result.action {
            handle_action(
                action,
                msg_tx.clone(),
                cmd_sender.clone(),
                session_task.clone(),
                shutdown_rx.clone(),
                project_path,
            );
        }

        // Continue with follow-up message
        msg = result.message;
    }
}

/// Execute an action by spawning a background task
fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) {
    match action {
        UpdateAction::SpawnTask(task) => {
            // Spawn async task for command execution
            let cmd_sender_clone = cmd_sender.clone();
            tokio::spawn(async move {
                // Get the command sender from the mutex
                let sender = cmd_sender_clone.lock().await.clone();
                execute_task(task, msg_tx, sender).await;
            });
        }

        UpdateAction::DiscoverDevices => {
            // Spawn device discovery in background
            spawn_device_discovery(msg_tx);
        }

        UpdateAction::SpawnSession { device, config } => {
            // Spawn Flutter process for the selected device
            let project_path = project_path.to_path_buf();
            let msg_tx_clone = msg_tx.clone();
            let cmd_sender_clone = cmd_sender.clone();
            let session_task_clone = session_task.clone();
            let mut shutdown_rx_clone = shutdown_rx.clone();
            let device_id = device.id.clone();
            let device_name = device.name.clone();
            let device_platform = device.platform.clone();

            let handle = tokio::spawn(async move {
                info!(
                    "Spawning Flutter session on device: {} ({})",
                    device_name, device_id
                );

                // Create event channel for this session
                let (daemon_tx, mut daemon_rx) = mpsc::channel::<DaemonEvent>(256);

                // Spawn the Flutter process
                let spawn_result = if let Some(cfg) = config {
                    FlutterProcess::spawn_with_config(&project_path, &device_id, &cfg, daemon_tx)
                        .await
                } else {
                    FlutterProcess::spawn_with_device(&project_path, &device_id, daemon_tx).await
                };

                match spawn_result {
                    Ok(mut process) => {
                        info!("Flutter process started (PID: {:?})", process.id());

                        // Create command sender and update shared state
                        let request_tracker = Arc::new(RequestTracker::default());
                        let sender = process.command_sender(request_tracker);
                        *cmd_sender_clone.lock().await = Some(sender);

                        // Send session started message
                        let _ = msg_tx_clone
                            .send(Message::SessionStarted {
                                device_id: device_id.clone(),
                                device_name: device_name.clone(),
                                platform: device_platform.clone(),
                                pid: process.id(),
                            })
                            .await;

                        // Track app_id from events for shutdown
                        let mut app_id: Option<String> = None;

                        // Forward daemon events to the main message channel
                        // This runs until the process exits, main loop closes, or shutdown signal
                        loop {
                            tokio::select! {
                                event = daemon_rx.recv() => {
                                    match event {
                                        Some(event) => {
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

                                            if msg_tx_clone.send(Message::Daemon(event)).await.is_err() {
                                                // Main loop closed, need to shutdown
                                                break;
                                            }
                                        }
                                        None => {
                                            // Channel closed, process ended
                                            break;
                                        }
                                    }
                                }
                                _ = shutdown_rx_clone.changed() => {
                                    // Shutdown signal received
                                    info!("Shutdown signal received, stopping session...");
                                    break;
                                }
                            }
                        }

                        // Graceful shutdown when loop ends
                        info!("Session ending, initiating shutdown...");
                        let sender_guard = cmd_sender_clone.lock().await;
                        if let Err(e) = process
                            .shutdown(app_id.as_deref(), sender_guard.as_ref())
                            .await
                        {
                            warn!("Shutdown error (process may already be gone): {}", e);
                        }
                        drop(sender_guard);

                        // Clear the command sender
                        *cmd_sender_clone.lock().await = None;
                    }
                    Err(e) => {
                        error!("Failed to spawn Flutter process: {}", e);
                        let _ = msg_tx_clone
                            .send(Message::SessionSpawnFailed {
                                device_id,
                                error: e.to_string(),
                            })
                            .await;
                    }
                }

                // Clear the session task handle when done
                *session_task_clone.lock().await = None;
            });

            // Store the handle so we can await it during cleanup
            if let Ok(mut guard) = session_task.try_lock() {
                *guard = Some(handle);
            }
        }

        UpdateAction::DiscoverEmulators => {
            // Spawn emulator discovery in background
            spawn_emulator_discovery(msg_tx);
        }

        UpdateAction::LaunchEmulator { emulator_id } => {
            // Spawn emulator launch in background
            spawn_emulator_launch(msg_tx, emulator_id);
        }

        UpdateAction::LaunchIOSSimulator => {
            // Spawn iOS Simulator launch in background
            spawn_ios_simulator_launch(msg_tx);
        }
    }
}

/// Execute a task and send completion message
async fn execute_task(
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
        Task::Reload { app_id } => {
            let start = std::time::Instant::now();
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
        Task::Restart { app_id } => match sender.send(DaemonCommand::Restart { app_id }).await {
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
        },
        Task::Stop { app_id } => {
            if let Err(e) = sender.send(DaemonCommand::Stop { app_id }).await {
                error!("Failed to stop app: {}", e);
            }
        }
    }
}
