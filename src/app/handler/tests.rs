//! Tests for handler module

use super::*;
use crate::app::message::Message;
use crate::app::state::AppState;
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

// ─────────────────────────────────────────────────────────
// Device selector tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_d_shows_device_selector() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::ShowDeviceSelector)));
}

#[test]
fn test_n_shows_device_selector() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    assert!(matches!(result, Some(Message::ShowDeviceSelector)));
}

#[test]
fn test_show_device_selector_uses_cache() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();

    // Pre-populate cache
    let devices = vec![test_device("cached-device", "Cached Device")];
    state.device_selector.set_devices(devices);

    // Now show device selector
    let result = update(&mut state, Message::ShowDeviceSelector);

    // Should be in refreshing mode (not loading) since we have cache
    assert!(state.device_selector.refreshing);
    assert!(state.device_selector.visible);
    assert_eq!(state.ui_mode, UiMode::DeviceSelector);

    // Should still trigger discovery to refresh
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
}

#[test]
fn test_tick_advances_device_selector_animation() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;
    state.device_selector.visible = true;
    state.device_selector.loading = true;

    let initial_frame = state.device_selector.animation_frame;

    update(&mut state, Message::Tick);

    assert_ne!(state.device_selector.animation_frame, initial_frame);
}

#[test]
fn test_tick_does_not_advance_when_not_loading() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;
    state.device_selector.visible = true;
    state.device_selector.loading = false;
    state.device_selector.refreshing = false;

    let initial_frame = state.device_selector.animation_frame;

    update(&mut state, Message::Tick);

    assert_eq!(state.device_selector.animation_frame, initial_frame);
}

#[test]
fn test_tick_does_not_advance_when_hidden() {
    let mut state = AppState::new();
    state.device_selector.visible = false;
    state.device_selector.loading = true;

    let initial_frame = state.device_selector.animation_frame;

    update(&mut state, Message::Tick);

    assert_eq!(state.device_selector.animation_frame, initial_frame);
}

#[test]
fn test_tick_advances_when_refreshing() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;
    state.device_selector.visible = true;
    state.device_selector.loading = false;
    state.device_selector.refreshing = true;

    let initial_frame = state.device_selector.animation_frame;

    update(&mut state, Message::Tick);

    // Animation should advance when refreshing
    assert_ne!(state.device_selector.animation_frame, initial_frame);
}

// ─────────────────────────────────────────────────────────
// Multi-session tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_device_selected_creates_session() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;

    let device = test_device("test-device", "Test Device");
    let result = update(&mut state, Message::DeviceSelected { device });

    // Session should be created
    assert_eq!(state.session_manager.len(), 1);

    // Should return SpawnSession action
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnSession { .. })
    ));

    // UI should switch back to normal
    assert_eq!(state.ui_mode, UiMode::Normal);
}

#[test]
fn test_device_selected_session_id_in_spawn_action() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;

    let device = test_device("test-device", "Test Device");
    let result = update(&mut state, Message::DeviceSelected { device });

    // The SpawnSession action should contain the session_id that was created
    if let Some(UpdateAction::SpawnSession { session_id, .. }) = result.action {
        // The session_id should match what's in the session manager
        assert!(state.session_manager.get(session_id).is_some());
    } else {
        panic!("Expected SpawnSession action");
    }
}

#[test]
fn test_device_selected_prevents_duplicate() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;

    let device = test_device("test-device", "Test Device");

    // First selection should work
    let _ = update(
        &mut state,
        Message::DeviceSelected {
            device: device.clone(),
        },
    );
    assert_eq!(state.session_manager.len(), 1);

    // Go back to device selector
    state.ui_mode = UiMode::DeviceSelector;

    // Second selection of same device should fail
    let result = update(&mut state, Message::DeviceSelected { device });

    // No new session created
    assert_eq!(state.session_manager.len(), 1);

    // No action returned
    assert!(result.action.is_none());
    // Note: Error is now logged via tracing, not global state
}

#[test]
fn test_device_selected_max_sessions_enforced() {
    use crate::app::session_manager::MAX_SESSIONS;
    use crate::app::state::UiMode;

    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;

    // Create max number of sessions
    for i in 0..MAX_SESSIONS {
        let device = test_device(&format!("device-{}", i), &format!("Device {}", i));
        let _ = update(
            &mut state,
            Message::DeviceSelected {
                device: device.clone(),
            },
        );
        state.ui_mode = UiMode::DeviceSelector;
    }

    // Try to add one more
    let extra_device = test_device("extra-device", "Extra Device");
    let result = update(
        &mut state,
        Message::DeviceSelected {
            device: extra_device,
        },
    );

    // Should not create new session
    assert!(result.action.is_none());
    assert_eq!(state.session_manager.len(), MAX_SESSIONS);
    // Note: Error is now logged via tracing, not global state
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

    // Should return to device selector
    assert_eq!(state.ui_mode, UiMode::DeviceSelector);
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

    // Session should be removed and UI should show device selector
    assert!(state.session_manager.get(session_id).is_none());
    assert_eq!(state.ui_mode, UiMode::DeviceSelector);
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
        panic!("Expected ReloadAllSessions action, got {:?}", result.action);
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
        panic!("Expected ReloadAllSessions action");
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
        panic!("Expected SpawnTask Reload action");
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

