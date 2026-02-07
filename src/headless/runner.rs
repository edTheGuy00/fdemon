//! Headless mode runner - main event loop without TUI
//!
//! This module implements the headless (non-TUI) event loop for fdemon.
//! It processes daemon events and emits JSON events to stdout for E2E testing.

use std::path::Path;

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use fdemon_app::{actions::handle_action, message::Message, state::AppState, Engine, UpdateAction};
use fdemon_core::prelude::*;
use fdemon_daemon::devices;

use super::HeadlessEvent;

/// Run in headless mode - output JSON events instead of TUI
pub async fn run_headless(project_path: &Path) -> Result<()> {
    info!("═══════════════════════════════════════════════════════");
    info!("Flutter Demon starting in HEADLESS mode");
    info!("Project: {}", project_path.display());
    info!("═══════════════════════════════════════════════════════");

    // Create engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Spawn headless-specific stdin reader
    let stdin_tx = engine.msg_sender();
    std::thread::spawn(move || {
        spawn_stdin_reader_blocking(stdin_tx);
    });

    // Auto-start: discover devices and spawn session
    // In headless mode, always auto-start regardless of config setting
    headless_auto_start(&mut engine).await;

    // Main event loop
    let result = headless_event_loop(&mut engine).await;

    // Shutdown
    engine.shutdown().await;

    info!("Flutter Demon headless mode exiting");
    result
}

/// Main headless event loop
async fn headless_event_loop(engine: &mut Engine) -> Result<()> {
    loop {
        // Check for shutdown
        if engine.should_quit() {
            info!("Quit requested");
            break;
        }

        // Wait for next message
        match engine.msg_rx.recv().await {
            Some(msg) => {
                // Emit events based on message type before processing
                emit_pre_message_events(&engine.state, &msg);

                // Process through engine
                engine.process_message(msg);

                // Flush pending logs
                engine.flush_pending_logs();

                // Emit events based on state changes after processing
                emit_post_message_events(&engine.state);
            }
            None => {
                // Channel closed
                info!("Message channel closed");
                break;
            }
        }
    }

    Ok(())
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
                fdemon_core::LogLevel::Debug => "debug",
                fdemon_core::LogLevel::Info => "info",
                fdemon_core::LogLevel::Warning => "warning",
                fdemon_core::LogLevel::Error => "error",
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

/// Auto-start in headless mode: discover devices and create session
async fn headless_auto_start(engine: &mut Engine) {
    // Discover devices
    info!("Discovering devices for headless auto-start...");
    match devices::discover_devices().await {
        Ok(result) => {
            info!("Found {} device(s)", result.devices.len());

            // Emit device_detected events for each device
            for device in &result.devices {
                HeadlessEvent::device_detected(&device.id, &device.name, &device.platform).emit();
            }

            // Cache devices in state
            engine.state.set_device_cache(result.devices.clone());

            // Pick first device for auto-start
            if let Some(device) = result.devices.first() {
                info!("Auto-starting with device: {} ({})", device.name, device.id);

                // Create session via SessionManager
                match engine.state.session_manager.create_session(device) {
                    Ok(session_id) => {
                        info!("Created session {}", session_id);

                        // Emit session_created event
                        HeadlessEvent::session_created(&session_id.to_string(), &device.name)
                            .emit();

                        // Dispatch SpawnSession action via handle_action
                        // This uses the shared spawn_session from app/actions
                        let action = UpdateAction::SpawnSession {
                            session_id,
                            device: device.clone(),
                            config: None,
                        };

                        handle_action(
                            action,
                            engine.msg_tx.clone(),
                            None,       // session_cmd_sender - not needed for spawn
                            Vec::new(), // session_senders - not needed for spawn
                            engine.session_tasks.clone(),
                            engine.shutdown_rx.clone(),
                            &engine.project_path,
                            Default::default(), // tool_availability
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to create session: {}", e);
                        HeadlessEvent::error(format!("Failed to create session: {}", e), true)
                            .emit();
                    }
                }
            } else {
                tracing::error!("No devices found");
                HeadlessEvent::error("No devices found".to_string(), true).emit();
            }
        }
        Err(e) => {
            tracing::error!("Device discovery failed: {}", e);
            HeadlessEvent::error(format!("Device discovery failed: {}", e), true).emit();
        }
    }
}
