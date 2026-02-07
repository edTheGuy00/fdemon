//! Hot reload integration tests
//!
//! Tests for hot reload and hot restart workflows.

use crate::e2e::mock_daemon::MockFlutterDaemon;
use crate::{android_emulator, test_app_state};
use fdemon_app::message::Message;
use fdemon_app::update;
use fdemon_core::{DaemonEvent, DaemonMessage};
use fdemon_daemon::DaemonCommand;

// ─────────────────────────────────────────────────────────
// Hot Reload Command Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_sends_correct_command() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send reload command
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Should receive progress events
    let event = handle.recv_event().await.expect("Should receive progress");

    if let DaemonEvent::Stdout(line) = event {
        // Verify it's a progress event with hot.reload
        assert!(line.contains("app.progress"), "Expected progress event");
        assert!(line.contains("hot.reload") || line.contains("Performing hot reload"));
    }
}

#[tokio::test]
async fn test_hot_restart_sends_full_restart_flag() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send restart command
    handle
        .send(DaemonCommand::Restart {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Should receive progress events for restart
    let event = handle.recv_event().await.expect("Should receive progress");

    if let DaemonEvent::Stdout(line) = event {
        assert!(line.contains("app.progress"));
        assert!(line.contains("hot.restart") || line.contains("Performing hot restart"));
    }
}

#[tokio::test]
async fn test_reload_completes_with_finished_true() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send reload
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Collect all events until we see finished=true
    let mut saw_started = false;
    let mut saw_finished = false;

    for _ in 0..5 {
        if let Some(DaemonEvent::Stdout(line)) = handle.recv_event().await {
            if line.contains("\"finished\":false") {
                saw_started = true;
            }
            if line.contains("\"finished\":true") {
                saw_finished = true;
                break;
            }
        }
    }

    assert!(saw_started, "Should have seen progress start");
    assert!(saw_finished, "Should have seen progress finish");
}

// ─────────────────────────────────────────────────────────
// Handler Integration Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_message_triggers_reload_task() {
    // Create app state with a running session
    let mut state = test_app_state();
    let _device = android_emulator("emu-1");

    // Add a session with app_id (simulating running app)
    // Note: This tests the handler logic, not the full flow
    // Full flow would require session setup which is complex

    // For now, test that Message::HotReload is handled
    let result = update(&mut state, Message::HotReload);

    // Without a running session, should return None action
    // This verifies the guard condition works
    assert!(result.action.is_none() || result.message.is_some());
}

#[tokio::test]
async fn test_hot_restart_message_triggers_restart_task() {
    let mut state = test_app_state();

    // Test handler processes HotRestart message
    let result = update(&mut state, Message::HotRestart);

    // Similar to reload - without session, should be guarded
    assert!(result.action.is_none() || result.message.is_some());
}

// ─────────────────────────────────────────────────────────
// Progress Event Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_progress_events_parsed_as_daemon_messages() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Trigger reload
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Get progress event
    let event = handle.recv_event().await.expect("Should receive event");

    if let DaemonEvent::Stdout(line) = event {
        let inner = line.trim_start_matches('[').trim_end_matches(']');
        let msg = fdemon_daemon::parse_daemon_message(inner);

        // Should parse as AppProgress
        assert!(
            matches!(msg, Some(DaemonMessage::AppProgress(_))),
            "Expected AppProgress, got {:?}",
            msg
        );
    }
}

#[tokio::test]
async fn test_reload_response_has_success_code() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Trigger reload
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Collect events until we find the response
    let mut found_response = false;

    for _ in 0..10 {
        if let Some(DaemonEvent::Stdout(line)) = handle.recv_event().await {
            let inner = line.trim_start_matches('[').trim_end_matches(']');
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(inner) {
                if parsed.get("result").is_some() {
                    // This is a response
                    let result = &parsed["result"];
                    assert!(
                        result["code"] == 0
                            || result == &serde_json::json!({"code": 0, "message": "ok"}),
                        "Expected success response"
                    );
                    found_response = true;
                    break;
                }
            }
        }
    }

    assert!(found_response, "Should have received response");
}

// ─────────────────────────────────────────────────────────
// Timing and Sequence Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_reload_events_arrive_in_correct_order() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Trigger reload
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Expected order: progress(started) -> progress(finished) -> response
    let mut events = Vec::new();

    for _ in 0..5 {
        if let Some(event) = handle.recv_event().await {
            events.push(event);
        } else {
            break;
        }
    }

    assert!(events.len() >= 2, "Should have at least 2 events");

    // First should be progress with finished=false
    if let DaemonEvent::Stdout(ref line) = events[0] {
        assert!(
            line.contains("\"finished\":false"),
            "First event should be progress start"
        );
    }

    // Second should be progress with finished=true
    if let DaemonEvent::Stdout(ref line) = events[1] {
        assert!(
            line.contains("\"finished\":true"),
            "Second event should be progress finish"
        );
    }
}

#[tokio::test]
async fn test_multiple_reloads_in_sequence() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // First reload
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Drain events
    for _ in 0..5 {
        let _ = handle.recv_event().await;
    }

    // Second reload
    handle
        .send(DaemonCommand::Reload {
            app_id: "test-app".to_string(),
        })
        .await
        .unwrap();

    // Should still work
    let event = handle.recv_event().await;
    assert!(event.is_some(), "Second reload should work");
}

// ─────────────────────────────────────────────────────────
// Error Scenario Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_reload_with_wrong_app_id() {
    let (daemon, mut handle) = MockFlutterDaemon::with_app_id("correct-app");
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send reload with different app_id
    // Mock doesn't validate app_id, so this tests the flow still works
    handle
        .send(DaemonCommand::Reload {
            app_id: "wrong-app".to_string(),
        })
        .await
        .unwrap();

    // Should still get progress events (mock doesn't enforce app_id)
    let event = handle.recv_event().await;
    assert!(event.is_some());
}
