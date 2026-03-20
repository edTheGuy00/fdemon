//! Tests for the `callService` custom DAP request handler.
//!
//! `callService` is a Dart-specific custom request that forwards arbitrary VM
//! Service RPCs to the backend. The VS Code Dart extension uses it to invoke
//! service extensions such as `ext.flutter.debugDumpApp`,
//! `ext.flutter.showPerformanceOverlay`, and similar DevTools RPCs.

use crate::adapter::test_helpers::{MockBackend, MockTestBackend};
use crate::adapter::{BackendError, DapAdapter};
use crate::DapRequest;

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `callService` request with the given method and optional params.
fn make_call_service_request(
    seq: i64,
    method: &str,
    params: Option<serde_json::Value>,
) -> DapRequest {
    let mut args = serde_json::json!({ "method": method });
    if let Some(p) = params {
        args["params"] = p;
    }
    DapRequest {
        seq,
        command: "callService".into(),
        arguments: Some(args),
    }
}

/// Build a `callService` request with no arguments at all.
fn make_call_service_no_args(seq: i64) -> DapRequest {
    DapRequest {
        seq,
        command: "callService".into(),
        arguments: None,
    }
}

/// Build a `callService` request with a body that has no `"method"` key.
fn make_call_service_missing_method(seq: i64) -> DapRequest {
    DapRequest {
        seq,
        command: "callService".into(),
        arguments: Some(serde_json::json!({ "params": { "enabled": true } })),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends
// ─────────────────────────────────────────────────────────────────────────────

/// A backend that returns a canned JSON result for `call_service`.
struct CallServiceMockBackend {
    result: serde_json::Value,
}

impl CallServiceMockBackend {
    fn returning(result: serde_json::Value) -> Self {
        Self { result }
    }
}

impl MockTestBackend for CallServiceMockBackend {
    async fn call_service(
        &self,
        _method: &str,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(self.result.clone())
    }
}

/// A backend whose `call_service` always fails with a VM Service error.
struct FailingCallServiceBackend {
    error_message: String,
}

impl FailingCallServiceBackend {
    fn with_message(msg: &str) -> Self {
        Self {
            error_message: msg.to_string(),
        }
    }
}

impl MockTestBackend for FailingCallServiceBackend {
    async fn call_service(
        &self,
        _method: &str,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        Err(BackendError::VmServiceError(self.error_message.clone()))
    }
}

/// A backend that records the last `call_service` invocation for inspection.
struct RecordingCallServiceBackend {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(String, Option<serde_json::Value>)>>>,
    result: serde_json::Value,
}

impl RecordingCallServiceBackend {
    fn new(
        result: serde_json::Value,
    ) -> (
        Self,
        std::sync::Arc<std::sync::Mutex<Vec<(String, Option<serde_json::Value>)>>>,
    ) {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let backend = Self {
            calls: calls.clone(),
            result,
        };
        (backend, calls)
    }
}

impl MockTestBackend for RecordingCallServiceBackend {
    async fn call_service(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params));
        Ok(self.result.clone())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: success paths
// ─────────────────────────────────────────────────────────────────────────────

/// Test 1: `callService` with a known method forwards to the backend and
/// returns a success response containing the VM Service result.
#[tokio::test]
async fn test_call_service_forwards_method_and_returns_result() {
    let backend = CallServiceMockBackend::returning(serde_json::json!({
        "type": "Success",
        "tree": { "description": "MaterialApp" }
    }));
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let req = make_call_service_request(1, "ext.flutter.debugDumpApp", None);
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "callService should succeed when backend returns Ok"
    );
    let body = resp.body.expect("response should have a body");
    assert!(
        body.get("result").is_some(),
        "response body should contain 'result' key"
    );
    assert_eq!(body["result"]["type"], "Success");
}

/// Test 2: `callService` forwards params to the backend unmodified.
#[tokio::test]
async fn test_call_service_forwards_params_to_backend() {
    let canned_result = serde_json::json!({ "enabled": true });
    let (backend, calls) = RecordingCallServiceBackend::new(canned_result);
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let params = serde_json::json!({ "enabled": true });
    let req = make_call_service_request(
        1,
        "ext.flutter.showPerformanceOverlay",
        Some(params.clone()),
    );
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "callService should succeed");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1, "backend should have been called once");
    assert_eq!(recorded[0].0, "ext.flutter.showPerformanceOverlay");
    assert_eq!(
        recorded[0].1.as_ref().expect("params should be present"),
        &params,
        "params should be forwarded verbatim"
    );
}

/// Test 3: `callService` without optional `params` still succeeds.
#[tokio::test]
async fn test_call_service_without_params_succeeds() {
    let backend = CallServiceMockBackend::returning(serde_json::json!({ "result": "ok" }));
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let req = make_call_service_request(2, "ext.flutter.debugPaint", None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "callService without params should succeed");
}

/// Test 4: `callService` with no arguments returns an error response.
#[tokio::test]
async fn test_call_service_missing_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);

    let req = make_call_service_no_args(3);
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "callService with no arguments should return an error"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("missing arguments"),
        "error message should mention missing arguments, got: {:?}",
        msg
    );
}

/// Test 5: `callService` with a body but no `"method"` field returns an error.
#[tokio::test]
async fn test_call_service_missing_method_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);

    let req = make_call_service_missing_method(4);
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "callService missing 'method' should return an error"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("method"),
        "error message should mention 'method', got: {:?}",
        msg
    );
}

/// Test 6: `callService` propagates VM Service errors as a DAP error response.
/// The error text comes from the VM Service, not the adapter.
#[tokio::test]
async fn test_call_service_backend_error_returns_error_response() {
    let backend = FailingCallServiceBackend::with_message("method not found: ext.flutter.unknown");
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let req = make_call_service_request(5, "ext.flutter.unknown", None);
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "callService should return error when backend fails"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("callService failed"),
        "error message should wrap the backend error, got: {:?}",
        msg
    );
}

/// Test 7: response `command` field echoes the request command.
#[tokio::test]
async fn test_call_service_response_command_matches_request() {
    let backend = CallServiceMockBackend::returning(serde_json::json!({}));
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let req = make_call_service_request(6, "ext.flutter.reassemble", None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    assert_eq!(
        resp.command, "callService",
        "response command should echo the request command"
    );
}

/// Test 8: `callService` with a method that returns a complex nested object.
/// Verifies that arbitrarily shaped VM Service responses are passed through
/// to the `result` field without modification.
#[tokio::test]
async fn test_call_service_complex_result_is_preserved() {
    let complex_result = serde_json::json!({
        "type": "WidgetTree",
        "children": [
            { "description": "Column", "children": [] },
            { "description": "Text", "children": [] }
        ],
        "renderObject": { "size": { "width": 375.0, "height": 812.0 } }
    });
    let backend = CallServiceMockBackend::returning(complex_result.clone());
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let req = make_call_service_request(7, "ext.flutter.inspector.getRootWidget", None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "callService should succeed");
    let body = resp.body.expect("response should have a body");
    assert_eq!(
        body["result"], complex_result,
        "complex result should be preserved verbatim in the 'result' field"
    );
}
