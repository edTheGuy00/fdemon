//! Main TUI runner - entry points and event loop
//!
//! Contains the core application lifecycle:
//! - `run_with_project`: Main entry point with Flutter project
//! - `run`: Demo/test entry point without Flutter
//! - `run_loop`: Main event loop processing terminal and daemon events

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};
use tracing::warn;

use crate::app::message::Message;
use crate::app::state::AppState;
use crate::common::{prelude::*, signals};
use crate::config;
use crate::core::{DaemonEvent, LogSource};
use crate::daemon::{protocol, CommandSender, DaemonMessage};
use crate::watcher::{FileWatcher, WatcherConfig};

use super::actions::SessionTaskMap;
use super::{event, process, render, startup, terminal};

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

    // Per-session task handles - for cleanup (HashMap allows multiple concurrent sessions)
    let session_tasks: SessionTaskMap = Arc::new(Mutex::new(std::collections::HashMap::new()));

    // Shutdown signal for background tasks
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Determine startup behavior based on settings
    let (flutter, initial_cmd_sender) =
        startup::startup_flutter(&mut state, &settings, project_path, daemon_tx, msg_tx.clone())
            .await;

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
        session_tasks.clone(),
        shutdown_rx,
        project_path,
    );

    // Stop file watcher
    file_watcher.stop();

    // Cleanup Flutter process gracefully
    startup::cleanup_sessions(
        &mut state,
        &mut term,
        flutter,
        cmd_sender,
        session_tasks,
        shutdown_tx,
    )
    .await;

    // Restore terminal
    ratatui::restore();

    result
}

/// Run TUI without Flutter (for testing/demo)
pub async fn run() -> Result<()> {
    terminal::install_panic_hook();
    let mut term = ratatui::init();
    let mut state = AppState::new();

    let (msg_tx, msg_rx) = mpsc::channel::<Message>(1);
    let (_daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(1);
    let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));
    let session_tasks: SessionTaskMap = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let dummy_path = Path::new(".");
    let result = run_loop(
        &mut term,
        &mut state,
        msg_rx,
        daemon_rx,
        msg_tx,
        cmd_sender,
        session_tasks,
        shutdown_rx,
        dummy_path,
    );
    ratatui::restore();
    result
}

/// Main event loop
#[allow(clippy::too_many_arguments)]
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut msg_rx: mpsc::Receiver<Message>,
    mut daemon_rx: mpsc::Receiver<DaemonEvent>,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) -> Result<()> {
    while !state.should_quit() {
        // Process external messages (from signal handler, etc.)
        while let Ok(msg) = msg_rx.try_recv() {
            process::process_message(
                state,
                msg,
                &msg_tx,
                &cmd_sender,
                &session_tasks,
                &shutdown_rx,
                project_path,
            );
        }

        // Process daemon events (non-blocking)
        while let Ok(event) = daemon_rx.try_recv() {
            // Route JSON-RPC responses to RequestTracker before processing
            route_daemon_response(&event, &cmd_sender);
            // Still pass to handler for logging/other processing
            process::process_message(
                state,
                Message::Daemon(event),
                &msg_tx,
                &cmd_sender,
                &session_tasks,
                &shutdown_rx,
                project_path,
            );
        }

        // Render
        terminal.draw(|frame| render::view(frame, state))?;

        // Handle terminal events
        if let Some(message) = event::poll()? {
            process::process_message(
                state,
                message,
                &msg_tx,
                &cmd_sender,
                &session_tasks,
                &shutdown_rx,
                project_path,
            );
        }
    }

    Ok(())
}

/// Route daemon responses to request tracker (legacy mode)
fn route_daemon_response(event: &DaemonEvent, cmd_sender: &Arc<Mutex<Option<CommandSender>>>) {
    if let DaemonEvent::Stdout(ref line) = event {
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
}
