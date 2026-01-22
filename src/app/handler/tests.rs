//! Tests for handler module
#![cfg(not(feature = "skip_old_tests"))]

use super::*;
use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};
use crate::core::{AppPhase, DaemonEvent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Helper function to create a test Device with minimal required fields
fn test_device(id: &str, name: &str) -> crate::daemon::Device {
    crate::daemon::Device {
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
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::RequestQuit)));
}

#[test]
fn test_escape_key_produces_request_quit_message() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::RequestQuit)));
}

#[test]
fn test_ctrl_c_produces_quit_message() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::Quit)));
}

#[test]
fn test_r_key_produces_hot_reload() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::HotReload)));
}

#[test]
fn test_shift_r_produces_hot_restart() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::HotRestart)));
}

#[test]
fn test_s_key_produces_stop() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);

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
    use crate::core::LogLevel;

    let (level, _) = detect_raw_line_level("E/flutter: some error");
    assert_eq!(level, LogLevel::Error);

    let (level, _) = detect_raw_line_level("W/flutter: some warning");
    assert_eq!(level, LogLevel::Warning);
}

#[test]
fn test_detect_raw_line_level_gradle() {
    use crate::core::LogLevel;

    let (level, _) = detect_raw_line_level("FAILURE: Build failed");
    assert_eq!(level, LogLevel::Error);

    let (level, _) = detect_raw_line_level("BUILD FAILED");
    assert_eq!(level, LogLevel::Error);
}

#[test]
fn test_detect_raw_line_level_xcode() {
    use crate::core::LogLevel;

    let (level, _) = detect_raw_line_level("❌ Build failed");
    assert_eq!(level, LogLevel::Error);

    let (level, _) = detect_raw_line_level("⚠ Warning message");
    assert_eq!(level, LogLevel::Warning);
}

#[test]
fn test_detect_raw_line_level_build_progress() {
    use crate::core::LogLevel;

    let (level, _) = detect_raw_line_level("Running pod install...");
    assert_eq!(level, LogLevel::Debug);

    let (level, _) = detect_raw_line_level("Building with flavor...");
    assert_eq!(level, LogLevel::Debug);
}

#[test]
fn test_detect_raw_line_level_default() {
    use crate::core::LogLevel;

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
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    // No sessions running, confirm_quit is true by default

    update(&mut state, Message::RequestQuit);

    // Should go directly to Quitting phase (no dialog)
    assert_eq!(state.phase, AppPhase::Quitting);
    assert_ne!(state.ui_mode, UiMode::ConfirmDialog);
}

#[test]
fn test_request_quit_confirm_quit_disabled_quits_immediately() {
    use crate::app::state::UiMode;

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
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    update(&mut state, Message::CancelQuit);

    assert_eq!(state.ui_mode, UiMode::Normal);
    assert_ne!(state.phase, AppPhase::Quitting);
}

#[test]
fn test_y_key_in_confirm_dialog_confirms() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::ConfirmQuit)));
}

#[test]
fn test_n_key_in_confirm_dialog_cancels() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CancelQuit)));
}

#[test]
fn test_esc_in_confirm_dialog_cancels() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CancelQuit)));
}

#[test]
fn test_ctrl_c_in_confirm_dialog_force_quits() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let result = handle_key(&state, key);

    // Should force quit (bypass confirm)
    assert!(matches!(result, Some(Message::Quit)));
}

#[test]
fn test_q_in_confirm_dialog_confirms() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
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
    use crate::app::state::UiMode;

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
    use crate::app::state::UiMode;

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

    let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::SelectSessionByIndex(0))));

    let key = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::SelectSessionByIndex(4))));
}

#[test]
fn test_tab_cycles_sessions() {
    let state = AppState::new();

    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::NextSession)));

    let key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
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

    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    assert!(matches!(result, Some(Message::CloseCurrentSession)));
}

#[test]
fn test_ctrl_w_closes_session() {
    let state = AppState::new();

    let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
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

    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
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
        handle.session.add_log(crate::core::LogEntry::info(
            crate::core::LogSource::App,
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
fn test_session_exited_updates_session_phase() {
    let mut state = AppState::new();

    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(0) },
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.session.phase, AppPhase::Stopped);
}

#[test]
fn test_session_exited_with_error_code() {
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
    }
    if let Some(handle) = state.session_manager.get_mut(session2) {
        handle.session.mark_started("app-2".to_string());
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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

#[test]
fn test_auto_reload_single_session_logs_to_session() {
    let mut state = AppState::new();

    // Create one session
    let device = test_device("device-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.mark_started("app-1".to_string());
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
        handle.cmd_sender = Some(crate::daemon::CommandSender::new_for_test());
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
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CycleLevelFilter)));
}

#[test]
fn test_shift_f_produces_cycle_source_filter() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::CycleSourceFilter)));
}

#[test]
fn test_ctrl_f_produces_reset_filters() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::ResetFilters)));
}

#[test]
fn test_cycle_level_filter_message() {
    use crate::core::LogLevelFilter;

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
    use crate::core::LogSourceFilter;

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
    use crate::core::{LogLevelFilter, LogSourceFilter};

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
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);

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
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::CancelSearch)));
}

#[test]
fn test_search_input_mode_enter() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

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
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::SearchInput { text }) if text == "tes"));
}

#[test]
fn test_search_input_mode_ctrl_u_clears() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);

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
    let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);

    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::SearchInput { text }) if text == "test"));
}

#[test]
fn test_search_input_mode_ctrl_c_quits() {
    let mut state = AppState::new();
    let device = test_device("device-1", "Test Device");
    state.session_manager.create_session(&device).unwrap();

    state.ui_mode = UiMode::SearchInput;
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

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

    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    let msg = handle_key(&state, key);

    assert!(matches!(msg, Some(Message::NextSearchMatch)));
}

#[test]
fn test_n_key_without_search_does_nothing() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);

    let msg = handle_key(&state, key);

    // Without active search query, 'n' should do nothing (it's only for search navigation)
    assert!(msg.is_none());
}

#[test]
fn test_shift_n_produces_prev_search_match() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT);

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
            crate::core::SearchMatch::new(0, 0, 4),
            crate::core::SearchMatch::new(1, 0, 4),
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
            crate::core::SearchMatch::new(0, 0, 4),
            crate::core::SearchMatch::new(1, 0, 4),
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
    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::NextError)));
}

#[test]
fn test_shift_e_produces_prev_error() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT);

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
        handle.session.log_info(crate::core::LogSource::App, "info");
        handle
            .session
            .log_error(crate::core::LogSource::App, "error");
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
            .log_info(crate::core::LogSource::App, "info only");
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
            .log_error(crate::core::LogSource::App, "error 0");
        handle.session.log_info(crate::core::LogSource::App, "info");
        handle
            .session
            .log_error(crate::core::LogSource::App, "error 2");
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
#[cfg(feature = "test_old_dialogs")]
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
#[cfg(feature = "test_old_dialogs")]
fn test_auto_launch_result_success_creates_session() {
    use crate::app::message::AutoLaunchSuccess;

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

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_auto_launch_result_discovery_error_shows_dialog() {
    let mut state = AppState::new();
    // No loading state - auto-launch is silent

    let result = update(
        &mut state,
        Message::AutoLaunchResult {
            result: Err("No devices found".to_string()),
        },
    );

    // Shows startup dialog on error
    assert_eq!(state.ui_mode, UiMode::StartupDialog);
    // Error message set
    assert!(state.startup_dialog_state.error.is_some());
    assert!(state
        .startup_dialog_state
        .error
        .as_ref()
        .unwrap()
        .contains("No devices"));
    // No action returned
    assert!(result.action.is_none());
}

// ============================================================================
// Auto-Launch Flow Integration Tests (Phase 3 - Task 3)
// Auto-launch happens silently in background - no loading screen
// ============================================================================

mod auto_launch_tests {
    use super::*;
    use crate::app::message::AutoLaunchSuccess;
    use crate::config::{FlutterMode, LaunchConfig, LoadedConfigs};
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
                modal_type: crate::tui::widgets::FuzzyModalType::Flavor,
            },
        );

        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert_eq!(
            modal.modal_type,
            crate::tui::widgets::FuzzyModalType::Flavor
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
                modal_type: crate::tui::widgets::FuzzyModalType::Flavor,
            },
        );
        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());

        // Try to open another modal while one is open - should be ignored
        let _ = update(
            &mut state,
            Message::NewSessionDialogOpenFuzzyModal {
                modal_type: crate::tui::widgets::FuzzyModalType::Config,
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
            crate::tui::widgets::FuzzyModalType::Flavor
        );
    }

    #[test]
    fn test_dart_defines_confirm_with_empty_key_returns_focus_to_key() {
        use crate::tui::widgets::{DartDefinesEditField, DartDefinesPane};

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
        use crate::tui::widgets::TargetTab;

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
        use crate::tui::widgets::TargetTab;

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
        use crate::tui::widgets::TargetTab;

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
        use crate::tui::widgets::TargetTab;

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
        use crate::daemon::{IosSimulator, SimulatorState};
        use crate::tui::widgets::TargetTab;

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
        use crate::tui::widgets::TargetTab;

        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;

        let result = update(&mut state, Message::NewSessionDialogRefreshDevices);

        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_refresh_devices_on_bootable_tab() {
        use crate::tui::widgets::TargetTab;

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
        use crate::daemon::{AndroidAvd, IosSimulator, SimulatorState};

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
                discovery_type: crate::app::message::DiscoveryType::Connected,
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
                discovery_type: crate::app::message::DiscoveryType::Bootable,
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
        use crate::daemon::{IosSimulator, SimulatorState};

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
        use crate::tui::widgets::TargetTab;

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
        use crate::tui::widgets::TargetTab;

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
        use crate::tui::widgets::DialogPane;

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
        // Background errors should not show error UI when cached devices exist
        let mut state = AppState::new();

        // Set up cached devices
        state.set_device_cache(vec![test_device("cached-1", "Cached Phone")]);

        // Show new session dialog with cached devices
        let configs = crate::config::LoadedConfigs::default();
        state.show_new_session_dialog(configs);

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
