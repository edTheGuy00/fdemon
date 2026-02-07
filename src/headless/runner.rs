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
    // Track how many logs we've already emitted to prevent duplicates
    let mut last_emitted_log_count: usize = 0;

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
                emit_post_message_events(&engine.state, &mut last_emitted_log_count);
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
fn emit_post_message_events(state: &AppState, last_emitted: &mut usize) {
    if let Some(session) = state.session_manager.selected() {
        let current_count = session.session.logs.len();

        // Handle VecDeque eviction: if logs were evicted from front,
        // our index may be past the current length
        if *last_emitted > current_count {
            *last_emitted = 0; // Reset -- we lost track due to eviction
        }

        if current_count > *last_emitted {
            // Emit only new logs (skip already-emitted ones)
            for log in session.session.logs.iter().skip(*last_emitted) {
                // Convert LogLevel to lowercase string using prefix method
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
            *last_emitted = current_count;
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

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::{LogEntry, LogSource};

    #[test]
    fn test_last_emitted_advances_with_new_logs() {
        // Setup: session with 3 logs, last_emitted = 0
        let mut state = AppState::new();

        // Add session to manager
        let device = fdemon_daemon::devices::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "linux".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Add 3 logs to the session
        if let Some(session_handle) = state.session_manager.get_mut(session_id) {
            for i in 0..3 {
                session_handle.session.add_log(LogEntry::info(
                    LogSource::Flutter,
                    format!("Log message {}", i),
                ));
            }
        }

        // Select the session
        state.session_manager.select_by_id(session_id);

        let mut last_emitted = 0;

        // Act: Simulate emission tracking (without actually emitting to stdout)
        if let Some(session_handle) = state.session_manager.selected() {
            let current_count = session_handle.session.logs.len();
            if current_count > last_emitted {
                last_emitted = current_count;
            }
        }

        // Assert: last_emitted now equals 3
        assert_eq!(last_emitted, 3);
    }

    #[test]
    fn test_no_emission_when_no_new_logs() {
        // Setup: session with 3 logs, last_emitted = 3
        let mut state = AppState::new();

        let device = fdemon_daemon::devices::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "linux".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Add 3 logs
        if let Some(session_handle) = state.session_manager.get_mut(session_id) {
            for i in 0..3 {
                session_handle.session.add_log(LogEntry::info(
                    LogSource::Flutter,
                    format!("Log message {}", i),
                ));
            }
        }

        state.session_manager.select_by_id(session_id);

        let mut last_emitted = 3;

        // Act: Simulate emission tracking
        if let Some(session_handle) = state.session_manager.selected() {
            let current_count = session_handle.session.logs.len();
            if current_count > last_emitted {
                last_emitted = current_count;
            }
        }

        // Assert: last_emitted still 3, no new logs processed
        assert_eq!(last_emitted, 3);
    }

    #[test]
    fn test_eviction_resets_index() {
        // Setup: last_emitted = 100, but session.logs.len() = 50 (simulating eviction)
        let mut state = AppState::new();

        let device = fdemon_daemon::devices::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "linux".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Add 50 logs
        if let Some(session_handle) = state.session_manager.get_mut(session_id) {
            for i in 0..50 {
                session_handle.session.add_log(LogEntry::info(
                    LogSource::Flutter,
                    format!("Log message {}", i),
                ));
            }
        }

        state.session_manager.select_by_id(session_id);

        let mut last_emitted = 100;

        // Act: Simulate emission tracking with eviction handling
        if let Some(session_handle) = state.session_manager.selected() {
            let current_count = session_handle.session.logs.len();

            // Handle VecDeque eviction: if logs were evicted from front,
            // our index may be past the current length
            if last_emitted > current_count {
                last_emitted = 0; // Reset -- we lost track due to eviction
            }

            if current_count > last_emitted {
                last_emitted = current_count;
            }
        }

        // Assert: last_emitted reset to 50 (current count after eviction reset)
        assert_eq!(last_emitted, 50);
    }

    #[test]
    fn test_emission_tracking_with_incremental_logs() {
        // Setup: session starts with 2 logs, we emit them, then 3 more are added
        let mut state = AppState::new();

        let device = fdemon_daemon::devices::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "linux".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Add 2 initial logs
        if let Some(session_handle) = state.session_manager.get_mut(session_id) {
            for i in 0..2 {
                session_handle.session.add_log(LogEntry::info(
                    LogSource::Flutter,
                    format!("Log message {}", i),
                ));
            }
        }

        state.session_manager.select_by_id(session_id);

        let mut last_emitted = 0;

        // First emission: should emit 2 logs
        if let Some(session_handle) = state.session_manager.selected() {
            let current_count = session_handle.session.logs.len();
            if current_count > last_emitted {
                last_emitted = current_count;
            }
        }
        assert_eq!(last_emitted, 2);

        // Add 3 more logs to the session via session_manager
        if let Some(session_handle) = state.session_manager.selected_mut() {
            for i in 2..5 {
                session_handle.session.add_log(LogEntry::info(
                    LogSource::Flutter,
                    format!("Log message {}", i),
                ));
            }
        }

        // Second emission: should emit only the 3 new logs
        if let Some(session_handle) = state.session_manager.selected() {
            let current_count = session_handle.session.logs.len();
            if current_count > last_emitted {
                let new_logs_count = current_count - last_emitted;
                assert_eq!(new_logs_count, 3); // Only 3 new logs
                last_emitted = current_count;
            }
        }
        assert_eq!(last_emitted, 5);
    }
}
