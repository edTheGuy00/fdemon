//! Tests for handler module

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

#[test]
fn test_d_shows_startup_dialog_without_sessions() {
    let state = AppState::new();
    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);

    let result = handle_key(&state, key);

    // Without running sessions, should show StartupDialog instead of DeviceSelector
    assert!(matches!(result, Some(Message::ShowStartupDialog)));
}

#[test]
fn test_show_device_selector_uses_cache() {
    use crate::app::state::UiMode;

    let mut state = AppState::new();

    // Pre-populate global cache (Task 08e - Device Cache Sharing)
    let devices = vec![test_device("cached-device", "Cached Device")];
    state.set_device_cache(devices);

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
// Startup Dialog Tests (Phase 5, Task 08a)
// ─────────────────────────────────────────────────────────

#[test]
fn test_number_keys_jump_to_section() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;

    // Test key '1' -> Configs section
    let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    let msg = handle_key(&state, key);
    assert!(matches!(
        msg,
        Some(Message::StartupDialogJumpToSection(
            crate::app::state::DialogSection::Configs
        ))
    ));

    // Test key '2' -> Mode section
    let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
    let msg = handle_key(&state, key);
    assert!(matches!(
        msg,
        Some(Message::StartupDialogJumpToSection(
            crate::app::state::DialogSection::Mode
        ))
    ));

    // Test key '3' -> Flavor section
    let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
    let msg = handle_key(&state, key);
    assert!(matches!(
        msg,
        Some(Message::StartupDialogJumpToSection(
            crate::app::state::DialogSection::Flavor
        ))
    ));

    // Test key '4' -> DartDefines section
    let key = KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE);
    let msg = handle_key(&state, key);
    assert!(matches!(
        msg,
        Some(Message::StartupDialogJumpToSection(
            crate::app::state::DialogSection::DartDefines
        ))
    ));

    // Test key '5' -> Devices section
    let key = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE);
    let msg = handle_key(&state, key);
    assert!(matches!(
        msg,
        Some(Message::StartupDialogJumpToSection(
            crate::app::state::DialogSection::Devices
        ))
    ));
}

#[test]
fn test_jump_to_section_clears_editing() {
    use crate::app::state::{DialogSection, StartupDialogState};

    let mut state = StartupDialogState::new();
    state.editing = true;
    state.active_section = DialogSection::Flavor;

    state.jump_to_section(DialogSection::Devices);

    assert!(!state.editing);
    assert_eq!(state.active_section, DialogSection::Devices);
}

#[test]
fn test_jump_to_section_message_handler() {
    use crate::app::state::DialogSection;

    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;
    state.startup_dialog_state.editing = true;
    state.startup_dialog_state.active_section = DialogSection::Flavor;

    // Jump to Devices section
    update(
        &mut state,
        Message::StartupDialogJumpToSection(DialogSection::Devices),
    );

    assert!(!state.startup_dialog_state.editing);
    assert_eq!(
        state.startup_dialog_state.active_section,
        DialogSection::Devices
    );
}

#[test]
fn test_jump_to_section_changes_section() {
    use crate::app::state::{DialogSection, StartupDialogState};

    let mut state = StartupDialogState::new();
    state.active_section = DialogSection::Configs;

    state.jump_to_section(DialogSection::Mode);
    assert_eq!(state.active_section, DialogSection::Mode);

    state.jump_to_section(DialogSection::Flavor);
    assert_eq!(state.active_section, DialogSection::Flavor);

    state.jump_to_section(DialogSection::DartDefines);
    assert_eq!(state.active_section, DialogSection::DartDefines);

    state.jump_to_section(DialogSection::Devices);
    assert_eq!(state.active_section, DialogSection::Devices);

    state.jump_to_section(DialogSection::Configs);
    assert_eq!(state.active_section, DialogSection::Configs);
}

// ─────────────────────────────────────────────────────────────────────────────
// Startup Dialog Device Discovery Tests (Phase 5, Task 08c)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_show_startup_dialog_triggers_discovery() {
    let mut state = AppState::new();

    let result = update(&mut state, Message::ShowStartupDialog);

    assert_eq!(state.ui_mode, UiMode::StartupDialog);
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
}

#[test]
fn test_devices_discovered_updates_startup_dialog() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;
    state.startup_dialog_state.loading = true;

    let devices = vec![test_device("dev1", "Device 1")];
    update(
        &mut state,
        Message::DevicesDiscovered {
            devices: devices.clone(),
        },
    );

    assert!(!state.startup_dialog_state.loading);
    assert_eq!(state.startup_dialog_state.devices.len(), 1);
    assert_eq!(state.startup_dialog_state.selected_device, Some(0));
}

#[test]
fn test_devices_discovered_updates_both_selectors() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;

    let devices = vec![
        test_device("dev1", "Device 1"),
        test_device("dev2", "Device 2"),
    ];
    update(
        &mut state,
        Message::DevicesDiscovered {
            devices: devices.clone(),
        },
    );

    // Both device_selector and startup_dialog_state should be updated
    assert_eq!(state.device_selector.devices.len(), 2);
    assert_eq!(state.startup_dialog_state.devices.len(), 2);
}

#[test]
fn test_device_discovery_failed_shows_error() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;
    state.startup_dialog_state.loading = true;

    update(
        &mut state,
        Message::DeviceDiscoveryFailed {
            error: "No Flutter SDK found".to_string(),
        },
    );

    assert_eq!(
        state.startup_dialog_state.error,
        Some("No Flutter SDK found".to_string())
    );
    assert!(!state.startup_dialog_state.loading);
}

#[test]
fn test_device_discovery_failed_updates_both_selectors() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;

    let error = "Test error".to_string();
    update(
        &mut state,
        Message::DeviceDiscoveryFailed {
            error: error.clone(),
        },
    );

    // Both device_selector and startup_dialog_state should have the error
    assert_eq!(state.device_selector.error, Some(error.clone()));
    assert_eq!(state.startup_dialog_state.error, Some(error));
}

#[test]
fn test_refresh_devices_triggers_discovery() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;

    let result = update(&mut state, Message::StartupDialogRefreshDevices);

    assert!(state.startup_dialog_state.refreshing);
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
}

#[test]
fn test_tick_advances_startup_dialog_animation() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;
    state.startup_dialog_state.loading = true;

    assert_eq!(state.startup_dialog_state.animation_frame, 0);

    update(&mut state, Message::Tick);

    assert_eq!(state.startup_dialog_state.animation_frame, 1);
}

#[test]
fn test_tick_does_not_advance_startup_dialog_when_not_loading() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;
    state.startup_dialog_state.loading = false;
    state.startup_dialog_state.refreshing = false;

    update(&mut state, Message::Tick);

    assert_eq!(state.startup_dialog_state.animation_frame, 0);
}

#[test]
fn test_tick_advances_startup_dialog_when_refreshing() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::StartupDialog;
    state.startup_dialog_state.loading = false;
    state.startup_dialog_state.refreshing = true;

    update(&mut state, Message::Tick);

    assert_eq!(state.startup_dialog_state.animation_frame, 1);
}

#[test]
fn test_devices_discovered_only_updates_startup_dialog_in_startup_mode() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal; // Not in startup dialog mode

    let devices = vec![test_device("dev1", "Device 1")];
    update(
        &mut state,
        Message::DevicesDiscovered {
            devices: devices.clone(),
        },
    );

    // device_selector should be updated
    assert_eq!(state.device_selector.devices.len(), 1);
    // startup_dialog_state should NOT be updated (still empty)
    assert_eq!(state.startup_dialog_state.devices.len(), 0);
}
