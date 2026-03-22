//! Tests for DAP progress events during hot reload and hot restart.
//!
//! Verifies that `progressStart` / `progressEnd` events are emitted when the
//! client advertises `supportsProgressReporting`, and that the custom
//! `dart.hotReloadComplete` / `dart.hotRestartComplete` events are always
//! emitted on success.

use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::{DapMessage, DapRequest};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a hot-reload or hot-restart request.
fn hot_request(command: &str) -> DapRequest {
    DapRequest {
        seq: 1,
        command: command.into(),
        arguments: Some(serde_json::json!({})),
    }
}

/// Drain all pending events from the channel and return their event names.
fn drain_event_names(rx: &mut tokio::sync::mpsc::Receiver<DapMessage>) -> Vec<String> {
    let mut names = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let DapMessage::Event(ev) = msg {
            names.push(ev.event);
        }
    }
    names
}

/// Drain all pending events from the channel and return them as `(event_name, body)` pairs.
fn drain_events_with_body(
    rx: &mut tokio::sync::mpsc::Receiver<DapMessage>,
) -> Vec<(String, Option<serde_json::Value>)> {
    let mut events = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let DapMessage::Event(ev) = msg {
            events.push((ev.event, ev.body));
        }
    }
    events
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: progressStart + progressEnd emitted during hot reload when supported
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_emits_progress_events_when_supported() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotReload");
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "hotReload should succeed");

    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"progressStart".to_string()),
        "Expected progressStart event, got: {:?}",
        names
    );
    assert!(
        names.contains(&"progressEnd".to_string()),
        "Expected progressEnd event, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: no progress events when client does not support them
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_no_progress_when_unsupported() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    // client_supports_progress defaults to false

    let req = hot_request("hotReload");
    let _resp = adapter.handle_request(&req).await;

    let names = drain_event_names(&mut rx);
    assert!(
        !names.contains(&"progressStart".to_string()),
        "Should not emit progressStart when client does not support progress, got: {:?}",
        names
    );
    assert!(
        !names.contains(&"progressEnd".to_string()),
        "Should not emit progressEnd when client does not support progress, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: dart.hotReloadComplete emitted on success
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_emits_completion_event_on_success() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());

    let req = hot_request("hotReload");
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"dart.hotReloadComplete".to_string()),
        "Expected dart.hotReloadComplete event, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4: dart.hotReloadComplete NOT emitted on failure
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_no_completion_event_on_failure() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::failing());

    let req = hot_request("hotReload");
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success);
    let names = drain_event_names(&mut rx);
    assert!(
        !names.contains(&"dart.hotReloadComplete".to_string()),
        "Should not emit dart.hotReloadComplete on failure, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5: progressStart + progressEnd emitted during hot restart when supported
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_restart_emits_progress_events_when_supported() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotRestart");
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "hotRestart should succeed");

    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"progressStart".to_string()),
        "Expected progressStart event for hotRestart, got: {:?}",
        names
    );
    assert!(
        names.contains(&"progressEnd".to_string()),
        "Expected progressEnd event for hotRestart, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 6: dart.hotRestartComplete emitted on success
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_restart_emits_completion_event_on_success() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());

    let req = hot_request("hotRestart");
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"dart.hotRestartComplete".to_string()),
        "Expected dart.hotRestartComplete event, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 7: progressEnd emitted even when hot reload fails (DAP spec requirement)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_progress_end_emitted_on_failure() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::failing());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotReload");
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "hotReload should fail with failing backend");

    let names = drain_event_names(&mut rx);
    // progressStart must be emitted before the backend call.
    assert!(
        names.contains(&"progressStart".to_string()),
        "Expected progressStart even on failure, got: {:?}",
        names
    );
    // progressEnd must be emitted even when the reload fails.
    assert!(
        names.contains(&"progressEnd".to_string()),
        "Expected progressEnd even on failure (IDE must see progress closed), got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 8: progressEnd emitted even when hot restart fails
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_restart_progress_end_emitted_on_failure() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::failing());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotRestart");
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "hotRestart should fail with failing backend");

    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"progressStart".to_string()),
        "Expected progressStart even on failure, got: {:?}",
        names
    );
    assert!(
        names.contains(&"progressEnd".to_string()),
        "Expected progressEnd even on failure, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 9: progress IDs are unique per operation
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_progress_ids_are_unique_per_operation() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    // First hot reload
    let req1 = hot_request("hotReload");
    adapter.handle_request(&req1).await;
    let events1 = drain_events_with_body(&mut rx);

    // Second hot reload
    let req2 = hot_request("hotReload");
    adapter.handle_request(&req2).await;
    let events2 = drain_events_with_body(&mut rx);

    // Extract progressId from each progressStart event.
    fn progress_id_from_start(events: &[(String, Option<serde_json::Value>)]) -> Option<String> {
        events
            .iter()
            .find(|(name, _)| name == "progressStart")
            .and_then(|(_, body)| body.as_ref())
            .and_then(|b| b.get("progressId"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    let id1 = progress_id_from_start(&events1).expect("first hotReload should have progressStart");
    let id2 = progress_id_from_start(&events2).expect("second hotReload should have progressStart");

    assert_ne!(
        id1, id2,
        "Each hot reload operation must have a unique progress ID, got id1={id1}, id2={id2}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 10: progressStart title is "Hot Reload" for hotReload
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_reload_progress_start_has_correct_title() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotReload");
    adapter.handle_request(&req).await;
    let events = drain_events_with_body(&mut rx);

    let start_body = events
        .iter()
        .find(|(name, _)| name == "progressStart")
        .and_then(|(_, body)| body.as_ref())
        .expect("progressStart event should have a body");

    let title = start_body
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        title, "Hot Reload",
        "hotReload progressStart title should be 'Hot Reload'"
    );

    let cancellable = start_body
        .get("cancellable")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    assert!(
        !cancellable,
        "hotReload progressStart should have cancellable: false"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 11: progressStart title is "Hot Restart" for hotRestart
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_restart_progress_start_has_correct_title() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotRestart");
    adapter.handle_request(&req).await;
    let events = drain_events_with_body(&mut rx);

    let start_body = events
        .iter()
        .find(|(name, _)| name == "progressStart")
        .and_then(|(_, body)| body.as_ref())
        .expect("progressStart event should have a body");

    let title = start_body
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        title, "Hot Restart",
        "hotRestart progressStart title should be 'Hot Restart'"
    );

    let cancellable = start_body
        .get("cancellable")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    assert!(
        !cancellable,
        "hotRestart progressStart should have cancellable: false"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 12: progressEnd carries the matching progressId
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_progress_end_carries_matching_progress_id() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("hotReload");
    adapter.handle_request(&req).await;
    let events = drain_events_with_body(&mut rx);

    let start_id = events
        .iter()
        .find(|(name, _)| name == "progressStart")
        .and_then(|(_, body)| body.as_ref())
        .and_then(|b| b.get("progressId"))
        .and_then(|v| v.as_str())
        .expect("progressStart should contain progressId")
        .to_string();

    let end_id = events
        .iter()
        .find(|(name, _)| name == "progressEnd")
        .and_then(|(_, body)| body.as_ref())
        .and_then(|b| b.get("progressId"))
        .and_then(|v| v.as_str())
        .expect("progressEnd should contain progressId")
        .to_string();

    assert_eq!(
        start_id, end_id,
        "progressEnd must carry the same progressId as progressStart"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 13: hot restart does not emit dart.hotRestartComplete on failure
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_hot_restart_no_completion_event_on_failure() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::failing());

    let req = hot_request("hotRestart");
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success);
    let names = drain_event_names(&mut rx);
    assert!(
        !names.contains(&"dart.hotRestartComplete".to_string()),
        "Should not emit dart.hotRestartComplete on failure, got: {:?}",
        names
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for standard DAP `restart` request (Task 03: hot-operation refactor)
//
// After the refactor, `restart` delegates to `execute_hot_operation` and must
// behave identically to `hotRestart`: it emits progress events and the
// `dart.hotRestartComplete` custom event.
// ─────────────────────────────────────────────────────────────────────────────

/// Standard DAP `restart` emits `progressStart` and `progressEnd` events
/// when the client advertises `supportsProgressReporting`.
#[tokio::test]
async fn test_restart_emits_progress_events() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("restart");
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "restart should succeed, got: {:?}",
        resp.message
    );

    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"progressStart".to_string()),
        "restart should emit progressStart, got: {:?}",
        names
    );
    assert!(
        names.contains(&"progressEnd".to_string()),
        "restart should emit progressEnd, got: {:?}",
        names
    );
}

/// Standard DAP `restart` emits `dart.hotRestartComplete` on success.
#[tokio::test]
async fn test_restart_emits_hot_restart_complete_event() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());

    let req = hot_request("restart");
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "restart should succeed, got: {:?}",
        resp.message
    );

    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"dart.hotRestartComplete".to_string()),
        "restart should emit dart.hotRestartComplete on success, got: {:?}",
        names
    );
}

/// Standard DAP `restart` emits `progressEnd` even when the backend fails,
/// so the IDE spinner always closes.
#[tokio::test]
async fn test_restart_error_still_emits_progress_end() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::failing());
    adapter.set_client_supports_progress(true);

    let req = hot_request("restart");
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "restart should fail with failing backend");

    let names = drain_event_names(&mut rx);
    assert!(
        names.contains(&"progressStart".to_string()),
        "restart should emit progressStart even on failure, got: {:?}",
        names
    );
    assert!(
        names.contains(&"progressEnd".to_string()),
        "restart should emit progressEnd even on failure, got: {:?}",
        names
    );
}

/// Standard DAP `restart` does NOT emit `dart.hotRestartComplete` on failure.
#[tokio::test]
async fn test_restart_no_completion_event_on_failure() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::failing());

    let req = hot_request("restart");
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success);
    let names = drain_event_names(&mut rx);
    assert!(
        !names.contains(&"dart.hotRestartComplete".to_string()),
        "restart should not emit dart.hotRestartComplete on failure, got: {:?}",
        names
    );
}

/// Standard DAP `restart` uses the `"Hot Restart"` title in `progressStart`.
#[tokio::test]
async fn test_restart_progress_start_has_hot_restart_title() {
    let (mut adapter, mut rx) = DapAdapter::new(HotOpMockBackend::ok());
    adapter.set_client_supports_progress(true);

    let req = hot_request("restart");
    adapter.handle_request(&req).await;
    let events = drain_events_with_body(&mut rx);

    let start_body = events
        .iter()
        .find(|(name, _)| name == "progressStart")
        .and_then(|(_, body)| body.as_ref())
        .expect("restart should emit a progressStart with body");

    let title = start_body
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        title, "Hot Restart",
        "restart progressStart title should be 'Hot Restart'"
    );
}
