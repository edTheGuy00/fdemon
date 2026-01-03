//! TUI presentation layer with signal handling

pub mod event;
pub mod layout;
pub mod render;
pub mod terminal;
pub mod widgets;

use std::path::Path;
use tokio::sync::mpsc;

use crate::app::{handler, message::Message, state::AppState};
use crate::common::{prelude::*, signals};
use crate::core::{AppPhase, DaemonEvent, LogSource};
use crate::daemon::FlutterProcess;

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Initialize terminal
    let mut term = ratatui::init();

    // Create initial state
    let mut state = AppState::new();
    state.log_info(LogSource::App, "Flutter Demon starting...");

    // Create unified message channel (for signal handler, etc.)
    let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

    // Create channel for daemon events
    let (daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(256);

    // Spawn signal handler (sends Message::Quit on SIGINT/SIGTERM)
    signals::spawn_signal_handler(msg_tx.clone());

    // Spawn Flutter process
    let flutter = match FlutterProcess::spawn(project_path, daemon_tx).await {
        Ok(p) => {
            state.log_info(
                LogSource::App,
                format!("Flutter process started (PID: {:?})", p.id()),
            );
            state.phase = AppPhase::Running;
            Some(p)
        }
        Err(e) => {
            state.log_error(LogSource::App, format!("Failed to start Flutter: {}", e));
            None
        }
    };

    // Run the main loop
    let result = run_loop(&mut term, &mut state, msg_rx, daemon_rx);

    // Cleanup Flutter process gracefully
    if let Some(mut p) = flutter {
        state.log_info(LogSource::App, "Shutting down Flutter process...");

        // Draw one more frame to show shutdown message
        let _ = term.draw(|frame| render::view(frame, &mut state));

        if let Err(e) = p.shutdown().await {
            error!("Error during Flutter shutdown: {}", e);
        } else {
            info!("Flutter process shut down cleanly");
        }
    }

    // Restore terminal
    ratatui::restore();

    result
}

/// Run TUI without Flutter (for testing/demo)
pub async fn run() -> Result<()> {
    terminal::install_panic_hook();
    let mut term = ratatui::init();
    let mut state = AppState::new();

    let (_msg_tx, msg_rx) = mpsc::channel::<Message>(1);
    let (_daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(1);

    let result = run_loop(&mut term, &mut state, msg_rx, daemon_rx);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut msg_rx: mpsc::Receiver<Message>,
    mut daemon_rx: mpsc::Receiver<DaemonEvent>,
) -> Result<()> {
    while !state.should_quit() {
        // Process external messages (from signal handler, etc.)
        while let Ok(msg) = msg_rx.try_recv() {
            process_message(state, msg);
        }

        // Process daemon events (non-blocking)
        while let Ok(event) = daemon_rx.try_recv() {
            process_message(state, Message::Daemon(event));
        }

        // Render
        terminal.draw(|frame| render::view(frame, state))?;

        // Handle terminal events
        if let Some(message) = event::poll()? {
            process_message(state, message);
        }
    }

    Ok(())
}

/// Process a message through the TEA update function
fn process_message(state: &mut AppState, message: Message) {
    let mut msg = Some(message);
    while let Some(m) = msg {
        msg = handler::update(state, m);
    }
}
