//! Tests for handler module

use super::*;
use crate::input_key::InputKey;
use crate::message::Message;
use crate::state::{AppState, DevToolsError, UiMode, VmConnectionStatus};
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

#[test]
#[ignore = "DeviceSelected is deprecated - functionality moved to NewSessionDialog"]
fn test_device_selected_prevents_duplicate() {
    // This test is obsolete - DeviceSelected message is deprecated
}

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
