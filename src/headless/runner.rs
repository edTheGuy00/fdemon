//! Headless mode runner - main event loop without TUI
//!
//! This module implements the headless (non-TUI) event loop for fdemon.
//! It processes daemon events and emits JSON events to stdout for E2E testing.

use std::path::Path;

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use fdemon_app::{
    config::{emit_migration_nudge, load_all_configs, should_auto_start_dap, NudgeMode},
    message::{AutoLaunchSuccess, Message},
    spawn::find_auto_launch_target,
    state::AppState,
    Engine,
};
use fdemon_core::prelude::*;
use fdemon_daemon::devices;

use super::HeadlessEvent;

/// Run in headless mode - output JSON events instead of TUI.
///
/// # Arguments
///
/// * `project_path` — Path to the Flutter project directory.
/// * `dap_port` — If `Some(port)`, overrides `settings.dap.port` and forces
///   `settings.dap.enabled = true`. The server's actual port is printed to
///   stdout as `{"event":"dap_server_started","port":<N>}` once it is bound.
///   Use `0` for an OS-assigned ephemeral port.
/// * `dap_config` — If `Some(ide)`, stores the CLI-provided IDE override on
///   `AppState` so `handle_started()` can pass it to `GenerateIdeConfig`,
///   bypassing environment-based IDE detection.
pub async fn run_headless(
    project_path: &Path,
    dap_port: Option<u16>,
    dap_config: Option<fdemon_app::config::ParentIde>,
) -> Result<()> {
    info!("═══════════════════════════════════════════════════════");
    info!("Flutter Demon starting in HEADLESS mode");
    info!("Project: {}", project_path.display());
    info!("═══════════════════════════════════════════════════════");

    // Create engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Apply --dap-port override: sets port and forces enabled = true in
    // both settings copies, keeping them in sync.
    if let Some(port) = dap_port {
        engine.apply_cli_dap_override(port);
    }

    // Apply --dap-config IDE override: stored on AppState so handle_started()
    // can pass it to GenerateIdeConfig, bypassing environment-based detection.
    if let Some(ide) = dap_config {
        engine.apply_cli_dap_config_override(ide);
    }

    // Spawn headless-specific stdin reader
    let stdin_tx = engine.msg_sender();
    std::thread::spawn(move || {
        spawn_stdin_reader_blocking(stdin_tx);
    });

    // Evaluate DAP auto-start (covers --dap-port, config-enabled, and IDE-detected scenarios).
    // --dap-port already sets dap.enabled=true above, so this single check handles all paths.
    if should_auto_start_dap(&engine.settings) {
        let _ = engine.msg_sender().send(Message::StartDapServer).await;
    }

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
        match engine.recv_message().await {
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
    match msg {
        Message::HotReload => {
            if let Some(session_id) = get_current_session_id(_state) {
                HeadlessEvent::hot_reload_started(&session_id).emit();
            }
        }
        Message::SessionStarted {
            session_id,
            device_name,
            ..
        } => {
            let sid = session_id.to_string();
            HeadlessEvent::daemon_connected(device_name).emit();
            HeadlessEvent::app_started(&sid, device_name).emit();
        }
        Message::SessionReloadCompleted {
            session_id,
            time_ms,
        } => {
            HeadlessEvent::hot_reload_completed(&session_id.to_string(), *time_ms).emit();
        }
        Message::SessionReloadFailed { session_id, reason } => {
            HeadlessEvent::hot_reload_failed(&session_id.to_string(), reason.clone()).emit();
        }
        // Emit DAP server port to stdout so external tooling can discover it.
        Message::DapServerStarted { port } => {
            HeadlessEvent::dap_server_started(*port).emit();
        }
        _ => {}
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

/// **Sibling-bug coordination note (added 2026-04-29):**
/// The `find_auto_launch_target` integration in this function was originally
/// scoped to sibling bug `launch-toml-device-ignored` Task 03. It was absorbed
/// inline by `cache-auto-launch-gate` Task 04 (option b) on 2026-04-29 because
/// the sibling task had not been implemented anywhere. When the sibling bug's
/// Task 03 is reviewed next, close it as resolved-by-absorption. See:
/// - workflow/plans/bugs/cache-auto-launch-gate/tasks/04-headless-gate.md
/// - workflow/plans/bugs/launch-toml-device-ignored/TASKS.md (Task 03)
///
/// Auto-start in headless mode: discover devices and create session.
///
/// Headless always passes `cache_allowed = false` to `find_auto_launch_target`
/// per decision 2(b): headless mode preserves the "always auto-launch on first
/// device" semantic and is intentionally cache-blind regardless of the user's
/// `[behavior] auto_launch` flag.
async fn headless_auto_start(engine: &mut Engine) {
    // Require Flutter SDK to be resolved
    let flutter = match engine.state.flutter_executable() {
        Some(f) => f,
        None => {
            tracing::error!(
                "No Flutter SDK resolved; cannot discover devices for headless auto-start"
            );
            HeadlessEvent::error("No Flutter SDK found".to_string(), true).emit();
            return;
        }
    };

    let project_path = engine.project_path.clone();

    // Load launch.toml configs to drive tier-1 (auto_start) and tier-3 (first config) resolution
    let configs = load_all_configs(&project_path);

    // Migration nudge: user has a cached device but the flag is not set. In
    // headless mode the cache is never consulted, so this helps CI/script users
    // understand why fdemon didn't pick the previously-used device.
    // The headless message explicitly avoids referencing [behavior] auto_launch
    // as a remediation since that flag does NOT apply in headless mode.
    let _ = emit_migration_nudge(NudgeMode::Headless, &project_path, &engine.settings);

    // Discover devices
    info!("Discovering devices for headless auto-start...");
    match devices::discover_devices(&flutter).await {
        Ok(result) => {
            info!("Found {} device(s)", result.devices.len());

            // Emit device_detected events for each device
            for device in &result.devices {
                HeadlessEvent::device_detected(&device.id, &device.name, &device.platform).emit();
            }

            // Cache devices in state
            engine.state.set_device_cache(result.devices.clone());

            if result.devices.is_empty() {
                tracing::error!("No devices found");
                HeadlessEvent::error("No devices found".to_string(), true).emit();
                return;
            }

            // Resolve target via the 4-tier cascade.
            // cache_allowed is hard-wired to false: headless is always cache-blind.
            let Some(AutoLaunchSuccess { device, config }) =
                find_auto_launch_target(&configs, &result.devices, &project_path, false)
            else {
                tracing::error!("Auto-launch resolution returned no target");
                HeadlessEvent::error(
                    "Auto-launch resolution returned no target".to_string(),
                    true,
                )
                .emit();
                return;
            };

            info!("Auto-starting with device: {} ({})", device.name, device.id);

            // Create session via SessionManager
            match engine.state.session_manager.create_session(&device) {
                Ok(session_id) => {
                    info!("Created session {}", session_id);

                    // Emit session_created event
                    HeadlessEvent::session_created(&session_id.to_string(), &device.name).emit();

                    // Dispatch SpawnSession action via Engine
                    engine.dispatch_spawn_session(session_id, device, config.map(Box::new));
                }
                Err(e) => {
                    tracing::error!("Failed to create session: {}", e);
                    HeadlessEvent::error(format!("Failed to create session: {}", e), true).emit();
                }
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

    // ── Headless auto-start gate tests ─────────────────────────────────────
    //
    // These tests verify the cache-blind behaviour of headless mode by calling
    // `find_auto_launch_target` directly with `cache_allowed = false`, which is
    // the value hard-wired in `headless_auto_start`. They do not start a real
    // Flutter process or perform network I/O.

    use fdemon_app::{
        config::{has_cached_last_device, load_all_configs, save_last_selection},
        spawn::find_auto_launch_target,
    };
    use fdemon_daemon::devices::Device;

    fn make_device(id: &str) -> Device {
        Device {
            id: id.to_string(),
            name: id.to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    /// H1: Cache present + no auto_launch + no auto_start → first device wins.
    ///
    /// Headless always passes cache_allowed=false, so even a valid cached device
    /// is ignored. The function should fall through to Tier 3 (first config +
    /// first device) or Tier 4 (bare flutter run) and return the first device.
    #[test]
    fn headless_ignores_cache_uses_first_device() {
        let temp = tempfile::tempdir().unwrap();
        let project_path = temp.path();

        // Write a valid cache pointing to the second device
        save_last_selection(project_path, None, Some("device-2")).unwrap();

        // No launch.toml → no configs
        let configs = load_all_configs(project_path);

        let devices = vec![make_device("device-1"), make_device("device-2")];

        // cache_allowed=false (headless hard-wire): cache is skipped, Tier 4 fires
        let result = find_auto_launch_target(&configs, &devices, project_path, false)
            .expect("test setup guarantees Tier 4 resolves with non-empty devices");

        assert_eq!(
            result.device.id, "device-1",
            "headless must ignore cache and pick first device"
        );
        assert!(
            result.config.is_none(),
            "no configs present → bare flutter run (no config)"
        );
    }

    /// H2: auto_launch = true + cache present + no auto_start → first device wins.
    ///
    /// Headless ignores the auto_launch setting entirely — it never reads it when
    /// calling find_auto_launch_target. cache_allowed is always false.
    #[test]
    fn headless_ignores_auto_launch_flag_still_uses_first_device() {
        let temp = tempfile::tempdir().unwrap();
        let project_path = temp.path();

        // Write a valid cache pointing to the second device
        save_last_selection(project_path, None, Some("device-2")).unwrap();
        assert!(
            has_cached_last_device(project_path),
            "precondition: cache exists"
        );

        // No launch.toml → no auto_start config
        let configs = load_all_configs(project_path);

        let devices = vec![make_device("device-1"), make_device("device-2")];

        // Regardless of what auto_launch would be, headless always uses false
        let result = find_auto_launch_target(&configs, &devices, project_path, false)
            .expect("test setup guarantees Tier 4 resolves with non-empty devices");

        assert_eq!(
            result.device.id, "device-1",
            "headless must use first device even when cache is present"
        );
    }

    /// H3: auto_start = true in launch.toml → that config's device wins (Tier 1).
    ///
    /// Tier 1 fires regardless of cache_allowed, so headless correctly uses the
    /// explicitly configured auto_start device.
    #[test]
    fn headless_tier1_auto_start_config_wins() {
        let temp = tempfile::tempdir().unwrap();
        let project_path = temp.path();

        // Write a cache pointing to the first device (should be overridden by Tier 1)
        save_last_selection(project_path, None, Some("device-1")).unwrap();

        // Write launch.toml with auto_start = true targeting device-2
        let fdemon_dir = project_path.join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "ProdConfig"
device = "device-2"
auto_start = true
"#,
        )
        .unwrap();

        let configs = load_all_configs(project_path);

        let devices = vec![make_device("device-1"), make_device("device-2")];

        // cache_allowed=false but Tier 1 fires before cache is consulted
        let result = find_auto_launch_target(&configs, &devices, project_path, false)
            .expect("test setup guarantees Tier 1 auto_start resolves");

        assert_eq!(
            result.device.id, "device-2",
            "Tier 1 auto_start config must win even in headless mode"
        );
        assert_eq!(
            result.config.as_ref().unwrap().name,
            "ProdConfig",
            "auto_start config name must be carried through"
        );
    }
}
