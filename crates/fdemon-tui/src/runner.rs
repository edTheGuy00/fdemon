//! Main TUI runner - entry points and event loop
//!
//! Contains the core application lifecycle:
//! - `run_with_project`: Main entry point with Flutter project
//! - `run_with_project_and_dap`: Like `run_with_project` but with DAP port override and auto-start
//! - `run`: Demo/test entry point without Flutter
//! - `run_loop`: Main event loop processing terminal and daemon events

use std::path::Path;

use tracing::error;

use fdemon_app::config::should_auto_start_dap;
use fdemon_app::message::Message;
use fdemon_app::spawn;
use fdemon_app::Engine;
use fdemon_core::prelude::*;

use crate::{event, render, startup, terminal};

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Create the engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Initialize terminal (TUI-specific)
    let mut term = ratatui::init();

    // TUI-specific startup: detect auto-start or show NewSessionDialog
    let startup_result =
        startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Render first frame
    if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
        error!("Failed to render initial frame: {}", e);
    }

    // Trigger startup discovery (non-blocking)
    spawn::spawn_tool_availability_check(engine.msg_sender());

    // Dispatch based on auto-start detection
    dispatch_startup_action(&mut engine, startup_result);

    // Run the main loop
    let result = run_loop(&mut term, &mut engine);

    // Shutdown engine (stops watcher, cleans up sessions)
    engine.shutdown().await;

    // Restore terminal (TUI-specific)
    ratatui::restore();

    result
}

/// Run the TUI application with a Flutter project and optional DAP configuration.
///
/// This is identical to [`run_with_project`] but also:
/// 1. Applies a `--dap-port` CLI override to `settings.dap.port` and forces
///    `settings.dap.enabled = true` when `dap_port` is `Some(port)`.
/// 2. Applies a `--dap-config` IDE override to `AppState.cli_dap_config_override`
///    when `dap_config` is `Some(ide)`, bypassing environment-based detection.
/// 3. Evaluates [`should_auto_start_dap`] after CLI flag processing and sends
///    `Message::StartDapServer` if the result is `true`.
///
/// This covers all startup paths:
/// - `--dap-port` CLI flag → `dap.enabled = true` → auto-starts
/// - `dap.enabled = true` in config → auto-starts
/// - `dap.auto_start_in_ide = true` + IDE detected → auto-starts
/// - No DAP config + no IDE → does not auto-start
pub async fn run_with_project_and_dap(
    project_path: &Path,
    dap_port: Option<u16>,
    dap_config: Option<fdemon_app::config::ParentIde>,
) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Create the engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Apply --dap-port CLI override: sets port and forces enabled = true in
    // both settings copies, keeping them in sync.
    if let Some(port) = dap_port {
        engine.apply_cli_dap_override(port);
    }

    // Apply --dap-config IDE override: stored on AppState so handle_started()
    // can pass it to GenerateIdeConfig, bypassing environment-based detection.
    if let Some(ide) = dap_config {
        engine.apply_cli_dap_config_override(ide);
    }

    // Evaluate DAP auto-start (covers config-enabled and IDE-detected scenarios).
    // --dap-port already sets dap.enabled=true above, so this handles all paths.
    //
    // ORDERING: process_message is called synchronously before run_loop starts.
    // This is safe because StartDapServer returns an UpdateAction (async side
    // effect), not a follow-up Message. If the handler is changed to return a
    // follow-up Message, this call site must switch to
    // engine.msg_sender().try_send() to preserve ordering.
    if should_auto_start_dap(&engine.settings) {
        engine.process_message(Message::StartDapServer);
    }

    // Initialize terminal (TUI-specific)
    let mut term = ratatui::init();

    // TUI-specific startup: detect auto-start or show NewSessionDialog
    let startup_result =
        startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Render first frame
    if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
        error!("Failed to render initial frame: {}", e);
    }

    // Trigger startup discovery (non-blocking)
    spawn::spawn_tool_availability_check(engine.msg_sender());

    // Dispatch based on auto-start detection
    dispatch_startup_action(&mut engine, startup_result);

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

/// Dispatch the startup action returned by [`startup::startup_flutter`].
///
/// Auto-start sends `StartAutoLaunch` (which internally triggers device
/// discovery and auto-launches the session). Ready state triggers device
/// discovery directly so the NewSessionDialog is populated.
///
/// # Ordering
///
/// `process_message` is called synchronously before `run_loop` starts.
/// This is safe because `StartAutoLaunch` returns an `UpdateAction` (async
/// side effect), not a follow-up `Message`. If the handler is changed to
/// return a follow-up `Message`, this call site must switch to
/// `engine.msg_sender().try_send()` to preserve ordering.
fn dispatch_startup_action(engine: &mut Engine, action: startup::StartupAction) {
    match action {
        startup::StartupAction::AutoStart { configs } => {
            // Auto-start detected: send StartAutoLaunch which triggers device
            // discovery and auto-launches the session. spawn_device_discovery()
            // is NOT called here — the StartAutoLaunch handler dispatches
            // DiscoverDevicesAndAutoLaunch internally.
            engine.process_message(Message::StartAutoLaunch { configs });
        }
        startup::StartupAction::Ready => {
            // No auto-start — discover devices for the NewSessionDialog
            if let Some(flutter) = engine.state.flutter_executable() {
                spawn::spawn_device_discovery(engine.msg_sender(), flutter);
            } else {
                // SDK not found — clear the loading spinner with an error
                let _ = engine.msg_sender().try_send(Message::DeviceDiscoveryFailed {
                    error: "Flutter SDK not found. Configure sdk_path in .fdemon/config.toml or ensure flutter is on your PATH.".into(),
                    is_background: false,
                });
            }
        }
    }
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
