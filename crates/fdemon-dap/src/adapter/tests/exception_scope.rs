//! Tests for the "Exceptions" scope feature.
//!
//! Covers:
//! - `ScopeKind::Exceptions` appearing in `handle_scopes` when paused at an exception
//! - `ScopeKind::Exceptions` absent when paused at a breakpoint or step
//! - Exception variable returned in `handle_variables` for the Exceptions scope
//! - Exception ref cleared on resume
//! - `$_threadException` magic evaluate expression

use super::register_isolate;
use crate::adapter::test_helpers::*;
use crate::adapter::*;

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Simulate a pause at an exception with a given InstanceRef JSON.
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

/// Build a minimal InstanceRef JSON for a `FormatException`.
fn format_exception_ref() -> serde_json::Value {
    serde_json::json!({
        "type": "InstanceRef",
        "kind": "PlainInstance",
        "id": "objects/exc1",
        "classRef": { "name": "FormatException", "id": "classes/FormatException" }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Exception scope visibility
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_scopes_include_exceptions_on_pause_exception() {
    // Simulate PauseException with an exception ref — scopes should have 3 entries.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    // Request a stack trace to allocate frame IDs.
    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    assert!(stack_resp.success, "stackTrace should succeed");
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // Request scopes for the top frame.
    let scopes_req = crate::DapRequest {
        seq: 11,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let resp = adapter.handle_request(&scopes_req).await;
    assert!(resp.success, "scopes should succeed: {:?}", resp.message);

    let scopes = resp.body.unwrap()["scopes"].as_array().unwrap().clone();
    assert_eq!(
        scopes.len(),
        3,
        "Expected Locals + Globals + Exceptions, got {} scopes: {:?}",
        scopes.len(),
        scopes
    );

    let names: Vec<&str> = scopes.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"Locals"), "Missing Locals scope");
    assert!(names.contains(&"Globals"), "Missing Globals scope");
    assert!(names.contains(&"Exceptions"), "Missing Exceptions scope");
}

#[tokio::test]
async fn test_scopes_no_exceptions_on_pause_breakpoint() {
    // Paused at breakpoint (no exception) → only Locals + Globals.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: None,
            exception: None,
        })
        .await;
    rx.try_recv().ok(); // Drain stopped event.

    // Get the top frame ID.
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_req = crate::DapRequest {
        seq: 11,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let resp = adapter.handle_request(&scopes_req).await;
    assert!(resp.success);
    let scopes = resp.body.unwrap()["scopes"].as_array().unwrap().clone();
    assert_eq!(
        scopes.len(),
        2,
        "Expected only Locals + Globals, got {} scopes",
        scopes.len()
    );
    let names: Vec<&str> = scopes.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        !names.contains(&"Exceptions"),
        "Exceptions scope should be absent at breakpoint"
    );
}

#[tokio::test]
async fn test_scopes_no_exceptions_on_pause_step() {
    // Paused after a step (no exception) → only Locals + Globals.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Step,
            breakpoint_id: None,
            exception: None,
        })
        .await;
    rx.try_recv().ok();

    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_req = crate::DapRequest {
        seq: 11,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let resp = adapter.handle_request(&scopes_req).await;
    assert!(resp.success);
    let scopes = resp.body.unwrap()["scopes"].as_array().unwrap().clone();
    assert_eq!(
        scopes.len(),
        2,
        "Expected only Locals + Globals on step pause, got {} scopes",
        scopes.len()
    );
    let names: Vec<&str> = scopes.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        !names.contains(&"Exceptions"),
        "Exceptions scope must be absent on step"
    );
}

#[tokio::test]
async fn test_exceptions_scope_has_correct_attributes() {
    // Verify the Exceptions scope has expected DAP attributes.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let frame_id = adapter.handle_request(&stack_req).await.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_req = crate::DapRequest {
        seq: 11,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let resp = adapter.handle_request(&scopes_req).await;
    let scopes = resp.body.unwrap()["scopes"].as_array().unwrap().clone();

    let exc_scope = scopes
        .iter()
        .find(|s| s["name"] == "Exceptions")
        .expect("Exceptions scope must be present");

    // Must have a non-zero variablesReference for expansion.
    assert!(
        exc_scope["variablesReference"].as_i64().unwrap_or(0) > 0,
        "Exceptions scope must have a non-zero variablesReference"
    );
    // Should not be marked expensive.
    assert_eq!(
        exc_scope["expensive"], false,
        "Exceptions scope should not be expensive"
    );
    // Presentation hint should be "locals".
    assert_eq!(
        exc_scope["presentationHint"], "locals",
        "Exceptions scope should use 'locals' presentation hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Exception variable content
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_scope_returns_single_variable() {
    // Expanding the Exceptions scope returns exactly one variable.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let frame_id = adapter.handle_request(&stack_req).await.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_req = crate::DapRequest {
        seq: 11,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let scopes_resp = adapter.handle_request(&scopes_req).await;
    let scopes = scopes_resp.body.unwrap()["scopes"]
        .as_array()
        .unwrap()
        .clone();
    let exc_scope = scopes
        .iter()
        .find(|s| s["name"] == "Exceptions")
        .expect("Exceptions scope must be present");
    let exc_ref = exc_scope["variablesReference"].as_i64().unwrap();

    // Expand the Exceptions scope.
    let vars_req = crate::DapRequest {
        seq: 12,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": exc_ref })),
    };
    let vars_resp = adapter.handle_request(&vars_req).await;
    assert!(
        vars_resp.success,
        "variables should succeed: {:?}",
        vars_resp.message
    );

    let variables = vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        variables.len(),
        1,
        "Exceptions scope must return exactly one variable, got {}",
        variables.len()
    );
}

#[tokio::test]
async fn test_exception_variable_named_by_class() {
    // The exception variable name should be the exception class name.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let frame_id = adapter.handle_request(&stack_req).await.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 11,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    let exc_ref = scopes_resp.body.unwrap()["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Exceptions")
        .unwrap()["variablesReference"]
        .as_i64()
        .unwrap();

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 12,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": exc_ref })),
        })
        .await;
    let variables = vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        variables[0]["name"], "FormatException",
        "Exception variable name must match the exception class name"
    );
}

#[tokio::test]
async fn test_exception_variable_has_nonzero_variables_reference() {
    // A `PlainInstance` exception should have a non-zero variablesReference
    // so the IDE can expand its fields.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let frame_id = adapter.handle_request(&stack_req).await.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 11,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    let exc_ref = scopes_resp.body.unwrap()["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Exceptions")
        .unwrap()["variablesReference"]
        .as_i64()
        .unwrap();

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 12,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": exc_ref })),
        })
        .await;
    let variables = vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        variables[0]["variablesReference"].as_i64().unwrap_or(0) > 0,
        "Exception PlainInstance variable must be expandable (variablesReference > 0)"
    );
}

#[tokio::test]
async fn test_exception_class_name_from_class_field() {
    // The class name should be resolved from "class" (raw wire format) when
    // "classRef" is absent — testing the fallback path.
    let exc_json = serde_json::json!({
        "type": "InstanceRef",
        "kind": "PlainInstance",
        "id": "objects/exc2",
        "class": { "name": "RangeError", "id": "classes/RangeError" }
    });

    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", exc_json).await;

    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();
    let stack_req = crate::DapRequest {
        seq: 10,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let frame_id = adapter.handle_request(&stack_req).await.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 11,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    let exc_ref = scopes_resp.body.unwrap()["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Exceptions")
        .unwrap()["variablesReference"]
        .as_i64()
        .unwrap();

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 12,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": exc_ref })),
        })
        .await;
    let variables = vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        variables[0]["name"], "RangeError",
        "class field fallback should yield class name 'RangeError'"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Exception cleared on resume
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_cleared_on_resume() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);

    // Register isolate and ensure thread_map has a mapping.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let thread_id = adapter.thread_map.thread_id_for("isolates/1").unwrap();

    // Pause at exception — exception_refs should be populated.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Exception,
            breakpoint_id: None,
            exception: Some(format_exception_ref()),
        })
        .await;
    rx.try_recv().ok(); // Drain stopped event.
    assert!(
        adapter.exception_refs.contains_key(&thread_id),
        "exception_refs should have entry for thread {} after exception pause",
        thread_id
    );

    // Resume — exception_refs should be cleared.
    adapter
        .handle_debug_event(DebugEvent::Resumed {
            isolate_id: "isolates/1".into(),
        })
        .await;
    rx.try_recv().ok();
    assert!(
        !adapter.exception_refs.contains_key(&thread_id),
        "exception_refs should be empty after resume, still has key for thread {}",
        thread_id
    );
}

#[tokio::test]
async fn test_exception_not_stored_without_exception_value() {
    // PauseException with exception: None should not store anything.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Exception,
            breakpoint_id: None,
            exception: None,
        })
        .await;
    rx.try_recv().ok();

    assert!(
        adapter.exception_refs.is_empty(),
        "exception_refs should be empty when exception is None"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// $_ threadException magic expression
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_thread_exception_returns_exception_value() {
    // `$_threadException` should return the current exception.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    pause_at_exception(&mut adapter, &mut rx, "isolates/1", format_exception_ref()).await;

    let req = crate::DapRequest {
        seq: 20,
        command: "evaluate".into(),
        arguments: Some(serde_json::json!({
            "expression": "$_threadException",
        })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "$_threadException should succeed: {:?}",
        resp.message
    );

    let body = resp.body.unwrap();
    // variablesReference should be non-zero for a PlainInstance exception.
    assert!(
        body["variablesReference"].as_i64().unwrap_or(0) > 0,
        "$_threadException should return expandable result, got body: {:?}",
        body
    );
}

#[tokio::test]
async fn test_thread_exception_error_when_not_paused_at_exception() {
    // `$_threadException` when not paused at an exception returns an error.
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Pause at breakpoint (no exception).
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
        seq: 20,
        command: "evaluate".into(),
        arguments: Some(serde_json::json!({
            "expression": "$_threadException",
        })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "$_threadException should fail when not at exception"
    );
    assert!(
        resp.message
            .as_deref()
            .unwrap_or("")
            .contains("not paused at an exception"),
        "Error message should mention 'not paused at an exception', got: {:?}",
        resp.message
    );
}

#[tokio::test]
async fn test_thread_exception_error_when_no_paused_isolate() {
    // `$_threadException` with no paused isolate returns an error.
    let (mut adapter, _rx) = DapAdapter::new(StackMockBackend);

    let req = crate::DapRequest {
        seq: 20,
        command: "evaluate".into(),
        arguments: Some(serde_json::json!({
            "expression": "$_threadException",
        })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "$_threadException should fail with no paused isolate"
    );
}
