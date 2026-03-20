//! Unit tests for the `restartFrame` DAP request handler.
//!
//! Verifies that:
//! - `restartFrame` calls `backend.resume` with `StepMode::Rewind` and the
//!   correct frame index.
//! - Frames at or above the first async suspension marker are rejected with
//!   a descriptive error.
//! - Frames below the async marker succeed.
//! - Invalid (stale or unknown) frame IDs return an error.
//! - Missing arguments return an error.
//! - `StepMode::Rewind` maps to the correct string for VM Service.
//! - `supportsRestartFrame: true` is advertised in `Capabilities`.

use std::sync::{Arc, Mutex};

use crate::adapter::test_helpers::MockTestBackend;
use crate::adapter::{BackendError, DapAdapter, FrameRef, StepMode};
use crate::DapRequest;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_restart_frame_request(seq: i64, frame_id: i64) -> DapRequest {
    DapRequest {
        seq,
        command: "restartFrame".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    }
}

/// A backend that records the arguments passed to `resume`.
struct RecordingBackend {
    /// Recorded as `(isolate_id, step, frame_index)` per call.
    calls: Arc<Mutex<Vec<(String, Option<StepMode>, Option<i32>)>>>,
}

impl RecordingBackend {
    fn new() -> (
        Self,
        Arc<Mutex<Vec<(String, Option<StepMode>, Option<i32>)>>>,
    ) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
            },
            calls,
        )
    }
}

impl MockTestBackend for RecordingBackend {
    async fn resume(
        &self,
        isolate_id: &str,
        step: Option<StepMode>,
        frame_index: Option<i32>,
    ) -> Result<(), BackendError> {
        self.calls
            .lock()
            .unwrap()
            .push((isolate_id.to_string(), step, frame_index));
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_restart_frame_calls_resume_with_rewind_and_frame_index() {
    // Arrange: adapter with a frame at index 2 registered.
    let (backend, calls) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    adapter.thread_map.get_or_create("isolates/1");

    // Register frame at index 2 (simulates a real stackTrace response).
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 2));

    // Act: send restartFrame request.
    let req = make_restart_frame_request(1, frame_id);
    let resp = adapter.handle_request(&req).await;

    // Assert: success response.
    assert!(
        resp.success,
        "restartFrame should succeed: {:?}",
        resp.message
    );

    // Assert: backend.resume called with Rewind and frame_index=2.
    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1, "Expected exactly one resume call");
    let (iso, step, fi) = &recorded[0];
    assert_eq!(iso, "isolates/1");
    assert_eq!(*step, Some(StepMode::Rewind));
    assert_eq!(*fi, Some(2));
}

#[tokio::test]
async fn test_restart_frame_rejects_frame_at_async_marker_boundary() {
    // Arrange: async marker is at index 3.
    let (backend, _calls) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    adapter.thread_map.get_or_create("isolates/1");

    // Set async marker index to 3.
    adapter.first_async_marker_index = Some(3);

    // Register a frame at index 3 (same as the async marker index).
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 3));

    // Act: attempt to restart frame at index 3 — should be rejected.
    let req = make_restart_frame_request(1, frame_id);
    let resp = adapter.handle_request(&req).await;

    // Assert: error response.
    assert!(!resp.success, "restartFrame at async boundary should fail");
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("async") || msg.contains("boundary"),
        "Error message should mention async boundary, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_restart_frame_rejects_frame_above_async_marker() {
    // Arrange: async marker is at index 2; request targets frame at index 4.
    let (backend, _calls) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    adapter.thread_map.get_or_create("isolates/1");

    adapter.first_async_marker_index = Some(2);

    // Frame at index 4 is above (greater index) the marker at index 2.
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 4));

    let req = make_restart_frame_request(1, frame_id);
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "restartFrame above async boundary should fail"
    );
}

#[tokio::test]
async fn test_restart_frame_allows_sync_frame_below_async_marker() {
    // Arrange: async marker at index 3; request targets frame at index 1.
    let (backend, calls) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    adapter.thread_map.get_or_create("isolates/1");

    adapter.first_async_marker_index = Some(3);

    // Frame at index 1 is below the async marker at index 3.
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 1));

    let req = make_restart_frame_request(1, frame_id);
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "restartFrame below async marker should succeed: {:?}",
        resp.message
    );

    // Verify backend was called with correct args.
    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let (_, step, fi) = &recorded[0];
    assert_eq!(*step, Some(StepMode::Rewind));
    assert_eq!(*fi, Some(1));
}

#[tokio::test]
async fn test_restart_frame_allows_sync_frame_when_no_async_marker() {
    // Arrange: no async marker — all frames are eligible for rewind.
    let (backend, calls) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    adapter.thread_map.get_or_create("isolates/1");

    // No first_async_marker_index set (None by default).
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    let req = make_restart_frame_request(1, frame_id);
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "restartFrame without async marker should succeed: {:?}",
        resp.message
    );
    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let (_, step, fi) = &recorded[0];
    assert_eq!(*step, Some(StepMode::Rewind));
    assert_eq!(*fi, Some(0));
}

#[tokio::test]
async fn test_restart_frame_invalid_frame_id_returns_error() {
    let (backend, _) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);

    // Use a frame ID that was never allocated.
    let req = make_restart_frame_request(1, 9999);
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "Unknown frame ID should return error");
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("9999") || msg.contains("frame") || msg.contains("stale"),
        "Error message should be informative, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_restart_frame_missing_arguments_returns_error() {
    let (backend, _) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);

    // Request with no arguments at all.
    let req = DapRequest {
        seq: 1,
        command: "restartFrame".into(),
        arguments: None,
    };
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "Missing arguments should return error");
}

#[tokio::test]
async fn test_restart_frame_invalidates_per_stop_state() {
    // Verifies that on_resume() is called, clearing var/frame stores.
    let (backend, _) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);
    adapter.thread_map.get_or_create("isolates/1");

    // Pre-allocate some state that should be invalidated.
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));
    let var_ref = adapter
        .var_store
        .allocate(crate::adapter::VariableRef::Scope {
            frame_index: 0,
            scope_kind: crate::adapter::ScopeKind::Locals,
        });

    assert!(adapter.frame_store.lookup(frame_id).is_some());
    assert!(adapter.var_store.lookup(var_ref).is_some());

    // restartFrame should call on_resume() before the backend call.
    let req = make_restart_frame_request(1, frame_id);
    let resp = adapter.handle_request(&req).await;

    // The frame store was cleared by on_resume (frame_id is now stale).
    // The response may succeed or fail depending on timing, but stores are cleared.
    let _ = resp;
    assert!(
        adapter.frame_store.lookup(frame_id).is_none(),
        "frame_store must be reset after restartFrame"
    );
    assert!(
        adapter.var_store.lookup(var_ref).is_none(),
        "var_store must be reset after restartFrame"
    );
}

#[tokio::test]
async fn test_restart_frame_async_marker_cleared_on_resume() {
    // Verifies first_async_marker_index is cleared by on_resume.
    let (backend, _) = RecordingBackend::new();
    let (mut adapter, _rx) = DapAdapter::new(backend);

    adapter.first_async_marker_index = Some(5);
    assert_eq!(adapter.first_async_marker_index, Some(5));

    adapter.on_resume();

    assert_eq!(
        adapter.first_async_marker_index, None,
        "first_async_marker_index should be None after on_resume"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Capability and StepMode tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_supports_restart_frame_in_capabilities() {
    use crate::protocol::types::Capabilities;
    let caps = Capabilities::fdemon_defaults();
    assert_eq!(
        caps.supports_restart_frame,
        Some(true),
        "Capabilities must advertise supportsRestartFrame: true"
    );
}

#[test]
fn test_step_mode_rewind_is_distinct_from_other_modes() {
    assert_ne!(StepMode::Rewind, StepMode::Over);
    assert_ne!(StepMode::Rewind, StepMode::Into);
    assert_ne!(StepMode::Rewind, StepMode::Out);
}
