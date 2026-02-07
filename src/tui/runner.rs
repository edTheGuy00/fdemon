//! Main TUI runner - entry points and event loop
//!
//! Contains the core application lifecycle:
//! - `run_with_project`: Main entry point with Flutter project
//! - `run`: Demo/test entry point without Flutter
//! - `run_loop`: Main event loop processing terminal and daemon events

use std::path::Path;

use tracing::error;

use crate::app::spawn;
use crate::app::Engine;
use crate::common::prelude::*;

use super::{event, render, startup, terminal};

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Create the engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Initialize terminal (TUI-specific)
    let mut term = ratatui::init();

    // TUI-specific startup: show NewSessionDialog, load configs
    let _startup_result =
        startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Render first frame
    if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
        error!("Failed to render initial frame: {}", e);
    }

    // Trigger startup discovery (non-blocking)
    spawn::spawn_tool_availability_check(engine.msg_sender());
    spawn::spawn_device_discovery(engine.msg_sender());

    // Run the main loop
    let result = run_loop(&mut term, &mut engine);

    // Shutdown engine (stops watcher, cleans up sessions)
    engine.shutdown().await;

    // Restore terminal (TUI-specific)
    ratatui::restore();

    result
}

/// Run TUI without Flutter (for testing/demo)
pub async fn run() -> Result<()> {
    terminal::install_panic_hook();

    // Create engine with dummy path
    let dummy_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut engine = Engine::new(dummy_path);

    // Initialize terminal
    let mut term = ratatui::init();

    // Run the main loop
    let result = run_loop(&mut term, &mut engine);

    // Shutdown engine
    engine.shutdown().await;

    // Restore terminal
    ratatui::restore();
    result
}

/// Main event loop
fn run_loop(terminal: &mut ratatui::DefaultTerminal, engine: &mut Engine) -> Result<()> {
    while !engine.should_quit() {
        // Drain and process all pending messages
        engine.drain_pending_messages();

        // Flush batched logs
        engine.flush_pending_logs();

        // Render
        terminal.draw(|frame| render::view(frame, &mut engine.state))?;

        // Handle terminal events (TUI-specific)
        if let Some(message) = event::poll()? {
            engine.process_message(message);
        }
    }

    Ok(())
}
