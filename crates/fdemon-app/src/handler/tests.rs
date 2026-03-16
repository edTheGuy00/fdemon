//! Tests for handler module

use super::*;
use crate::input_key::InputKey;
use crate::message::Message;
use crate::state::{
    AppState, DevToolsError, UiMode, VmConnectionStatus, MAX_PENDING_WATCHER_ERRORS,
};
use fdemon_core::{AppPhase, DaemonEvent};

/// Helper function to create a test Device with minimal required fields
fn test_device(id: &str, name: &str) -> fdemon_daemon::Device {
    fdemon_daemon::Device {
        id: id.to_string(),
        name: name.to_string(),
        platform: "android".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }
}

#[test]
fn test_quit_message_sets_quitting_phase() {
    let mut state = AppState::new();
    assert_ne!(state.phase, AppPhase::Quitting);

    update(&mut state, Message::Quit);

    assert_eq!(state.phase, AppPhase::Quitting);
    assert!(state.should_quit());
}

#[test]
fn test_should_quit_returns_true_when_quitting() {
    let mut state = AppState::new();
    state.phase = AppPhase::Quitting;
    assert!(state.should_quit());
}

#[test]
fn test_should_quit_returns_false_when_running() {
    let mut state = AppState::new();
    state.phase = AppPhase::Running;
    assert!(!state.should_quit());
}

#[test]
fn test_q_key_produces_request_quit_message() {
    let state = AppState::new();
    let key = InputKey::Char('q');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::RequestQuit)));
}

#[test]
fn test_escape_key_produces_request_quit_message() {
    let state = AppState::new();
    let key = InputKey::Esc;

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::RequestQuit)));
}

#[test]
fn test_ctrl_c_produces_quit_message() {
    let state = AppState::new();
    let key = InputKey::CharCtrl('c');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::Quit)));
}

#[test]
fn test_r_key_produces_hot_reload() {
    let state = AppState::new();
    let key = InputKey::Char('r');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::HotReload)));
}

#[test]
fn test_shift_r_produces_hot_restart() {
    let state = AppState::new();
    let key = InputKey::Char('R');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::HotRestart)));
}

#[test]
fn test_s_key_produces_stop() {
    let state = AppState::new();
    let key = InputKey::Char('s');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::StopApp)));
}

#[test]
fn test_auto_reload_skipped_when_no_app() {
    let mut state = AppState::new();
    // No sessions, so should skip auto-reload

    let result = update(&mut state, Message::AutoReloadTriggered);

    assert!(result.action.is_none());
}

#[test]
fn test_auto_reload_skipped_when_busy() {
    let mut state = AppState::new();
    // With multi-session, busy check uses session_manager.any_session_busy()
    // Since no sessions exist, this should skip (can't be busy without sessions)

    let result = update(&mut state, Message::AutoReloadTriggered);

    assert!(result.action.is_none());
}

// ─────────────────────────────────────────────────────────
// Raw line level detection tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_detect_raw_line_level_android() {
    use fdemon_core::LogLevel;

    let (level, _) = detect_raw_line_level("E/flutter: some error");
    assert_eq!(level, LogLevel::Error);

    let (level, _) = detect_raw_line_level("W/flutter: some warning");
    assert_eq!(level, LogLevel::Warning);
}

#[test]
fn test_detect_raw_line_level_gradle() {
    use fdemon_core::LogLevel;

    let (level, _) = detect_raw_line_level("FAILURE: Build failed");
    assert_eq!(level, LogLevel::Error);

    let (level, _) = detect_raw_line_level("BUILD FAILED");
    assert_eq!(level, LogLevel::Error);
}

#[test]
fn test_detect_raw_line_level_xcode() {
    use fdemon_core::LogLevel;

    let (level, _) = detect_raw_line_level("❌ Build failed");
    assert_eq!(level, LogLevel::Error);

    let (level, _) = detect_raw_line_level("⚠ Warning message");
    assert_eq!(level, LogLevel::Warning);
}

#[test]
fn test_detect_raw_line_level_build_progress() {
    use fdemon_core::LogLevel;

    let (level, _) = detect_raw_line_level("Running pod install...");
    assert_eq!(level, LogLevel::Debug);

    let (level, _) = detect_raw_line_level("Building with flavor...");
    assert_eq!(level, LogLevel::Debug);
}

#[test]
fn test_detect_raw_line_level_default() {
    use fdemon_core::LogLevel;

    let (level, _) = detect_raw_line_level("Some regular output");
    assert_eq!(level, LogLevel::Info);
}

#[test]
fn test_detect_raw_line_level_trims_whitespace() {
    let (_, message) = detect_raw_line_level("  Some message  ");
    assert_eq!(message, "Some message");
}

// ─────────────────────────────────────────────────────────
// Quit flow tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_request_quit_no_sessions_quits_immediately() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    // No sessions running, confirm_quit is true by default

    update(&mut state, Message::RequestQuit);

    // Should go directly to Quitting phase (no dialog)
    assert_eq!(state.phase, AppPhase::Quitting);
    assert_ne!(state.ui_mode, UiMode::ConfirmDialog);
}

#[test]
fn test_request_quit_confirm_quit_disabled_quits_immediately() {
    use crate::state::UiMode;

    let mut state = AppState::new();

    // Create a session
    let device = test_device("test-device", "Test Device");
    let _ = state.session_manager.create_session(&device);

    // Disable confirm_quit via settings
    state.settings.behavior.confirm_quit = false;

    update(&mut state, Message::RequestQuit);

    // Should go directly to Quitting phase (no dialog)
    assert_eq!(state.phase, AppPhase::Quitting);
    assert_ne!(state.ui_mode, UiMode::ConfirmDialog);
}

// Note: Quit flow tests for dialog behavior removed - confirm dialog behavior changed

#[test]
fn test_cancel_quit_returns_to_normal() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    update(&mut state, Message::CancelQuit);

    assert_eq!(state.ui_mode, UiMode::Normal);
    assert_ne!(state.phase, AppPhase::Quitting);
}

#[test]
fn test_y_key_in_confirm_dialog_confirms() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = InputKey::Char('y');
    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::ConfirmQuit)));
}

#[test]
fn test_n_key_in_confirm_dialog_cancels() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = InputKey::Char('n');
    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CancelQuit)));
}

#[test]
fn test_esc_in_confirm_dialog_cancels() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = InputKey::Esc;
    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CancelQuit)));
}

#[test]
fn test_ctrl_c_in_confirm_dialog_force_quits() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = InputKey::CharCtrl('c');
    let result = handle_key(&state, key);

    // Should force quit (bypass confirm)
    assert!(matches!(result, Some(Message::Quit)));
}

#[test]
fn test_q_in_confirm_dialog_confirms() {
    use crate::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = InputKey::Char('q');
    let result = handle_key(&state, key);

    // 'q' in confirm dialog should confirm (enables "qq" quick quit)
    assert!(matches!(result, Some(Message::ConfirmQuit)));
}

// ─────────────────────────────────────────────────────────
// Device selector tests
// ─────────────────────────────────────────────────────────

// Old dialog tests removed - DeviceSelector and StartupDialog no longer exist

// ─────────────────────────────────────────────────────────
// Multi-session tests
// ─────────────────────────────────────────────────────────

#[test]
#[ignore = "DeviceSelected is deprecated - functionality moved to NewSessionDialog"]
fn test_device_selected_creates_session() {
    // This test is obsolete - DeviceSelected message is deprecated
    // Session creation now happens through NewSessionDialog flow
    // See tests in new_session_dialog section below
}

#[test]
#[ignore = "DeviceSelected is deprecated - functionality moved to NewSessionDialog"]
fn test_device_selected_session_id_in_spawn_action() {
    // This test is obsolete - DeviceSelected message is deprecated
}

// test_device_selected_prevents_duplicate was removed (phase 4 task 04).
// Device-reuse guard logic is now covered by:
//   handler::new_session::launch_context::tests::test_handle_launch_allows_device_reuse_when_session_stopped
//   handler::new_session::launch_context::tests::test_handle_launch_blocks_device_with_running_session
//   handler::new_session::launch_context::tests::test_handle_launch_blocks_device_with_initializing_session

#[test]
#[ignore = "DeviceSelected is deprecated - functionality moved to NewSessionDialog"]
fn test_device_selected_max_sessions_enforced() {
    // This test is obsolete - DeviceSelected message is deprecated
}

#[test]
fn test_session_started_updates_session_state() {
    let mut state = AppState::new();

    // Create a session first
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Send SessionStarted
    update(
        &mut state,
        Message::SessionStarted {
            session_id,
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            platform: "android".to_string(),
            pid: Some(1234),
        },
    );

    // Session should be running
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.session.phase, AppPhase::Running);
    assert!(handle.session.started_at.is_some());
}

#[test]
fn test_session_started_with_unknown_session() {
    let mut state = AppState::new();

    // Send SessionStarted for non-existent session
    update(
        &mut state,
        Message::SessionStarted {
            session_id: 999, // Non-existent
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            platform: "android".to_string(),
            pid: Some(1234),
        },
    );

    // Should not panic - just doesn't update anything
    assert!(state.session_manager.get(999).is_none());
}

#[test]
fn test_session_spawn_failed_removes_session() {
    use crate::state::UiMode;

    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();
    assert_eq!(state.session_manager.len(), 1);

    update(
        &mut state,
        Message::SessionSpawnFailed {
            session_id,
            device_id: "test-device".to_string(),
            error: "test error".to_string(),
        },
    );

    // Session should be removed
    assert_eq!(state.session_manager.len(), 0);

    // Should show new session dialog to allow retry
    assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
}

#[test]
fn test_session_spawn_failed_logs_and_removes() {
    use crate::state::UiMode;

    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(
        &mut state,
        Message::SessionSpawnFailed {
            session_id,
            device_id: "test-device".to_string(),
            error: "spawn failed".to_string(),
        },
    );

    // Session should be removed and UI should show new session dialog
    assert!(state.session_manager.get(session_id).is_none());
    assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
}

// ─────────────────────────────────────────────────────────
// Session navigation tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_number_keys_select_session() {
    let state = AppState::new();

    let key = InputKey::Char('1');
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::SelectSessionByIndex(0))));

    let key = InputKey::Char('5');
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::SelectSessionByIndex(4))));
}

#[test]
fn test_tab_cycles_sessions() {
    let state = AppState::new();

    let key = InputKey::Tab;
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::NextSession)));

    let key = InputKey::BackTab;
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::PreviousSession)));
}

#[test]
fn test_select_session_by_index_message() {
    let mut state = AppState::new();

    // Create multiple sessions
    for i in 0..3 {
        let device = test_device(&format!("device-{}", i), &format!("Device {}", i));
        let _ = state.session_manager.create_session(&device);
    }

    // Currently first session is selected
    let first_id = state.session_manager.selected_id();

    // Select second session (index 1)
    update(&mut state, Message::SelectSessionByIndex(1));

    // Selection should change
    assert_ne!(state.session_manager.selected_id(), first_id);
}

#[test]
fn test_next_previous_session_messages() {
    let mut state = AppState::new();

    for i in 0..3 {
        let device = test_device(&format!("device-{}", i), &format!("Device {}", i));
        let _ = state.session_manager.create_session(&device);
    }

    let initial_id = state.session_manager.selected_id();

    // Go next
    update(&mut state, Message::NextSession);
    let after_next = state.session_manager.selected_id();
    assert_ne!(after_next, initial_id);

    // Go previous
    update(&mut state, Message::PreviousSession);
    assert_eq!(state.session_manager.selected_id(), initial_id);
}

// ─────────────────────────────────────────────────────────
// Close session tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_x_closes_session() {
    let state = AppState::new();

    let key = InputKey::Char('x');
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::CloseCurrentSession)));
}

#[test]
fn test_ctrl_w_closes_session() {
    let state = AppState::new();

    let key = InputKey::CharCtrl('w');
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::CloseCurrentSession)));
}

#[test]
fn test_close_single_session_triggers_quit_confirmation() {
    let mut state = AppState::new();
    state.settings.behavior.confirm_quit = true;

    // Create only one session
    let device = test_device("device-1", "Device 1");
    let _ = state.session_manager.create_session(&device);

    // Try to close the only session
    update(&mut state, Message::CloseCurrentSession);

    // Should trigger quit confirmation (via request_quit)
    // Note: If confirm_quit is true and sessions exist, it shows dialog
    // But request_quit might just quit if only 1 session
}

#[test]
#[ignore = "Old dialog removed"]
fn test_close_session_shows_device_selector_when_multiple() {
    let mut state = AppState::new();

    // Create multiple sessions
    for i in 0..2 {
        let device = test_device(&format!("device-{}", i), &format!("Device {}", i));
        let _ = state.session_manager.create_session(&device);
    }

    // Close current session
    update(&mut state, Message::CloseCurrentSession);

    // One session should remain
    assert_eq!(state.session_manager.len(), 1);
}

// ─────────────────────────────────────────────────────────
// Clear logs tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_c_clears_logs() {
    let state = AppState::new();

    let key = InputKey::Char('c');
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::ClearLogs)));
}

#[test]
fn test_clear_logs_message() {
    let mut state = AppState::new();

    // Create a session and add a log to it
    let device = test_device("d1", "Device");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.add_log(fdemon_core::LogEntry::info(
            fdemon_core::LogSource::App,
            "test log",
        ));
        assert!(!handle.session.logs.is_empty());
    }

    update(&mut state, Message::ClearLogs);

    // Session logs should be cleared
    if let Some(handle) = state.session_manager.get(session_id) {
        assert!(handle.session.logs.is_empty());
    }
}

// ─────────────────────────────────────────────────────────
// Session daemon event tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_event_for_closed_session_is_discarded() {
    let mut state = AppState::new();

    // Send event for non-existent session
    update(
        &mut state,
        Message::SessionDaemon {
            session_id: 999,
            event: DaemonEvent::Stdout("test".to_string()),
        },
    );

    // Should not crash, logs should be unchanged
}

#[test]
fn test_session_daemon_stderr_routes_correctly() {
    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Stderr("error message".to_string()),
        },
    );

    // Flush any batched logs (logs are batched for performance - Task 04)
    state.session_manager.flush_all_pending_logs();

    // Session should have error log
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(handle
        .session
        .logs
        .iter()
        .any(|e| e.message.contains("error message")));
}

#[test]
fn test_handle_session_exited_nonzero_code_logs_error() {
    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(1) },
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    // Should log warning
    assert!(handle
        .session
        .logs
        .iter()
        .any(|e| e.message.contains("exited with code")));
}

#[test]
fn test_handle_session_exited_code_zero_logs_normal_exit() {
    // Setup: create state with an active session
    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Action: send DaemonEvent::Exited with code Some(0)
    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(0) },
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    // Assert: session phase is Stopped
    assert_eq!(
        handle.session.phase,
        AppPhase::Stopped,
        "session phase should be Stopped after exit code 0"
    );
    // Assert: session log contains "exited normally"
    assert!(
        handle
            .session
            .logs
            .iter()
            .any(|e| e.message.contains("exited normally")),
        "session log should contain 'exited normally' for exit code 0"
    );
}

#[test]
fn test_handle_session_exited_no_code_logs_unknown_exit() {
    // Setup: create state with an active session
    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Action: send DaemonEvent::Exited with code None
    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: None },
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    // Assert: session phase is Stopped
    assert_eq!(
        handle.session.phase,
        AppPhase::Stopped,
        "session phase should be Stopped after exit with no code"
    );
    // Assert: session log contains "Flutter process exited" (but not "normally" or "with code")
    assert!(
        handle
            .session
            .logs
            .iter()
            .any(|e| e.message.contains("Flutter process exited")),
        "session log should contain 'Flutter process exited' for None exit code"
    );
}

#[test]
fn test_handle_vm_service_disconnected_clears_vm_connected_and_shutdown_tx() {
    // Verify that VmServiceDisconnected clears all DevTools task handles and
    // shutdown senders for both performance and network monitoring.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Simulate VM being connected with active perf and network monitoring
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.vm_connected = true;
        handle.session.performance.monitoring_active = true;

        // Attach a perf shutdown sender
        let (perf_tx, _perf_rx) = tokio::sync::watch::channel(false);
        handle.perf_shutdown_tx = Some(std::sync::Arc::new(perf_tx));

        // Attach a network shutdown sender
        let (net_tx, _net_rx) = tokio::sync::watch::channel(false);
        handle.network_shutdown_tx = Some(std::sync::Arc::new(net_tx));
    }

    // Pre-condition: handles are set
    {
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.session.vm_connected,
            "vm_connected should be true before disconnect"
        );
        assert!(
            handle.perf_shutdown_tx.is_some(),
            "perf_shutdown_tx should be Some before disconnect"
        );
        assert!(
            handle.network_shutdown_tx.is_some(),
            "network_shutdown_tx should be Some before disconnect"
        );
    }

    // Action: send VmServiceDisconnected
    update(&mut state, Message::VmServiceDisconnected { session_id });

    // Assert: all cleanup occurred
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        !handle.session.vm_connected,
        "vm_connected should be false after VmServiceDisconnected"
    );
    assert!(
        !handle.session.performance.monitoring_active,
        "monitoring_active should be false after VmServiceDisconnected"
    );
    assert!(
        handle.perf_shutdown_tx.is_none(),
        "perf_shutdown_tx should be cleared after VmServiceDisconnected"
    );
    assert!(
        handle.perf_task_handle.is_none(),
        "perf_task_handle should be cleared after VmServiceDisconnected"
    );
    assert!(
        handle.network_shutdown_tx.is_none(),
        "network_shutdown_tx should be cleared after VmServiceDisconnected"
    );
    assert!(
        handle.network_task_handle.is_none(),
        "network_task_handle should be cleared after VmServiceDisconnected"
    );
}

#[test]
fn test_session_daemon_spawn_failed() {
    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::SpawnFailed {
                reason: "test spawn failure".to_string(),
            },
        },
    );

    // Session should have error log
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(handle
        .session
        .logs
        .iter()
        .any(|e| e.message.contains("Failed to start Flutter")));
}

#[test]
fn test_handle_session_exited_duplicate_exit_is_idempotent() {
    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // First exit: should process normally
    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(0) },
        },
    );

    // Second exit: should be silently ignored
    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(1) },
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.session.phase, AppPhase::Stopped);

    // Only one exit log entry should exist (from the first exit, not the second)
    let exit_logs: Vec<_> = handle
        .session
        .logs
        .iter()
        .filter(|e| e.message.contains("exited"))
        .collect();
    assert_eq!(
        exit_logs.len(),
        1,
        "duplicate exit should not add a second log entry"
    );
    assert!(
        exit_logs[0].message.contains("exited normally"),
        "the first exit (code 0) log should be preserved, not overwritten by code 1"
    );
}

#[test]
fn test_multiple_sessions_have_independent_start_state() {
    let mut state = AppState::new();

    // Create two sessions
    let device1 = test_device("device-1", "Device 1");
    let mut device2 = test_device("device-2", "Device 2");
    device2.platform = "ios".to_string();

    let session1 = state.session_manager.create_session(&device1).unwrap();
    let session2 = state.session_manager.create_session(&device2).unwrap();

    // Start only session 1
    update(
        &mut state,
        Message::SessionStarted {
            session_id: session1,
            device_id: "device-1".to_string(),
            device_name: "Device 1".to_string(),
            platform: "android".to_string(),
            pid: Some(1234),
        },
    );

    // Session 1 should be running
    assert_eq!(
        state.session_manager.get(session1).unwrap().session.phase,
        AppPhase::Running
    );

    // Session 2 should still be initializing
    assert_eq!(
        state.session_manager.get(session2).unwrap().session.phase,
        AppPhase::Initializing
    );
}

#[test]
fn test_session_duration_calculation() {
    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(
        &mut state,
        Message::SessionStarted {
            session_id,
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            platform: "android".to_string(),
            pid: Some(1234),
        },
    );

    // Session should have started_at set
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(handle.session.started_at.is_some());

    // Duration should be computable (will be close to 0)
    let duration = handle.session.session_duration();
    assert!(duration.is_some());
}

// ─────────────────────────────────────────────────────────
// Task enum tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_task_enum_includes_session_id() {
    let task = Task::Reload {
        session_id: 42,
        app_id: "test-app".to_string(),
    };

    if let Task::Reload { session_id, app_id } = task {
        assert_eq!(session_id, 42);
        assert_eq!(app_id, "test-app");
    }

    let task = Task::Restart {
        session_id: 43,
        app_id: "test-app-2".to_string(),
    };

    if let Task::Restart { session_id, app_id } = task {
        assert_eq!(session_id, 43);
        assert_eq!(app_id, "test-app-2");
    }

    let task = Task::Stop {
        session_id: 44,
        app_id: "test-app-3".to_string(),
    };

    if let Task::Stop { session_id, app_id } = task {
        assert_eq!(session_id, 44);
        assert_eq!(app_id, "test-app-3");
    }
}

// ─────────────────────────────────────────────────────────
// Multi-session auto-reload tests (Task 05)
// ─────────────────────────────────────────────────────────

#[test]
fn test_auto_reload_triggers_all_sessions() {
    let mut state = AppState::new();

    // Create two running sessions with app_ids and fake cmd_senders
    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let session2 = state.session_manager.create_session(&device2).unwrap();

    // Mark sessions as running with app_ids
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }

    // Trigger auto-reload
    let result = update(&mut state, Message::AutoReloadTriggered);

    // Should return ReloadAllSessions action
    if let Some(UpdateAction::ReloadAllSessions { sessions }) = result.action {
        assert_eq!(sessions.len(), 2);
        // Should contain both sessions
        assert!(sessions.iter().any(|(id, _)| *id == session1));
        assert!(sessions.iter().any(|(id, _)| *id == session2));
    } else {
        assert!(
            matches!(result.action, Some(UpdateAction::ReloadAllSessions { .. })),
            "Expected ReloadAllSessions action, got {:?}",
            result.action
        );
    }
}

#[test]
fn test_auto_reload_skips_all_when_any_busy() {
    let mut state = AppState::new();

    // Create two sessions
    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let session2 = state.session_manager.create_session(&device2).unwrap();

    // Mark both as running
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
        // Make session 2 busy (reloading)
        handle.session.start_reload();
    }

    // Trigger auto-reload
    let result = update(&mut state, Message::AutoReloadTriggered);

    // Should skip all since one is busy
    assert!(result.action.is_none());
}

#[test]
fn test_auto_reload_skips_sessions_without_app_id() {
    let mut state = AppState::new();

    // Create two sessions, only one running
    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let _session2 = state.session_manager.create_session(&device2).unwrap();

    // Only session 1 has app_id
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }
    // Session 2 is still initializing (no app_id)

    // Trigger auto-reload
    let result = update(&mut state, Message::AutoReloadTriggered);

    // Should only reload session 1
    if let Some(UpdateAction::ReloadAllSessions { sessions }) = result.action {
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0, session1);
    } else {
        assert!(
            matches!(result.action, Some(UpdateAction::ReloadAllSessions { .. })),
            "Expected ReloadAllSessions action, got {:?}",
            result.action
        );
    }
}

#[test]
fn test_auto_reload_marks_sessions_as_reloading() {
    let mut state = AppState::new();

    // Create one session
    let device = test_device("device-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }

    // Session should be Running initially
    assert_eq!(
        state.session_manager.get(session_id).unwrap().session.phase,
        AppPhase::Running
    );

    // Trigger auto-reload
    let _ = update(&mut state, Message::AutoReloadTriggered);

    // Session should now be Reloading
    assert_eq!(
        state.session_manager.get(session_id).unwrap().session.phase,
        AppPhase::Reloading
    );
}

#[test]
fn test_manual_reload_still_uses_selected_session() {
    let mut state = AppState::new();

    // Create two sessions
    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let session2 = state.session_manager.create_session(&device2).unwrap();

    // Mark both as running with app_ids
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }

    // Select session 2
    state.session_manager.select_by_id(session2);

    // Manual reload (r key)
    let result = update(&mut state, Message::HotReload);

    // Should only reload session 2 (the selected one)
    if let Some(UpdateAction::SpawnTask(Task::Reload { session_id, app_id })) = result.action {
        assert_eq!(session_id, session2);
        assert_eq!(app_id, "app-2");
    } else {
        assert!(
            matches!(
                result.action,
                Some(UpdateAction::SpawnTask(Task::Reload { .. }))
            ),
            "Expected SpawnTask Reload action, got {:?}",
            result.action
        );
    }
}

#[test]
fn test_auto_reload_logs_to_each_session() {
    let mut state = AppState::new();

    // Create two sessions
    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let session2 = state.session_manager.create_session(&device2).unwrap();

    // Mark both as running
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }

    // Trigger auto-reload
    let _ = update(&mut state, Message::AutoReloadTriggered);

    // Each session should have the reload message in its logs
    if let Some(handle) = state.session_manager.get(session1) {
        assert!(handle
            .session
            .logs
            .iter()
            .any(|e| e.message.contains("File change detected")));
    }
    if let Some(handle) = state.session_manager.get(session2) {
        assert!(handle
            .session
            .logs
            .iter()
            .any(|e| e.message.contains("File change detected")));
    }
}

// ─────────────────────────────────────────────────────────
// Isolate cache invalidation tests (Phase 3, Task 02)
// ─────────────────────────────────────────────────────────

#[test]
fn test_restart_completed_invalidates_isolate_cache() {
    let mut state = AppState::new();

    let device = test_device("device-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Attach a vm_request_handle with a pre-populated cache.
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.mark_started("app-1".to_string());
        handle.session.start_reload(); // session must be reloading before complete_reload()
        handle.vm_request_handle = Some(fdemon_daemon::vm_service::VmRequestHandle::new_for_test(
            Some("isolates/12345".to_string()),
        ));
    }

    // Confirm cache is populated before the message.
    {
        let h = state.session_manager.get(session_id).unwrap();
        let cached = h.vm_request_handle.as_ref().unwrap().cached_isolate_id();
        assert_eq!(
            cached,
            Some("isolates/12345".to_string()),
            "cache should be populated before restart"
        );
    }

    // Process SessionRestartCompleted.
    update(&mut state, Message::SessionRestartCompleted { session_id });

    // Cache should now be cleared.
    let h = state.session_manager.get(session_id).unwrap();
    let cached_after = h.vm_request_handle.as_ref().unwrap().cached_isolate_id();
    assert!(
        cached_after.is_none(),
        "isolate cache should be cleared after SessionRestartCompleted"
    );
}

#[test]
fn test_restart_completed_without_vm_handle_does_not_panic() {
    // SessionRestartCompleted must succeed even when vm_request_handle is None.
    let mut state = AppState::new();

    let device = test_device("device-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.mark_started("app-1".to_string());
        handle.session.start_reload();
        // vm_request_handle intentionally left as None
    }

    // Should not panic.
    let result = update(&mut state, Message::SessionRestartCompleted { session_id });
    assert!(result.action.is_none());

    // Session should have completed the reload.
    let h = state.session_manager.get(session_id).unwrap();
    assert_eq!(h.session.phase, AppPhase::Running);
}

#[test]
fn test_reload_completed_does_not_invalidate_isolate_cache() {
    // Hot reload does NOT create a new isolate, so the cache must be preserved.
    let mut state = AppState::new();

    let device = test_device("device-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.mark_started("app-1".to_string());
        handle.session.start_reload();
        handle.vm_request_handle = Some(fdemon_daemon::vm_service::VmRequestHandle::new_for_test(
            Some("isolates/99".to_string()),
        ));
    }

    // Process SessionReloadCompleted (not restart).
    update(
        &mut state,
        Message::SessionReloadCompleted {
            session_id,
            time_ms: 250,
        },
    );

    // Cache should still be populated — reload keeps the same isolate.
    let h = state.session_manager.get(session_id).unwrap();
    let cached_after = h.vm_request_handle.as_ref().unwrap().cached_isolate_id();
    assert_eq!(
        cached_after,
        Some("isolates/99".to_string()),
        "isolate cache should NOT be cleared after hot reload"
    );
}

#[test]
fn test_auto_reload_single_session_logs_to_session() {
    let mut state = AppState::new();

    // Create one session
    let device = test_device("device-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }

    // Trigger auto-reload
    let _ = update(&mut state, Message::AutoReloadTriggered);

    // Session should have the reload message in its logs
    if let Some(handle) = state.session_manager.get(session_id) {
        assert!(handle
            .session
            .logs
            .iter()
            .any(|e| e.message.contains("File change detected")));
    }
}

#[test]
fn test_reloadable_sessions_helper() {
    let mut state = AppState::new();

    // Create 3 sessions
    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let device3 = test_device("device-3", "Device 3");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let session2 = state.session_manager.create_session(&device2).unwrap();
    let _session3 = state.session_manager.create_session(&device3).unwrap();

    // Session 1: running, has cmd_sender
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
    }
    // Session 2: running, NO cmd_sender
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        // No cmd_sender
    }
    // Session 3: not running (no app_id)

    let reloadable = state.session_manager.reloadable_sessions();

    // Only session 1 should be reloadable
    assert_eq!(reloadable.len(), 1);
    assert_eq!(reloadable[0].0, session1);
    assert_eq!(reloadable[0].1, "app-1");
}

#[test]
fn test_any_session_busy() {
    let mut state = AppState::new();

    let device1 = test_device("device-1", "Device 1");
    let device2 = test_device("device-2", "Device 2");
    let session1 = state.session_manager.create_session(&device1).unwrap();
    let _session2 = state.session_manager.create_session(&device2).unwrap();

    // Neither busy initially
    assert!(!state.session_manager.any_session_busy());

    // Mark session 1 as reloading
    if let Some(handle) = state.session_manager.get_mut(session1) {
        handle.session.start_reload();
    }

    // Should detect busy
    assert!(state.session_manager.any_session_busy());
}

// ─────────────────────────────────────────────────────────
// Log Filter Tests (Phase 1 - Task 4)
// ─────────────────────────────────────────────────────────

#[test]
fn test_f_key_produces_cycle_level_filter() {
    let state = AppState::new();
    let key = InputKey::Char('f');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CycleLevelFilter)));
}

#[test]
fn test_shift_f_produces_cycle_source_filter() {
    let state = AppState::new();
    let key = InputKey::Char('F');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CycleSourceFilter)));
}

#[test]
fn test_ctrl_f_produces_reset_filters() {
    let state = AppState::new();
    let key = InputKey::CharCtrl('f');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::ResetFilters)));
}

#[test]
fn test_cycle_level_filter_message() {
    use fdemon_core::LogLevelFilter;

    let mut state = AppState::new();

    // Create a session first
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Initial state should be All
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .level_filter,
        LogLevelFilter::All
    );

    // Cycle to Errors
    update(&mut state, Message::CycleLevelFilter);
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .level_filter,
        LogLevelFilter::Errors
    );

    // Cycle to Warnings
    update(&mut state, Message::CycleLevelFilter);
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .level_filter,
        LogLevelFilter::Warnings
    );
}

#[test]
fn test_cycle_source_filter_message() {
    use fdemon_core::LogSourceFilter;

    let mut state = AppState::new();

    // Create a session first
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Initial state should be All
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .source_filter,
        LogSourceFilter::All
    );

    // Cycle to App
    update(&mut state, Message::CycleSourceFilter);
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .source_filter,
        LogSourceFilter::App
    );

    // Cycle to Daemon
    update(&mut state, Message::CycleSourceFilter);
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .source_filter,
        LogSourceFilter::Daemon
    );
}

#[test]
fn test_reset_filters_message() {
    use fdemon_core::{LogLevelFilter, LogSourceFilter};

    let mut state = AppState::new();

    // Create a session first
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Set some filters
    update(&mut state, Message::CycleLevelFilter);
    update(&mut state, Message::CycleSourceFilter);

    // Verify filters are active
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .level_filter,
        LogLevelFilter::Errors
    );
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .filter_state
            .source_filter,
        LogSourceFilter::App
    );

    // Reset filters
    update(&mut state, Message::ResetFilters);

    // Verify filters are reset
    let filter_state = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .filter_state;
    assert_eq!(filter_state.level_filter, LogLevelFilter::All);
    assert_eq!(filter_state.source_filter, LogSourceFilter::All);
}

#[test]
fn test_filter_messages_without_session() {
    let mut state = AppState::new();

    // No session - should not panic
    let result = update(&mut state, Message::CycleLevelFilter);
    assert!(matches!(result, UpdateResult { .. }));

    let result = update(&mut state, Message::CycleSourceFilter);
    assert!(matches!(result, UpdateResult { .. }));

    let result = update(&mut state, Message::ResetFilters);
    assert!(matches!(result, UpdateResult { .. }));
}

// ─────────────────────────────────────────────────────────
// Log Search Tests (Phase 1 - Task 5)
// ─────────────────────────────────────────────────────────

#[test]
fn test_slash_key_produces_start_search() {
    let state = AppState::new();
    let key = InputKey::Char('/');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::StartSearch)));
}

#[test]
fn test_start_search_changes_ui_mode() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    assert_eq!(state.ui_mode, UiMode::Normal);

    update(&mut state, Message::StartSearch);
    assert_eq!(state.ui_mode, UiMode::SearchInput);
}

#[test]
fn test_cancel_search_returns_to_normal() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    update(&mut state, Message::StartSearch);
    assert_eq!(state.ui_mode, UiMode::SearchInput);

    update(&mut state, Message::CancelSearch);
    assert_eq!(state.ui_mode, UiMode::Normal);
}

#[test]
fn test_search_input_updates_query() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(&mut state, Message::StartSearch);
    update(
        &mut state,
        Message::SearchInput {
            text: "error".to_string(),
        },
    );

    let query = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .search_state
        .query;
    assert_eq!(query, "error");
}

#[test]
fn test_search_input_mode_escape() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = InputKey::Esc;

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::CancelSearch)));
}

#[test]
fn test_search_input_mode_enter() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = InputKey::Enter;

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::CancelSearch)));
}

#[test]
fn test_search_input_mode_backspace() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Set initial query
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.set_search_query("test");
    }

    state.ui_mode = UiMode::SearchInput;
    let key = InputKey::Backspace;

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::SearchInput { text }) if text == "tes"));
}

#[test]
fn test_search_input_mode_ctrl_u_clears() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = InputKey::CharCtrl('u');

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::SearchInput { text }) if text.is_empty()));
}

#[test]
fn test_search_input_mode_typing() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Set initial query
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.set_search_query("tes");
    }

    state.ui_mode = UiMode::SearchInput;
    let key = InputKey::Char('t');

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::SearchInput { text }) if text == "test"));
}

#[test]
fn test_search_input_mode_ctrl_c_quits() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = InputKey::CharCtrl('c');

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::Quit)));
}

#[test]
fn test_n_key_with_search_query_navigates() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Set a search query
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.set_search_query("error");
    }

    let key = InputKey::Char('n');
    let msg = handle_key(&state, key);

    assert!(matches!(msg, Some(Message::NextSearchMatch)));
}

#[test]
fn test_n_key_without_search_does_nothing() {
    let state = AppState::new();
    let key = InputKey::Char('n');

    let msg = handle_key(&state, key);

    // Without active search query, 'n' should do nothing (it's only for search navigation)
    assert!(msg.is_none());
}

#[test]
fn test_shift_n_produces_prev_search_match() {
    let state = AppState::new();
    let key = InputKey::Char('N');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::PrevSearchMatch)));
}

#[test]
fn test_next_search_match_message() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add some matches
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.search_state.update_matches(vec![
            fdemon_core::SearchMatch::new(0, 0, 4),
            fdemon_core::SearchMatch::new(1, 0, 4),
        ]);
        handle.session.search_state.current_match = Some(0);
    }

    update(&mut state, Message::NextSearchMatch);

    let current = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .search_state
        .current_match;
    assert_eq!(current, Some(1));
}

#[test]
fn test_prev_search_match_message() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add some matches
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.search_state.update_matches(vec![
            fdemon_core::SearchMatch::new(0, 0, 4),
            fdemon_core::SearchMatch::new(1, 0, 4),
        ]);
        handle.session.search_state.current_match = Some(1);
    }

    update(&mut state, Message::PrevSearchMatch);

    let current = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .search_state
        .current_match;
    assert_eq!(current, Some(0));
}

#[test]
fn test_clear_search_resets_ui_mode() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    update(&mut state, Message::ClearSearch);

    assert_eq!(state.ui_mode, UiMode::Normal);
}

// ─────────────────────────────────────────────────────────
// Error Navigation Tests (Task 7)
// ─────────────────────────────────────────────────────────

#[test]
fn test_e_key_produces_next_error() {
    let state = AppState::new();
    let key = InputKey::Char('e');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::NextError)));
}

#[test]
fn test_shift_e_produces_prev_error() {
    let state = AppState::new();
    let key = InputKey::Char('E');

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::PrevError)));
}

#[test]
fn test_next_error_scrolls_to_error() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add some logs including errors
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.log_info(fdemon_core::LogSource::App, "info");
        handle
            .session
            .log_error(fdemon_core::LogSource::App, "error");
        handle.session.log_view_state.visible_lines = 10;
        handle.session.log_view_state.total_lines = 2;
    }

    update(&mut state, Message::NextError);

    // Should have scrolled (auto_scroll disabled)
    let session = &state.session_manager.get(session_id).unwrap().session;
    assert!(!session.log_view_state.auto_scroll);
}

#[test]
fn test_error_navigation_no_errors() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add only info logs
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle
            .session
            .log_info(fdemon_core::LogSource::App, "info only");
    }

    // Should not crash or change state
    let result = update(&mut state, Message::NextError);
    assert!(result.action.is_none());
}

#[test]
fn test_prev_error_message() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add some logs with errors
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle
            .session
            .log_error(fdemon_core::LogSource::App, "error 0");
        handle.session.log_info(fdemon_core::LogSource::App, "info");
        handle
            .session
            .log_error(fdemon_core::LogSource::App, "error 2");
        handle.session.log_view_state.offset = 3; // Position after errors
        handle.session.log_view_state.visible_lines = 10;
    }

    update(&mut state, Message::PrevError);

    // Should have scrolled to previous error
    let session = &state.session_manager.get(session_id).unwrap().session;
    assert!(!session.log_view_state.auto_scroll);
}

// ─────────────────────────────────────────────────────────
// Old Startup Dialog Tests - Removed
// Tests for StartupDialog and DialogSection removed as those types no longer exist.
// NewSessionDialog tests are located below in the new session dialog section.
// ─────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// Settings Toggle Tests (for bug demonstration)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_settings_toggle_bool_flips_value() {
    // This test verifies that SettingsToggleBool correctly flips boolean values.
    // The handler identifies which setting is selected and flips its boolean value.

    let mut state = AppState::new();

    // Open settings page (set UI mode to Settings)
    state.ui_mode = UiMode::Settings;

    // Set initial boolean value to true for auto_reload setting
    state.settings.watcher.auto_reload = true;

    // Select the auto_reload item (index 4 in Project tab)
    state.settings_view_state.selected_index = 4;

    // Handle the toggle message
    update(&mut state, Message::SettingsToggleBool);

    // Value should be flipped to false
    assert_eq!(
        state.settings.watcher.auto_reload, false,
        "auto_reload should be flipped to false"
    );

    // Test flipping back
    update(&mut state, Message::SettingsToggleBool);
    assert_eq!(
        state.settings.watcher.auto_reload, true,
        "auto_reload should be flipped back to true"
    );
}

#[test]
fn test_settings_toggle_bool_sets_dirty_flag() {
    // This test verifies that the SettingsToggleBool handler correctly sets
    // the dirty flag. This part of the implementation works correctly.

    let mut state = AppState::new();

    // Ensure dirty flag starts as false
    state.settings_view_state.dirty = false;
    assert!(!state.settings_view_state.dirty);

    // Handle the toggle message
    update(&mut state, Message::SettingsToggleBool);

    // Dirty flag should be set (this part works correctly)
    assert!(
        state.settings_view_state.dirty,
        "SettingsToggleBool should set the dirty flag"
    );
}

// ─────────────────────────────────────────────────────────
// Auto-Launch Handler Tests (Startup Flow Consistency)
// ─────────────────────────────────────────────────────────

#[test]
fn test_start_auto_launch_shows_loading_overlay() {
    use crate::config::LoadedConfigs;

    let mut state = AppState::new();
    let configs = LoadedConfigs::default();

    let result = update(&mut state, Message::StartAutoLaunch { configs });

    // Loading overlay is shown on top of normal UI
    assert!(state.loading_state.is_some());
    assert_eq!(state.ui_mode, UiMode::Loading);
    assert!(matches!(
        result.action,
        Some(UpdateAction::DiscoverDevicesAndAutoLaunch { .. })
    ));
}

#[test]
fn test_auto_launch_progress_updates_message() {
    let mut state = AppState::new();
    state.set_loading_phase("Initial");

    let result = update(
        &mut state,
        Message::AutoLaunchProgress {
            message: "Detecting devices...".to_string(),
        },
    );

    // Progress updates loading message
    assert!(state.loading_state.is_some());
    assert_eq!(
        state.loading_state.as_ref().unwrap().message,
        "Detecting devices..."
    );
    assert!(result.action.is_none());
}

#[test]
fn test_auto_launch_result_success_creates_session() {
    use crate::message::AutoLaunchSuccess;

    let mut state = AppState::new();
    // No loading state - auto-launch is silent

    let device = test_device("test-device", "Test Device");

    let success = AutoLaunchSuccess {
        device: device.clone(),
        config: None,
    };

    let result = update(
        &mut state,
        Message::AutoLaunchResult {
            result: Ok(success),
        },
    );

    // Still in Normal mode
    assert_eq!(state.ui_mode, UiMode::Normal);
    // Session created
    assert_eq!(state.session_manager.len(), 1);
    // SpawnSession action returned
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnSession { .. })
    ));
}

// Note: test_auto_launch_result_discovery_error_shows_dialog removed
// because StartupDialog and startup_dialog_state were replaced with NewSessionDialog
// in Phase 3 redesign. Auto-launch errors now show NewSessionDialog.

// ============================================================================
// Auto-Launch Flow Integration Tests (Phase 3 - Task 3)
// Auto-launch happens silently in background - no loading screen
// ============================================================================

mod auto_launch_tests {
    use super::*;
    use crate::config::{FlutterMode, LaunchConfig, LoadedConfigs};
    use crate::message::AutoLaunchSuccess;
    use std::path::PathBuf;

    #[test]
    fn test_auto_launch_flow_success() {
        let mut state = AppState::new();
        let project_path = PathBuf::from("/tmp/test");
        state.project_path = project_path.clone();

        // Step 1: StartAutoLaunch - shows loading overlay
        let configs = LoadedConfigs::default();
        let result = update(&mut state, Message::StartAutoLaunch { configs });

        assert_eq!(state.ui_mode, UiMode::Loading);
        assert!(state.loading_state.is_some());
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverDevicesAndAutoLaunch { .. })
        ));

        // Step 2: Progress update
        let _ = update(
            &mut state,
            Message::AutoLaunchProgress {
                message: "Detecting devices...".to_string(),
            },
        );

        assert!(state.loading_state.is_some());
        assert_eq!(
            state.loading_state.as_ref().unwrap().message,
            "Detecting devices..."
        );

        // Step 3: Successful result
        let device = test_device("test-device", "Test Device");

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        // Verify final state - loading cleared, back to normal
        assert!(state.loading_state.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert_eq!(state.session_manager.len(), 1); // Session created
        assert!(matches!(
            result.action,
            Some(UpdateAction::SpawnSession { .. })
        ));
    }

    #[test]
    fn test_auto_launch_with_config() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        // No loading state - auto-launch is silent

        // Skip to result with config
        let device = test_device("test-device", "Test Device");
        let config = LaunchConfig {
            name: "debug".to_string(),
            device: "auto".to_string(),
            mode: FlutterMode::Debug,
            flavor: Some("dev".to_string()),
            dart_defines: std::collections::HashMap::new(),
            entry_point: None,
            extra_args: vec![],
            auto_start: false,
        };

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: Some(config.clone()),
                }),
            },
        );

        // Verify session was created with config
        assert!(matches!(
            result.action,
            Some(UpdateAction::SpawnSession {
                config: Some(_),
                ..
            })
        ));

        // Verify the config is passed through
        if let Some(UpdateAction::SpawnSession { config: cfg, .. }) = result.action {
            assert!(cfg.is_some());
            let cfg = cfg.unwrap();
            assert_eq!(cfg.name, "debug");
            assert_eq!(cfg.flavor, Some("dev".to_string()));
        } else {
            assert!(
                matches!(result.action, Some(UpdateAction::SpawnSession { .. })),
                "Expected SpawnSession action with config, got {:?}",
                result.action
            );
        }
    }

    #[test]
    fn test_auto_launch_no_devices_shows_dialog() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        // No loading state - auto-launch is silent

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Err("No devices found".to_string()),
            },
        );

        // Shows new session dialog on error
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .as_ref()
            .unwrap()
            .contains("No devices"));
        assert!(result.action.is_none());
    }

    #[test]
    fn test_auto_launch_discovery_error() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        // No loading state - auto-launch is silent

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Err("Flutter SDK not found".to_string()),
            },
        );

        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .as_ref()
            .unwrap()
            .contains("Flutter SDK"));
        assert!(result.action.is_none());
    }

    #[test]
    fn test_auto_launch_progress_without_loading_is_safe() {
        let mut state = AppState::new();
        // No loading state set

        // Should not panic - progress is a no-op
        let result = update(
            &mut state,
            Message::AutoLaunchProgress {
                message: "Testing...".to_string(),
            },
        );

        assert!(result.action.is_none());
        // Loading state should still be None
        assert!(state.loading_state.is_none());
    }

    #[test]
    fn test_auto_launch_creates_session_with_correct_device() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        // No loading state - auto-launch is silent

        let device = test_device("android-123", "Pixel 5");

        update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        // Verify session was created
        assert_eq!(state.session_manager.len(), 1);

        // Verify device info matches
        let session_id = state.session_manager.selected_id().unwrap();
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.session.device_id, "android-123");
        assert_eq!(handle.session.device_name, "Pixel 5");
    }

    #[test]
    fn test_auto_launch_error_preserves_error_message() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        // No loading state - auto-launch is silent

        let error_msg = "Device offline: emulator-5554";

        update(
            &mut state,
            Message::AutoLaunchResult {
                result: Err(error_msg.to_string()),
            },
        );

        // Error message should be preserved in new session dialog
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .error
                .as_ref()
                .unwrap(),
            error_msg
        );
    }

    #[test]
    fn test_start_auto_launch_ignored_if_already_loading() {
        let mut state = AppState::new();
        // Simulate already in loading mode
        state.set_loading_phase("Already loading...");

        let configs = LoadedConfigs::default();
        let result = update(&mut state, Message::StartAutoLaunch { configs });

        // Should be ignored - no action spawned
        assert!(result.action.is_none());
        // Still in loading mode
        assert_eq!(state.ui_mode, UiMode::Loading);
    }

    // ─────────────────────────────────────────────────────────
    // Pre-app custom source gating for AutoLaunchResult
    // (pre-app-custom-sources Phase 1, followup Task 01)
    // ─────────────────────────────────────────────────────────

    /// Helper: build a `CustomSourceConfig` with `start_before_app = true`.
    fn pre_app_source(name: &str) -> crate::config::types::CustomSourceConfig {
        crate::config::types::CustomSourceConfig {
            name: name.to_string(),
            command: "server".to_string(),
            args: vec![],
            format: fdemon_core::types::OutputFormat::Raw,
            working_dir: None,
            env: std::collections::HashMap::new(),
            start_before_app: true,
            shared: false,
            ready_check: None,
        }
    }

    #[test]
    fn test_auto_launch_with_pre_app_sources_returns_spawn_pre_app() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");

        // Enable native logs with a pre-app source
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(pre_app_source("test-server"));

        let device = test_device("test-device", "Test Device");

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnPreAppSources { .. })),
            "Expected SpawnPreAppSources when pre-app sources are configured, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_auto_launch_without_pre_app_sources_returns_spawn_session() {
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");

        // Enable native logs but no pre-app sources (custom_sources is empty)
        state.settings.native_logs.enabled = true;

        let device = test_device("test-device", "Test Device");

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnSession { .. })),
            "Expected SpawnSession when no pre-app sources configured, got {:?}",
            result.action
        );
    }

    // ─────────────────────────────────────────────────────────
    // Pre-app gate skip: already-running shared sources (auto-launch path)
    // (pre-app-custom-sources Phase 2, Task 07)
    // ─────────────────────────────────────────────────────────

    /// Helper: build a shared `CustomSourceConfig` with `start_before_app = true`.
    fn shared_pre_app_source(name: &str) -> crate::config::types::CustomSourceConfig {
        crate::config::types::CustomSourceConfig {
            name: name.to_string(),
            command: "server".to_string(),
            args: vec![],
            format: fdemon_core::types::OutputFormat::Raw,
            working_dir: None,
            env: std::collections::HashMap::new(),
            start_before_app: true,
            shared: true,
            ready_check: None,
        }
    }

    /// Helper: push a `SharedSourceHandle` onto `state.shared_source_handles`
    /// to simulate an already-running shared source.
    fn mark_shared_source_running(state: &mut AppState, name: &str) {
        use crate::session::SharedSourceHandle;
        let (tx, _rx) = tokio::sync::watch::channel(false);
        state.shared_source_handles.push(SharedSourceHandle {
            name: name.to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: true,
        });
    }

    #[test]
    fn test_auto_launch_skips_gate_when_all_shared_pre_app_running() {
        // Second session scenario: the only pre-app source is shared and
        // already running. The gate should be skipped → SpawnSession.
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(shared_pre_app_source("logcat"));

        // Simulate the shared source already running
        mark_shared_source_running(&mut state, "logcat");

        let device = test_device("test-device", "Test Device");

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnSession { .. })),
            "Expected SpawnSession when all shared pre-app sources are already running, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_auto_launch_gates_when_non_shared_pre_app_present() {
        // Non-shared pre-app sources always require the gate regardless of
        // whether any shared sources are running.
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(shared_pre_app_source("logcat"));
        state
            .settings
            .native_logs
            .custom_sources
            .push(pre_app_source("my-server"));

        mark_shared_source_running(&mut state, "logcat");

        let device = test_device("test-device", "Test Device");

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnPreAppSources { .. })),
            "Expected SpawnPreAppSources when non-shared pre-app source is present, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_auto_launch_gates_when_shared_pre_app_not_yet_running() {
        // First session scenario: the shared source has never been started.
        // The gate must fire.
        let mut state = AppState::new();
        state.project_path = PathBuf::from("/tmp/test");
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(shared_pre_app_source("logcat"));

        // Do NOT mark the source as running

        let device = test_device("test-device", "Test Device");

        let result = update(
            &mut state,
            Message::AutoLaunchResult {
                result: Ok(AutoLaunchSuccess {
                    device: device.clone(),
                    config: None,
                }),
            },
        );

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnPreAppSources { .. })),
            "Expected SpawnPreAppSources when shared pre-app source not yet running, got {:?}",
            result.action
        );
    }

    // ─────────────────────────────────────────────────────────
    // Fuzzy Modal Handler Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_open_fuzzy_modal_for_flavor() {
        let mut state = AppState::new();
        state.new_session_dialog_state.launch_context.flavor = Some("existing".into());

        let _ = update(
            &mut state,
            Message::NewSessionDialogOpenFuzzyModal {
                modal_type: crate::new_session_dialog::FuzzyModalType::Flavor,
            },
        );

        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert_eq!(
            modal.modal_type,
            crate::new_session_dialog::FuzzyModalType::Flavor
        );
    }

    #[test]
    fn test_fuzzy_confirm_sets_flavor() {
        let mut state = AppState::new();
        state
            .new_session_dialog_state
            .open_flavor_modal(vec!["dev".into(), "staging".into()]);

        // Select "staging" (index 1)
        let _ = update(&mut state, Message::NewSessionDialogFuzzyDown);
        let _ = update(&mut state, Message::NewSessionDialogFuzzyConfirm);

        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
        assert_eq!(
            state.new_session_dialog_state.launch_context.flavor,
            Some("staging".into())
        );
    }

    #[test]
    fn test_fuzzy_custom_input() {
        let mut state = AppState::new();
        state.new_session_dialog_state.open_flavor_modal(vec![]); // Empty list

        // Type custom value
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 'c' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 'u' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 's' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 't' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyConfirm);

        assert_eq!(
            state.new_session_dialog_state.launch_context.flavor,
            Some("cust".into())
        );
    }

    #[test]
    fn test_fuzzy_modal_mutual_exclusion() {
        let mut state = AppState::new();

        // Open a fuzzy modal
        let _ = update(
            &mut state,
            Message::NewSessionDialogOpenFuzzyModal {
                modal_type: crate::new_session_dialog::FuzzyModalType::Flavor,
            },
        );
        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());

        // Try to open another modal while one is open - should be ignored
        let _ = update(
            &mut state,
            Message::NewSessionDialogOpenFuzzyModal {
                modal_type: crate::new_session_dialog::FuzzyModalType::Config,
            },
        );

        // Should still be Flavor modal, not Config
        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());
        assert_eq!(
            state
                .new_session_dialog_state
                .fuzzy_modal
                .as_ref()
                .unwrap()
                .modal_type,
            crate::new_session_dialog::FuzzyModalType::Flavor
        );
    }

    #[test]
    fn test_dart_defines_confirm_with_empty_key_returns_focus_to_key() {
        use crate::new_session_dialog::{DartDefinesEditField, DartDefinesPane};

        let mut state = AppState::new();

        // Open dart defines modal
        let _ = update(&mut state, Message::NewSessionDialogOpenDartDefinesModal);
        assert!(state.new_session_dialog_state.dart_defines_modal.is_some());

        // Switch to edit pane and navigate to Save button
        {
            let modal = state
                .new_session_dialog_state
                .dart_defines_modal
                .as_mut()
                .unwrap();
            modal.active_pane = DartDefinesPane::Edit;
            modal.edit_field = DartDefinesEditField::Save;
            modal.editing_key = "   ".into(); // Empty/whitespace key
            modal.editing_value = "some_value".into();
            modal.is_new = true;
        }

        // Confirm (press Enter on Save) - should fail and return focus to Key
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesConfirm);

        let modal = state
            .new_session_dialog_state
            .dart_defines_modal
            .as_ref()
            .unwrap();

        // Save should have failed (no new define added)
        assert!(modal.defines.is_empty());
        // Focus should return to Key field
        assert_eq!(modal.edit_field, DartDefinesEditField::Key);
    }

    // ─────────────────────────────────────────────────────────────
    // Target Selector Message Tests (Phase 5, Task 05)
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_switch_tab_to_connected() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Bootable;

        let _ = update(
            &mut state,
            Message::NewSessionDialogSwitchTab(TargetTab::Connected),
        );

        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Connected
        );
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            0
        );
    }

    #[test]
    fn test_switch_tab_to_bootable_triggers_discovery() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators = vec![]; // Empty
        state.new_session_dialog_state.target_selector.android_avds = vec![]; // Empty
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = false;

        let result = update(
            &mut state,
            Message::NewSessionDialogSwitchTab(TargetTab::Bootable),
        );

        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Bootable
        );
        assert!(
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverBootableDevices)
        ));
    }

    #[test]
    fn test_toggle_tab_switches_between_tabs() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;

        let _ = update(&mut state, Message::NewSessionDialogToggleTab);
        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Bootable
        );

        // Avoid triggering discovery for clean test
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = false;
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators = vec![]; // Add dummy to prevent discovery
        state.new_session_dialog_state.target_selector.android_avds = vec![]; // Add dummy to prevent discovery

        let _ = update(&mut state, Message::NewSessionDialogToggleTab);
        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Connected
        );
    }

    #[test]
    fn test_device_navigation_up_down() {
        let mut state = AppState::new();
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices = vec![
            test_device("d1", "Device 1"),
            test_device("d2", "Device 2"),
            test_device("d3", "Device 3"),
        ];
        // Start at first selectable device (index 1 in flat list, after header at 0)
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // Navigate down through devices (flat list has header at 0, devices at 1,2,3)
        let _ = update(&mut state, Message::NewSessionDialogDeviceDown);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            2
        );

        let _ = update(&mut state, Message::NewSessionDialogDeviceDown);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            3
        );

        // Wrap to first selectable (skips header at 0)
        let _ = update(&mut state, Message::NewSessionDialogDeviceDown);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            1
        );

        // Navigate up - wrap to end
        let _ = update(&mut state, Message::NewSessionDialogDeviceUp);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            3
        );
    }

    #[test]
    fn test_device_select_on_connected_tab() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices = vec![test_device("d1", "Device 1")];

        let result = update(&mut state, Message::NewSessionDialogDeviceSelect);

        // For now, just returns none (actual launch happens in Launch Context)
        assert!(result.action.is_none());
    }

    #[test]
    fn test_device_select_on_bootable_tab_triggers_boot() {
        use crate::new_session_dialog::TargetTab;
        use fdemon_daemon::{IosSimulator, SimulatorState};

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Bootable;
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators = vec![IosSimulator {
            udid: "sim-123".into(),
            name: "iPhone 15 Pro".into(),
            runtime: "iOS 17.2".into(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15 Pro".into(),
        }];
        // Flat list has header at 0, device at 1
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = update(&mut state, Message::NewSessionDialogDeviceSelect);

        assert!(matches!(
            result.action,
            Some(UpdateAction::BootDevice { .. })
        ));
    }

    #[test]
    fn test_refresh_devices_on_connected_tab() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;

        let result = update(&mut state, Message::NewSessionDialogRefreshDevices);

        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_refresh_devices_on_bootable_tab() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Bootable;

        let result = update(&mut state, Message::NewSessionDialogRefreshDevices);

        assert!(
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverBootableDevices)
        ));
    }

    #[test]
    fn test_connected_devices_received() {
        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.loading = true;

        let devices = vec![test_device("d1", "Device 1"), test_device("d2", "Device 2")];

        let _ = update(
            &mut state,
            Message::NewSessionDialogConnectedDevicesReceived(devices.clone()),
        );

        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .connected_devices
                .len(),
            2
        );
        assert!(!state.new_session_dialog_state.target_selector.loading);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .connected_devices[0]
                .id,
            "d1"
        );
    }

    #[test]
    fn test_bootable_devices_received() {
        use fdemon_daemon::{AndroidAvd, IosSimulator, SimulatorState};

        let mut state = AppState::new();
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = true;

        let ios_sims = vec![IosSimulator {
            udid: "sim-1".into(),
            name: "iPhone 15".into(),
            state: SimulatorState::Shutdown,
            runtime: "iOS 17.2".into(),
            device_type: "iPhone 15 Pro".into(),
        }];

        let android_avds = vec![AndroidAvd {
            name: "Pixel_8".into(),
            display_name: "Pixel 8 API 34".into(),
            api_level: Some(34),
            target: Some("android-34".into()),
        }];

        let _ = update(
            &mut state,
            Message::NewSessionDialogBootableDevicesReceived {
                ios_simulators: ios_sims,
                android_avds,
            },
        );

        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .ios_simulators
                .len(),
            1
        );
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .android_avds
                .len(),
            1
        );
        assert!(
            !state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .ios_simulators[0]
                .udid,
            "sim-1"
        );
    }

    #[test]
    fn test_device_discovery_failed() {
        let mut state = AppState::new();
        // Test connected discovery failure
        state.new_session_dialog_state.target_selector.loading = true;
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = true;

        let _ = update(
            &mut state,
            Message::NewSessionDialogDeviceDiscoveryFailed {
                error: "Connected discovery error".into(),
                discovery_type: crate::message::DiscoveryType::Connected,
            },
        );

        // NOTE: set_error() currently clears loading unconditionally, so both flags get cleared
        assert!(!state.new_session_dialog_state.target_selector.loading);
        assert!(
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        ); // Remains true
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());

        // Test bootable discovery failure
        state.new_session_dialog_state.target_selector.loading = true;
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = true;
        state.new_session_dialog_state.target_selector.error = None;

        let _ = update(
            &mut state,
            Message::NewSessionDialogDeviceDiscoveryFailed {
                error: "Bootable discovery error".into(),
                discovery_type: crate::message::DiscoveryType::Bootable,
            },
        );

        // NOTE: set_error() currently clears loading unconditionally
        assert!(!state.new_session_dialog_state.target_selector.loading); // Gets cleared by set_error
        assert!(
            !state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert_eq!(
            state.new_session_dialog_state.target_selector.error,
            Some("Bootable discovery error".into())
        );
    }

    #[test]
    fn test_boot_started() {
        use fdemon_daemon::{IosSimulator, SimulatorState};

        let mut state = AppState::new();
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators = vec![IosSimulator {
            udid: "sim-123".into(),
            name: "iPhone 15".into(),
            runtime: "iOS 17.2".into(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15 Pro".into(),
        }];

        let result = update(
            &mut state,
            Message::NewSessionDialogBootStarted {
                device_id: "sim-123".into(),
            },
        );

        // Boot started message is acknowledged but doesn't change state yet
        // (Device state tracking not implemented - boot process handled elsewhere)
        assert!(result.action.is_none());
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .ios_simulators[0]
                .state,
            SimulatorState::Shutdown // State unchanged
        );
    }

    #[test]
    fn test_boot_completed_switches_tab_and_triggers_refresh() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Bootable;

        let result = update(
            &mut state,
            Message::NewSessionDialogBootCompleted {
                device_id: "sim-123".into(),
            },
        );

        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Connected
        );
        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_boot_failed_sets_error() {
        let mut state = AppState::new();

        let _ = update(
            &mut state,
            Message::NewSessionDialogBootFailed {
                device_id: "sim-123".into(),
                error: "Boot timeout".into(),
            },
        );

        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .as_ref()
            .unwrap()
            .contains("sim-123"));
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .as_ref()
            .unwrap()
            .contains("Boot timeout"));
    }

    #[test]
    fn test_device_booted_redirects_to_boot_completed() {
        use crate::new_session_dialog::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Bootable;

        // Use deprecated message - should redirect
        let result = update(
            &mut state,
            Message::NewSessionDialogDeviceBooted {
                device_id: "sim-123".into(),
            },
        );

        // Should have same effect as NewSessionDialogBootCompleted
        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Connected
        );
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_switch_pane() {
        use crate::new_session_dialog::DialogPane;

        let mut state = AppState::new();
        state.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;

        let _ = update(&mut state, Message::NewSessionDialogSwitchPane);
        assert_eq!(
            state.new_session_dialog_state.focused_pane,
            DialogPane::LaunchContext
        );

        let _ = update(&mut state, Message::NewSessionDialogSwitchPane);
        assert_eq!(
            state.new_session_dialog_state.focused_pane,
            DialogPane::TargetSelector
        );
    }

    // ─────────────────────────────────────────────────────────
    // Device Selection Preservation Tests (Task 10)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_selection_preserved_on_background_refresh() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Startup;

        // Initial devices
        let initial_devices = vec![
            test_device("device-a", "iPhone"),
            test_device("device-b", "Pixel"),
        ];
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(initial_devices);

        // Select second device
        state.new_session_dialog_state.target_selector.select_next();
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_device_id(),
            Some("device-b".to_string())
        );

        // Background refresh returns devices in different order with new device
        let refreshed_devices = vec![
            test_device("device-c", "iPad"),
            test_device("device-b", "Pixel"), // Same device, different position
            test_device("device-a", "iPhone"),
        ];

        // Simulate DevicesDiscovered message
        let _ = update(
            &mut state,
            Message::DevicesDiscovered {
                devices: refreshed_devices,
            },
        );

        // Selection should still be device-b (Pixel), not device-c (iPad)
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_device_id(),
            Some("device-b".to_string())
        );
    }

    #[test]
    fn test_selection_resets_when_device_removed() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Startup;

        // Initial devices
        let initial_devices = vec![
            test_device("device-a", "iPhone"),
            test_device("device-b", "Pixel"),
        ];
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(initial_devices);

        // Select second device
        state.new_session_dialog_state.target_selector.select_next();

        // Refresh without the selected device
        let refreshed_devices = vec![
            test_device("device-a", "iPhone"),
            test_device("device-c", "iPad"),
        ];

        let _ = update(
            &mut state,
            Message::DevicesDiscovered {
                devices: refreshed_devices,
            },
        );

        // Selection should fall back to first device since device-b is gone
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_device_id(),
            Some("device-a".to_string())
        );
    }

    #[test]
    fn test_background_discovery_error_is_silent() {
        use crate::handler::new_session::handle_open_new_session_dialog;

        // Background errors should not show error UI when cached devices exist
        let mut state = AppState::new();

        // Set up cached devices
        state.set_device_cache(vec![test_device("cached-1", "Cached Phone")]);

        // Show new session dialog with cached devices via handler
        handle_open_new_session_dialog(&mut state);

        // Simulate background discovery failure
        let _ = update(
            &mut state,
            Message::DeviceDiscoveryFailed {
                error: "Network error".to_string(),
                is_background: true,
            },
        );

        // Cached devices should still be available
        assert!(!state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .is_empty());

        // No error should be shown to user
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_none());
    }

    #[test]
    fn test_foreground_discovery_error_shows_ui() {
        // Foreground errors should show error UI to user
        let mut state = AppState::new();

        // Show new session dialog
        let configs = crate::config::LoadedConfigs::default();
        state.show_new_session_dialog(configs);

        // Simulate foreground discovery failure
        let _ = update(
            &mut state,
            Message::DeviceDiscoveryFailed {
                error: "Flutter SDK not found".to_string(),
                is_background: false,
            },
        );

        // Error should be shown to user
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
        assert_eq!(
            state.new_session_dialog_state.target_selector.error,
            Some("Flutter SDK not found".to_string())
        );
    }

    #[test]
    fn test_foreground_discovery_error_shows_ui_startup_mode() {
        // Foreground errors should show error UI in Startup mode
        let mut state = AppState::new();
        state.ui_mode = UiMode::Startup;

        // Simulate foreground discovery failure
        let _ = update(
            &mut state,
            Message::DeviceDiscoveryFailed {
                error: "Connection timeout".to_string(),
                is_background: false,
            },
        );

        // Error should be shown to user
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
        assert_eq!(
            state.new_session_dialog_state.target_selector.error,
            Some("Connection timeout".to_string())
        );
    }

    #[test]
    fn test_background_error_does_not_show_ui_normal_mode() {
        // Background errors should not show error UI even when not in dialog mode
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;

        // Simulate background discovery failure
        let _ = update(
            &mut state,
            Message::DeviceDiscoveryFailed {
                error: "Background refresh failed".to_string(),
                is_background: true,
            },
        );

        // No error should be shown to user
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_none());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for Bug 2: Bootable Device Discovery at Startup (Phase 1, Task 02)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tool_availability_triggers_bootable_discovery() {
    use fdemon_daemon::ToolAvailability;

    let mut state = AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;

    let availability = ToolAvailability {
        xcrun_simctl: true,
        android_emulator: false,
        emulator_path: None,
        adb: false,
        #[cfg(target_os = "macos")]
        macos_log: false,
        #[cfg(target_os = "macos")]
        idevicesyslog: false,
    };

    let result = update(
        &mut state,
        Message::ToolAvailabilityChecked { availability },
    );

    assert!(state.tool_availability.xcrun_simctl);
    assert!(
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading
    );
    assert!(matches!(
        result.action,
        Some(UpdateAction::DiscoverBootableDevices)
    ));
}

#[test]
fn test_no_tools_available_no_discovery() {
    use fdemon_daemon::ToolAvailability;

    let mut state = AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;

    let availability = ToolAvailability {
        xcrun_simctl: false,
        android_emulator: false,
        emulator_path: None,
        adb: false,
        #[cfg(target_os = "macos")]
        macos_log: false,
        #[cfg(target_os = "macos")]
        idevicesyslog: false,
    };

    let result = update(
        &mut state,
        Message::ToolAvailabilityChecked { availability },
    );

    assert!(result.action.is_none());
}

#[test]
fn test_target_selector_default_shows_bootable_loading() {
    use crate::new_session_dialog::TargetSelectorState;

    let state = TargetSelectorState::default();
    assert!(state.bootable_loading);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for Bug 2: Bootable Device Caching (Phase 1, Task 03)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_bootable_devices_discovered_updates_cache() {
    use fdemon_daemon::{AndroidAvd, IosSimulator, SimulatorState};

    let mut state = AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;

    let ios_sims = vec![IosSimulator {
        udid: "sim-1".into(),
        name: "iPhone 15".into(),
        state: SimulatorState::Shutdown,
        runtime: "iOS 17.2".into(),
        device_type: "iPhone 15 Pro".into(),
    }];

    let android_avds = vec![AndroidAvd {
        name: "Pixel_8".into(),
        display_name: "Pixel 8 API 34".into(),
        api_level: Some(34),
        target: Some("android-34".into()),
    }];

    let _ = update(
        &mut state,
        Message::BootableDevicesDiscovered {
            ios_simulators: ios_sims.clone(),
            android_avds: android_avds.clone(),
        },
    );

    // Verify cache was updated
    assert!(state.ios_simulators_cache.is_some());
    assert!(state.android_avds_cache.is_some());
    assert!(state.bootable_last_updated.is_some());

    // Verify dialog was updated
    assert_eq!(
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators
            .len(),
        1
    );
    assert_eq!(
        state
            .new_session_dialog_state
            .target_selector
            .android_avds
            .len(),
        1
    );
    assert!(
        !state
            .new_session_dialog_state
            .target_selector
            .bootable_loading
    );
}

#[test]
fn test_bootable_cache_persists_across_dialog_reopens() {
    use crate::handler::new_session::handle_open_new_session_dialog;
    use fdemon_daemon::{AndroidAvd, IosSimulator, SimulatorState};

    let mut state = AppState::new();

    // First: Discover devices and cache them
    let ios_sims = vec![IosSimulator {
        udid: "sim-1".into(),
        name: "iPhone 15".into(),
        state: SimulatorState::Shutdown,
        runtime: "iOS 17.2".into(),
        device_type: "iPhone 15 Pro".into(),
    }];

    let android_avds = vec![AndroidAvd {
        name: "Pixel_8".into(),
        display_name: "Pixel 8 API 34".into(),
        api_level: Some(34),
        target: Some("android-34".into()),
    }];

    state.set_bootable_cache(ios_sims.clone(), android_avds.clone());

    // Open dialog via handler - should use cache
    handle_open_new_session_dialog(&mut state);

    assert_eq!(
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators
            .len(),
        1
    );
    assert_eq!(
        state
            .new_session_dialog_state
            .target_selector
            .android_avds
            .len(),
        1
    );
    // Should not show loading because cache was used
    assert!(
        !state
            .new_session_dialog_state
            .target_selector
            .bootable_loading
    );

    // Close dialog
    state.hide_new_session_dialog();

    // Reopen dialog via handler - should still use cache
    handle_open_new_session_dialog(&mut state);

    assert_eq!(
        state
            .new_session_dialog_state
            .target_selector
            .ios_simulators
            .len(),
        1
    );
    assert_eq!(
        state
            .new_session_dialog_state
            .target_selector
            .android_avds
            .len(),
        1
    );
    assert!(
        !state
            .new_session_dialog_state
            .target_selector
            .bootable_loading
    );
}

// ─────────────────────────────────────────────────────────
// VM Service Message Tests (Phase 1 DevTools Integration)
// ─────────────────────────────────────────────────────────

#[test]
fn test_vm_service_connected_sets_flag() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(&mut state, Message::VmServiceConnected { session_id });

    // VmServiceConnected now triggers StartPerformanceMonitoring
    assert!(
        matches!(
            result.action,
            Some(UpdateAction::StartPerformanceMonitoring { .. })
        ),
        "VmServiceConnected should trigger StartPerformanceMonitoring"
    );
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.session.vm_connected,
        "vm_connected should be true after VmServiceConnected"
    );
}

#[test]
fn test_vm_service_connected_ignores_unknown_session() {
    let mut state = AppState::new();

    // Should not panic when session doesn't exist, but still returns the action
    let result = update(&mut state, Message::VmServiceConnected { session_id: 9999 });

    // Action is still returned even if session not found (process.rs handles discarding)
    assert!(matches!(
        result.action,
        Some(UpdateAction::StartPerformanceMonitoring { .. })
    ));
}

#[test]
fn test_vm_service_disconnected_clears_flag() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // First connect (now returns StartPerformanceMonitoring action)
    update(&mut state, Message::VmServiceConnected { session_id });
    {
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(handle.session.vm_connected, "Should be connected first");
    }

    // Then disconnect
    let result = update(&mut state, Message::VmServiceDisconnected { session_id });

    assert!(result.action.is_none());
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        !handle.session.vm_connected,
        "vm_connected should be false after VmServiceDisconnected"
    );
}

// ─────────────────────────────────────────────────────────
// VM Service Reconnection Tests (Phase 2, Task 07)
// ─────────────────────────────────────────────────────────

#[test]
fn test_vm_service_reconnecting_sets_connection_status() {
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    // Verify initial state
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected
    );

    // Action
    let result = update(
        &mut state,
        Message::VmServiceReconnecting {
            session_id,
            attempt: 2,
            max_attempts: 10,
        },
    );

    // Assert
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Reconnecting {
            attempt: 2,
            max_attempts: 10,
        }
    );
    assert!(result.action.is_none());
}

#[test]
fn test_vm_service_reconnecting_ignores_inactive_session() {
    let mut state = AppState::new();
    let device1 = test_device("dev-1", "Device 1");
    let device2 = test_device("dev-2", "Device 2");
    let session_1 = state.session_manager.create_session(&device1).unwrap();
    let session_2 = state.session_manager.create_session(&device2).unwrap();

    // Select session 2 (session 1 is inactive)
    state.session_manager.select_by_id(session_2);

    // Action: reconnecting event for inactive session 1
    update(
        &mut state,
        Message::VmServiceReconnecting {
            session_id: session_1,
            attempt: 3,
            max_attempts: 10,
        },
    );

    // Assert: connection_status should NOT be Reconnecting (it's for inactive session)
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
        "connection_status should not change for inactive session"
    );
}

#[test]
fn test_vm_service_connected_after_reconnecting_resets_status() {
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    // First: simulate reconnecting
    update(
        &mut state,
        Message::VmServiceReconnecting {
            session_id,
            attempt: 1,
            max_attempts: 10,
        },
    );
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Reconnecting {
            attempt: 1,
            max_attempts: 10,
        }
    );

    // Then: simulate successful reconnection
    update(&mut state, Message::VmServiceConnected { session_id });

    // Assert: status should be back to Connected
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
    );
}

#[test]
fn test_vm_service_reconnecting_progressive_attempts() {
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    for attempt in 1..=3 {
        update(
            &mut state,
            Message::VmServiceReconnecting {
                session_id,
                attempt,
                max_attempts: 10,
            },
        );
        assert_eq!(
            state.devtools_view_state.connection_status,
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts: 10,
            }
        );
    }
}

#[test]
fn test_vm_service_flutter_error_adds_log() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    let initial_log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();

    let log_entry = fdemon_core::LogEntry::error(
        fdemon_core::LogSource::VmService,
        "Test Flutter error".to_string(),
    );

    let result = update(
        &mut state,
        Message::VmServiceFlutterError {
            session_id,
            log_entry,
        },
    );

    assert!(result.action.is_none());
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.session.logs.len(),
        initial_log_count + 1,
        "Log should be added for VmServiceFlutterError"
    );
    assert_eq!(
        handle.session.logs.back().unwrap().message,
        "Test Flutter error"
    );
}

#[test]
fn test_vm_service_log_record_adds_log() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    let initial_log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();

    let log_entry = fdemon_core::LogEntry::new(
        fdemon_core::LogLevel::Warning,
        fdemon_core::LogSource::VmService,
        "Test log record".to_string(),
    );

    let result = update(
        &mut state,
        Message::VmServiceLogRecord {
            session_id,
            log_entry,
        },
    );

    assert!(result.action.is_none());
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.session.logs.len(),
        initial_log_count + 1,
        "Log should be added for VmServiceLogRecord"
    );
    assert_eq!(
        handle.session.logs.back().unwrap().message,
        "Test log record"
    );
    assert_eq!(
        handle.session.logs.back().unwrap().level,
        fdemon_core::LogLevel::Warning
    );
}

#[test]
fn test_duplicate_log_detection_filters_vm_error() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Create a log entry — both sends use clone() so same timestamp triggers dedup
    let log_entry = fdemon_core::LogEntry::error(
        fdemon_core::LogSource::VmService,
        "Duplicate error message".to_string(),
    );

    // First entry — should be added
    update(
        &mut state,
        Message::VmServiceFlutterError {
            session_id,
            log_entry: log_entry.clone(),
        },
    );

    let count_after_first = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();

    // Second entry with identical message and same timestamp — should be deduped
    update(
        &mut state,
        Message::VmServiceFlutterError {
            session_id,
            log_entry: log_entry.clone(),
        },
    );

    let count_after_second = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        count_after_first, count_after_second,
        "Duplicate log within 100ms threshold should be filtered"
    );
}

#[test]
fn test_connection_failure_does_not_crash() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    let initial_log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();

    // Should not panic and should not modify session state
    let result = update(
        &mut state,
        Message::VmServiceConnectionFailed {
            session_id,
            error: "Connection refused".to_string(),
        },
    );

    assert!(result.action.is_none());
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        !handle.session.vm_connected,
        "vm_connected should remain false on failure"
    );
    assert_eq!(
        handle.session.logs.len(),
        initial_log_count + 1,
        "Connection failure should add a warning log to session"
    );
}

#[test]
fn test_vm_service_attached_stores_shutdown_tx() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Verify no shutdown tx initially
    assert!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .vm_shutdown_tx
            .is_none(),
        "vm_shutdown_tx should be None initially"
    );

    // Create a watch channel and send VmServiceAttached
    let (tx, _rx) = tokio::sync::watch::channel(false);
    let vm_shutdown_tx = std::sync::Arc::new(tx);

    let result = update(
        &mut state,
        Message::VmServiceAttached {
            session_id,
            vm_shutdown_tx,
        },
    );

    assert!(result.action.is_none());
    assert!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .vm_shutdown_tx
            .is_some(),
        "vm_shutdown_tx should be stored after VmServiceAttached"
    );
}

// ─────────────────────────────────────────────────────────
// VM Service Performance Tests (Phase 3, Task 05)
// ─────────────────────────────────────────────────────────

#[test]
fn test_memory_snapshot_handler() {
    use fdemon_core::performance::MemoryUsage;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let memory = MemoryUsage {
        heap_usage: 50_000_000,
        heap_capacity: 100_000_000,
        external_usage: 10_000_000,
        timestamp: chrono::Local::now(),
    };

    let msg = Message::VmServiceMemorySnapshot {
        session_id,
        memory: memory.clone(),
    };
    let result = update(&mut state, msg);

    assert!(
        result.action.is_none(),
        "VmServiceMemorySnapshot should have no action"
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(perf.memory_history.len(), 1);
    assert_eq!(perf.memory_history.latest().unwrap().heap_usage, 50_000_000);
    assert!(
        perf.monitoring_active,
        "monitoring_active should be set to true"
    );
}

#[test]
fn test_gc_event_handler() {
    use fdemon_core::performance::GcEvent;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // MarkSweep is a major GC event and should be stored.
    let gc = GcEvent {
        gc_type: "MarkSweep".into(),
        reason: Some("allocation".into()),
        isolate_id: None,
        timestamp: chrono::Local::now(),
    };

    let msg = Message::VmServiceGcEvent {
        session_id,
        gc_event: gc,
    };
    let result = update(&mut state, msg);

    assert!(
        result.action.is_none(),
        "VmServiceGcEvent should have no action"
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(perf.gc_history.len(), 1);
    assert_eq!(perf.gc_history.latest().unwrap().gc_type, "MarkSweep");
}

#[test]
fn test_scavenge_gc_events_filtered() {
    use fdemon_core::performance::GcEvent;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Scavenge is a minor GC event and should be filtered out.
    let msg = Message::VmServiceGcEvent {
        session_id,
        gc_event: GcEvent {
            gc_type: "Scavenge".into(),
            reason: Some("allocation".into()),
            isolate_id: None,
            timestamp: chrono::Local::now(),
        },
    };
    let result = update(&mut state, msg);

    assert!(
        result.action.is_none(),
        "VmServiceGcEvent should have no action"
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(
        perf.gc_history.len(),
        0,
        "Scavenge events should be filtered out of gc_history"
    );
}

#[test]
fn test_major_gc_events_stored() {
    use fdemon_core::performance::GcEvent;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Send a MarkSweep and a MarkCompact event — both should be stored.
    for gc_type in ["MarkSweep", "MarkCompact"] {
        let msg = Message::VmServiceGcEvent {
            session_id,
            gc_event: GcEvent {
                gc_type: gc_type.into(),
                reason: None,
                isolate_id: None,
                timestamp: chrono::Local::now(),
            },
        };
        update(&mut state, msg);
    }

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(
        perf.gc_history.len(),
        2,
        "Both MarkSweep and MarkCompact events should be stored in gc_history"
    );
}

#[test]
fn test_vm_connected_starts_monitoring() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let msg = Message::VmServiceConnected { session_id };
    let result = update(&mut state, msg);

    // Should trigger StartPerformanceMonitoring action
    assert!(
        matches!(
            result.action,
            Some(UpdateAction::StartPerformanceMonitoring { .. })
        ),
        "VmServiceConnected should trigger StartPerformanceMonitoring"
    );
}

#[test]
fn test_vm_connected_resets_performance_state() {
    use fdemon_core::performance::MemoryUsage;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add some performance data to simulate stale data from previous connection
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.performance.memory_history.push(MemoryUsage {
            heap_usage: 1_000_000,
            heap_capacity: 2_000_000,
            external_usage: 0,
            timestamp: chrono::Local::now(),
        });
        handle.session.performance.monitoring_active = true;
    }

    // Reconnect — should reset performance state
    update(&mut state, Message::VmServiceConnected { session_id });

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert!(
        perf.memory_history.is_empty(),
        "memory_history should be cleared on reconnect"
    );
    assert!(
        !perf.monitoring_active,
        "monitoring_active should be reset on reconnect"
    );
}

#[test]
fn test_vm_disconnected_stops_monitoring() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Simulate monitoring being active
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.performance.monitoring_active = true;
    }

    let msg = Message::VmServiceDisconnected { session_id };
    update(&mut state, msg);

    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        !handle.session.performance.monitoring_active,
        "monitoring_active should be false after VmServiceDisconnected"
    );
    assert!(
        handle.perf_shutdown_tx.is_none(),
        "perf_shutdown_tx should be cleared after VmServiceDisconnected"
    );
}

#[test]
fn test_performance_monitoring_started_stores_shutdown_tx() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Verify no perf_shutdown_tx initially
    assert!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .perf_shutdown_tx
            .is_none(),
        "perf_shutdown_tx should be None initially"
    );

    // Create a watch channel and send VmServicePerformanceMonitoringStarted
    let (tx, _rx) = tokio::sync::watch::channel(false);
    let perf_shutdown_tx = std::sync::Arc::new(tx);

    let result = update(
        &mut state,
        Message::VmServicePerformanceMonitoringStarted {
            session_id,
            perf_shutdown_tx,
            perf_task_handle: std::sync::Arc::new(std::sync::Mutex::new(None)),
        },
    );

    assert!(result.action.is_none());
    assert!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .perf_shutdown_tx
            .is_some(),
        "perf_shutdown_tx should be stored after VmServicePerformanceMonitoringStarted"
    );
}

#[test]
fn test_memory_snapshot_ignored_for_unknown_session() {
    use fdemon_core::performance::MemoryUsage;

    let mut state = AppState::new();

    // Should not panic for unknown session
    let result = update(
        &mut state,
        Message::VmServiceMemorySnapshot {
            session_id: 9999,
            memory: MemoryUsage {
                heap_usage: 1000,
                heap_capacity: 2000,
                external_usage: 0,
                timestamp: chrono::Local::now(),
            },
        },
    );
    assert!(result.action.is_none());
}

#[test]
fn test_gc_event_ignored_for_unknown_session() {
    use fdemon_core::performance::GcEvent;

    let mut state = AppState::new();

    // Should not panic for unknown session
    let result = update(
        &mut state,
        Message::VmServiceGcEvent {
            session_id: 9999,
            gc_event: GcEvent {
                gc_type: "Scavenge".into(),
                reason: None,
                isolate_id: None,
                timestamp: chrono::Local::now(),
            },
        },
    );
    assert!(result.action.is_none());
}

// ─────────────────────────────────────────────────────────
// VM Service Frame Timing Tests (Phase 3, Task 06)
// ─────────────────────────────────────────────────────────

#[test]
fn test_frame_timing_handler() {
    use fdemon_core::performance::FrameTiming;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let timing = FrameTiming {
        number: 1,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    };

    let msg = Message::VmServiceFrameTiming { session_id, timing };
    let result = update(&mut state, msg);

    assert!(
        result.action.is_none(),
        "VmServiceFrameTiming should have no action"
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(perf.frame_history.len(), 1);
    assert_eq!(perf.frame_history.latest().unwrap().elapsed_micros, 10_000);
}

#[test]
fn test_frame_timing_stats_recomputed_every_interval() {
    use fdemon_core::performance::FrameTiming;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Push STATS_RECOMPUTE_INTERVAL - 1 frames: stats should still be default
    for i in 0..9 {
        update(
            &mut state,
            Message::VmServiceFrameTiming {
                session_id,
                timing: FrameTiming {
                    number: i,
                    build_micros: 5_000,
                    raster_micros: 5_000,
                    elapsed_micros: 10_000,
                    timestamp: chrono::Local::now(),
                    phases: None,
                    shader_compilation: false,
                },
            },
        );
    }

    // Stats not yet recomputed (interval is 10)
    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(
        perf.stats.buffered_frames, 0,
        "stats should not be recomputed yet"
    );

    // Push the 10th frame — triggers recomputation
    update(
        &mut state,
        Message::VmServiceFrameTiming {
            session_id,
            timing: FrameTiming {
                number: 9,
                build_micros: 5_000,
                raster_micros: 5_000,
                elapsed_micros: 10_000,
                timestamp: chrono::Local::now(),
                phases: None,
                shader_compilation: false,
            },
        },
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(
        perf.stats.buffered_frames, 10,
        "stats should be recomputed after 10 frames"
    );
}

#[test]
fn test_frame_timing_ignored_for_unknown_session() {
    use fdemon_core::performance::FrameTiming;

    let mut state = AppState::new();

    // Should not panic for unknown session
    let result = update(
        &mut state,
        Message::VmServiceFrameTiming {
            session_id: 9999,
            timing: FrameTiming {
                number: 1,
                build_micros: 5_000,
                raster_micros: 5_000,
                elapsed_micros: 10_000,
                timestamp: chrono::Local::now(),
                phases: None,
                shader_compilation: false,
            },
        },
    );
    assert!(result.action.is_none());
}

// ─────────────────────────────────────────────────────────
// selected_frame ring-buffer wrap compensation tests
// (Phase 3 Fixes, Task 03)
// ─────────────────────────────────────────────────────────

/// Helper: build a minimal `FrameTiming` with the given frame number.
fn make_frame_timing(number: u64) -> fdemon_core::performance::FrameTiming {
    fdemon_core::performance::FrameTiming {
        number,
        build_micros: 5_000,
        raster_micros: 5_000,
        elapsed_micros: 10_000,
        timestamp: chrono::Local::now(),
        phases: None,
        shader_compilation: false,
    }
}

/// Helper: retrieve `selected_frame` from the given session.
fn get_selected_frame(state: &AppState, session_id: crate::session::SessionId) -> Option<usize> {
    state
        .session_manager
        .get(session_id)
        .map(|h| h.session.performance.selected_frame)
        .flatten()
}

#[test]
fn test_selected_frame_decrements_on_buffer_wrap() {
    use crate::session::performance::DEFAULT_FRAME_HISTORY_SIZE;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Fill the buffer to capacity (300 frames).
    for i in 0..DEFAULT_FRAME_HISTORY_SIZE as u64 {
        update(
            &mut state,
            Message::VmServiceFrameTiming {
                session_id,
                timing: make_frame_timing(i),
            },
        );
    }

    // Verify buffer is now full.
    {
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(perf.frame_history.len(), DEFAULT_FRAME_HISTORY_SIZE);
        assert!(perf.frame_history.is_full());
    }

    // Select frame at index 50.
    update(
        &mut state,
        Message::SelectPerformanceFrame { index: Some(50) },
    );
    assert_eq!(get_selected_frame(&state, session_id), Some(50));

    // Push one more frame — causes eviction of oldest, shifting all indices by -1.
    update(
        &mut state,
        Message::VmServiceFrameTiming {
            session_id,
            timing: make_frame_timing(DEFAULT_FRAME_HISTORY_SIZE as u64),
        },
    );

    // selected_frame must decrement to 49 to continue pointing at the same logical frame.
    assert_eq!(
        get_selected_frame(&state, session_id),
        Some(49),
        "selected_frame should decrement from 50 to 49 after one eviction"
    );
}

#[test]
fn test_selected_frame_clears_when_evicted() {
    use crate::session::performance::DEFAULT_FRAME_HISTORY_SIZE;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Fill the buffer to capacity.
    for i in 0..DEFAULT_FRAME_HISTORY_SIZE as u64 {
        update(
            &mut state,
            Message::VmServiceFrameTiming {
                session_id,
                timing: make_frame_timing(i),
            },
        );
    }

    // Select the oldest frame (index 0).
    update(
        &mut state,
        Message::SelectPerformanceFrame { index: Some(0) },
    );
    assert_eq!(get_selected_frame(&state, session_id), Some(0));

    // Push one more frame — the oldest (index 0) is evicted.
    update(
        &mut state,
        Message::VmServiceFrameTiming {
            session_id,
            timing: make_frame_timing(DEFAULT_FRAME_HISTORY_SIZE as u64),
        },
    );

    // selected_frame must become None because the selected frame was evicted.
    assert_eq!(
        get_selected_frame(&state, session_id),
        None,
        "selected_frame should become None when the selected frame is evicted"
    );
}

#[test]
fn test_selected_frame_unchanged_when_buffer_not_full() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Push only 5 frames — well below capacity (300), no eviction.
    for i in 0..5_u64 {
        update(
            &mut state,
            Message::VmServiceFrameTiming {
                session_id,
                timing: make_frame_timing(i),
            },
        );
    }

    // Select frame at index 2.
    update(
        &mut state,
        Message::SelectPerformanceFrame { index: Some(2) },
    );
    assert_eq!(get_selected_frame(&state, session_id), Some(2));

    // Push one more frame — buffer still has room, no eviction occurs.
    update(
        &mut state,
        Message::VmServiceFrameTiming {
            session_id,
            timing: make_frame_timing(5),
        },
    );

    // selected_frame must remain unchanged.
    assert_eq!(
        get_selected_frame(&state, session_id),
        Some(2),
        "selected_frame should not change when the buffer is not full"
    );
}

#[test]
fn test_memory_snapshot_triggers_stats_recompute() {
    use fdemon_core::performance::{FrameTiming, MemoryUsage};

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Add some frames directly (without triggering interval recompute)
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        for i in 0..5 {
            handle.session.performance.frame_history.push(FrameTiming {
                number: i,
                build_micros: 5_000,
                raster_micros: 5_000,
                elapsed_micros: 10_000,
                timestamp: chrono::Local::now(),
                phases: None,
                shader_compilation: false,
            });
        }
    }

    // Stats are still default at this point (no recompute triggered yet)
    {
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(perf.stats.buffered_frames, 0);
    }

    // Send a memory snapshot — this should trigger recompute
    update(
        &mut state,
        Message::VmServiceMemorySnapshot {
            session_id,
            memory: MemoryUsage {
                heap_usage: 50_000_000,
                heap_capacity: 100_000_000,
                external_usage: 0,
                timestamp: chrono::Local::now(),
            },
        },
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(
        perf.stats.buffered_frames, 5,
        "memory snapshot should trigger stats recompute"
    );
}

// ─────────────────────────────────────────────────────────
// Perf Polling Lifecycle Tests (Phase 3 Fixes, Task 01)
// ─────────────────────────────────────────────────────────

/// Helper: attach a perf_shutdown_tx to a session handle.
/// Returns the watch receiver so the test can verify the signal.
fn attach_perf_shutdown(
    state: &mut AppState,
    session_id: crate::session::SessionId,
) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = state.session_manager.get_mut(session_id).unwrap();
    handle.perf_shutdown_tx = Some(std::sync::Arc::new(tx));
    rx
}

#[test]
fn test_close_session_signals_perf_shutdown() {
    // Setup: two sessions so close doesn't quit, one with perf_shutdown_tx
    let device1 = test_device("dev-1", "Device 1");
    let device2 = test_device("dev-2", "Device 2");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device1).unwrap();
    state.session_manager.create_session(&device2).unwrap();

    // Make session_id the selected one
    state.session_manager.select_by_id(session_id);

    let mut perf_rx = attach_perf_shutdown(&mut state, session_id);

    // Action: process CloseCurrentSession message
    super::session_lifecycle::handle_close_current_session(&mut state);

    // Assert: perf_shutdown_tx receiver sees true
    assert!(
        *perf_rx.borrow_and_update(),
        "perf_shutdown_tx should be signaled on CloseCurrentSession"
    );
}

#[test]
fn test_close_session_cleans_up_network_monitoring() {
    // Setup: two sessions so close doesn't quit, one with network monitoring active.
    // Uses a tokio runtime to create a real JoinHandle for network_task_handle.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device1 = test_device("dev-1", "Device 1");
        let device2 = test_device("dev-2", "Device 2");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device1).unwrap();
        state.session_manager.create_session(&device2).unwrap();

        // Make session_id the selected one
        state.session_manager.select_by_id(session_id);

        // Attach network_shutdown_tx and network_task_handle to the session
        let (tx, mut network_rx) = tokio::sync::watch::channel(false);
        let task: tokio::task::JoinHandle<()> =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(60)).await });
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.network_shutdown_tx = Some(std::sync::Arc::new(tx));
            handle.network_task_handle = Some(task);
        }

        // Confirm handles are set before close
        {
            let handle = state.session_manager.get(session_id).unwrap();
            assert!(
                handle.network_shutdown_tx.is_some(),
                "network_shutdown_tx should be Some before close"
            );
            assert!(
                handle.network_task_handle.is_some(),
                "network_task_handle should be Some before close"
            );
        }

        // Action: process CloseCurrentSession
        super::session_lifecycle::handle_close_current_session(&mut state);

        // Assert: network_shutdown_tx was signaled (receiver sees true) — this
        // verifies the channel was sent `true` before being dropped/taken.
        assert!(
            *network_rx.borrow_and_update(),
            "network_shutdown_tx should be signaled on CloseCurrentSession"
        );

        // Assert: session was removed from manager (close succeeded)
        assert!(
            state.session_manager.get(session_id).is_none(),
            "closed session should be removed from manager after CloseCurrentSession"
        );
    });
}

#[test]
fn test_session_exited_signals_perf_shutdown() {
    // Setup: create session with perf_shutdown_tx set
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let mut perf_rx = attach_perf_shutdown(&mut state, session_id);

    // Action: process SessionExited (via handle_session_exited)
    super::session::handle_session_exited(&mut state, session_id, Some(0));

    // Assert: perf_shutdown_tx receiver sees true
    assert!(
        *perf_rx.borrow_and_update(),
        "perf_shutdown_tx should be signaled on handle_session_exited"
    );

    // Assert: monitoring_active is false
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        !handle.session.performance.monitoring_active,
        "monitoring_active should be false after process exit"
    );
    assert!(
        handle.perf_shutdown_tx.is_none(),
        "perf_shutdown_tx should be cleared after process exit"
    );
}

#[test]
fn test_session_exited_cleans_up_network_monitoring() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let mut network_rx = attach_network_shutdown(&mut state, session_id);

    // Action
    super::session::handle_session_exited(&mut state, session_id, Some(0));

    // Assert: shutdown signal was sent
    assert!(
        *network_rx.borrow_and_update(),
        "network_shutdown_tx should be signaled on handle_session_exited"
    );

    // Assert: field was cleared
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.network_shutdown_tx.is_none(),
        "network_shutdown_tx should be cleared after process exit"
    );
    assert!(
        handle.network_task_handle.is_none(),
        "network_task_handle should be None after process exit"
    );
}

#[test]
fn test_app_stop_signals_perf_shutdown() {
    use fdemon_core::{AppStart, AppStop, DaemonMessage};

    // Setup: create session with perf_shutdown_tx and app_id
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark session as started with a known app_id
    let start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "dev-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });
    super::session::handle_session_message_state(&mut state, session_id, &start_msg);

    let mut perf_rx = attach_perf_shutdown(&mut state, session_id);

    // Action: process Daemon(AppStop) via handle_session_message_state
    let stop_msg = DaemonMessage::AppStop(AppStop {
        app_id: "test-app".to_string(),
        error: None,
    });
    super::session::handle_session_message_state(&mut state, session_id, &stop_msg);

    // Assert: perf_shutdown_tx receiver sees true
    assert!(
        *perf_rx.borrow_and_update(),
        "perf_shutdown_tx should be signaled on AppStop"
    );

    // Assert: monitoring_active is false
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        !handle.session.performance.monitoring_active,
        "monitoring_active should be false after AppStop"
    );
    assert!(
        handle.perf_shutdown_tx.is_none(),
        "perf_shutdown_tx should be cleared after AppStop"
    );
}

#[test]
fn test_app_stop_cleans_up_network_monitoring() {
    use fdemon_core::{AppStart, AppStop, DaemonMessage};

    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark session as started with a known app_id
    let start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "dev-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });
    super::session::handle_session_message_state(&mut state, session_id, &start_msg);

    let mut network_rx = attach_network_shutdown(&mut state, session_id);

    // Action
    let stop_msg = DaemonMessage::AppStop(AppStop {
        app_id: "test-app".to_string(),
        error: None,
    });
    super::session::handle_session_message_state(&mut state, session_id, &stop_msg);

    // Assert: shutdown signal was sent
    assert!(
        *network_rx.borrow_and_update(),
        "network_shutdown_tx should be signaled on AppStop"
    );

    // Assert: field was cleared
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.network_shutdown_tx.is_none(),
        "network_shutdown_tx should be cleared after AppStop"
    );
    assert!(
        handle.network_task_handle.is_none(),
        "network_task_handle should be None after AppStop"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// DevTools: RequestWidgetTree / RequestLayoutData VM guard regression tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_request_widget_tree_without_vm_sets_error() {
    // Setup: session with vm_connected = false (the default).
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(&mut state, Message::RequestWidgetTree { session_id });

    // No action should be returned — the handler bails out early.
    assert!(
        result.action.is_none(),
        "Should not return an action when VM is not connected"
    );
    // loading must not be set — doing so would leave the UI stuck.
    assert!(
        !state.devtools_view_state.inspector.loading,
        "inspector.loading must remain false when VM is not connected"
    );
    // A user-visible error message should be set instead.
    assert!(
        state.devtools_view_state.inspector.error.is_some(),
        "inspector.error should be set when VM is not connected"
    );
    let error = state.devtools_view_state.inspector.error.as_ref().unwrap();
    assert!(
        error.message.contains("VM Service") || error.hint.contains("debug mode"),
        "Error message should mention VM Service or debug mode: {:?}",
        error
    );
}

#[test]
fn test_request_widget_tree_with_vm_sets_loading() {
    // Setup: session with vm_connected = true.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark the session as VM-connected.
    update(&mut state, Message::VmServiceConnected { session_id });

    let result = update(&mut state, Message::RequestWidgetTree { session_id });

    // Should set loading = true and return a FetchWidgetTree action.
    assert!(
        state.devtools_view_state.inspector.loading,
        "inspector.loading should be true when VM is connected"
    );
    assert!(
        matches!(result.action, Some(UpdateAction::FetchWidgetTree { .. })),
        "Should return FetchWidgetTree action when VM is connected"
    );
    // Error must be cleared (not set) when successfully starting a fetch.
    assert!(
        state.devtools_view_state.inspector.error.is_none(),
        "inspector.error should not be set when VM is connected"
    );
}

#[test]
fn test_request_layout_data_without_vm_sets_error() {
    // Setup: session with vm_connected = false (the default).
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(
        &mut state,
        Message::RequestLayoutData {
            session_id,
            node_id: "node-123".to_string(),
        },
    );

    // No action should be returned.
    assert!(
        result.action.is_none(),
        "Should not return an action when VM is not connected"
    );
    // loading must not be set.
    assert!(
        !state.devtools_view_state.inspector.layout_loading,
        "inspector.layout_loading must remain false when VM is not connected"
    );
    // A user-visible error message should be set.
    assert!(
        state.devtools_view_state.inspector.layout_error.is_some(),
        "inspector.layout_error should be set when VM is not connected"
    );
    let error = state
        .devtools_view_state
        .inspector
        .layout_error
        .as_ref()
        .unwrap();
    assert!(
        error.message.contains("VM Service") || error.hint.contains("debug mode"),
        "Error message should mention VM Service or debug mode: {:?}",
        error
    );
}

#[test]
fn test_request_layout_data_with_vm_sets_loading() {
    // Setup: session with vm_connected = true.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark the session as VM-connected.
    update(&mut state, Message::VmServiceConnected { session_id });

    let result = update(
        &mut state,
        Message::RequestLayoutData {
            session_id,
            node_id: "node-123".to_string(),
        },
    );

    // Should set loading = true and return a FetchLayoutData action.
    assert!(
        state.devtools_view_state.inspector.layout_loading,
        "inspector.layout_loading should be true when VM is connected"
    );
    assert!(
        matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
        "Should return FetchLayoutData action when VM is connected"
    );
    assert!(
        state.devtools_view_state.inspector.layout_error.is_none(),
        "inspector.layout_error should not be set when VM is connected"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// VM Connection Lifecycle Regression Tests (Phase 4 Fix: Task 02)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_vm_disconnected_clears_shutdown_tx() {
    // Regression: VmServiceDisconnected must clear vm_shutdown_tx so that
    // maybe_connect_vm_service can attempt a fresh connection on the next
    // AppDebugPort message.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Simulate VmServiceAttached having stored a shutdown sender.
    let (tx, _rx) = tokio::sync::watch::channel(false);
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.vm_shutdown_tx = Some(std::sync::Arc::new(tx));
        assert!(
            handle.vm_shutdown_tx.is_some(),
            "vm_shutdown_tx should be Some before disconnect"
        );
    }

    update(&mut state, Message::VmServiceDisconnected { session_id });

    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.vm_shutdown_tx.is_none(),
        "vm_shutdown_tx must be None after VmServiceDisconnected to allow reconnection"
    );
}

#[test]
fn test_vm_connection_failed_sets_devtools_error() {
    // VmServiceConnectionFailed should store a human-readable error in
    // DevToolsViewState so the Performance panel can show an actionable message.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    assert!(
        state.devtools_view_state.vm_connection_error.is_none(),
        "vm_connection_error should be None initially"
    );

    update(
        &mut state,
        Message::VmServiceConnectionFailed {
            session_id,
            error: "Connection refused".to_string(),
        },
    );

    assert_eq!(
        state.devtools_view_state.vm_connection_error.as_deref(),
        Some("Connection failed: Connection refused"),
        "vm_connection_error should be set after VmServiceConnectionFailed"
    );
}

#[test]
fn test_vm_connected_clears_devtools_error() {
    // VmServiceConnected should clear any previously stored connection error so
    // the DevTools panel no longer shows a stale failure message.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Pre-populate an error (as if a prior connection attempt had failed).
    state.devtools_view_state.vm_connection_error =
        Some("Connection failed: Connection refused".to_string());

    update(&mut state, Message::VmServiceConnected { session_id });

    assert!(
        state.devtools_view_state.vm_connection_error.is_none(),
        "vm_connection_error must be cleared after VmServiceConnected"
    );
}

#[test]
fn test_maybe_connect_succeeds_after_disconnect() {
    // After VmServiceDisconnected, vm_shutdown_tx is None, which means
    // maybe_connect_vm_service will not be blocked on the next AppDebugPort.
    // We verify the post-condition: vm_shutdown_tx is None after disconnect,
    // which is exactly the guard that maybe_connect_vm_service checks.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Simulate the full connect → disconnect lifecycle.
    let (tx, _rx) = tokio::sync::watch::channel(false);
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.vm_shutdown_tx = Some(std::sync::Arc::new(tx));
        handle.session.vm_connected = true;
    }

    // Disconnect
    update(&mut state, Message::VmServiceDisconnected { session_id });

    // Post-condition: the guard in maybe_connect_vm_service checks
    // `handle.vm_shutdown_tx.is_none()`. Verify it is None.
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.vm_shutdown_tx.is_none(),
        "vm_shutdown_tx must be None after disconnect so maybe_connect_vm_service can reconnect"
    );
    assert!(
        !handle.session.vm_connected,
        "vm_connected must be false after VmServiceDisconnected"
    );
}

// ─────────────────────────────────────────────────────────
// Session switch DevTools reset tests (Phase 4, Task 04)
// ─────────────────────────────────────────────────────────

/// Helper that creates an AppState with two sessions and returns it.
fn make_state_with_two_sessions() -> AppState {
    let mut state = AppState::new();
    let device0 = test_device("device-0", "Device 0");
    let device1 = test_device("device-1", "Device 1");
    let _ = state.session_manager.create_session(&device0);
    let _ = state.session_manager.create_session(&device1);
    // Ensure session 0 is selected to start
    state.session_manager.select_by_index(0);
    state
}

#[test]
fn test_session_switch_resets_devtools_state() {
    use crate::state::DevToolsPanel;

    let mut state = make_state_with_two_sessions();

    // Populate devtools state for session 0
    state.devtools_view_state.inspector.loading = true;
    state.devtools_view_state.inspector.error =
        Some(DevToolsError::new("old error", "Press [r] to retry"));
    state.devtools_view_state.inspector.layout_loading = true;
    state.devtools_view_state.inspector.layout_error =
        Some(DevToolsError::new("layout error", "Press [r] to retry"));
    state.devtools_view_state.overlay_repaint_rainbow = true;
    state.devtools_view_state.overlay_debug_paint = true;
    state.devtools_view_state.overlay_performance = true;
    state.devtools_view_state.vm_connection_error = Some("Connection failed".into());
    state.devtools_view_state.active_panel = DevToolsPanel::Performance;

    // Switch to session 1
    update(&mut state, Message::SelectSessionByIndex(1));

    // All session-specific data should be cleared
    assert!(
        !state.devtools_view_state.inspector.loading,
        "inspector.loading should be cleared on session switch"
    );
    assert!(
        state.devtools_view_state.inspector.error.is_none(),
        "inspector.error should be cleared on session switch"
    );
    assert!(
        state.devtools_view_state.inspector.root.is_none(),
        "inspector.root should be cleared on session switch"
    );
    assert!(
        !state.devtools_view_state.inspector.layout_loading,
        "inspector.layout_loading should be cleared on session switch"
    );
    assert!(
        state.devtools_view_state.inspector.layout_error.is_none(),
        "inspector.layout_error should be cleared on session switch"
    );
    assert!(
        state.devtools_view_state.inspector.layout.is_none(),
        "inspector.layout should be cleared on session switch"
    );
    assert!(
        !state.devtools_view_state.overlay_repaint_rainbow,
        "overlay_repaint_rainbow should be cleared on session switch"
    );
    assert!(
        !state.devtools_view_state.overlay_debug_paint,
        "overlay_debug_paint should be cleared on session switch"
    );
    assert!(
        !state.devtools_view_state.overlay_performance,
        "overlay_performance should be cleared on session switch"
    );
    assert!(
        state.devtools_view_state.vm_connection_error.is_none(),
        "vm_connection_error should be cleared on session switch"
    );
}

#[test]
fn test_session_switch_preserves_active_panel() {
    use crate::state::DevToolsPanel;

    let mut state = make_state_with_two_sessions();
    state.devtools_view_state.active_panel = DevToolsPanel::Performance;

    // Switch to session 1
    update(&mut state, Message::SelectSessionByIndex(1));

    assert_eq!(
        state.devtools_view_state.active_panel,
        DevToolsPanel::Performance,
        "active_panel must be preserved across session switches"
    );
}

#[test]
fn test_session_switch_same_session_does_not_reset() {
    let mut state = make_state_with_two_sessions();
    // Currently on session 0
    state.devtools_view_state.inspector.loading = true;
    state.devtools_view_state.inspector.error =
        Some(DevToolsError::new("existing error", "Press [r] to retry"));

    // Switch to the same session (index 0 is already selected)
    update(&mut state, Message::SelectSessionByIndex(0));

    // loading and error should NOT be cleared
    assert!(
        state.devtools_view_state.inspector.loading,
        "inspector.loading must not be cleared when switching to the already-selected session"
    );
    assert!(
        state.devtools_view_state.inspector.error.is_some(),
        "inspector.error must not be cleared when switching to the already-selected session"
    );
    assert_eq!(
        state
            .devtools_view_state
            .inspector
            .error
            .as_ref()
            .map(|e| e.message.as_str()),
        Some("existing error"),
        "inspector.error message must not be cleared when switching to the already-selected session"
    );
}

#[test]
fn test_next_session_resets_devtools_state() {
    let mut state = make_state_with_two_sessions();
    state.devtools_view_state.overlay_repaint_rainbow = true;
    state.devtools_view_state.vm_connection_error = Some("error".into());

    // Advance to session 1
    update(&mut state, Message::NextSession);

    assert!(
        !state.devtools_view_state.overlay_repaint_rainbow,
        "overlay_repaint_rainbow should be cleared after NextSession"
    );
    assert!(
        state.devtools_view_state.vm_connection_error.is_none(),
        "vm_connection_error should be cleared after NextSession"
    );
}

#[test]
fn test_previous_session_resets_devtools_state() {
    let mut state = make_state_with_two_sessions();
    // Start on session 1
    state.session_manager.select_by_index(1);
    state.devtools_view_state.overlay_debug_paint = true;
    state.devtools_view_state.inspector.loading = true;

    // Go back to session 0
    update(&mut state, Message::PreviousSession);

    assert!(
        !state.devtools_view_state.overlay_debug_paint,
        "overlay_debug_paint should be cleared after PreviousSession"
    );
    assert!(
        !state.devtools_view_state.inspector.loading,
        "inspector.loading should be cleared after PreviousSession"
    );
}

#[test]
fn test_next_session_single_session_no_reset() {
    use crate::state::DevToolsPanel;

    let mut state = AppState::new();
    let device = test_device("device-0", "Device 0");
    let _ = state.session_manager.create_session(&device);

    state.devtools_view_state.inspector.loading = true;
    state.devtools_view_state.active_panel = DevToolsPanel::Performance;

    // With a single session, NextSession is a no-op (wraps to itself)
    update(&mut state, Message::NextSession);

    // State should NOT be reset because the selected session did not change
    assert!(
        state.devtools_view_state.inspector.loading,
        "inspector.loading must not be cleared when NextSession wraps to same session"
    );
    assert_eq!(
        state.devtools_view_state.active_panel,
        DevToolsPanel::Performance,
        "active_panel must not change when NextSession wraps to same session"
    );
}

// ─────────────────────────────────────────────────────────
// Object Group Disposal Tests (Phase 4, Task 07)
// ─────────────────────────────────────────────────────────

#[test]
fn test_inspector_has_object_group_set_after_widget_tree_fetched() {
    use crate::state::DevToolsPanel;
    use fdemon_core::DiagnosticsNode;

    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Initially false
    assert!(
        !state.devtools_view_state.inspector.has_object_group,
        "has_object_group should start false"
    );

    let node: DiagnosticsNode = serde_json::from_value(serde_json::json!({
        "description": "MaterialApp"
    }))
    .unwrap();

    // Simulate a successful widget tree fetch
    update(
        &mut state,
        Message::WidgetTreeFetched {
            session_id,
            root: Box::new(node),
        },
    );

    assert!(
        state.devtools_view_state.inspector.has_object_group,
        "has_object_group should be true after WidgetTreeFetched"
    );
    let _ = DevToolsPanel::Inspector; // suppress unused warning
}

#[test]
fn test_inspector_has_object_group_cleared_after_reset() {
    let mut state = AppState::new();

    // Simulate an object group existing
    state.devtools_view_state.inspector.has_object_group = true;

    // Reset clears it
    state.devtools_view_state.inspector.reset();

    assert!(
        !state.devtools_view_state.inspector.has_object_group,
        "has_object_group should be false after InspectorState::reset()"
    );
}

#[test]
fn test_layout_object_group_cleared_after_inspector_reset() {
    let mut state = AppState::new();

    // Simulate an object group existing
    state.devtools_view_state.inspector.has_layout_object_group = true;

    // Reset clears it
    state.devtools_view_state.inspector.reset();

    assert!(
        !state.devtools_view_state.inspector.has_layout_object_group,
        "has_layout_object_group should be false after InspectorState::reset()"
    );
}

#[test]
fn test_handle_exit_devtools_mode_returns_dispose_action_when_vm_connected() {
    use crate::handler::devtools::handle_exit_devtools_mode;
    use crate::handler::UpdateAction;

    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark the session as VM-connected
    state
        .session_manager
        .selected_mut()
        .unwrap()
        .session
        .vm_connected = true;
    state.ui_mode = crate::state::UiMode::DevTools;

    let result = handle_exit_devtools_mode(&mut state);

    assert_eq!(
        state.ui_mode,
        crate::state::UiMode::Normal,
        "ui_mode should return to Normal"
    );
    assert!(
        result.action.is_some(),
        "Should return DisposeDevToolsGroups action when VM is connected"
    );
    if let Some(UpdateAction::DisposeDevToolsGroups {
        session_id: sid,
        vm_handle,
    }) = result.action
    {
        assert_eq!(sid, session_id, "session_id should match active session");
        assert!(
            vm_handle.is_none(),
            "vm_handle should be None before hydration"
        );
    } else {
        panic!("Expected DisposeDevToolsGroups action");
    }
}

#[test]
fn test_handle_exit_devtools_mode_no_action_when_vm_not_connected() {
    use crate::handler::devtools::handle_exit_devtools_mode;

    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    // VM is NOT connected (default)
    state.ui_mode = crate::state::UiMode::DevTools;

    let result = handle_exit_devtools_mode(&mut state);

    assert_eq!(
        state.ui_mode,
        crate::state::UiMode::Normal,
        "ui_mode should return to Normal even without VM"
    );
    assert!(
        result.action.is_none(),
        "Should not return action when VM is not connected"
    );
}

#[test]
fn test_handle_exit_devtools_mode_no_action_when_no_session() {
    use crate::handler::devtools::handle_exit_devtools_mode;

    let mut state = AppState::new();
    state.ui_mode = crate::state::UiMode::DevTools;

    // No sessions at all
    let result = handle_exit_devtools_mode(&mut state);

    assert_eq!(state.ui_mode, crate::state::UiMode::Normal);
    assert!(result.action.is_none());
}

#[test]
fn test_devtools_view_state_reset_clears_has_object_group_flags() {
    let mut state = AppState::new();

    state.devtools_view_state.inspector.has_object_group = true;
    state.devtools_view_state.inspector.has_layout_object_group = true;

    state.devtools_view_state.reset();

    assert!(
        !state.devtools_view_state.inspector.has_object_group,
        "inspector.has_object_group should be false after DevToolsViewState::reset()"
    );
    assert!(
        !state.devtools_view_state.inspector.has_layout_object_group,
        "inspector.has_layout_object_group should be false after DevToolsViewState::reset()"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Allocation Profile Polling Tests (Phase 3, Task 08)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_vm_connected_starts_perf_monitoring_with_allocation_interval() {
    // Verify that VmServiceConnected produces a StartPerformanceMonitoring action
    // that includes the allocation_profile_interval_ms from settings.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();

    // Set a custom allocation profile interval to verify it is threaded through.
    state.settings.devtools.allocation_profile_interval_ms = 8000;
    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(&mut state, Message::VmServiceConnected { session_id });

    match result.action {
        Some(UpdateAction::StartPerformanceMonitoring {
            allocation_profile_interval_ms,
            ..
        }) => {
            assert_eq!(
                allocation_profile_interval_ms, 8000,
                "allocation_profile_interval_ms should match settings value"
            );
        }
        other => panic!(
            "Expected StartPerformanceMonitoring action, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_vm_connected_uses_default_allocation_interval() {
    // Verify that the default allocation_profile_interval_ms (5000ms) is used
    // when the settings value has not been overridden.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Default settings — allocation_profile_interval_ms defaults to 5000.
    let result = update(&mut state, Message::VmServiceConnected { session_id });

    match result.action {
        Some(UpdateAction::StartPerformanceMonitoring {
            allocation_profile_interval_ms,
            performance_refresh_ms,
            ..
        }) => {
            assert_eq!(
                allocation_profile_interval_ms, 5000,
                "Default allocation_profile_interval_ms should be 5000ms"
            );
            assert_eq!(
                performance_refresh_ms, 2000,
                "Default performance_refresh_ms should be 2000ms"
            );
        }
        other => panic!(
            "Expected StartPerformanceMonitoring action, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_memory_snapshot_still_works_alongside_sample() {
    // Verify VmServiceMemorySnapshot still populates memory_history (no regression).
    use fdemon_core::performance::MemoryUsage;

    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    let memory = MemoryUsage {
        heap_usage: 10_000_000,
        heap_capacity: 20_000_000,
        external_usage: 5_000_000,
        timestamp: chrono::Local::now(),
    };

    update(
        &mut state,
        Message::VmServiceMemorySnapshot { session_id, memory },
    );

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert_eq!(
        perf.memory_history.len(),
        1,
        "VmServiceMemorySnapshot should populate memory_history"
    );
    assert_eq!(perf.memory_history.latest().unwrap().heap_usage, 10_000_000);
}

#[test]
fn test_disconnect_clears_allocation_profile() {
    // After VmServiceDisconnected, the performance state is reset on reconnect.
    // Verify that allocation_profile is None after a fresh VmServiceConnected.
    use fdemon_core::performance::AllocationProfile;

    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Populate allocation_profile with synthetic data.
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.performance.allocation_profile = Some(AllocationProfile {
            members: vec![],
            timestamp: chrono::Local::now(),
        });
    }

    // Simulate reconnect — VmServiceConnected resets PerformanceState.
    update(&mut state, Message::VmServiceConnected { session_id });

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert!(
        perf.allocation_profile.is_none(),
        "allocation_profile should be None after VmServiceConnected resets PerformanceState"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Network monitor duplicate-spawn prevention tests (Phase 4 Fixes, Task 01)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: attach a live `network_shutdown_tx` to a session, simulating a
/// polling task that is already running.  Returns the watch receiver so callers
/// can verify whether the sender was signalled.
fn attach_network_shutdown(
    state: &mut AppState,
    session_id: crate::session::SessionId,
) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = state.session_manager.get_mut(session_id).unwrap();
    handle.network_shutdown_tx = Some(std::sync::Arc::new(tx));
    rx
}

#[test]
fn test_switch_panel_network_already_running_returns_no_action() {
    // Arrange: session with vm_connected=true and network_shutdown_tx=Some(…)
    // (i.e. a polling task is already running).
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark VM as connected so the guard would normally fire.
    state
        .session_manager
        .get_mut(session_id)
        .unwrap()
        .session
        .vm_connected = true;

    // Simulate an already-running polling task via a live shutdown sender.
    let _rx = attach_network_shutdown(&mut state, session_id);

    // Act: switch to the Network panel a second time.
    let result = update(
        &mut state,
        Message::SwitchDevToolsPanel(crate::state::DevToolsPanel::Network),
    );

    // Assert: idempotency guard prevents a duplicate StartNetworkMonitoring action.
    assert!(
        result.action.is_none(),
        "SwitchDevToolsPanel(Network) should return no action when a polling task is already running (got: {:?})",
        result.action,
    );
}

#[test]
fn test_switch_panel_network_not_running_returns_start_action() {
    // Arrange: session with vm_connected=true and network_shutdown_tx=None
    // (no polling task running yet — normal first-switch case).
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state
        .session_manager
        .get_mut(session_id)
        .unwrap()
        .session
        .vm_connected = true;

    // Ensure network_shutdown_tx is None (the default).
    assert!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .network_shutdown_tx
            .is_none(),
        "Precondition: network_shutdown_tx must be None"
    );

    // Act: switch to the Network panel for the first time.
    let result = update(
        &mut state,
        Message::SwitchDevToolsPanel(crate::state::DevToolsPanel::Network),
    );

    // Assert: StartNetworkMonitoring action is returned.
    assert!(
        matches!(
            result.action,
            Some(UpdateAction::StartNetworkMonitoring { .. })
        ),
        "SwitchDevToolsPanel(Network) should return StartNetworkMonitoring when no task is running (got: {:?})",
        result.action,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Recording toggle guard tests (Phase 4 Fixes, Task 02)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: build a minimal `HttpProfileEntry` for testing.
fn make_test_http_entry(id: &str) -> fdemon_core::network::HttpProfileEntry {
    fdemon_core::network::HttpProfileEntry {
        id: id.to_string(),
        method: "GET".to_string(),
        uri: format!("https://example.com/{id}"),
        status_code: Some(200),
        content_type: Some("application/json".to_string()),
        start_time_us: 1_000_000,
        end_time_us: Some(1_050_000),
        request_content_length: None,
        response_content_length: Some(128),
        error: None,
    }
}

#[test]
fn test_http_profile_received_merges_entries_when_recording_on() {
    // Arrange: fresh session with recording=true (the default).
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    assert!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .network
            .recording,
        "Precondition: recording should be true by default"
    );

    // Act: deliver entries via the TEA update path.
    update(
        &mut state,
        Message::VmServiceHttpProfileReceived {
            session_id,
            timestamp: 5000,
            entries: vec![make_test_http_entry("req-1")],
        },
    );

    // Assert: entry is stored and timestamp is advanced.
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.session.network.entries.len(),
        1,
        "Entry should be merged when recording is on"
    );
    assert_eq!(
        handle.session.network.last_poll_timestamp,
        Some(5000),
        "Timestamp should be advanced"
    );
}

#[test]
fn test_http_profile_received_discards_entries_when_recording_off_but_advances_timestamp() {
    // Arrange: fresh session, then toggle recording off.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(&mut state, Message::ToggleNetworkRecording);

    assert!(
        !state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .network
            .recording,
        "Precondition: recording should be false after toggle"
    );

    // Act: deliver entries while paused.
    update(
        &mut state,
        Message::VmServiceHttpProfileReceived {
            session_id,
            timestamp: 9000,
            entries: vec![make_test_http_entry("req-paused")],
        },
    );

    // Assert: entries are NOT merged, but timestamp IS advanced.
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.session.network.entries.len(),
        0,
        "Entries must NOT be merged when recording is off"
    );
    assert_eq!(
        handle.session.network.last_poll_timestamp,
        Some(9000),
        "Timestamp must still be advanced even when recording is off"
    );
}

#[test]
fn test_http_profile_received_only_shows_entries_after_recording_resumed() {
    // Arrange: fresh session.
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Deliver one entry while recording is on.
    update(
        &mut state,
        Message::VmServiceHttpProfileReceived {
            session_id,
            timestamp: 1000,
            entries: vec![make_test_http_entry("before-pause")],
        },
    );

    // Toggle recording off.
    update(&mut state, Message::ToggleNetworkRecording);

    // Deliver entries that should be silently discarded.
    update(
        &mut state,
        Message::VmServiceHttpProfileReceived {
            session_id,
            timestamp: 2000,
            entries: vec![make_test_http_entry("during-pause")],
        },
    );

    // Toggle recording back on.
    update(&mut state, Message::ToggleNetworkRecording);

    // Deliver a new entry — this one should be merged.
    update(
        &mut state,
        Message::VmServiceHttpProfileReceived {
            session_id,
            timestamp: 3000,
            entries: vec![make_test_http_entry("after-resume")],
        },
    );

    // Assert: only the two entries delivered while recording was on are present.
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.session.network.entries.len(),
        2,
        "Only entries sent while recording was on should be present"
    );
    let ids: Vec<&str> = handle
        .session
        .network
        .entries
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    assert!(
        ids.contains(&"before-pause"),
        "Entry before pause should be present"
    );
    assert!(
        ids.contains(&"after-resume"),
        "Entry after resume should be present"
    );
    assert!(
        !ids.contains(&"during-pause"),
        "Entry during pause must NOT be present"
    );
    assert_eq!(
        handle.session.network.last_poll_timestamp,
        Some(3000),
        "Timestamp should reflect the last poll"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Reconnect Handler Tests (Phase 2b, Task 04)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_vm_service_reconnected_preserves_performance_state() {
    use fdemon_core::performance::MemoryUsage;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    // Populate performance state with some data to confirm it is NOT wiped.
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.performance.memory_history.push(MemoryUsage {
            heap_usage: 42_000_000,
            heap_capacity: 100_000_000,
            external_usage: 5_000_000,
            timestamp: chrono::Local::now(),
        });
        handle.session.performance.monitoring_active = true;
    }

    // Send VmServiceReconnected — must NOT clear perf history.
    update(&mut state, Message::VmServiceReconnected { session_id });

    let handle = state.session_manager.get(session_id).unwrap();

    // Performance data must still be present (not wiped on reconnect).
    assert_eq!(
        handle.session.performance.memory_history.len(),
        1,
        "memory_history must be preserved across reconnect"
    );
    assert_eq!(
        handle
            .session
            .performance
            .memory_history
            .latest()
            .unwrap()
            .heap_usage,
        42_000_000,
        "heap_usage value must be intact after reconnect"
    );

    // vm_connected must be true after reconnection.
    assert!(
        handle.session.vm_connected,
        "vm_connected should be true after VmServiceReconnected"
    );

    // connection_status must be Connected for the active session.
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
        "connection_status should be Connected after VmServiceReconnected"
    );

    // The session log should contain the word "reconnected".
    let reconnected_log = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .iter()
        .any(|e| e.message.to_lowercase().contains("reconnected"));
    assert!(
        reconnected_log,
        "Session log should contain a 'reconnected' message"
    );
}

#[test]
fn test_vm_service_reconnected_restarts_monitoring() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    let result = update(&mut state, Message::VmServiceReconnected { session_id });

    // VmServiceReconnected must return a StartPerformanceMonitoring action.
    assert!(
        matches!(
            result.action,
            Some(UpdateAction::StartPerformanceMonitoring { .. })
        ),
        "VmServiceReconnected should trigger StartPerformanceMonitoring"
    );
}

#[test]
fn test_vm_service_connected_still_resets_performance() {
    // Regression test: VmServiceConnected (initial connection / hot-restart)
    // must still clear accumulated performance state.
    use fdemon_core::performance::MemoryUsage;

    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    // Pre-populate performance state.
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.performance.memory_history.push(MemoryUsage {
            heap_usage: 99_000_000,
            heap_capacity: 200_000_000,
            external_usage: 1_000_000,
            timestamp: chrono::Local::now(),
        });
        handle.session.performance.monitoring_active = true;
    }

    // Send VmServiceConnected — must reset perf state.
    update(&mut state, Message::VmServiceConnected { session_id });

    let perf = &state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .performance;
    assert!(
        perf.memory_history.is_empty(),
        "memory_history must be cleared by VmServiceConnected (initial connection / hot-restart)"
    );
    assert!(
        !perf.monitoring_active,
        "monitoring_active must be reset by VmServiceConnected"
    );
}

#[test]
fn test_vm_service_reconnected_cleans_up_perf_task() {
    // Uses a tokio runtime to create a real JoinHandle, mirroring the pattern
    // used in test_close_session_cleans_up_network_monitoring.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = test_device("dev-1", "Device 1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();
        state.session_manager.select_by_id(session_id);

        // Attach a real perf_shutdown_tx and a long-running perf_task_handle.
        let (tx, mut perf_rx) = tokio::sync::watch::channel(false);
        let task: tokio::task::JoinHandle<()> =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(60)).await });
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.perf_shutdown_tx = Some(std::sync::Arc::new(tx));
            handle.perf_task_handle = Some(task);
        }

        // Confirm handles are present before reconnect.
        {
            let handle = state.session_manager.get(session_id).unwrap();
            assert!(
                handle.perf_shutdown_tx.is_some(),
                "perf_shutdown_tx should be Some before VmServiceReconnected"
            );
            assert!(
                handle.perf_task_handle.is_some(),
                "perf_task_handle should be Some before VmServiceReconnected"
            );
        }

        // Send VmServiceReconnected.
        update(&mut state, Message::VmServiceReconnected { session_id });

        // Both handles must be taken (cleared) after reconnect.
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.perf_task_handle.is_none(),
            "perf_task_handle must be None after VmServiceReconnected (old task aborted)"
        );
        assert!(
            handle.perf_shutdown_tx.is_none(),
            "perf_shutdown_tx must be None after VmServiceReconnected (shutdown signaled)"
        );

        // The shutdown sender should have been sent `true` before being dropped.
        assert!(
            *perf_rx.borrow_and_update(),
            "perf_shutdown_tx should have been signaled true on VmServiceReconnected"
        );
    });
}

#[test]
fn test_vm_service_connected_background_session_no_status_change() {
    // Two sessions: A is active, B is background.
    // Sending VmServiceConnected for B must NOT overwrite the foreground status.
    let mut state = AppState::new();
    let device_a = test_device("dev-a", "Device A");
    let device_b = test_device("dev-b", "Device B");
    let session_a = state.session_manager.create_session(&device_a).unwrap();
    let session_b = state.session_manager.create_session(&device_b).unwrap();

    // Make A the active (selected) session.
    state.session_manager.select_by_id(session_a);

    // Simulate A currently reconnecting.
    state.devtools_view_state.connection_status = VmConnectionStatus::Reconnecting {
        attempt: 3,
        max_attempts: 10,
    };

    // Send VmServiceConnected for the background session B.
    update(
        &mut state,
        Message::VmServiceConnected {
            session_id: session_b,
        },
    );

    // Foreground status must still be Reconnecting — B's connect must not pollute it.
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Reconnecting {
            attempt: 3,
            max_attempts: 10,
        },
        "connection_status must not be overwritten by a background session's VmServiceConnected"
    );
}

#[test]
fn test_vm_service_disconnected_background_session_no_status_change() {
    // Two sessions: A is active (Connected), B is background.
    // Sending VmServiceDisconnected for B must NOT change the foreground status.
    let mut state = AppState::new();
    let device_a = test_device("dev-a", "Device A");
    let device_b = test_device("dev-b", "Device B");
    let session_a = state.session_manager.create_session(&device_a).unwrap();
    let session_b = state.session_manager.create_session(&device_b).unwrap();

    // A is active and Connected (default status).
    state.session_manager.select_by_id(session_a);
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
        "initial status should be Connected"
    );

    // Send VmServiceDisconnected for the background session B.
    update(
        &mut state,
        Message::VmServiceDisconnected {
            session_id: session_b,
        },
    );

    // Foreground status must still be Connected.
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
        "connection_status must not change when a background session disconnects"
    );
}

#[test]
fn test_vm_service_connection_failed_background_session_no_error() {
    // Two sessions: A is active, B is background.
    // Sending VmServiceConnectionFailed for B must NOT set vm_connection_error
    // on the foreground devtools_view_state.
    let mut state = AppState::new();
    let device_a = test_device("dev-a", "Device A");
    let device_b = test_device("dev-b", "Device B");
    let session_a = state.session_manager.create_session(&device_a).unwrap();
    let session_b = state.session_manager.create_session(&device_b).unwrap();

    // A is active.
    state.session_manager.select_by_id(session_a);

    // Confirm no error initially.
    assert!(
        state.devtools_view_state.vm_connection_error.is_none(),
        "vm_connection_error should be None initially"
    );

    // Send VmServiceConnectionFailed for the background session B.
    update(
        &mut state,
        Message::VmServiceConnectionFailed {
            session_id: session_b,
            error: "timeout".to_string(),
        },
    );

    // vm_connection_error must still be None — background session must not pollute it.
    assert!(
        state.devtools_view_state.vm_connection_error.is_none(),
        "vm_connection_error must not be set by a background session's VmServiceConnectionFailed"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 4, Task 03: Coordinated Pause / File-Watcher Gate tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_suspend_file_watcher_sets_flag() {
    let mut state = AppState::new();
    assert!(!state.file_watcher_suspended);

    let result = update(&mut state, Message::SuspendFileWatcher);

    assert!(state.file_watcher_suspended);
    assert!(result.message.is_none());
    assert!(result.action.is_none());
}

#[test]
fn test_suspend_file_watcher_is_idempotent() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;

    let result = update(&mut state, Message::SuspendFileWatcher);

    // Should still be suspended and no follow-up message.
    assert!(state.file_watcher_suspended);
    assert!(result.message.is_none());
}

#[test]
fn test_resume_file_watcher_clears_flag() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;

    let result = update(&mut state, Message::ResumeFileWatcher);

    assert!(!state.file_watcher_suspended);
    assert!(result.action.is_none());
}

#[test]
fn test_resume_file_watcher_no_reload_when_no_pending_changes() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;
    state.pending_file_changes = 0;

    let result = update(&mut state, Message::ResumeFileWatcher);

    assert!(!state.file_watcher_suspended);
    assert!(
        result.message.is_none(),
        "No reload when no pending changes"
    );
}

#[test]
fn test_resume_file_watcher_triggers_reload_when_pending_changes() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;
    state.pending_file_changes = 5;

    let result = update(&mut state, Message::ResumeFileWatcher);

    assert!(!state.file_watcher_suspended);
    // pending_file_changes must be cleared.
    assert_eq!(state.pending_file_changes, 0);
    // AutoReloadTriggered must be emitted.
    assert!(
        matches!(result.message, Some(Message::AutoReloadTriggered)),
        "Should emit AutoReloadTriggered when pending changes exist"
    );
}

#[test]
fn test_resume_file_watcher_clears_pending_changes() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;
    state.pending_file_changes = 3;

    update(&mut state, Message::ResumeFileWatcher);

    assert_eq!(
        state.pending_file_changes, 0,
        "pending_file_changes must be reset to 0 on resume"
    );
}

#[test]
fn test_files_changed_queued_when_suspended() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;
    // suppress_reload_on_pause defaults to true.

    let result = update(&mut state, Message::FilesChanged { count: 3 });

    assert_eq!(state.pending_file_changes, 3);
    assert!(
        result.message.is_none(),
        "No reload should be triggered while suspended"
    );
    assert!(result.action.is_none());
}

#[test]
fn test_files_changed_accumulates_when_suspended() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;
    state.pending_file_changes = 2;

    update(&mut state, Message::FilesChanged { count: 3 });

    assert_eq!(
        state.pending_file_changes, 5,
        "Changes should accumulate while suspended"
    );
}

#[test]
fn test_files_changed_not_queued_when_suppress_disabled() {
    let mut state = AppState::new();
    state.file_watcher_suspended = true;
    state.settings.dap.suppress_reload_on_pause = false;

    // With suppress disabled, even a suspended watcher should not queue changes.
    let result = update(&mut state, Message::FilesChanged { count: 3 });

    assert_eq!(
        state.pending_file_changes, 0,
        "No queuing when suppress is disabled"
    );
    // The normal FilesChanged handler doesn't produce a message/action on its own
    // (AutoReloadTriggered is a separate message). Confirm no regression.
    assert!(result.message.is_none());
}

#[test]
fn test_files_changed_not_queued_when_not_suspended() {
    let mut state = AppState::new();
    // file_watcher_suspended defaults to false.

    let result = update(&mut state, Message::FilesChanged { count: 5 });

    assert_eq!(
        state.pending_file_changes, 0,
        "Changes should not be queued when watcher is active"
    );
    assert!(result.message.is_none());
}

#[test]
fn test_suspend_then_resume_full_flow() {
    let mut state = AppState::new();
    assert!(!state.file_watcher_suspended);

    // Step 1: suspend.
    update(&mut state, Message::SuspendFileWatcher);
    assert!(state.file_watcher_suspended);

    // Step 2: file changes arrive while suspended.
    update(&mut state, Message::FilesChanged { count: 2 });
    update(&mut state, Message::FilesChanged { count: 1 });
    assert_eq!(state.pending_file_changes, 3);

    // Step 3: resume.
    let result = update(&mut state, Message::ResumeFileWatcher);
    assert!(!state.file_watcher_suspended);
    assert_eq!(state.pending_file_changes, 0);
    assert!(
        matches!(result.message, Some(Message::AutoReloadTriggered)),
        "Resume should trigger reload for 3 pending changes"
    );
}

#[test]
fn test_dap_client_disconnect_resumes_file_watcher() {
    use crate::state::DapStatus;
    use std::collections::HashSet;

    let mut state = AppState::new();
    state.dap_status = DapStatus::Running {
        port: 4711,
        clients: ["client-1".to_string()].into_iter().collect::<HashSet<_>>(),
    };
    state.file_watcher_suspended = true;
    state.pending_file_changes = 2;

    let result = update(
        &mut state,
        Message::DapClientDisconnected {
            client_id: "client-1".to_string(),
        },
    );

    // The dap handler should emit ResumeFileWatcher as a follow-up.
    assert!(
        matches!(result.message, Some(Message::ResumeFileWatcher)),
        "DapClientDisconnected while suspended should emit ResumeFileWatcher"
    );
}

#[test]
fn test_dap_client_disconnect_no_resume_when_not_suspended() {
    use crate::state::DapStatus;
    use std::collections::HashSet;

    let mut state = AppState::new();
    state.dap_status = DapStatus::Running {
        port: 4711,
        clients: ["client-1".to_string()].into_iter().collect::<HashSet<_>>(),
    };
    // Not suspended.
    assert!(!state.file_watcher_suspended);

    let result = update(
        &mut state,
        Message::DapClientDisconnected {
            client_id: "client-1".to_string(),
        },
    );

    // No ResumeFileWatcher when watcher was not suspended.
    assert!(
        !matches!(result.message, Some(Message::ResumeFileWatcher)),
        "Should not emit ResumeFileWatcher when watcher was not suspended"
    );
}

#[test]
fn test_state_defaults_to_not_suspended() {
    let state = AppState::new();
    assert!(
        !state.file_watcher_suspended,
        "file_watcher_suspended defaults to false"
    );
    assert_eq!(
        state.pending_file_changes, 0,
        "pending_file_changes defaults to 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Native Platform Log Handler Tests (Phase 1)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: create a `Device` with platform set to `"android"` — the platform
/// that requires native log capture via `adb logcat`.
fn android_device(id: &str) -> fdemon_daemon::Device {
    fdemon_daemon::Device {
        id: id.to_string(),
        name: format!("Android Device {}", id),
        platform: "android".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }
}

/// Helper: create a `Device` with `platform` set to `"linux"`.
fn linux_device(id: &str) -> fdemon_daemon::Device {
    fdemon_daemon::Device {
        id: id.to_string(),
        name: format!("Linux Device {}", id),
        platform: "linux".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }
}

/// Helper: attach a native_log_shutdown_tx to a session, simulating a capture
/// task that is already running. Returns the watch receiver so tests can verify
/// whether the shutdown signal was sent.
fn attach_native_log_shutdown(
    state: &mut AppState,
    session_id: crate::session::SessionId,
) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = state.session_manager.get_mut(session_id).unwrap();
    handle.native_log_shutdown_tx = Some(std::sync::Arc::new(tx));
    rx
}

#[test]
fn test_native_log_creates_log_entry_with_native_source() {
    use fdemon_core::LogLevel;
    use fdemon_core::LogSource;
    use fdemon_daemon::NativeLogEvent;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let event = NativeLogEvent {
        tag: "MyNativeTag".to_string(),
        level: LogLevel::Warning,
        message: "native warning message".to_string(),
        timestamp: Some("2024-01-01 00:00:00.000".to_string()),
    };

    update(&mut state, Message::NativeLog { session_id, event });

    // Flush any batched logs before asserting — native logs go through the
    // LogBatcher (same path as Flutter stdout logs).
    state.session_manager.flush_all_pending_logs();

    let handle = state.session_manager.get(session_id).unwrap();
    let last_log = handle.session.logs.back().unwrap();
    assert!(
        matches!(&last_log.source, LogSource::Native { tag } if tag == "MyNativeTag"),
        "Expected LogSource::Native {{ tag: \"MyNativeTag\" }}, got {:?}",
        last_log.source
    );
    assert_eq!(last_log.level, LogLevel::Warning);
    assert_eq!(last_log.message, "native warning message");
}

#[test]
fn test_native_log_for_missing_session_is_no_op() {
    use fdemon_core::LogLevel;
    use fdemon_daemon::NativeLogEvent;

    let mut state = AppState::new();
    // Use a session_id that was never registered in the session manager.
    let missing_id: crate::session::SessionId = u64::MAX;

    let event = NativeLogEvent {
        tag: "SomeTag".to_string(),
        level: LogLevel::Info,
        message: "should be discarded".to_string(),
        timestamp: None,
    };

    // Must not panic; result is a no-op UpdateResult.
    let result = update(
        &mut state,
        Message::NativeLog {
            session_id: missing_id,
            event,
        },
    );
    assert!(
        result.action.is_none(),
        "Missing session NativeLog should produce no action"
    );
    assert!(
        result.message.is_none(),
        "Missing session NativeLog should produce no follow-up message"
    );
}

#[test]
fn test_native_log_capture_started_stores_handles() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
        // Spawn a real (but trivially short-lived) task for the task_handle slot.
        let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(task)));

        update(
            &mut state,
            Message::NativeLogCaptureStarted {
                session_id,
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle,
            },
        );

        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.native_log_shutdown_tx.is_some(),
            "native_log_shutdown_tx should be Some after NativeLogCaptureStarted"
        );
        assert!(
            handle.native_log_task_handle.is_some(),
            "native_log_task_handle should be Some after NativeLogCaptureStarted"
        );
    });
}

#[test]
fn test_native_log_capture_started_for_closed_session_sends_shutdown() {
    // When a NativeLogCaptureStarted arrives for a session that no longer
    // exists, the handler must send `true` on the shutdown channel so the
    // orphaned capture task stops immediately.
    let mut state = AppState::new();
    let missing_id: crate::session::SessionId = u64::MAX;

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));

    update(
        &mut state,
        Message::NativeLogCaptureStarted {
            session_id: missing_id,
            shutdown_tx: std::sync::Arc::new(shutdown_tx),
            task_handle,
        },
    );

    assert_eq!(
        *shutdown_rx.borrow_and_update(),
        true,
        "shutdown_tx should have been sent true when session is missing"
    );
}

#[test]
fn test_native_log_capture_stopped_clears_handles() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Attach shutdown_tx and a real task_handle to simulate a running capture.
        let _rx = attach_native_log_shutdown(&mut state, session_id);
        {
            let task: tokio::task::JoinHandle<()> = tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await
            });
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.native_log_task_handle = Some(task);
        }

        // Verify both handles are set before the stop message.
        {
            let handle = state.session_manager.get(session_id).unwrap();
            assert!(
                handle.native_log_shutdown_tx.is_some(),
                "shutdown_tx should be Some before stop"
            );
            assert!(
                handle.native_log_task_handle.is_some(),
                "task_handle should be Some before stop"
            );
        }

        update(&mut state, Message::NativeLogCaptureStopped { session_id });

        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.native_log_shutdown_tx.is_none(),
            "native_log_shutdown_tx should be None after NativeLogCaptureStopped"
        );
        assert!(
            handle.native_log_task_handle.is_none(),
            "native_log_task_handle should be None after NativeLogCaptureStopped"
        );
    });
}

#[test]
fn test_maybe_start_native_log_capture_returns_action_for_android() {
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Enable adb so native_logs_available("android") returns true.
    state.tool_availability = ToolAvailability {
        adb: true,
        ..Default::default()
    };
    // Ensure native logs are enabled (they are by default, but be explicit).
    state.settings.native_logs.enabled = true;

    let msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    let action = super::session::maybe_start_native_log_capture(&state, session_id, &msg);

    assert!(
        matches!(
            action,
            Some(crate::handler::UpdateAction::StartNativeLogCapture { .. })
        ),
        "Expected Some(StartNativeLogCapture) for android + adb=true + enabled, got {:?}",
        action
    );
}

#[test]
fn test_maybe_start_native_log_capture_returns_none_when_tools_unavailable() {
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // adb is NOT available — native_logs_available("android") returns false.
    state.tool_availability = ToolAvailability {
        adb: false,
        ..Default::default()
    };
    state.settings.native_logs.enabled = true;

    let msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    let action = super::session::maybe_start_native_log_capture(&state, session_id, &msg);

    assert!(
        action.is_none(),
        "Expected None when adb is unavailable, got {:?}",
        action
    );
}

#[test]
fn test_maybe_start_native_log_capture_returns_none_when_disabled() {
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Tools are available but native logs are disabled in settings.
    state.tool_availability = ToolAvailability {
        adb: true,
        ..Default::default()
    };
    state.settings.native_logs.enabled = false;

    let msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    let action = super::session::maybe_start_native_log_capture(&state, session_id, &msg);

    assert!(
        action.is_none(),
        "Expected None when native_logs.enabled = false, got {:?}",
        action
    );
}

#[test]
fn test_maybe_start_native_log_capture_returns_none_when_already_running() {
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability {
        adb: true,
        ..Default::default()
    };
    state.settings.native_logs.enabled = true;

    // Simulate a capture already running by attaching a shutdown_tx.
    let _rx = attach_native_log_shutdown(&mut state, session_id);

    let msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    let action = super::session::maybe_start_native_log_capture(&state, session_id, &msg);

    assert!(
        action.is_none(),
        "Expected None when native_log_shutdown_tx is already Some (double-start guard), got {:?}",
        action
    );
}

#[test]
fn test_maybe_start_native_log_capture_returns_none_for_linux() {
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    // Linux does not need a separate capture process — Flutter's stdout pipe
    // already surfaces native logs on this platform.
    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Even with adb available (unusual, but defensive), Linux should return None.
    state.tool_availability = ToolAvailability {
        adb: true,
        ..Default::default()
    };
    state.settings.native_logs.enabled = true;

    let msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "linux-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    let action = super::session::maybe_start_native_log_capture(&state, session_id, &msg);

    assert!(
        action.is_none(),
        "Expected None for linux platform (no native log capture needed), got {:?}",
        action
    );
}

/// Helper: attach a `CustomSourceHandle` to a session, simulating a custom
/// source that is already running (native_log_shutdown_tx stays None).
/// Returns a receiver that can be used to verify the shutdown signal.
fn attach_custom_source_handle(
    state: &mut AppState,
    session_id: crate::session::SessionId,
    name: &str,
) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = state.session_manager.get_mut(session_id).unwrap();
    handle
        .custom_source_handles
        .push(crate::session::CustomSourceHandle {
            name: name.to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: false,
        });
    rx
}

#[test]
fn test_hot_restart_skips_duplicate_custom_sources() {
    // Regression guard: for sessions that only run custom sources (e.g. Linux /
    // Windows / Web targets where platform capture is skipped),
    // `native_log_shutdown_tx` is never set.  Without the extended guard the
    // function would return `Some(StartNativeLogCapture)` on every hot-restart,
    // spawning duplicate processes.
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    // Use a Linux device so needs_platform_capture = false.
    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability::default();
    state.settings.native_logs.enabled = true;
    // Configure a custom source so the first AppStart would normally emit the action.
    state.settings.native_logs.custom_sources = vec![crate::config::CustomSourceConfig {
        name: "GoLog".to_string(),
        command: "go".to_string(),
        args: vec!["run".to_string(), "logger.go".to_string()],
        format: fdemon_core::types::OutputFormat::default(),
        working_dir: None,
        env: std::collections::HashMap::new(),
        start_before_app: false,
        shared: false,
        ready_check: None,
    }];

    // Simulate that the first AppStart already spawned the custom source —
    // populate custom_source_handles without setting native_log_shutdown_tx.
    let _rx = attach_custom_source_handle(&mut state, session_id, "GoLog");

    // Sanity: native_log_shutdown_tx must still be None (it is never set for
    // custom-sources-only sessions).
    {
        let h = state.session_manager.get(session_id).unwrap();
        assert!(
            h.native_log_shutdown_tx.is_none(),
            "native_log_shutdown_tx must be None for this test to be meaningful"
        );
        assert_eq!(
            h.custom_source_handles.len(),
            1,
            "custom_source_handles must be non-empty"
        );
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "linux-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Now simulate a hot-restart: the guard must fire and return None.
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);

    assert!(
        action.is_none(),
        "Expected None (guard fired) when custom_source_handles is non-empty, got {:?}",
        action
    );
}

#[test]
fn test_guard_accounts_for_shared_post_app_sources() {
    // Regression guard: when a shared post-app source is already running globally
    // (stored on AppState.shared_source_handles), `maybe_start_native_log_capture`
    // must treat it as "running" and return None on hot-restart, avoiding a
    // spurious StartNativeLogCapture dispatch.
    //
    // Uses an Android session with native_log_shutdown_tx set (platform capture
    // already running) to exercise Guard Branch A:
    //   `native_log_shutdown_tx.is_some() && !has_unstarted_post_app`
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Enable adb so native_logs_available("android") returns true.
    state.tool_availability = ToolAvailability {
        adb: true,
        ..Default::default()
    };
    state.settings.native_logs.enabled = true;

    // Configure a shared post-app source only (no per-session custom sources).
    state.settings.native_logs.custom_sources = vec![crate::config::CustomSourceConfig {
        name: "my-shared-logger".to_string(),
        command: "tail".to_string(),
        args: vec!["-f".to_string(), "/tmp/log".to_string()],
        format: fdemon_core::types::OutputFormat::default(),
        working_dir: None,
        env: std::collections::HashMap::new(),
        start_before_app: false,
        shared: true,
        ready_check: None,
    }];

    // Register the shared source as already running on AppState.
    // Shared sources are stored here, not on per-session custom_source_handles.
    {
        use crate::session::SharedSourceHandle;
        let (tx, _rx) = tokio::sync::watch::channel(false);
        state.shared_source_handles.push(SharedSourceHandle {
            name: "my-shared-logger".to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: false,
        });
    }

    // Simulate platform capture already running (native_log_shutdown_tx is Some).
    // This allows Guard Branch A to fire.
    let _shutdown_rx = attach_native_log_shutdown(&mut state, session_id);

    // Sanity: per-session custom_source_handles must be empty (shared sources
    // aren't stored there), and native_log_shutdown_tx must be Some.
    {
        let h = state.session_manager.get(session_id).unwrap();
        assert!(
            h.custom_source_handles.is_empty(),
            "custom_source_handles must be empty for shared-source scenario"
        );
        assert!(
            h.native_log_shutdown_tx.is_some(),
            "native_log_shutdown_tx must be Some for Guard Branch A to fire"
        );
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Guard Branch A must fire: platform capture running + shared post-app source
    // is already running → return None.
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(
        action.is_none(),
        "Expected None (guard fired) when shared post-app source is already running, got {:?}",
        action
    );
}

#[test]
fn test_guard_emits_action_when_shared_post_app_source_not_yet_running() {
    // Complementary test: when a shared post-app source is configured but NOT yet
    // present in state.shared_source_handles (i.e., it genuinely needs to be
    // started), maybe_start_native_log_capture must still return Some.
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    // Use a Linux device so needs_platform_capture = false.
    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability::default();
    state.settings.native_logs.enabled = true;

    // Configure a shared post-app source.
    state.settings.native_logs.custom_sources = vec![crate::config::CustomSourceConfig {
        name: "my-shared-logger".to_string(),
        command: "tail".to_string(),
        args: vec!["-f".to_string(), "/tmp/log".to_string()],
        format: fdemon_core::types::OutputFormat::default(),
        working_dir: None,
        env: std::collections::HashMap::new(),
        start_before_app: false,
        shared: true,
        ready_check: None,
    }];

    // shared_source_handles is empty — source not yet running.
    assert!(state.shared_source_handles.is_empty());

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "linux-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Guard must NOT fire: the source is not yet running → return Some.
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(
        action.is_some(),
        "Expected Some(StartNativeLogCapture) when shared post-app source is not yet running"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Native Tag Filter Handler Tests (Phase 2, Task 07)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: send a NativeLog event to a session and flush batched logs.
fn send_native_log(
    state: &mut AppState,
    session_id: crate::session::SessionId,
    tag: &str,
    message: &str,
) {
    use fdemon_core::LogLevel;
    use fdemon_daemon::NativeLogEvent;
    let event = NativeLogEvent {
        tag: tag.to_string(),
        level: LogLevel::Info,
        message: message.to_string(),
        timestamp: None,
    };
    update(state, Message::NativeLog { session_id, event });
    state.session_manager.flush_all_pending_logs();
}

#[test]
fn test_native_log_observes_tag() {
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    send_native_log(&mut state, session_id, "GoLog", "hello");

    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.native_tag_state.tag_count(), 1);
    // Tags are normalised to ASCII lowercase at storage time.
    assert_eq!(handle.native_tag_state.discovered_tags["golog"], 1);
}

#[test]
fn test_native_log_increments_tag_count() {
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    send_native_log(&mut state, session_id, "GoLog", "msg 1");
    send_native_log(&mut state, session_id, "GoLog", "msg 2");
    send_native_log(&mut state, session_id, "OkHttp", "http msg");

    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.native_tag_state.tag_count(), 2);
    // Tags are normalised to ASCII lowercase at storage time.
    assert_eq!(handle.native_tag_state.discovered_tags["golog"], 2);
    assert_eq!(handle.native_tag_state.discovered_tags["okhttp"], 1);
}

#[test]
fn test_native_log_hidden_tag_not_added_to_buffer() {
    use fdemon_core::LogLevel;
    use fdemon_daemon::NativeLogEvent;

    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // First, send one visible event to observe the tag.
    send_native_log(&mut state, session_id, "GoLog", "visible message");

    // Hide the tag.
    update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "GoLog".to_string(),
        },
    );

    // Count logs before sending the hidden event.
    let log_count_before = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();

    // Send a native log for the hidden tag.
    let event = NativeLogEvent {
        tag: "GoLog".to_string(),
        level: LogLevel::Info,
        message: "should be hidden".to_string(),
        timestamp: None,
    };
    update(&mut state, Message::NativeLog { session_id, event });
    state.session_manager.flush_all_pending_logs();

    let log_count_after = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();

    assert_eq!(
        log_count_before, log_count_after,
        "Hidden tag entry should not be added to the log buffer"
    );

    // The tag count should still be incremented even for hidden entries.
    let handle = state.session_manager.get(session_id).unwrap();
    // Tags are normalised to ASCII lowercase at storage time.
    assert_eq!(handle.native_tag_state.discovered_tags["golog"], 2);
}

#[test]
fn test_toggle_native_tag_message_toggles_visibility() {
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    send_native_log(&mut state, session_id, "GoLog", "first message");

    // Initially visible.
    assert!(state
        .session_manager
        .get(session_id)
        .unwrap()
        .native_tag_state
        .is_tag_visible("GoLog"));

    // Toggle to hidden.
    update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "GoLog".to_string(),
        },
    );
    assert!(!state
        .session_manager
        .get(session_id)
        .unwrap()
        .native_tag_state
        .is_tag_visible("GoLog"));

    // Toggle back to visible.
    update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "GoLog".to_string(),
        },
    );
    assert!(state
        .session_manager
        .get(session_id)
        .unwrap()
        .native_tag_state
        .is_tag_visible("GoLog"));
}

#[test]
fn test_show_all_native_tags_clears_hidden() {
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    send_native_log(&mut state, session_id, "GoLog", "msg");
    send_native_log(&mut state, session_id, "OkHttp", "msg");

    // Hide both tags.
    update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "GoLog".to_string(),
        },
    );
    update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "OkHttp".to_string(),
        },
    );
    assert_eq!(
        state
            .session_manager
            .get(session_id)
            .unwrap()
            .native_tag_state
            .hidden_count(),
        2
    );

    // Show all.
    update(&mut state, Message::ShowAllNativeTags);
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.native_tag_state.hidden_count(), 0);
    assert!(handle.native_tag_state.is_tag_visible("GoLog"));
    assert!(handle.native_tag_state.is_tag_visible("OkHttp"));
}

#[test]
fn test_hide_all_native_tags_hides_discovered() {
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    send_native_log(&mut state, session_id, "GoLog", "msg");
    send_native_log(&mut state, session_id, "OkHttp", "msg");

    update(&mut state, Message::HideAllNativeTags);

    let handle = state.session_manager.get(session_id).unwrap();
    assert!(!handle.native_tag_state.is_tag_visible("GoLog"));
    assert!(!handle.native_tag_state.is_tag_visible("OkHttp"));
    assert_eq!(handle.native_tag_state.hidden_count(), 2);
}

#[test]
fn test_native_log_capture_stopped_resets_tag_state() {
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    send_native_log(&mut state, session_id, "GoLog", "msg");
    {
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.native_tag_state.tag_count(), 1);
    }

    update(&mut state, Message::NativeLogCaptureStopped { session_id });

    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.native_tag_state.tag_count(),
        0,
        "Tag state should be reset when native log capture stops"
    );
}

#[test]
fn test_native_log_capture_stopped_preserves_tags_when_custom_sources_running() {
    // When adb logcat exits while custom sources are still active, the user's
    // per-tag visibility choices must NOT be wiped out.
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Discover a tag via platform capture.
    send_native_log(&mut state, session_id, "GoLog", "msg");
    // Hide the tag so we can verify the choice is preserved.
    update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "GoLog".to_string(),
        },
    );
    {
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            !handle.native_tag_state.is_tag_visible("GoLog"),
            "precondition: tag should be hidden"
        );
        assert_eq!(handle.native_tag_state.hidden_count(), 1);
    }

    // Attach a custom source so custom_source_handles is non-empty.
    let _rx = attach_custom_source_handle(&mut state, session_id, "my-custom-source");

    // Platform capture (adb logcat) exits.
    update(&mut state, Message::NativeLogCaptureStopped { session_id });

    // Tag state must be preserved because custom sources are still running.
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.native_tag_state.tag_count(),
        1,
        "Tag state must be preserved when custom sources are still running"
    );
    assert!(
        !handle.native_tag_state.is_tag_visible("GoLog"),
        "Hidden tag must remain hidden when custom sources are still running"
    );
    assert_eq!(
        handle.native_tag_state.hidden_count(),
        1,
        "Hidden count must be preserved when custom sources are still running"
    );
    // Platform capture handles should be cleared.
    assert!(handle.native_log_shutdown_tx.is_none());
    assert!(handle.native_log_task_handle.is_none());
}

#[test]
fn test_native_log_capture_stopped_resets_tags_when_no_custom_sources() {
    // When adb logcat exits and no custom sources are running, tag state
    // should reset as before (no regression).
    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Discover a tag via platform capture.
    send_native_log(&mut state, session_id, "GoLog", "msg");
    {
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.native_tag_state.tag_count(),
            1,
            "precondition: tag present"
        );
    }

    // No custom sources attached — custom_source_handles is empty.

    // Platform capture exits.
    update(&mut state, Message::NativeLogCaptureStopped { session_id });

    // Tag state must be reset because no custom sources are running.
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.native_tag_state.tag_count(),
        0,
        "Tag state must be reset when no custom sources are running"
    );
}

#[test]
fn test_show_hide_tag_filter_messages_are_no_op() {
    // ShowTagFilter and HideTagFilter are UI-only messages (handled by task 09).
    // They must not panic and return no action or follow-up message.
    let mut state = AppState::new();

    let result = update(&mut state, Message::ShowTagFilter);
    assert!(result.action.is_none());
    assert!(result.message.is_none());

    let result = update(&mut state, Message::HideTagFilter);
    assert!(result.action.is_none());
    assert!(result.message.is_none());
}

#[test]
fn test_toggle_native_tag_no_session_is_no_op() {
    // No sessions in manager — should not panic.
    let mut state = AppState::new();
    let result = update(
        &mut state,
        Message::ToggleNativeTag {
            tag: "AnyTag".to_string(),
        },
    );
    assert!(result.action.is_none());
}

#[test]
fn test_show_all_native_tags_no_session_is_no_op() {
    let mut state = AppState::new();
    let result = update(&mut state, Message::ShowAllNativeTags);
    assert!(result.action.is_none());
}

#[test]
fn test_hide_all_native_tags_no_session_is_no_op() {
    let mut state = AppState::new();
    let result = update(&mut state, Message::HideAllNativeTags);
    assert!(result.action.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// Effective Min Level Handler Tests (Phase 2-fixes, Task 02)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: send a NativeLog event at a given level and flush batched logs.
fn send_native_log_with_level(
    state: &mut AppState,
    session_id: crate::session::SessionId,
    tag: &str,
    level: fdemon_core::LogLevel,
    message: &str,
) {
    use fdemon_daemon::NativeLogEvent;
    let event = NativeLogEvent {
        tag: tag.to_string(),
        level,
        message: message.to_string(),
        timestamp: None,
    };
    update(state, Message::NativeLog { session_id, event });
    state.session_manager.flush_all_pending_logs();
}

#[test]
fn test_native_log_filtered_by_effective_min_level() {
    use crate::config::{NativeLogsSettings, TagConfig};
    use fdemon_core::LogLevel;

    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Setup: global min_level = "info", per-tag "GoLog" = "warning"
    state.settings.native_logs = NativeLogsSettings {
        min_level: "info".to_string(),
        ..Default::default()
    };
    state.settings.native_logs.tags.insert(
        "GoLog".to_string(),
        TagConfig {
            min_level: Some("warning".to_string()),
        },
    );

    // Send Debug event for "GoLog" → should be filtered (below warning)
    send_native_log_with_level(
        &mut state,
        session_id,
        "GoLog",
        LogLevel::Debug,
        "debug msg",
    );
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 0,
        "Debug event for GoLog must be filtered (below per-tag warning floor)"
    );

    // Send Info event for "GoLog" → should be filtered (below warning)
    send_native_log_with_level(&mut state, session_id, "GoLog", LogLevel::Info, "info msg");
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 0,
        "Info event for GoLog must be filtered (below per-tag warning floor)"
    );

    // Send Warning event for "GoLog" → should pass (meets warning floor)
    send_native_log_with_level(
        &mut state,
        session_id,
        "GoLog",
        LogLevel::Warning,
        "warn msg",
    );
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 1,
        "Warning event for GoLog must pass (meets per-tag warning floor)"
    );

    // Send Error event for "GoLog" → should pass (above warning floor)
    send_native_log_with_level(
        &mut state,
        session_id,
        "GoLog",
        LogLevel::Error,
        "error msg",
    );
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 2,
        "Error event for GoLog must pass (above per-tag warning floor)"
    );

    // Send Debug event for "OtherTag" → should be filtered (global "info" floor)
    send_native_log_with_level(&mut state, session_id, "OtherTag", LogLevel::Debug, "debug");
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 2,
        "Debug event for OtherTag must be filtered (below global info floor)"
    );

    // Send Info event for "OtherTag" → should pass (meets global "info" floor)
    send_native_log_with_level(&mut state, session_id, "OtherTag", LogLevel::Info, "info");
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 3,
        "Info event for OtherTag must pass (meets global info floor)"
    );
}

#[test]
fn test_native_log_tag_observed_even_when_level_filtered() {
    use crate::config::{NativeLogsSettings, TagConfig};
    use fdemon_core::LogLevel;

    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Set per-tag min_level = "warning" for "GoLog"
    state.settings.native_logs = NativeLogsSettings::default();
    state.settings.native_logs.tags.insert(
        "GoLog".to_string(),
        TagConfig {
            min_level: Some("warning".to_string()),
        },
    );

    // Send a Debug event — it will be filtered (below warning floor)
    send_native_log_with_level(
        &mut state,
        session_id,
        "GoLog",
        LogLevel::Debug,
        "below threshold",
    );

    // Event must be dropped (no log entry in buffer)
    let log_count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        log_count, 0,
        "Filtered event must not be added to the log buffer"
    );

    // But the tag must still appear in native_tag_state (observe_tag called before filter)
    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(
        handle.native_tag_state.tag_count(),
        1,
        "Tag must be observed even when its event is level-filtered"
    );
    // Tags are normalised to ASCII lowercase at storage time.
    assert_eq!(
        handle.native_tag_state.discovered_tags["golog"], 1,
        "GoLog observation count must be 1 even though the event was dropped"
    );
    assert!(
        handle.native_tag_state.is_tag_visible("GoLog"),
        "GoLog must be visible in the T-overlay even though its event was dropped"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Custom Log Source Tests (Phase 3, Task 04)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: build a CustomSourceStarted message carrying a fresh watch channel
/// and a trivial tokio task. Returns the message along with the watch receiver
/// so tests can check whether the shutdown signal was sent.
fn make_custom_source_started(
    session_id: crate::session::SessionId,
    name: &str,
) -> (
    Message,
    tokio::sync::watch::Receiver<bool>,
    std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
) {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let msg = Message::CustomSourceStarted {
        session_id,
        name: name.to_string(),
        shutdown_tx: std::sync::Arc::new(shutdown_tx),
        task_handle: task_handle.clone(),
        start_before_app: false,
    };
    (msg, shutdown_rx, task_handle)
}

#[test]
fn test_custom_source_started_stores_handle() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
        let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(task)));

        update(
            &mut state,
            Message::CustomSourceStarted {
                session_id,
                name: "GoLog".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle,
                start_before_app: false,
            },
        );

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.custom_source_handles.len(),
            1,
            "custom_source_handles should have one entry"
        );
        assert_eq!(
            handle.custom_source_handles[0].name, "GoLog",
            "stored handle should have the correct name"
        );
        assert!(
            handle.custom_source_handles[0].task_handle.is_some(),
            "task_handle should be Some after CustomSourceStarted"
        );
    });
}

#[test]
fn test_custom_source_started_multiple_sources_stored() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        for name in &["source-a", "source-b", "source-c"] {
            let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
            let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
            let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
                std::sync::Arc::new(std::sync::Mutex::new(Some(task)));

            update(
                &mut state,
                Message::CustomSourceStarted {
                    session_id,
                    name: name.to_string(),
                    shutdown_tx: std::sync::Arc::new(shutdown_tx),
                    task_handle,
                    start_before_app: false,
                },
            );
        }

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.custom_source_handles.len(),
            3,
            "all three custom sources should be stored"
        );
        let names: Vec<&str> = handle
            .custom_source_handles
            .iter()
            .map(|h| h.name.as_str())
            .collect();
        assert!(names.contains(&"source-a"));
        assert!(names.contains(&"source-b"));
        assert!(names.contains(&"source-c"));
    });
}

#[test]
fn test_custom_source_stopped_removes_handle() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Add two custom source handles.
        for name in &["keep-me", "remove-me"] {
            let (msg, _rx, _task_slot) = make_custom_source_started(session_id, name);
            update(&mut state, msg);
        }

        {
            let h = state.session_manager.get(session_id).unwrap();
            assert_eq!(
                h.custom_source_handles.len(),
                2,
                "should have two handles before stop"
            );
        }

        // Stop one of them.
        update(
            &mut state,
            Message::CustomSourceStopped {
                session_id,
                name: "remove-me".to_string(),
            },
        );

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.custom_source_handles.len(),
            1,
            "one handle should remain after CustomSourceStopped"
        );
        assert_eq!(
            handle.custom_source_handles[0].name, "keep-me",
            "the surviving handle should be 'keep-me'"
        );
    });
}

#[test]
fn test_custom_source_stopped_missing_session_is_no_op() {
    // Sending CustomSourceStopped for a non-existent session must not panic.
    let mut state = AppState::new();
    let missing_id: crate::session::SessionId = u64::MAX;
    update(
        &mut state,
        Message::CustomSourceStopped {
            session_id: missing_id,
            name: "any-source".to_string(),
        },
    );
    // No assertion needed — test passes if no panic occurs.
}

#[test]
fn test_custom_source_started_for_closed_session_sends_shutdown() {
    // When CustomSourceStarted arrives for a session that no longer exists,
    // the handler must send `true` on the shutdown channel so the orphaned
    // custom source task stops immediately.
    let mut state = AppState::new();
    let missing_id: crate::session::SessionId = u64::MAX;

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));

    update(
        &mut state,
        Message::CustomSourceStarted {
            session_id: missing_id,
            name: "orphaned".to_string(),
            shutdown_tx: std::sync::Arc::new(shutdown_tx),
            task_handle,
            start_before_app: false,
        },
    );

    assert_eq!(
        *shutdown_rx.borrow_and_update(),
        true,
        "shutdown_tx should have been signalled true when session is missing"
    );
}

#[test]
fn test_session_shutdown_cleans_custom_sources() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Add two custom sources.
        let (msg_a, _rx_a, _slot_a) = make_custom_source_started(session_id, "source-a");
        let (msg_b, _rx_b, _slot_b) = make_custom_source_started(session_id, "source-b");
        update(&mut state, msg_a);
        update(&mut state, msg_b);

        {
            let h = state.session_manager.get(session_id).unwrap();
            assert_eq!(
                h.custom_source_handles.len(),
                2,
                "two custom sources should be present before shutdown"
            );
        }

        // Call shutdown_native_logs — this also clears custom source handles.
        {
            let handle = state.session_manager.get_mut(session_id).unwrap();
            handle.shutdown_native_logs();
        }

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.custom_source_handles.len(),
            0,
            "custom_source_handles should be empty after shutdown_native_logs"
        );
    });
}

#[test]
fn test_custom_source_events_use_native_log_handler() {
    // Verify that NativeLog events from custom sources (which have the
    // same shape as platform events) flow through the same handler path.
    use fdemon_core::LogLevel;
    use fdemon_daemon::NativeLogEvent;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Register the custom source tag name as if the source started.
    let (msg, _rx, _slot) = make_custom_source_started(session_id, "my-tool");
    update(&mut state, msg);

    // Send a NativeLog event with the custom source's tag.
    let event = NativeLogEvent {
        tag: "my-tool".to_string(),
        level: LogLevel::Info,
        message: "hello from custom source".to_string(),
        timestamp: None,
    };
    update(&mut state, Message::NativeLog { session_id, event });

    // Flush batched logs.
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.flush_batched_logs();
    }

    let handle = state.session_manager.get(session_id).unwrap();

    // The tag should appear in native_tag_state.
    assert_eq!(
        handle.native_tag_state.tag_count(),
        1,
        "custom source tag should be tracked in native_tag_state"
    );
    assert!(
        handle.native_tag_state.is_tag_visible("my-tool"),
        "custom source tag should be visible"
    );

    // The log entry should be in the buffer.
    assert_eq!(
        handle.session.logs.len(),
        1,
        "log entry from custom source should be in the log buffer"
    );
    assert_eq!(
        handle.session.logs[0].message, "hello from custom source",
        "log message should match the custom source event"
    );
}

// ── double-spawn guard tests (pre-app-custom-sources Phase 1, Task 07) ────────

#[test]
fn test_custom_source_handle_has_start_before_app_field() {
    // Acceptance criterion 1: CustomSourceHandle has start_before_app field.
    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
    let pre_app_handle = crate::session::CustomSourceHandle {
        name: "server".to_string(),
        shutdown_tx: std::sync::Arc::new(shutdown_tx),
        task_handle: None,
        start_before_app: true,
    };
    assert!(
        pre_app_handle.start_before_app,
        "pre-app handle should have start_before_app = true"
    );

    let (shutdown_tx2, _rx2) = tokio::sync::watch::channel(false);
    let post_app_handle = crate::session::CustomSourceHandle {
        name: "logger".to_string(),
        shutdown_tx: std::sync::Arc::new(shutdown_tx2),
        task_handle: None,
        start_before_app: false,
    };
    assert!(
        !post_app_handle.start_before_app,
        "post-app handle should have start_before_app = false"
    );
}

#[test]
fn test_custom_source_started_stores_start_before_app_true() {
    // Acceptance criterion 3: pre-app sources are tagged with start_before_app = true.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
        let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));

        update(
            &mut state,
            Message::CustomSourceStarted {
                session_id,
                name: "pre-app-source".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle,
                start_before_app: true,
            },
        );

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.custom_source_handles.len(), 1);
        assert!(
            handle.custom_source_handles[0].start_before_app,
            "stored handle should have start_before_app = true for a pre-app source"
        );
    });
}

#[test]
fn test_custom_source_started_stores_start_before_app_false() {
    // Acceptance criterion 4: post-app sources are tagged with start_before_app = false.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("android-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
        let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));

        update(
            &mut state,
            Message::CustomSourceStarted {
                session_id,
                name: "post-app-source".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle,
                start_before_app: false,
            },
        );

        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.custom_source_handles.len(), 1);
        assert!(
            !handle.custom_source_handles[0].start_before_app,
            "stored handle should have start_before_app = false for a post-app source"
        );
    });
}

#[test]
fn test_guard_fires_on_hot_restart_with_pre_app_sources_only_running() {
    // Acceptance criterion 7: hot restart must not re-spawn pre-app sources.
    // When only pre-app sources are running and no post-app sources are
    // configured, the guard should fire and return None on repeated AppStart.
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability::default();
    state.settings.native_logs.enabled = true;
    // Only a pre-app source configured — no post-app sources.
    state.settings.native_logs.custom_sources = vec![crate::config::CustomSourceConfig {
        name: "server".to_string(),
        command: "echo".to_string(),
        args: vec![],
        format: fdemon_core::types::OutputFormat::default(),
        working_dir: None,
        env: std::collections::HashMap::new(),
        start_before_app: true,
        shared: false,
        ready_check: None,
    }];

    // Simulate that the pre-app source is already running (start_before_app = true).
    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle
            .custom_source_handles
            .push(crate::session::CustomSourceHandle {
                name: "server".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle: None,
                start_before_app: true,
            });
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "linux-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Guard should fire: pre-app source is tracked, no post-app sources to start.
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(
        action.is_none(),
        "Expected None when only pre-app sources are configured and already running, got {:?}",
        action
    );
}

#[test]
fn test_guard_allows_post_app_sources_when_only_pre_app_running() {
    // Acceptance criterion 8: post-app sources must still spawn correctly when
    // pre-app sources are already tracked in custom_source_handles.
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability::default();
    state.settings.native_logs.enabled = true;
    // Configure one pre-app source and one post-app source.
    state.settings.native_logs.custom_sources = vec![
        crate::config::CustomSourceConfig {
            name: "server".to_string(),
            command: "echo".to_string(),
            args: vec![],
            format: fdemon_core::types::OutputFormat::default(),
            working_dir: None,
            env: std::collections::HashMap::new(),
            start_before_app: true,
            shared: false,
            ready_check: None,
        },
        crate::config::CustomSourceConfig {
            name: "logger".to_string(),
            command: "echo".to_string(),
            args: vec![],
            format: fdemon_core::types::OutputFormat::default(),
            working_dir: None,
            env: std::collections::HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        },
    ];

    // Simulate that only the pre-app source is already running.
    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle
            .custom_source_handles
            .push(crate::session::CustomSourceHandle {
                name: "server".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle: None,
                start_before_app: true,
            });
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "linux-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Guard must NOT fire: the post-app source "logger" has not been started yet.
    // The action should be emitted so spawn_custom_sources() can start "logger".
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(
        matches!(
            action,
            Some(crate::handler::UpdateAction::StartNativeLogCapture { .. })
        ),
        "Expected Some(StartNativeLogCapture) when post-app source 'logger' not yet running, got {:?}",
        action
    );
}

#[test]
fn test_android_pre_app_only_allows_platform_capture_start() {
    // Android session with:
    // - Pre-app custom source running (in custom_source_handles)
    // - No post-app sources configured
    // - native_log_shutdown_tx = None (platform capture NOT started)
    // Branch B must NOT fire → function returns Some(StartNativeLogCapture).
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Enable adb so native_logs_available("android") returns true.
    state.tool_availability = ToolAvailability {
        adb: true,
        ..Default::default()
    };
    state.settings.native_logs.enabled = true;

    // Only a pre-app custom source — no post-app sources.
    state.settings.native_logs.custom_sources = vec![crate::config::CustomSourceConfig {
        name: "server".to_string(),
        command: "python3".to_string(),
        args: vec!["server.py".to_string()],
        format: fdemon_core::types::OutputFormat::default(),
        working_dir: None,
        env: std::collections::HashMap::new(),
        start_before_app: true,
        shared: false,
        ready_check: None,
    }];

    // Simulate pre-app source already running (stored in custom_source_handles).
    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle
            .custom_source_handles
            .push(crate::session::CustomSourceHandle {
                name: "server".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle: None,
                start_before_app: true,
            });
        // native_log_shutdown_tx is None — platform capture not started.
        assert!(handle.native_log_shutdown_tx.is_none());
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Branch B must NOT fire for Android — platform capture is still needed.
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(
        matches!(
            action,
            Some(crate::handler::UpdateAction::StartNativeLogCapture { .. })
        ),
        "Android with pre-app-only sources must still start platform capture, got {:?}",
        action
    );
}

#[test]
fn test_linux_pre_app_only_guard_still_fires() {
    // Linux session with:
    // - Pre-app custom source running
    // - No post-app sources
    // - native_log_shutdown_tx = None (Linux never sets it)
    // Branch B should fire → returns None (Linux doesn't need platform capture).
    use fdemon_core::{AppStart, DaemonMessage};
    use fdemon_daemon::ToolAvailability;

    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability::default();
    state.settings.native_logs.enabled = true;
    // Only a pre-app custom source — no post-app sources.
    state.settings.native_logs.custom_sources = vec![crate::config::CustomSourceConfig {
        name: "server".to_string(),
        command: "python3".to_string(),
        args: vec!["server.py".to_string()],
        format: fdemon_core::types::OutputFormat::default(),
        working_dir: None,
        env: std::collections::HashMap::new(),
        start_before_app: true,
        shared: false,
        ready_check: None,
    }];

    // Simulate pre-app source already running.
    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle
            .custom_source_handles
            .push(crate::session::CustomSourceHandle {
                name: "server".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle: None,
                start_before_app: true,
            });
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "linux-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    // Branch B should fire for Linux — all sources running, nothing left to do.
    let action = super::session::maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(
        action.is_none(),
        "Linux with all sources running should return None, got {:?}",
        action
    );
}

// ── pending_watcher_errors cap tests ──────────────────────────────────────────

#[test]
fn test_pending_watcher_errors_buffered_when_no_session() {
    let mut state = AppState::new();
    assert!(state.session_manager.selected_mut().is_none());

    update(
        &mut state,
        Message::WatcherError {
            message: "watch failed".to_string(),
        },
    );

    assert_eq!(
        state.pending_watcher_errors.len(),
        1,
        "error should be buffered when no session exists"
    );
    assert_eq!(state.pending_watcher_errors[0], "watch failed");
}

#[test]
fn test_pending_watcher_errors_capped_at_maximum() {
    let mut state = AppState::new();
    assert!(state.session_manager.selected_mut().is_none());

    // Push one more than the cap.
    for i in 0..=MAX_PENDING_WATCHER_ERRORS {
        update(
            &mut state,
            Message::WatcherError {
                message: format!("error {i}"),
            },
        );
    }

    assert_eq!(
        state.pending_watcher_errors.len(),
        MAX_PENDING_WATCHER_ERRORS,
        "pending_watcher_errors must not exceed MAX_PENDING_WATCHER_ERRORS"
    );
}

#[test]
fn test_pending_watcher_errors_oldest_errors_kept_when_cap_reached() {
    let mut state = AppState::new();
    assert!(state.session_manager.selected_mut().is_none());

    // Fill to the cap.
    for i in 0..MAX_PENDING_WATCHER_ERRORS {
        update(
            &mut state,
            Message::WatcherError {
                message: format!("error {i}"),
            },
        );
    }

    // Push one more; it should be silently dropped.
    update(
        &mut state,
        Message::WatcherError {
            message: "overflow error".to_string(),
        },
    );

    assert_eq!(
        state.pending_watcher_errors.len(),
        MAX_PENDING_WATCHER_ERRORS,
        "buffer size unchanged after overflow"
    );
    assert!(
        !state
            .pending_watcher_errors
            .contains(&"overflow error".to_string()),
        "the overflow error should have been dropped, not appended"
    );
    assert_eq!(
        state.pending_watcher_errors[0], "error 0",
        "oldest error (index 0) should be retained"
    );
}

// ─────────────────────────────────────────────────────────
// Pre-App Custom Source Message Variant Tests
// (pre-app-custom-sources Phase 1, Task 03)
// ─────────────────────────────────────────────────────────

#[test]
fn test_pre_app_message_variants_construct() {
    let device = test_device("emulator-1", "Test Emulator");

    let _msg = Message::PreAppSourcesReady {
        session_id: 1,
        device: device.clone(),
        config: None,
    };
    let _msg = Message::PreAppSourceTimedOut {
        session_id: 1,
        source_name: "server".to_string(),
    };
    let _msg = Message::PreAppSourceProgress {
        session_id: 1,
        message: "Starting server...".to_string(),
    };
}

#[test]
fn test_pre_app_sources_ready_triggers_spawn_session() {
    // Real handler: PreAppSourcesReady returns SpawnSession for an existing session.
    let device = test_device("emulator-1", "Test Emulator");
    let mut state = AppState::new();

    // Create a session so the handler finds it
    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(
        &mut state,
        Message::PreAppSourcesReady {
            session_id,
            device: device.clone(),
            config: None,
        },
    );
    assert!(
        matches!(
            result.action,
            Some(UpdateAction::SpawnSession { session_id: sid, .. }) if sid == session_id
        ),
        "PreAppSourcesReady should return SpawnSession, got {:?}",
        result.action
    );
    assert!(
        result.message.is_none(),
        "should return no follow-up message"
    );
}

#[test]
fn test_pre_app_sources_ready_noop_for_missing_session() {
    // Real handler: PreAppSourcesReady for a non-existent session is a no-op (no panic).
    let device = test_device("emulator-1", "Test Emulator");
    let mut state = AppState::new();

    // session_id 99999 does not exist
    let result = update(
        &mut state,
        Message::PreAppSourcesReady {
            session_id: 99999,
            device,
            config: None,
        },
    );
    assert!(
        result.action.is_none(),
        "should be a no-op for missing session"
    );
    assert!(
        result.message.is_none(),
        "should return no follow-up message"
    );
}

#[test]
fn test_pre_app_source_timed_out_adds_warning_log() {
    // Real handler: PreAppSourceTimedOut logs a warning entry to the session.
    let device = test_device("emulator-1", "Test Emulator");
    let mut state = AppState::new();

    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(
        &mut state,
        Message::PreAppSourceTimedOut {
            session_id,
            source_name: "my-server".to_string(),
        },
    );
    assert!(result.action.is_none(), "should return no action");
    assert!(
        result.message.is_none(),
        "should return no follow-up message"
    );

    // The session should have a warning log entry about the timeout
    let handle = state.session_manager.get(session_id).unwrap();
    let logs = &handle.session.logs;
    assert!(
        logs.iter().any(|e| {
            e.level == fdemon_core::LogLevel::Warning
                && e.message.contains("my-server")
                && e.message.contains("timed out")
        }),
        "expected a timeout warning log, got: {:?}",
        logs.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}

#[test]
fn test_pre_app_source_timed_out_noop_for_missing_session() {
    // Real handler: PreAppSourceTimedOut for a non-existent session is a no-op (no panic).
    let mut state = AppState::new();

    let result = update(
        &mut state,
        Message::PreAppSourceTimedOut {
            session_id: 99999,
            source_name: "my-server".to_string(),
        },
    );
    assert!(
        result.action.is_none(),
        "should be a no-op for missing session"
    );
}

#[test]
fn test_pre_app_source_progress_adds_info_log() {
    // Real handler: PreAppSourceProgress adds an info log entry to the session.
    let device = test_device("emulator-1", "Test Emulator");
    let mut state = AppState::new();

    let session_id = state.session_manager.create_session(&device).unwrap();

    let result = update(
        &mut state,
        Message::PreAppSourceProgress {
            session_id,
            message: "Starting server 'my-server'...".to_string(),
        },
    );
    assert!(result.action.is_none(), "should return no action");
    assert!(
        result.message.is_none(),
        "should return no follow-up message"
    );

    // The session should have an info log entry with the progress message
    let handle = state.session_manager.get(session_id).unwrap();
    let logs = &handle.session.logs;
    assert!(
        logs.iter().any(|e| {
            e.level == fdemon_core::LogLevel::Info
                && e.message.contains("Starting server 'my-server'...")
        }),
        "expected a progress info log, got: {:?}",
        logs.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}

#[test]
fn test_pre_app_source_progress_noop_for_missing_session() {
    // Real handler: PreAppSourceProgress for a non-existent session is a no-op (no panic).
    let mut state = AppState::new();

    let result = update(
        &mut state,
        Message::PreAppSourceProgress {
            session_id: 99999,
            message: "Waiting for server to be ready...".to_string(),
        },
    );
    assert!(
        result.action.is_none(),
        "should be a no-op for missing session"
    );
}

// ── Shared Custom Source handler tests (Phase 2, Task 04) ──────────────────

/// Helper: build a `SharedSourceStarted` message carrying a fresh watch channel
/// and (optionally) a real tokio task. Returns the message along with the watch
/// receiver so tests can verify shutdown signals, and the task-slot Arc so tests
/// can pre-populate it before calling `update`.
fn make_shared_source_started(
    name: &str,
    task: Option<tokio::task::JoinHandle<()>>,
) -> (
    Message,
    tokio::sync::watch::Receiver<bool>,
    std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
) {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let task_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(task));
    let msg = Message::SharedSourceStarted {
        name: name.to_string(),
        shutdown_tx: std::sync::Arc::new(shutdown_tx),
        task_handle: task_slot.clone(),
        start_before_app: false,
    };
    (msg, shutdown_rx, task_slot)
}

/// Helper: send a `SharedSourceLog` event with a given tag, level, and message.
fn send_shared_source_log(
    state: &mut AppState,
    tag: &str,
    level: fdemon_core::LogLevel,
    message: &str,
) {
    use fdemon_daemon::NativeLogEvent;
    let event = NativeLogEvent {
        tag: tag.to_string(),
        level,
        message: message.to_string(),
        timestamp: None,
    };
    update(state, Message::SharedSourceLog { event });
    state.session_manager.flush_all_pending_logs();
}

#[test]
fn test_shared_source_log_broadcasts_to_all_sessions() {
    // SharedSourceLog must append a log entry to every active session.
    let device_a = android_device("dev-a");
    let device_b = android_device("dev-b");
    let mut state = AppState::new();
    let sid_a = state.session_manager.create_session(&device_a).unwrap();
    let sid_b = state.session_manager.create_session(&device_b).unwrap();

    send_shared_source_log(
        &mut state,
        "my-source",
        fdemon_core::LogLevel::Info,
        "hello from shared",
    );

    let logs_a = &state.session_manager.get(sid_a).unwrap().session.logs;
    let logs_b = &state.session_manager.get(sid_b).unwrap().session.logs;

    assert_eq!(logs_a.len(), 1, "session A should have received the log");
    assert_eq!(logs_b.len(), 1, "session B should have received the log");
    assert!(
        logs_a[0].message.contains("hello from shared"),
        "session A log content mismatch"
    );
    assert!(
        logs_b[0].message.contains("hello from shared"),
        "session B log content mismatch"
    );
}

#[test]
fn test_shared_source_log_with_no_sessions_is_noop() {
    // SharedSourceLog with zero sessions must not panic and must be a no-op.
    let mut state = AppState::new();

    // No sessions created.
    send_shared_source_log(
        &mut state,
        "my-source",
        fdemon_core::LogLevel::Info,
        "nobody home",
    );
    // Test passes if no panic occurs.
}

#[test]
fn test_shared_source_log_applies_tag_filter() {
    // SharedSourceLog must honour the per-tag min-level setting. Events below
    // the configured floor must not be appended to any session's log buffer.
    use crate::config::{NativeLogsSettings, TagConfig};

    let device = android_device("dev-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Global floor = "info"; per-tag floor for "GoLog" = "warning".
    state.settings.native_logs = NativeLogsSettings {
        min_level: "info".to_string(),
        ..Default::default()
    };
    state.settings.native_logs.tags.insert(
        "GoLog".to_string(),
        TagConfig {
            min_level: Some("warning".to_string()),
        },
    );

    // Debug event for "GoLog" → below per-tag warning floor → filtered.
    send_shared_source_log(&mut state, "GoLog", fdemon_core::LogLevel::Debug, "debug");
    let count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(count, 0, "debug event for GoLog must be filtered");

    // Warning event for "GoLog" → at per-tag floor → allowed.
    send_shared_source_log(&mut state, "GoLog", fdemon_core::LogLevel::Warning, "warn");
    let count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(count, 1, "warning event for GoLog must pass");

    // Debug event for "OtherTag" → below global "info" floor → filtered.
    send_shared_source_log(
        &mut state,
        "OtherTag",
        fdemon_core::LogLevel::Debug,
        "debug other",
    );
    let count = state
        .session_manager
        .get(session_id)
        .unwrap()
        .session
        .logs
        .len();
    assert_eq!(
        count, 1,
        "debug event for OtherTag must be filtered (below global info floor)"
    );
}

#[test]
fn test_shared_source_log_tag_observed_on_all_sessions() {
    // Tags from a SharedSourceLog must be observed on every session so they
    // appear in the T-overlay, even when the event is below the level floor.
    use crate::config::{NativeLogsSettings, TagConfig};

    let device_a = android_device("dev-a");
    let device_b = android_device("dev-b");
    let mut state = AppState::new();
    let sid_a = state.session_manager.create_session(&device_a).unwrap();
    let sid_b = state.session_manager.create_session(&device_b).unwrap();

    // Set a per-tag floor high enough to filter the event.
    state.settings.native_logs = NativeLogsSettings::default();
    state.settings.native_logs.tags.insert(
        "my-source".to_string(),
        TagConfig {
            min_level: Some("error".to_string()),
        },
    );

    // Send a debug event — it will be filtered, but the tag must still be observed.
    send_shared_source_log(
        &mut state,
        "my-source",
        fdemon_core::LogLevel::Debug,
        "filtered",
    );

    // No log entries should have been appended.
    assert_eq!(
        state.session_manager.get(sid_a).unwrap().session.logs.len(),
        0,
        "no log should be appended when event is below floor"
    );

    // But the tag must be tracked in both sessions' native_tag_state.
    let tags_a = state
        .session_manager
        .get(sid_a)
        .unwrap()
        .native_tag_state
        .sorted_tags();
    let tags_b = state
        .session_manager
        .get(sid_b)
        .unwrap()
        .native_tag_state
        .sorted_tags();

    assert!(
        tags_a.iter().any(|(t, _)| t.as_str() == "my-source"),
        "tag must be observed on session A even when event was filtered"
    );
    assert!(
        tags_b.iter().any(|(t, _)| t.as_str() == "my-source"),
        "tag must be observed on session B even when event was filtered"
    );
}

#[test]
fn test_shared_source_started_stores_handle() {
    // SharedSourceStarted must push a SharedSourceHandle onto state.shared_source_handles.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut state = AppState::new();

        let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let (msg, _rx, _slot) = make_shared_source_started("my-shared-source", Some(task));

        let result = update(&mut state, msg);

        assert!(result.action.is_none(), "should return no action");
        assert_eq!(
            state.shared_source_handles.len(),
            1,
            "one handle should be stored"
        );
        assert_eq!(
            state.shared_source_handles[0].name, "my-shared-source",
            "stored handle must have the correct name"
        );
        assert!(
            state.shared_source_handles[0].task_handle.is_some(),
            "task_handle must be Some after SharedSourceStarted"
        );
    });
}

#[test]
fn test_shared_source_started_multiple_handles_stored() {
    // Each SharedSourceStarted must push a new entry; multiple sources accumulate.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut state = AppState::new();

        for name in &["source-x", "source-y", "source-z"] {
            let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
            let (msg, _rx, _slot) = make_shared_source_started(name, Some(task));
            update(&mut state, msg);
        }

        assert_eq!(
            state.shared_source_handles.len(),
            3,
            "all three shared source handles should be stored"
        );
        let names: Vec<&str> = state
            .shared_source_handles
            .iter()
            .map(|h| h.name.as_str())
            .collect();
        assert!(names.contains(&"source-x"));
        assert!(names.contains(&"source-y"));
        assert!(names.contains(&"source-z"));
    });
}

#[test]
fn test_shared_source_stopped_removes_handle_and_warns() {
    // SharedSourceStopped must remove the matching handle and append a warning
    // log entry to every active session.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device_a = android_device("dev-a");
        let device_b = android_device("dev-b");
        let mut state = AppState::new();
        let sid_a = state.session_manager.create_session(&device_a).unwrap();
        let sid_b = state.session_manager.create_session(&device_b).unwrap();

        // Register two shared source handles.
        for name in &["keep-me", "remove-me"] {
            let (msg, _rx, _slot) = make_shared_source_started(name, None);
            update(&mut state, msg);
        }
        assert_eq!(state.shared_source_handles.len(), 2);

        // Stop one of them.
        let result = update(
            &mut state,
            Message::SharedSourceStopped {
                name: "remove-me".to_string(),
            },
        );

        assert!(result.action.is_none(), "should return no action");

        // Only the surviving handle should remain.
        assert_eq!(
            state.shared_source_handles.len(),
            1,
            "one handle must remain after SharedSourceStopped"
        );
        assert_eq!(
            state.shared_source_handles[0].name, "keep-me",
            "surviving handle must be 'keep-me'"
        );

        // Both sessions must have received a warning log.
        // Note: no manual flush needed — the handler calls flush_batched_logs() itself.
        let logs_a = &state.session_manager.get(sid_a).unwrap().session.logs;
        let logs_b = &state.session_manager.get(sid_b).unwrap().session.logs;

        assert!(
            logs_a.iter().any(|e| {
                e.level == fdemon_core::LogLevel::Warning
                    && e.message.contains("remove-me")
                    && e.message.contains("stopped")
            }),
            "session A must have a warning log mentioning 'remove-me', got: {:?}",
            logs_a.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
        assert!(
            logs_b.iter().any(|e| {
                e.level == fdemon_core::LogLevel::Warning
                    && e.message.contains("remove-me")
                    && e.message.contains("stopped")
            }),
            "session B must have a warning log mentioning 'remove-me', got: {:?}",
            logs_b.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
    });
}

#[test]
fn test_shared_source_stopped_no_sessions_is_noop() {
    // SharedSourceStopped with zero sessions must not panic.
    let mut state = AppState::new();

    // Register a handle, then stop it without any sessions present.
    let (msg, _rx, _slot) = make_shared_source_started("solo-source", None);
    update(&mut state, msg);
    assert_eq!(state.shared_source_handles.len(), 1);

    update(
        &mut state,
        Message::SharedSourceStopped {
            name: "solo-source".to_string(),
        },
    );

    assert_eq!(
        state.shared_source_handles.len(),
        0,
        "handle must be removed even when no sessions exist"
    );
    // Test passes if no panic occurs.
}

#[test]
fn test_shared_source_stopped_unknown_name_is_noop() {
    // SharedSourceStopped for a name that was never registered must not panic
    // and must leave existing handles intact.
    let mut state = AppState::new();

    let (msg, _rx, _slot) = make_shared_source_started("real-source", None);
    update(&mut state, msg);
    assert_eq!(state.shared_source_handles.len(), 1);

    update(
        &mut state,
        Message::SharedSourceStopped {
            name: "ghost-source".to_string(),
        },
    );

    assert_eq!(
        state.shared_source_handles.len(),
        1,
        "existing handle must not be removed when name does not match"
    );
}

// ── Gap-filling tests for task 09 (pre-app-custom-sources Phase 2) ─────────

#[test]
fn test_shared_source_started_stores_on_app_state() {
    // SharedSourceStarted must push onto state.shared_source_handles (AppState
    // level), and must NOT add anything to any SessionHandle's custom_source_handles.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("dev-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let (msg, _rx, _slot) = make_shared_source_started("shared-src", Some(task));
        update(&mut state, msg);

        // Handle is stored at the AppState level.
        assert_eq!(
            state.shared_source_handles.len(),
            1,
            "shared_source_handles should have one entry"
        );
        assert_eq!(state.shared_source_handles[0].name, "shared-src");

        // No per-session custom_source_handle must have been created.
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(
            handle.custom_source_handles.is_empty(),
            "SessionHandle.custom_source_handles must remain empty for a shared source"
        );
    });
}

#[test]
fn test_shared_source_survives_session_close() {
    // SharedSourceHandle lives on AppState, not on SessionHandle.
    // Removing a session must leave state.shared_source_handles intact.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("dev-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Register a shared source handle.
        let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let (msg, _rx, _slot) = make_shared_source_started("persistent-src", Some(task));
        update(&mut state, msg);
        assert_eq!(state.shared_source_handles.len(), 1);

        // Simulate session close: shut down native logs and remove from manager.
        if let Some(mut handle) = state.session_manager.remove_session(session_id) {
            handle.shutdown_native_logs();
        }
        assert_eq!(
            state.session_manager.len(),
            0,
            "session should have been removed"
        );

        // The shared source handle must still be present on AppState.
        assert_eq!(
            state.shared_source_handles.len(),
            1,
            "shared_source_handles must survive session close"
        );
        assert_eq!(state.shared_source_handles[0].name, "persistent-src");
    });
}

#[test]
fn test_shared_source_started_duplicate_is_rejected() {
    // A second SharedSourceStarted carrying the same name must be rejected by the
    // dedup guard: the handle count must stay at 1, the duplicate's shutdown
    // channel must receive `true`, and the duplicate's task must be aborted.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut state = AppState::new();

        // First SharedSourceStarted — should succeed and register the handle.
        let task1: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let (msg1, _shutdown_rx1, _slot1) = make_shared_source_started("my-source", Some(task1));
        let result1 = update(&mut state, msg1);
        assert!(
            result1.action.is_none(),
            "first message should return no action"
        );
        assert_eq!(
            state.shared_source_handles.len(),
            1,
            "first SharedSourceStarted must register the handle"
        );

        // Second SharedSourceStarted with the SAME name — must be rejected.
        let (shutdown_tx2, shutdown_rx2) = tokio::sync::watch::channel(false);
        let task2: tokio::task::JoinHandle<()> =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(60)).await });
        let task_slot2: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(task2)));
        let msg2 = Message::SharedSourceStarted {
            name: "my-source".to_string(),
            shutdown_tx: std::sync::Arc::new(shutdown_tx2),
            task_handle: task_slot2.clone(),
            start_before_app: true,
        };
        let result2 = update(&mut state, msg2);

        // Dedup guard must have fired: no action, handle count unchanged.
        assert!(
            result2.action.is_none(),
            "duplicate message should return no action"
        );
        assert_eq!(
            state.shared_source_handles.len(),
            1,
            "duplicate SharedSourceStarted must NOT push a second handle"
        );
        assert_eq!(
            state.shared_source_handles[0].name, "my-source",
            "the surviving handle must be the first one"
        );

        // The duplicate's shutdown channel must have received `true`.
        assert_eq!(
            *shutdown_rx2.borrow(),
            true,
            "dedup guard must signal the duplicate to shut down"
        );

        // The duplicate's task slot must now be empty (task was aborted and taken).
        let slot_after = task_slot2.lock().unwrap();
        assert!(
            slot_after.is_none(),
            "dedup guard must take (and abort) the duplicate task from the slot"
        );
    });
}

#[test]
fn test_non_shared_source_still_per_session() {
    // A non-shared CustomSourceStarted must store its handle on the SessionHandle
    // (custom_source_handles), NOT on AppState.shared_source_handles.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let device = android_device("dev-1");
        let mut state = AppState::new();
        let session_id = state.session_manager.create_session(&device).unwrap();

        let task: tokio::task::JoinHandle<()> = tokio::spawn(async {});
        let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
        let task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(task)));

        update(
            &mut state,
            Message::CustomSourceStarted {
                session_id,
                name: "per-session-src".to_string(),
                shutdown_tx: std::sync::Arc::new(shutdown_tx),
                task_handle,
                start_before_app: false,
            },
        );

        // Handle is on the SessionHandle.
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            handle.custom_source_handles.len(),
            1,
            "SessionHandle.custom_source_handles should have one entry for a non-shared source"
        );
        assert_eq!(handle.custom_source_handles[0].name, "per-session-src");

        // AppState.shared_source_handles must remain empty.
        assert!(
            state.shared_source_handles.is_empty(),
            "shared_source_handles must be empty — non-shared sources are per-session only"
        );
    });
}
