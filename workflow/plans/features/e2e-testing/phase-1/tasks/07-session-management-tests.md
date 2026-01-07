## Task: Implement Session Management Integration Tests

**Objective**: Write integration tests for session lifecycle including creation, starting, stopping, and multi-session scenarios.

**Depends on**: 04-mock-daemon

### Scope

- `tests/e2e/session_management.rs` - **NEW** Session lifecycle tests

### Details

Create tests that verify session management flows including:
- Session creation and initialization
- App start/stop lifecycle
- Multi-session coordination
- Session switching

**File: `tests/e2e/session_management.rs`**:

```rust
//! Session management integration tests
//!
//! Tests for session lifecycle, multi-session handling, and session state transitions.

use super::*;
use super::mock_daemon::{MockFlutterDaemon, MockScenarioBuilder};
use flutter_demon::core::DaemonEvent;
use flutter_demon::daemon::{DaemonMessage, DaemonCommand};
use flutter_demon::app::handler::update;
use flutter_demon::app::message::Message;
use flutter_demon::app::state::{AppState, UiMode};
use flutter_demon::app::session::{Session, SessionManager};
use flutter_demon::core::AppPhase;

// ─────────────────────────────────────────────────────────
// Session Creation Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_session_manager_starts_empty() {
    let manager = SessionManager::new();
    assert_eq!(manager.count(), 0);
    assert!(manager.selected().is_none());
}

#[test]
fn test_session_manager_max_capacity() {
    let mut manager = SessionManager::new();

    // Should allow up to 9 sessions
    for i in 0..9 {
        let device = test_device(&format!("device-{}", i), &format!("Device {}", i), "android");
        let result = manager.create_session(device.clone());
        assert!(result.is_ok(), "Should create session {}", i);
    }

    assert_eq!(manager.count(), 9);

    // 10th session should fail
    let device = test_device("device-9", "Device 9", "android");
    let result = manager.create_session(device);
    assert!(result.is_err(), "Should not allow 10th session");
}

#[test]
fn test_session_creation_sets_device_info() {
    let mut manager = SessionManager::new();
    let device = test_device("test-device", "Test Device", "ios");

    let session_id = manager.create_session(device).unwrap();
    let session = manager.get(session_id).unwrap();

    assert_eq!(session.session.device_id, "test-device");
    assert_eq!(session.session.device_name, "Test Device");
    assert_eq!(session.session.platform, "ios");
}

// ─────────────────────────────────────────────────────────
// Session Lifecycle Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_app_start_events_update_session() {
    let (daemon, mut handle) = MockScenarioBuilder::new()
        .with_app_id("lifecycle-test-app")
        .with_app_started()
        .build();

    tokio::spawn(daemon.run());

    // Receive events
    let connected = handle.recv_event().await;
    assert!(connected.is_some());

    let app_start = handle.recv_event().await;
    assert!(matches!(app_start, Some(DaemonEvent::Stdout(s)) if s.contains("app.start")));

    let app_started = handle.recv_event().await;
    assert!(matches!(app_started, Some(DaemonEvent::Stdout(s)) if s.contains("app.started")));
}

#[tokio::test]
async fn test_app_stop_event_handled() {
    let (daemon, mut handle) = MockScenarioBuilder::new()
        .with_app_id("stop-test-app")
        .build();

    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send stop command
    handle.send(DaemonCommand::Stop {
        app_id: "stop-test-app".to_string()
    }).await.unwrap();

    // Should receive app.stop event
    let event = handle.recv_event().await;
    if let Some(DaemonEvent::Stdout(line)) = event {
        assert!(line.contains("app.stop"), "Should receive stop event");
    }
}

// ─────────────────────────────────────────────────────────
// Session State Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_session_phase_transitions() {
    let device = test_device("test", "Test", "android");
    let mut session = Session::new(device, None);

    // Initial state
    assert!(matches!(session.phase, AppPhase::Starting | AppPhase::Idle));

    // Mark as started
    session.mark_started("app-123".to_string());
    assert_eq!(session.app_id, Some("app-123".to_string()));
    assert!(matches!(session.phase, AppPhase::Running));

    // Start reload
    session.start_reload();
    assert!(matches!(session.phase, AppPhase::Reloading));

    // Complete reload
    session.complete_reload(100);
    assert!(matches!(session.phase, AppPhase::Running));
    assert_eq!(session.reload_count, 1);
}

#[test]
fn test_session_tracks_reload_count() {
    let device = test_device("test", "Test", "android");
    let mut session = Session::new(device, None);
    session.mark_started("app".to_string());

    assert_eq!(session.reload_count, 0);

    // Perform multiple reloads
    for i in 1..=5 {
        session.start_reload();
        session.complete_reload(50);
        assert_eq!(session.reload_count, i);
    }
}

#[test]
fn test_session_logs_accumulate() {
    let device = test_device("test", "Test", "android");
    let mut session = Session::new(device, None);

    use flutter_demon::core::{LogEntry, LogLevel, LogSource};

    session.add_log(LogEntry::new(LogLevel::Info, LogSource::Flutter, "Log 1"));
    session.add_log(LogEntry::new(LogLevel::Info, LogSource::Flutter, "Log 2"));
    session.add_log(LogEntry::new(LogLevel::Error, LogSource::Flutter, "Error 1"));

    assert_eq!(session.logs.len(), 3);
    assert_eq!(session.error_count(), 1);
}

// ─────────────────────────────────────────────────────────
// Multi-Session Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_session_selection_by_index() {
    let mut manager = SessionManager::new();

    // Create 3 sessions
    for i in 0..3 {
        let device = test_device(&format!("d{}", i), &format!("Device {}", i), "android");
        manager.create_session(device).unwrap();
    }

    // Initially first is selected
    assert_eq!(manager.selected_index(), 0);

    // Select by index
    manager.select_by_index(1);
    assert_eq!(manager.selected_index(), 1);

    manager.select_by_index(2);
    assert_eq!(manager.selected_index(), 2);

    // Out of bounds should be clamped
    manager.select_by_index(99);
    assert_eq!(manager.selected_index(), 2);
}

#[test]
fn test_session_next_previous_navigation() {
    let mut manager = SessionManager::new();

    // Create 3 sessions
    for i in 0..3 {
        let device = test_device(&format!("d{}", i), &format!("Device {}", i), "android");
        manager.create_session(device).unwrap();
    }

    // Start at 0
    assert_eq!(manager.selected_index(), 0);

    // Next
    manager.select_next();
    assert_eq!(manager.selected_index(), 1);

    manager.select_next();
    assert_eq!(manager.selected_index(), 2);

    // Next at end wraps to 0
    manager.select_next();
    assert_eq!(manager.selected_index(), 0);

    // Previous at 0 wraps to end
    manager.select_previous();
    assert_eq!(manager.selected_index(), 2);
}

#[test]
fn test_session_removal() {
    let mut manager = SessionManager::new();

    // Create 2 sessions
    let device1 = test_device("d1", "Device 1", "android");
    let device2 = test_device("d2", "Device 2", "android");

    let id1 = manager.create_session(device1).unwrap();
    let id2 = manager.create_session(device2).unwrap();

    assert_eq!(manager.count(), 2);

    // Remove first session
    let removed = manager.remove(id1);
    assert!(removed.is_some());
    assert_eq!(manager.count(), 1);

    // Remaining session should still be accessible
    assert!(manager.get(id2).is_some());
}

#[test]
fn test_removing_selected_session_selects_another() {
    let mut manager = SessionManager::new();

    // Create 3 sessions
    let mut ids = Vec::new();
    for i in 0..3 {
        let device = test_device(&format!("d{}", i), &format!("Device {}", i), "android");
        ids.push(manager.create_session(device).unwrap());
    }

    // Select middle session
    manager.select_by_index(1);
    assert_eq!(manager.selected_index(), 1);

    // Remove selected
    manager.remove(ids[1]);

    // Should have selected another (not panic)
    assert!(manager.selected().is_some() || manager.count() == 0);
}

// ─────────────────────────────────────────────────────────
// Handler Integration Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_number_key_selects_session() {
    let mut state = test_app_state();

    // Create 3 sessions in state
    for i in 0..3 {
        let device = test_device(&format!("d{}", i), &format!("Device {}", i), "android");
        state.session_manager.create_session(device).ok();
    }

    // Press '2' to select second session (index 1)
    let result = update(&mut state, Message::SelectSessionByIndex(1));
    assert_eq!(state.session_manager.selected_index(), 1);

    // Press '3' to select third session (index 2)
    let result = update(&mut state, Message::SelectSessionByIndex(2));
    assert_eq!(state.session_manager.selected_index(), 2);
}

#[test]
fn test_tab_key_cycles_sessions() {
    let mut state = test_app_state();

    // Create 2 sessions
    for i in 0..2 {
        let device = test_device(&format!("d{}", i), &format!("Device {}", i), "android");
        state.session_manager.create_session(device).ok();
    }

    // Tab should go to next
    let _ = update(&mut state, Message::NextSession);
    assert_eq!(state.session_manager.selected_index(), 1);

    // Tab again wraps
    let _ = update(&mut state, Message::NextSession);
    assert_eq!(state.session_manager.selected_index(), 0);
}

#[test]
fn test_close_session_message() {
    let mut state = test_app_state();

    // Create 2 sessions
    for i in 0..2 {
        let device = test_device(&format!("d{}", i), &format!("Device {}", i), "android");
        state.session_manager.create_session(device).ok();
    }

    assert_eq!(state.session_manager.count(), 2);

    // Close current session
    let _ = update(&mut state, Message::CloseCurrentSession);

    // Should have 1 session remaining
    assert_eq!(state.session_manager.count(), 1);
}

// ─────────────────────────────────────────────────────────
// Edge Case Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_operations_on_empty_session_manager() {
    let mut manager = SessionManager::new();

    // These should not panic
    assert!(manager.selected().is_none());
    manager.select_next();
    manager.select_previous();
    manager.select_by_index(5);

    // Remove non-existent should return None
    let fake_id = flutter_demon::app::session::SessionId::new();
    assert!(manager.remove(fake_id).is_none());
}

#[test]
fn test_session_with_same_device_allowed() {
    let mut manager = SessionManager::new();
    let device = test_device("same-device", "Same Device", "android");

    // Should allow multiple sessions with same device
    let id1 = manager.create_session(device.clone());
    let id2 = manager.create_session(device.clone());

    assert!(id1.is_ok());
    assert!(id2.is_ok());
    assert_ne!(id1.unwrap(), id2.unwrap());
}
```

### Acceptance Criteria

1. All tests compile and pass
2. Tests cover:
   - Session manager initialization (1 test)
   - Max capacity enforcement (1 test)
   - Session device info (1 test)
   - App start event handling (1 test)
   - App stop event handling (1 test)
   - Session phase transitions (1 test)
   - Reload count tracking (1 test)
   - Log accumulation (1 test)
   - Session selection by index (1 test)
   - Next/previous navigation (1 test)
   - Session removal (1 test)
   - Remove selected behavior (1 test)
   - Handler message processing (3 tests)
   - Edge cases (2 tests)
3. Tests run in <3 seconds total
4. Tests are deterministic

### Testing

```bash
# Run session management tests
cargo test --test e2e session_management

# Run with output
cargo test --test e2e session_management -- --nocapture
```

### Notes

- Session tests are mostly synchronous (not async)
- Tests `SessionManager` and `Session` directly
- Handler integration tests verify message processing
- Multi-session tests cover the 1-9 keybinding flow
- Edge cases ensure no panics on unusual operations
- Some tests use mock daemon for lifecycle events
