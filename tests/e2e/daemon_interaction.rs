//! Daemon interaction integration tests
//!
//! Tests for daemon connection, disconnection, and device discovery.

use super::mock_daemon::{MockFlutterDaemon, MockScenarioBuilder};
use flutter_demon::core::DaemonEvent;
use flutter_demon::daemon::{DaemonCommand, DaemonMessage};

// ─────────────────────────────────────────────────────────
// Daemon Connection Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_daemon_connected_event_parsed_correctly() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Receive the daemon.connected event
    let event = handle.recv_event().await.expect("Should receive event");

    if let DaemonEvent::Stdout(line) = event {
        let inner = line.trim_start_matches('[').trim_end_matches(']');
        let msg = DaemonMessage::parse(inner);
        assert!(matches!(msg, Some(DaemonMessage::DaemonConnected(_))));

        if let Some(DaemonMessage::DaemonConnected(conn)) = msg {
            assert_eq!(conn.version, "0.6.1");
            assert_eq!(conn.pid, 99999);
        }
    } else {
        panic!("Expected Stdout event, got {:?}", event);
    }
}

#[tokio::test]
async fn test_daemon_shutdown_command() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    let daemon_task = tokio::spawn(daemon.run());

    // Skip connected event
    handle.recv_event().await;

    // Send shutdown command
    let shutdown_cmd = r#"[{"id":999,"method":"daemon.shutdown","params":{}}]"#;
    handle.send_command(shutdown_cmd).await.unwrap();

    // Daemon should respond and exit
    let response = handle.recv_event().await;
    assert!(response.is_some());

    // Task should complete
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), daemon_task).await;
    assert!(result.is_ok(), "Daemon should have shut down");
}

#[tokio::test]
async fn test_multiple_commands_in_sequence() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send device.enable
    let enable_cmd = r#"[{"id":1,"method":"device.enable","params":{}}]"#;
    handle.send_command(enable_cmd).await.unwrap();

    // Should get response
    let response1 = handle.recv_event().await;
    assert!(response1.is_some());

    // Send device.getDevices
    let devices_cmd = r#"[{"id":2,"method":"device.getDevices","params":{}}]"#;
    handle.send_command(devices_cmd).await.unwrap();

    // Should get device list response
    let response2 = handle.recv_event().await;
    assert!(response2.is_some());

    if let Some(DaemonEvent::Stdout(line)) = response2 {
        assert!(line.contains("emulator-5554"));
    }
}

// ─────────────────────────────────────────────────────────
// Device Discovery Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_device_list_response_format() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Request devices
    handle.send(DaemonCommand::GetDevices).await.unwrap();

    // Parse response
    let response = handle.recv_event().await.expect("Should get response");

    if let DaemonEvent::Stdout(line) = response {
        let inner = line.trim_start_matches('[').trim_end_matches(']');
        let parsed: serde_json::Value = serde_json::from_str(inner).unwrap();

        // Response should have result array
        let result = &parsed["result"];
        assert!(result.is_array());

        let devices = result.as_array().unwrap();
        assert!(!devices.is_empty());

        // First device should have expected fields
        let device = &devices[0];
        assert!(device["id"].is_string());
        assert!(device["name"].is_string());
        assert!(device["platform"].is_string());
    }
}

#[tokio::test]
async fn test_device_enable_before_get_devices() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Standard flow: enable then get
    handle.send(DaemonCommand::EnableDevices).await.unwrap();
    let _ = handle.recv_event().await; // Enable response

    handle.send(DaemonCommand::GetDevices).await.unwrap();
    let response = handle.recv_event().await;

    assert!(response.is_some());
}

// ─────────────────────────────────────────────────────────
// Error Handling Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_malformed_command_doesnt_crash_daemon() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    let daemon_task = tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send malformed JSON
    handle.send_command("not valid json").await.unwrap();
    handle.send_command("[{incomplete").await.unwrap();

    // Daemon should still be running - send valid command
    handle.send(DaemonCommand::GetDevices).await.unwrap();
    let response = handle.recv_event().await;
    assert!(response.is_some());

    // Verify daemon still running
    assert!(!daemon_task.is_finished());
}

#[tokio::test]
async fn test_unknown_method_handled_gracefully() {
    let (daemon, mut handle) = MockFlutterDaemon::new();
    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send unknown method
    let unknown_cmd = r#"[{"id":1,"method":"unknown.method","params":{}}]"#;
    handle.send_command(unknown_cmd).await.unwrap();

    // Should not crash - can continue with valid commands
    handle.send(DaemonCommand::GetDevices).await.unwrap();
    let response = handle.recv_event().await;
    assert!(response.is_some());
}

// ─────────────────────────────────────────────────────────
// Scenario Builder Tests
// ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_scenario_builder_with_app_started() {
    let (daemon, mut handle) = MockScenarioBuilder::new()
        .with_app_id("custom-app-123")
        .with_app_started()
        .build();

    tokio::spawn(daemon.run());

    // Should receive: daemon.connected, app.start, app.started
    let event1 = handle.recv_event().await;
    assert!(matches!(event1, Some(DaemonEvent::Stdout(s)) if s.contains("daemon.connected")));

    // The queued events should follow
    let event2 = handle.recv_event().await;
    assert!(matches!(event2, Some(DaemonEvent::Stdout(s)) if s.contains("app.start")));

    let event3 = handle.recv_event().await;
    assert!(matches!(event3, Some(DaemonEvent::Stdout(s)) if s.contains("app.started")));
}

#[tokio::test]
async fn test_scenario_builder_custom_response() {
    let custom_response = serde_json::json!({"status": "custom", "value": 42});

    let (daemon, mut handle) = MockScenarioBuilder::new()
        .with_response("custom.method", custom_response)
        .build();

    tokio::spawn(daemon.run());

    // Skip connected
    handle.recv_event().await;

    // Send custom method should return custom response
    let custom_cmd = r#"[{"id":1,"method":"custom.method","params":{}}]"#;
    handle.send_command(custom_cmd).await.unwrap();
    let response = handle.recv_event().await;

    if let Some(DaemonEvent::Stdout(line)) = response {
        assert!(line.contains("\"status\":\"custom\""));
        assert!(line.contains("\"value\":42"));
    } else {
        panic!("Expected custom response");
    }
}
