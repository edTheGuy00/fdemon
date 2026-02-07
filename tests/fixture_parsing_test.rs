//! Tests to verify JSON fixtures parse correctly

// DaemonMessage type is in core, but parse() method is in daemon/protocol
// Re-export from daemon makes this work
use flutter_demon::daemon::DaemonMessage;

#[test]
fn test_daemon_connected_fixture_parses() {
    let json = include_str!("fixtures/daemon_responses/daemon_connected.json");
    let msg = DaemonMessage::parse(json);
    assert!(matches!(msg, Some(DaemonMessage::DaemonConnected(_))));

    if let Some(DaemonMessage::DaemonConnected(conn)) = msg {
        assert_eq!(conn.version, "0.6.1");
        assert_eq!(conn.pid, 12345);
    }
}

#[test]
fn test_device_list_fixture_is_valid_json() {
    let json = include_str!("fixtures/daemon_responses/device_list.json");
    let devices: serde_json::Result<Vec<serde_json::Value>> = serde_json::from_str(json);
    assert!(devices.is_ok());

    let devices = devices.unwrap();
    assert_eq!(devices.len(), 2);

    // Verify first device (Android emulator)
    assert_eq!(devices[0]["id"], "emulator-5554");
    assert_eq!(devices[0]["platform"], "android");
    assert_eq!(devices[0]["emulator"], true);

    // Verify second device (iOS physical)
    assert_eq!(devices[1]["id"], "00008030-001A35E11234802E");
    assert_eq!(devices[1]["platform"], "ios");
    assert_eq!(devices[1]["emulator"], false);
}

#[test]
fn test_app_start_sequence_fixture_parses() {
    let json = include_str!("fixtures/daemon_responses/app_start_sequence.json");
    let events: serde_json::Result<Vec<serde_json::Value>> = serde_json::from_str(json);
    assert!(events.is_ok());

    let events = events.unwrap();
    assert_eq!(events.len(), 3);

    // Parse each event
    let event1_json = serde_json::to_string(&events[0]).unwrap();
    let msg1 = DaemonMessage::parse(&event1_json);
    assert!(matches!(msg1, Some(DaemonMessage::AppStart(_))));

    let event2_json = serde_json::to_string(&events[1]).unwrap();
    let msg2 = DaemonMessage::parse(&event2_json);
    assert!(matches!(msg2, Some(DaemonMessage::AppDebugPort(_))));

    let event3_json = serde_json::to_string(&events[2]).unwrap();
    let msg3 = DaemonMessage::parse(&event3_json);
    assert!(matches!(msg3, Some(DaemonMessage::AppStarted(_))));
}

#[test]
fn test_hot_reload_success_fixture_parses() {
    let json = include_str!("fixtures/daemon_responses/hot_reload_success.json");
    let events: serde_json::Result<Vec<serde_json::Value>> = serde_json::from_str(json);
    assert!(events.is_ok());

    let events = events.unwrap();
    assert_eq!(events.len(), 2);

    // Both should be AppProgress events
    for event in events {
        let event_json = serde_json::to_string(&event).unwrap();
        let msg = DaemonMessage::parse(&event_json);
        assert!(matches!(msg, Some(DaemonMessage::AppProgress(_))));
    }
}

#[test]
fn test_hot_reload_error_fixture_parses() {
    let json = include_str!("fixtures/daemon_responses/hot_reload_error.json");
    let events: serde_json::Result<Vec<serde_json::Value>> = serde_json::from_str(json);
    assert!(events.is_ok());

    let events = events.unwrap();
    assert_eq!(events.len(), 2);

    // First is AppProgress, second is AppLog
    let event1_json = serde_json::to_string(&events[0]).unwrap();
    let msg1 = DaemonMessage::parse(&event1_json);
    assert!(matches!(msg1, Some(DaemonMessage::AppProgress(_))));

    let event2_json = serde_json::to_string(&events[1]).unwrap();
    let msg2 = DaemonMessage::parse(&event2_json);
    assert!(matches!(msg2, Some(DaemonMessage::AppLog(_))));

    // Verify it's an error log
    if let Some(msg) = msg2 {
        assert!(msg.is_error());
    }
}

#[test]
fn test_app_stop_fixture_parses() {
    let json = include_str!("fixtures/daemon_responses/app_stop.json");
    let msg = DaemonMessage::parse(json);
    assert!(matches!(msg, Some(DaemonMessage::AppStop(_))));

    if let Some(DaemonMessage::AppStop(stop)) = msg {
        assert_eq!(stop.app_id, "test-app-id");
        assert!(stop.error.is_none());
    }
}

#[test]
fn test_all_fixtures_are_valid_json() {
    // This test ensures all fixtures can at least be parsed as JSON
    let fixtures = [
        include_str!("fixtures/daemon_responses/daemon_connected.json"),
        include_str!("fixtures/daemon_responses/device_list.json"),
        include_str!("fixtures/daemon_responses/app_start_sequence.json"),
        include_str!("fixtures/daemon_responses/hot_reload_success.json"),
        include_str!("fixtures/daemon_responses/hot_reload_error.json"),
        include_str!("fixtures/daemon_responses/app_stop.json"),
    ];

    for (idx, fixture) in fixtures.iter().enumerate() {
        let result: serde_json::Result<serde_json::Value> = serde_json::from_str(fixture);
        assert!(result.is_ok(), "Fixture {} failed to parse as JSON", idx);
    }
}
