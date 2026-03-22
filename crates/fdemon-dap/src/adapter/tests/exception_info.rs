//! Tests for the `exceptionInfo` DAP request handler.
//!
//! Covers:
//! - Structured exception data returned when paused at an exception
//! - `description` field contains `toString()` output
//! - `typeName` contains the exception class name
//! - `breakMode` reflects the current exception pause mode
//! - Error returned when no exception is available
//! - `supportsExceptionInfoRequest: true` in capabilities
//! - `details.evaluateName` is always set to `"$_threadException"`

use super::register_isolate;
use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::protocol::types::Capabilities;

// ─────────────────────────────────────────────────────────────────────────────
// Mock backend that returns realistic toString() and stackTrace results
// ─────────────────────────────────────────────────────────────────────────────

/// Backend returning a FormatException toString() result.
struct ExceptionInfoMockBackend {
    /// Canned `toString()` output for the exception.
    to_string_result: String,
    /// Canned `stackTrace?.toString()` output (empty string = null/none).
    stack_trace_result: Option<String>,
}

impl ExceptionInfoMockBackend {
    fn with_stack_trace(to_string: &str, stack_trace: &str) -> Self {
        Self {
            to_string_result: to_string.to_string(),
            stack_trace_result: Some(stack_trace.to_string()),
        }
    }

    fn without_stack_trace(to_string: &str) -> Self {
        Self {
            to_string_result: to_string.to_string(),
            stack_trace_result: None,
        }
    }
}

impl MockTestBackend for ExceptionInfoMockBackend {
    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        match expression {
            "toString()" => Ok(serde_json::json!({
                "type": "InstanceRef",
                "kind": "String",
                "valueAsString": self.to_string_result,
            })),
            "stackTrace?.toString()" => match &self.stack_trace_result {
                Some(st) => Ok(serde_json::json!({
                    "type": "InstanceRef",
                    "kind": "String",
                    "valueAsString": st,
                })),
                None => Err(BackendError::NotConnected),
            },
            _ => Ok(serde_json::json!({})),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: simulate pause at exception and return thread_id
// ─────────────────────────────────────────────────────────────────────────────

/// Simulate a `PauseException` event for the given isolate.
///
/// Returns the DAP thread ID for the paused isolate.
async fn pause_at_exception(
    adapter: &mut DapAdapter<impl DebugBackend>,
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
    isolate_id: &str,
    exception_json: serde_json::Value,
) -> i64 {
    let thread_id = register_isolate(adapter, rx, isolate_id).await;
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: isolate_id.into(),
            reason: PauseReason::Exception,
            breakpoint_id: None,
            exception: Some(exception_json),
        })
        .await;
    // Drain the stopped event.
    rx.try_recv().ok();
    thread_id
}

/// Build a minimal FormatException InstanceRef.
fn format_exception_ref() -> serde_json::Value {
    serde_json::json!({
        "type": "InstanceRef",
        "kind": "PlainInstance",
        "id": "objects/exc1",
        "classRef": { "name": "FormatException", "id": "classes/FormatException" }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Core functionality tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_info_returns_structured_data() {
    // exceptionInfo should return exceptionId, description, breakMode, details.
    let backend = ExceptionInfoMockBackend::with_stack_trace(
        "FormatException: Unexpected character",
        "#0 main (file:///app/lib/main.dart:42:5)",
    );
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 30,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "exceptionInfo should succeed: {:?}",
        resp.message
    );
    let body = resp.body.unwrap();
    assert!(
        !body["exceptionId"].as_str().unwrap_or("").is_empty(),
        "exceptionId should be non-empty"
    );
    assert!(
        !body["description"].as_str().unwrap_or("").is_empty(),
        "description should be non-empty"
    );
    assert!(
        !body["breakMode"].as_str().unwrap_or("").is_empty(),
        "breakMode should be non-empty"
    );
    assert!(body["details"].is_object(), "details should be an object");
}

#[tokio::test]
async fn test_exception_info_description_from_to_string() {
    // `description` should contain the result of toString().
    let backend = ExceptionInfoMockBackend::without_stack_trace(
        "FormatException: Unexpected character (at character 1)\n!@#$\n^",
    );
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 31,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["description"], "FormatException: Unexpected character (at character 1)\n!@#$\n^",
        "description should match toString() output"
    );
}

#[tokio::test]
async fn test_exception_info_type_name_from_class_ref() {
    // `details.typeName` should contain the exception class name from classRef.
    let backend = ExceptionInfoMockBackend::without_stack_trace("FormatException");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 32,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["details"]["typeName"], "FormatException",
        "typeName should be the exception class name"
    );
}

#[tokio::test]
async fn test_exception_info_type_name_from_class_field_fallback() {
    // `details.typeName` should fall back to the `class` field when `classRef` is absent.
    let exc_json = serde_json::json!({
        "type": "InstanceRef",
        "kind": "PlainInstance",
        "id": "objects/exc2",
        "class": { "name": "RangeError", "id": "classes/RangeError" }
    });
    let backend = ExceptionInfoMockBackend::without_stack_trace("RangeError");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id = pause_at_exception(&mut adapter, &mut rx, "isolates/1", exc_json).await;

    let req = crate::DapRequest {
        seq: 33,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["details"]["typeName"], "RangeError",
        "typeName should fall back to 'class' field when classRef is absent"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// breakMode mapping tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_info_break_mode_unhandled_by_default() {
    // Default exception_mode is Unhandled → breakMode = "unhandled".
    let backend = ExceptionInfoMockBackend::without_stack_trace("SomeException");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 40,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["breakMode"], "unhandled",
        "default breakMode should be 'unhandled'"
    );
}

#[tokio::test]
async fn test_exception_info_break_mode_always_when_all_mode() {
    // Setting exception_mode to All → breakMode = "always".
    let backend = ExceptionInfoMockBackend::without_stack_trace("SomeException");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    // Apply "All" filter to set the mode.
    let set_exc_req = crate::DapRequest {
        seq: 1,
        command: "setExceptionBreakpoints".into(),
        arguments: Some(serde_json::json!({ "filters": ["All"] })),
    };
    adapter.handle_request(&set_exc_req).await;

    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 41,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["breakMode"], "always",
        "breakMode should be 'always' when exception_mode is All"
    );
}

#[tokio::test]
async fn test_exception_info_break_mode_never_when_none_mode() {
    // Setting exception_mode to None (no filters) → breakMode = "never".
    let backend = ExceptionInfoMockBackend::without_stack_trace("SomeException");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    // Set no filters → None mode.
    let set_exc_req = crate::DapRequest {
        seq: 1,
        command: "setExceptionBreakpoints".into(),
        arguments: Some(serde_json::json!({ "filters": [] })),
    };
    adapter.handle_request(&set_exc_req).await;

    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 42,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["breakMode"], "never",
        "breakMode should be 'never' when exception_mode is None"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error path tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_info_no_exception_returns_error() {
    // No exception stored → error response.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Pause at a breakpoint, not an exception.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: None,
            exception: None,
        })
        .await;
    rx.try_recv().ok();

    let req = crate::DapRequest {
        seq: 50,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "exceptionInfo should fail when not paused at an exception"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("No exception available"),
        "error message should mention 'No exception available', got: {:?}",
        resp.message
    );
}

#[tokio::test]
async fn test_exception_info_unknown_thread_returns_error() {
    // Requesting exceptionInfo for an unknown thread ID → error response.
    let (mut adapter, _rx) = DapAdapter::new(StackMockBackend);

    let req = crate::DapRequest {
        seq: 51,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": 9999 })),
    };
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "exceptionInfo should fail for unknown thread ID"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Details field tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_info_details_includes_evaluate_name() {
    // `details.evaluateName` should always be set to "$_threadException".
    let backend = ExceptionInfoMockBackend::without_stack_trace("MyException");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 60,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert_eq!(
        body["details"]["evaluateName"], "$_threadException",
        "evaluateName should be '$_threadException'"
    );
}

#[tokio::test]
async fn test_exception_info_details_includes_stack_trace_when_available() {
    // `details.stackTrace` should be present when stackTrace?.toString() succeeds.
    let backend = ExceptionInfoMockBackend::with_stack_trace(
        "FormatException",
        "#0 main (file:///app/lib/main.dart:42:5)\n#1 _runMain (dart:ui/hooks.dart:100:1)",
    );
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 61,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    let stack_trace = body["details"]["stackTrace"].as_str().unwrap_or("");
    assert!(
        !stack_trace.is_empty(),
        "details.stackTrace should be present when stackTrace?.toString() succeeds"
    );
    assert!(
        stack_trace.contains("#0 main"),
        "stackTrace should contain frame info, got: {:?}",
        stack_trace
    );
}

#[tokio::test]
async fn test_exception_info_details_no_stack_trace_when_unavailable() {
    // `details.stackTrace` should be absent when stackTrace?.toString() fails.
    let backend = ExceptionInfoMockBackend::without_stack_trace("SomeException");
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    let thread_id =
        pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 62,
        command: "exceptionInfo".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let body = resp.body.unwrap();
    assert!(
        body["details"]["stackTrace"].is_null() || body["details"].get("stackTrace").is_none(),
        "details.stackTrace should be absent when evaluation fails, got: {:?}",
        body["details"]["stackTrace"]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Capability test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_supports_exception_info_request_in_capabilities() {
    // supportsExceptionInfoRequest should be true in fdemon_defaults().
    let caps = Capabilities::fdemon_defaults();
    assert_eq!(
        caps.supports_exception_info_request,
        Some(true),
        "supportsExceptionInfoRequest must be true in fdemon_defaults()"
    );
}
