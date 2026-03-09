//! Tests for custom DAP events: `dart.debuggerUris`, `flutter.appStart`,
//! and `flutter.appStarted`.

use super::drain_events;
use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;

#[tokio::test]
async fn test_attach_no_ws_uri_skips_debugger_uris_event() {
    // MockBackend returns None for ws_uri, so dart.debuggerUris should NOT be emitted.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    let req = super::make_request(1, "attach");
    adapter.handle_request(&req).await;

    let events = drain_events(&mut rx);
    let has_debugger_uris = events
        .iter()
        .any(|m| matches!(m, DapMessage::Event(e) if e.event == "dart.debuggerUris"));
    assert!(
        !has_debugger_uris,
        "dart.debuggerUris should not be emitted when ws_uri is None"
    );
}

#[tokio::test]
async fn test_attach_with_ws_uri_emits_debugger_uris_event() {
    // MockBackendWithUri returns a known URI for ws_uri.
    let backend = MockBackendWithUri::new("ws://127.0.0.1:12345/ws", "emulator-5554", "debug");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let req = super::make_request(1, "attach");
    adapter.handle_request(&req).await;

    let events = drain_events(&mut rx);
    let debugger_uris_event = events.iter().find_map(|m| {
        if let DapMessage::Event(e) = m {
            if e.event == "dart.debuggerUris" {
                return Some(e);
            }
        }
        None
    });

    let ev = debugger_uris_event.expect("dart.debuggerUris event must be emitted");
    let body = ev
        .body
        .as_ref()
        .expect("dart.debuggerUris event must have a body");
    assert_eq!(
        body["vmServiceUri"].as_str(),
        Some("ws://127.0.0.1:12345/ws"),
        "vmServiceUri must match the backend's ws_uri"
    );
}

#[tokio::test]
async fn test_attach_emits_flutter_app_start_event() {
    let backend = MockBackendWithUri::new("ws://127.0.0.1:8181/ws", "emulator-5554", "debug");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let req = super::make_request(1, "attach");
    adapter.handle_request(&req).await;

    let events = drain_events(&mut rx);
    let app_start_event = events.iter().find_map(|m| {
        if let DapMessage::Event(e) = m {
            if e.event == "flutter.appStart" {
                return Some(e);
            }
        }
        None
    });

    let ev = app_start_event.expect("flutter.appStart event must be emitted");
    let body = ev
        .body
        .as_ref()
        .expect("flutter.appStart event must have a body");
    assert_eq!(
        body["deviceId"].as_str(),
        Some("emulator-5554"),
        "deviceId must match the backend's device_id"
    );
    assert_eq!(
        body["mode"].as_str(),
        Some("debug"),
        "mode must match the backend's build_mode"
    );
    assert_eq!(
        body["supportsRestart"].as_bool(),
        Some(true),
        "supportsRestart must be true for debug mode"
    );
}

#[tokio::test]
async fn test_attach_profile_mode_supports_restart_false() {
    // Profile/release builds should not support hot restart.
    let backend = MockBackendWithUri::new("ws://127.0.0.1:8181/ws", "emulator-5554", "profile");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let req = super::make_request(1, "attach");
    adapter.handle_request(&req).await;

    let events = drain_events(&mut rx);
    let app_start_event = events.iter().find_map(|m| {
        if let DapMessage::Event(e) = m {
            if e.event == "flutter.appStart" {
                return Some(e);
            }
        }
        None
    });

    let ev = app_start_event.expect("flutter.appStart event must be emitted");
    let body = ev.body.as_ref().unwrap();
    assert_eq!(
        body["supportsRestart"].as_bool(),
        Some(false),
        "supportsRestart must be false for profile mode"
    );
}

#[tokio::test]
async fn test_app_started_event_emitted_on_debug_event() {
    // When DebugEvent::AppStarted is received, flutter.appStarted must be emitted.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter.handle_debug_event(DebugEvent::AppStarted).await;

    let events = drain_events(&mut rx);
    let app_started_event = events.iter().find_map(|m| {
        if let DapMessage::Event(e) = m {
            if e.event == "flutter.appStarted" {
                return Some(e);
            }
        }
        None
    });

    assert!(
        app_started_event.is_some(),
        "flutter.appStarted event must be emitted for DebugEvent::AppStarted"
    );
}

#[tokio::test]
async fn test_app_started_event_body_is_empty_object() {
    // flutter.appStarted body should be an empty JSON object per Flutter DAP convention.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter.handle_debug_event(DebugEvent::AppStarted).await;

    let events = drain_events(&mut rx);
    let ev = events.iter().find_map(|m| {
        if let DapMessage::Event(e) = m {
            if e.event == "flutter.appStarted" {
                return Some(e);
            }
        }
        None
    });

    let ev = ev.expect("flutter.appStarted event must be emitted");
    let body = ev
        .body
        .as_ref()
        .expect("flutter.appStarted must have a body");
    assert!(
        body.as_object().is_some_and(|o| o.is_empty()),
        "flutter.appStarted body must be an empty JSON object, got: {:?}",
        body
    );
}

#[tokio::test]
async fn test_attach_emits_app_start_before_response_events() {
    // flutter.appStart must be emitted during handle_attach (after successful get_vm).
    // Verify ordering: thread events (if any) come first, then custom events.
    let backend = MockBackendWithUri::new("ws://127.0.0.1:8181/ws", "pixel-4a", "debug");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let req = super::make_request(1, "attach");
    let resp = adapter.handle_request(&req).await;

    // Attach must succeed.
    assert!(
        resp.success,
        "attach should succeed, got: {:?}",
        resp.message
    );

    // Both flutter.appStart and dart.debuggerUris must be in the event stream.
    let events = drain_events(&mut rx);
    let event_names: Vec<&str> = events
        .iter()
        .filter_map(|m| {
            if let DapMessage::Event(e) = m {
                Some(e.event.as_str())
            } else {
                None
            }
        })
        .collect();

    assert!(
        event_names.contains(&"dart.debuggerUris"),
        "dart.debuggerUris must be emitted, events: {:?}",
        event_names
    );
    assert!(
        event_names.contains(&"flutter.appStart"),
        "flutter.appStart must be emitted, events: {:?}",
        event_names
    );
}
