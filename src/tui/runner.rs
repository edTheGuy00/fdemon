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
use crate::core::LogSource;
use crate::watcher::{FileWatcher, WatcherConfig};

use super::actions::{handle_action, SessionTaskMap};
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

    // Create unified message channel (for signal handler, etc.)
    let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

    // Spawn signal handler (sends Message::Quit on SIGINT/SIGTERM)
    signals::spawn_signal_handler(msg_tx.clone());

    // Per-session task handles - for cleanup (HashMap allows multiple concurrent sessions)
    let session_tasks: SessionTaskMap = Arc::new(Mutex::new(std::collections::HashMap::new()));

    // Shutdown signal for background tasks
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Determine startup behavior based on settings
    // Returns an action to spawn a session if auto-start is configured
    let startup_action =
        startup::startup_flutter(&mut state, &settings, project_path, msg_tx.clone()).await;

    // If we have a startup action (auto-start session), execute it
    if let Some(action) = startup_action {
        handle_action(
            action,
            msg_tx.clone(),
            None,       // No session-specific cmd_sender yet
            Vec::new(), // No session senders yet
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

    if let Err(e) = file_watcher.start(msg_tx.clone()) {
        warn!("Failed to start file watcher: {}", e);
        if let Some(session) = state.session_manager.selected_mut() {
            session.session.log_error(
                LogSource::Watcher,
                format!("Failed to start file watcher: {}", e),
            );
        }
    }

    // Run the main loop
    let result = run_loop(
        &mut term,
        &mut state,
        msg_rx,
        msg_tx,
        session_tasks.clone(),
        shutdown_rx,
        project_path,
    );

    // Stop file watcher
    file_watcher.stop();

    // Cleanup Flutter sessions gracefully
    startup::cleanup_sessions(&mut state, &mut term, session_tasks, shutdown_tx).await;

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
    let session_tasks: SessionTaskMap = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let dummy_path = Path::new(".");
    let result = run_loop(
        &mut term,
        &mut state,
        msg_rx,
        msg_tx,
        session_tasks,
        shutdown_rx,
        dummy_path,
    );
    ratatui::restore();
    result
}

/// Main event loop
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut msg_rx: mpsc::Receiver<Message>,
    msg_tx: mpsc::Sender<Message>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) -> Result<()> {
    while !state.should_quit() {
        // Process external messages (from signal handler, session tasks, etc.)
        while let Ok(msg) = msg_rx.try_recv() {
            process::process_message(
                state,
                msg,
                &msg_tx,
                &session_tasks,
                &shutdown_rx,
                project_path,
            );
        }

        // Flush any pending batched logs before rendering (Task 04)
        // This ensures logs are processed at ~60fps during high-volume bursts
        state.session_manager.flush_all_pending_logs();

        // Render
        terminal.draw(|frame| render::view(frame, state))?;

        // Handle terminal events
        if let Some(message) = event::poll()? {
            process::process_message(
                state,
                message,
                &msg_tx,
                &session_tasks,
                &shutdown_rx,
                project_path,
            );
        }
    }

    Ok(())
}
