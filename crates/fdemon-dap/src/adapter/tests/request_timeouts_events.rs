//! Tests for Task 17: request timeouts, `restart` handler, `dart.serviceExtensionAdded`
//! event, and variable store memory cap.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use super::{drain_events, make_request};
use crate::adapter::stack::{VariableRef, VariableStore, MAX_VARIABLE_REFS};
use crate::adapter::test_helpers::*;
use crate::adapter::types::DebugEvent;
use crate::adapter::*;
use crate::protocol::types::Capabilities;
use crate::DapMessage;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Backend that records whether `hot_restart` was called.
struct HotRestartRecorder {
    pub called: Arc<AtomicBool>,
}

impl HotRestartRecorder {
    fn new() -> (Self, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(false));
        (
            Self {
                called: flag.clone(),
            },
            flag,
        )
    }
}

impl MockTestBackend for HotRestartRecorder {
    async fn hot_restart(&self) -> Result<(), BackendError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Backend that always fails `hot_restart`.
struct FailingRestartBackend;

impl MockTestBackend for FailingRestartBackend {
    async fn hot_restart(&self) -> Result<(), BackendError> {
        Err(BackendError::NotConnected)
    }
}

/// Backend that sleeps longer than `REQUEST_TIMEOUT` in `get_vm`.
///
/// Used to test that the timeout fires and returns an error rather than hanging.
/// The timeout is very short (1 ms) in tests because we use `tokio::time::pause`.
struct HangingGetVmBackend;

impl MockTestBackend for HangingGetVmBackend {
    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        // Simulate a hung call by sleeping for a very long time.
        tokio::time::sleep(Duration::from_secs(3600)).await;
        Ok(serde_json::json!({}))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: variable store memory cap
// ─────────────────────────────────────────────────────────────────────────────

/// Allocating up to the limit works normally; the next allocation returns 0.
#[test]
fn test_variable_store_cap_at_limit() {
    let mut store = VariableStore::new();

    // Fill the store to exactly the cap.
    for _ in 0..MAX_VARIABLE_REFS {
        let r = store.allocate(VariableRef::Object {
            isolate_id: "iso".into(),
            object_id: "obj".into(),
        });
        assert!(r > 0, "allocation below cap should succeed");
    }

    assert_eq!(store.len(), MAX_VARIABLE_REFS);

    // The next allocation must return 0 (non-expandable).
    let overflow = store.allocate(VariableRef::Object {
        isolate_id: "iso".into(),
        object_id: "overflow".into(),
    });
    assert_eq!(
        overflow, 0,
        "allocation at cap+1 should return 0 (non-expandable)"
    );
}

/// The store is unchanged after a cap-overflow allocation.
#[test]
fn test_variable_store_cap_does_not_insert() {
    let mut store = VariableStore::new();

    for _ in 0..MAX_VARIABLE_REFS {
        store.allocate(VariableRef::Object {
            isolate_id: "iso".into(),
            object_id: "obj".into(),
        });
    }

    // Attempt overflow allocation.
    store.allocate(VariableRef::Object {
        isolate_id: "iso".into(),
        object_id: "overflow".into(),
    });

    // Store length should still be MAX_VARIABLE_REFS, not MAX + 1.
    assert_eq!(
        store.len(),
        MAX_VARIABLE_REFS,
        "store length must not exceed MAX_VARIABLE_REFS after cap overflow"
    );
}

/// After `reset()`, the store can allocate fresh references from 1 again.
#[test]
fn test_variable_store_reset_clears_cap() {
    let mut store = VariableStore::new();

    // Fill to cap.
    for _ in 0..MAX_VARIABLE_REFS {
        store.allocate(VariableRef::Object {
            isolate_id: "iso".into(),
            object_id: "obj".into(),
        });
    }
    assert_eq!(
        store.allocate(VariableRef::Object {
            isolate_id: "iso".into(),
            object_id: "x".into()
        }),
        0,
        "must return 0 when full"
    );

    // Reset and verify a fresh allocation succeeds.
    store.reset();
    let r = store.allocate(VariableRef::Object {
        isolate_id: "iso".into(),
        object_id: "fresh".into(),
    });
    assert!(r > 0, "allocation after reset should succeed");
}

/// Allocating a small number of entries well below the cap works normally.
#[test]
fn test_variable_store_normal_allocation_below_cap() {
    let mut store = VariableStore::new();

    let r1 = store.allocate(VariableRef::Object {
        isolate_id: "iso".into(),
        object_id: "a".into(),
    });
    let r2 = store.allocate(VariableRef::Object {
        isolate_id: "iso".into(),
        object_id: "b".into(),
    });

    assert_eq!(r1, 1);
    assert_eq!(r2, 2);
    assert_eq!(store.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: `restart` handler
// ─────────────────────────────────────────────────────────────────────────────

/// `restart` request calls `backend.hot_restart` and returns a success response.
#[tokio::test]
async fn test_restart_calls_hot_restart() {
    let (backend, called_flag) = HotRestartRecorder::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    let req = make_request(1, "restart");

    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "restart should succeed, got: {:?}",
        resp.message
    );
    assert!(
        called_flag.load(Ordering::SeqCst),
        "hot_restart should have been called"
    );
}

/// `restart` returns no body on success (consistent with `hotRestart`).
///
/// After the hot-operation refactor, `restart` delegates to
/// `execute_hot_operation` which returns `None` body on success — matching
/// the `hotRestart` handler. The DAP spec allows either `{}` or omitting the
/// body for success responses.
#[tokio::test]
async fn test_restart_response_body_is_none() {
    let (backend, _flag) = HotRestartRecorder::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    let req = make_request(1, "restart");

    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    assert!(
        resp.body.is_none(),
        "restart success response should have no body (consistent with hotRestart), got: {:?}",
        resp.body
    );
}

/// `restart` returns an error response when the backend fails.
#[tokio::test]
async fn test_restart_error_when_backend_fails() {
    let (mut adapter, _rx) = DapAdapter::new(FailingRestartBackend);
    let req = make_request(1, "restart");

    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "restart should fail when backend returns error"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(!msg.is_empty(), "error response should contain a message");
}

/// `restart` is in the dispatch table and does not return an "unsupported command" error.
#[tokio::test]
async fn test_restart_is_dispatched() {
    let (backend, _flag) = HotRestartRecorder::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    let req = make_request(1, "restart");

    let resp = adapter.handle_request(&req).await;

    // The response must not be the generic "unsupported command" error.
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        !msg.contains("unsupported command"),
        "restart must be dispatched, not rejected as unsupported"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: `supportsRestartRequest` capability
// ─────────────────────────────────────────────────────────────────────────────

/// `fdemon_defaults()` must advertise `supportsRestartRequest: true` now that
/// the `restart` handler is implemented.
#[test]
fn test_supports_restart_request_in_capabilities() {
    let caps = Capabilities::fdemon_defaults();
    assert_eq!(
        caps.supports_restart_request,
        Some(true),
        "supportsRestartRequest must be true in fdemon_defaults()"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: `dart.serviceExtensionAdded` event
// ─────────────────────────────────────────────────────────────────────────────

/// Receiving a `ServiceExtensionAdded` debug event must emit `dart.serviceExtensionAdded`.
#[tokio::test]
async fn test_service_extension_added_emits_custom_event() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    adapter
        .handle_debug_event(DebugEvent::ServiceExtensionAdded {
            isolate_id: "isolates/1".into(),
            extension_rpc: "ext.flutter.debugDumpApp".into(),
        })
        .await;

    let events = drain_events(&mut rx);
    let ev = events.iter().find_map(|m| {
        if let DapMessage::Event(e) = m {
            if e.event == "dart.serviceExtensionAdded" {
                return Some(e);
            }
        }
        None
    });

    assert!(
        ev.is_some(),
        "dart.serviceExtensionAdded event must be emitted"
    );
}

/// The `dart.serviceExtensionAdded` body must contain `extensionRPC` and `isolateId`.
#[tokio::test]
async fn test_service_extension_added_body_fields() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    adapter
        .handle_debug_event(DebugEvent::ServiceExtensionAdded {
            isolate_id: "isolates/42".into(),
            extension_rpc: "ext.flutter.showPerformanceOverlay".into(),
        })
        .await;

    let events = drain_events(&mut rx);
    let ev = events
        .iter()
        .find_map(|m| {
            if let DapMessage::Event(e) = m {
                if e.event == "dart.serviceExtensionAdded" {
                    return Some(e);
                }
            }
            None
        })
        .expect("dart.serviceExtensionAdded must be emitted");

    let body = ev
        .body
        .as_ref()
        .expect("dart.serviceExtensionAdded must have a body");

    assert_eq!(
        body["extensionRPC"].as_str(),
        Some("ext.flutter.showPerformanceOverlay"),
        "extensionRPC field must match"
    );
    assert_eq!(
        body["isolateId"].as_str(),
        Some("isolates/42"),
        "isolateId field must match"
    );
}

/// Multiple `ServiceExtensionAdded` events produce separate `dart.serviceExtensionAdded`
/// events in the channel.
#[tokio::test]
async fn test_service_extension_added_multiple_events() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    for rpc in &["ext.flutter.a", "ext.flutter.b", "ext.flutter.c"] {
        adapter
            .handle_debug_event(DebugEvent::ServiceExtensionAdded {
                isolate_id: "isolates/1".into(),
                extension_rpc: rpc.to_string(),
            })
            .await;
    }

    let events = drain_events(&mut rx);
    let extension_events: Vec<_> = events
        .iter()
        .filter_map(|m| {
            if let DapMessage::Event(e) = m {
                if e.event == "dart.serviceExtensionAdded" {
                    return Some(e);
                }
            }
            None
        })
        .collect();

    assert_eq!(
        extension_events.len(),
        3,
        "three dart.serviceExtensionAdded events must be emitted"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: `with_timeout` helper
// ─────────────────────────────────────────────────────────────────────────────

/// `with_timeout` returns the value when the future resolves before the deadline.
#[tokio::test]
async fn test_with_timeout_success() {
    use crate::adapter::handlers::with_timeout;
    use crate::adapter::types::BackendError;

    let result: Result<i32, String> = with_timeout(async { Ok::<i32, BackendError>(42) }).await;
    assert_eq!(result, Ok(42));
}

/// `with_timeout` propagates backend errors as `Err(String)`.
#[tokio::test]
async fn test_with_timeout_propagates_backend_error() {
    use crate::adapter::handlers::with_timeout;
    use crate::adapter::types::BackendError;

    let result: Result<i32, String> =
        with_timeout(async { Err::<i32, BackendError>(BackendError::NotConnected) }).await;
    assert!(result.is_err(), "backend error should propagate");
    let msg = result.unwrap_err();
    assert!(!msg.is_empty(), "error message should be non-empty");
}

/// `with_timeout` returns an error when the future takes longer than the timeout.
///
/// Uses `tokio::time::pause` so the test runs at virtual time and does not
/// actually block.
#[tokio::test]
async fn test_with_timeout_fires_on_hung_future() {
    use crate::adapter::handlers::with_timeout;
    use crate::adapter::types::BackendError;

    tokio::time::pause();

    // This future sleeps for 1 hour — far longer than REQUEST_TIMEOUT (10 s).
    let future = async {
        tokio::time::sleep(Duration::from_secs(3600)).await;
        Ok::<i32, BackendError>(1)
    };

    // Advance time past the request timeout.
    let handle = tokio::spawn(async move { with_timeout(future).await });
    tokio::time::advance(Duration::from_secs(11)).await;

    let result = handle.await.expect("task should not panic");
    assert!(result.is_err(), "hung future should time out");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("timed out"),
        "timeout message should mention 'timed out', got: {:?}",
        msg
    );
}
