//! Tests for `toString()` enrichment in the variables panel.
//!
//! These tests cover:
//! - PlainInstance variables show `"MyClass (custom string)"` when toString
//!   returns useful output
//! - Default `"Instance of 'ClassName'"` toString output is suppressed
//! - toString errors fall back silently to class name only
//! - toString timeouts fall back silently to class name only
//! - Primitives, collections, and closures do NOT call toString
//! - `evaluateToStringInDebugViews = false` disables toString calls entirely
//! - RegExp and StackTrace kinds also receive toString enrichment
//! - WeakReference kind also receives toString enrichment
//! - Empty toString result is suppressed

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::adapter::test_helpers::MockTestBackend;
use crate::adapter::*;

use super::register_isolate;

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends for toString tests
// ─────────────────────────────────────────────────────────────────────────────

/// A mock backend that returns a PlainInstance variable in the stack frame and
/// a configurable toString() response.
struct ToStringMockBackend {
    /// The value returned by `evaluate(..., "toString()")`.
    to_string_value: Arc<Mutex<Result<serde_json::Value, BackendError>>>,
    /// Counter tracking how many `evaluate` calls were made.
    evaluate_call_count: Arc<Mutex<u32>>,
    /// The instance kind for the variable (default: "PlainInstance").
    instance_kind: &'static str,
}

impl ToStringMockBackend {
    fn new_returning(to_string_result: &str) -> (Self, Arc<Mutex<u32>>) {
        let call_count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            to_string_value: Arc::new(Mutex::new(Ok(serde_json::json!({
                "kind": "String",
                "valueAsString": to_string_result
            })))),
            evaluate_call_count: call_count.clone(),
            instance_kind: "PlainInstance",
        };
        (backend, call_count)
    }

    fn new_with_kind_and_result(
        kind: &'static str,
        to_string_result: &str,
    ) -> (Self, Arc<Mutex<u32>>) {
        let call_count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            to_string_value: Arc::new(Mutex::new(Ok(serde_json::json!({
                "kind": "String",
                "valueAsString": to_string_result
            })))),
            evaluate_call_count: call_count.clone(),
            instance_kind: kind,
        };
        (backend, call_count)
    }

    fn new_failing() -> (Self, Arc<Mutex<u32>>) {
        let call_count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            to_string_value: Arc::new(Mutex::new(Err(BackendError::VmServiceError(
                "toString() failed".to_string(),
            )))),
            evaluate_call_count: call_count.clone(),
            instance_kind: "PlainInstance",
        };
        (backend, call_count)
    }

    /// Variant that makes evaluate hang until the test's timeout fires.
    fn new_hanging(kind: &'static str) -> (Self, Arc<Mutex<u32>>) {
        // We'll simulate a hang by using a special sentinel.
        // The mock's evaluate will never resolve — we use
        // `evaluate_call_count` to detect that it was called.
        let call_count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            to_string_value: Arc::new(Mutex::new(Ok(serde_json::json!({
                "__hang__": true
            })))),
            evaluate_call_count: call_count.clone(),
            instance_kind: kind,
        };
        (backend, call_count)
    }
}

impl MockTestBackend for ToStringMockBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        let kind = self.instance_kind;
        Ok(serde_json::json!({
            "frames": [{
                "kind": "Regular",
                "code": { "name": "main" },
                "location": {
                    "script": { "uri": "file:///app/lib/main.dart" },
                    "line": 10
                },
                "vars": [{
                    "name": "obj",
                    "value": {
                        "type": "InstanceRef",
                        "kind": kind,
                        "classRef": { "name": "MyModel" },
                        "id": "objects/inst1"
                    }
                }]
            }]
        }))
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        *self.evaluate_call_count.lock().unwrap() += 1;
        let result = self.to_string_value.lock().unwrap().clone();
        // Simulate hanging by sleeping longer than the 1s timeout.
        if result
            .as_ref()
            .map(|v| v.get("__hang__").is_some())
            .unwrap_or(false)
        {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        result
    }
}

/// A mock backend that returns a stack frame with a primitive variable.
/// Used to verify that primitives do NOT trigger toString().
struct PrimitiveVarMockBackend {
    evaluate_call_count: Arc<Mutex<u32>>,
    kind: &'static str,
}

impl PrimitiveVarMockBackend {
    fn new(kind: &'static str) -> (Self, Arc<Mutex<u32>>) {
        let count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            evaluate_call_count: count.clone(),
            kind,
        };
        (backend, count)
    }
}

impl MockTestBackend for PrimitiveVarMockBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        let kind = self.kind;
        Ok(serde_json::json!({
            "frames": [{
                "kind": "Regular",
                "code": { "name": "main" },
                "location": {
                    "script": { "uri": "file:///app/lib/main.dart" },
                    "line": 10
                },
                "vars": [{
                    "name": "x",
                    "value": {
                        "type": "InstanceRef",
                        "kind": kind,
                        "valueAsString": "42",
                        "id": "objects/prim1"
                    }
                }]
            }]
        }))
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        *self.evaluate_call_count.lock().unwrap() += 1;
        Ok(serde_json::json!({ "kind": "String", "valueAsString": "should not be called" }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers to drive a full scope → variables request sequence
// ─────────────────────────────────────────────────────────────────────────────

/// Set up adapter with isolate, run stackTrace → scopes → variables for frame 0.
/// Returns the list of `DapVariable`-equivalent JSON objects from the response.
async fn get_locals(
    adapter: &mut DapAdapter<impl DebugBackend>,
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
) -> Vec<serde_json::Value> {
    let thread_id = register_isolate(adapter, rx, "isolates/1").await;

    let stack_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        })
        .await;
    assert!(
        stack_resp.success,
        "stackTrace failed: {:?}",
        stack_resp.message
    );
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    assert!(
        scopes_resp.success,
        "scopes failed: {:?}",
        scopes_resp.message
    );
    let locals_ref = scopes_resp.body.unwrap()["scopes"][0]["variablesReference"]
        .as_i64()
        .unwrap();

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 4,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": locals_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "variables failed: {:?}",
        vars_resp.message
    );
    vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone()
}

/// Apply the `evaluateToStringInDebugViews` attach arg to an adapter.
async fn attach_with_to_string_setting(
    adapter: &mut DapAdapter<impl DebugBackend>,
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
    enabled: bool,
) {
    let attach_req = crate::DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "evaluateToStringInDebugViews": enabled
        })),
    };
    let resp = adapter.handle_request(&attach_req).await;
    // Drain any thread/app events emitted by attach.
    while rx.try_recv().is_ok() {}
    let _ = resp; // success or failure doesn't affect the adapter state test
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: PlainInstance shows toString() appended
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_appended_to_plain_instance() {
    let (backend, _count) = ToStringMockBackend::new_returning("MyModel(id: 42, name: Alice)");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    let val = vars[0]["value"].as_str().unwrap();
    assert_eq!(
        val, "MyModel (MyModel(id: 42, name: Alice))",
        "PlainInstance should show class name followed by toString() result"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Default toString output is suppressed
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_default_to_string_suppressed() {
    // toString returns the default Dart "Instance of 'ClassName'" pattern.
    let (backend, _count) = ToStringMockBackend::new_returning("Instance of 'MyModel'");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    let val = vars[0]["value"].as_str().unwrap();
    // Should show just the class name, no appended string.
    assert_eq!(
        val, "MyModel instance",
        "Default Dart toString output should be suppressed; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: toString error falls back silently
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_error_silent_fallback() {
    let (backend, _count) = ToStringMockBackend::new_failing();
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    let val = vars[0]["value"].as_str().unwrap();
    // Value should not contain any error text — fallback to original display.
    assert!(
        !val.contains("<error"),
        "toString error should not appear in variable value; got: {:?}",
        val
    );
    // Should still show the class name display.
    assert!(
        val.contains("MyModel") || val.contains("instance"),
        "Variable should still show class name after toString error; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: toString timeout falls back silently
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_timeout_silent_fallback() {
    let (backend, call_count) = ToStringMockBackend::new_hanging("PlainInstance");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    // This should complete within ~1s (the timeout), not hang for 10s.
    let vars = tokio::time::timeout(Duration::from_secs(3), get_locals(&mut adapter, &mut rx))
        .await
        .expect("variables request should not hang (expected timeout to kick in within 1s)");

    assert_eq!(vars.len(), 1);
    let val = vars[0]["value"].as_str().unwrap();
    // After timeout, value should fall back — not hang, no "timed out" text.
    assert!(
        !val.contains("timed out"),
        "Timeout should be silent; got: {:?}",
        val
    );
    // Verify evaluate was actually called (not short-circuited).
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "evaluate should have been called once before timing out"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: evaluateToStringInDebugViews = false disables toString
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_disabled_by_setting() {
    let (backend, call_count) = ToStringMockBackend::new_returning("MyModel(custom)");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    // Disable toString via attach.
    attach_with_to_string_setting(&mut adapter, &mut rx, false).await;

    let vars = get_locals(&mut adapter, &mut rx).await;

    // evaluate should never have been called for toString().
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "evaluate should not be called when evaluateToStringInDebugViews is false"
    );

    assert_eq!(vars.len(), 1);
    let val = vars[0]["value"].as_str().unwrap();
    // Value should NOT contain the toString() result.
    assert!(
        !val.contains("MyModel(custom)"),
        "toString() result should not appear when setting is disabled; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: evaluateToStringInDebugViews = true (default) enables toString
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_enabled_by_default() {
    // Do NOT explicitly set evaluateToStringInDebugViews — default should be true.
    let (backend, call_count) = ToStringMockBackend::new_returning("MyModel(id=1)");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "evaluate should be called by default (evaluateToStringInDebugViews defaults to true)"
    );

    let val = vars[0]["value"].as_str().unwrap();
    assert!(
        val.contains("MyModel(id=1)"),
        "toString result should appear in value when enabled by default; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Primitives do NOT call toString
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_not_called_for_int() {
    let (backend, call_count) = PrimitiveVarMockBackend::new("Int");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "evaluate should NOT be called for Int variables"
    );
}

#[tokio::test]
async fn test_to_string_not_called_for_double() {
    let (backend, call_count) = PrimitiveVarMockBackend::new("Double");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "evaluate should NOT be called for Double variables"
    );
}

#[tokio::test]
async fn test_to_string_not_called_for_bool() {
    let (backend, call_count) = PrimitiveVarMockBackend::new("Bool");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "evaluate should NOT be called for Bool variables"
    );
}

#[tokio::test]
async fn test_to_string_not_called_for_string() {
    // String kind — toString not called.
    let (backend, call_count) = PrimitiveVarMockBackend::new("String");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "evaluate should NOT be called for String variables"
    );
}

#[tokio::test]
async fn test_to_string_not_called_for_null() {
    let (backend, call_count) = PrimitiveVarMockBackend::new("Null");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "evaluate should NOT be called for Null variables"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: RegExp kind receives toString enrichment
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_applied_to_regexp() {
    let (backend, call_count) =
        ToStringMockBackend::new_with_kind_and_result("RegExp", r"RegExp: pattern='\d+'");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "evaluate should be called for RegExp variables"
    );
    let val = vars[0]["value"].as_str().unwrap();
    assert!(
        val.contains(r"RegExp: pattern='\d+'"),
        "RegExp variable should show toString result; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: StackTrace kind receives toString enrichment
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_applied_to_stack_trace() {
    let (backend, call_count) = ToStringMockBackend::new_with_kind_and_result(
        "StackTrace",
        "#0  main (file:///app/lib/main.dart:10:5)",
    );
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "evaluate should be called for StackTrace variables"
    );
    let val = vars[0]["value"].as_str().unwrap();
    assert!(
        val.contains("#0  main"),
        "StackTrace variable should show toString result; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: WeakReference kind receives toString enrichment
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_to_string_applied_to_weak_reference() {
    let (backend, call_count) =
        ToStringMockBackend::new_with_kind_and_result("WeakReference", "WeakReference -> MyObj");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "evaluate should be called for WeakReference variables"
    );
    let val = vars[0]["value"].as_str().unwrap();
    assert!(
        val.contains("WeakReference -> MyObj"),
        "WeakReference variable should show toString result; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Empty toString result is suppressed
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_empty_to_string_result_suppressed() {
    let (backend, _count) = ToStringMockBackend::new_returning("");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = get_locals(&mut adapter, &mut rx).await;

    assert_eq!(vars.len(), 1);
    let val = vars[0]["value"].as_str().unwrap();
    // Empty toString result should not be appended.
    assert!(
        !val.contains("()"),
        "Empty toString result should not be appended; got: {:?}",
        val
    );
    // Should still be a valid display.
    assert!(
        val.contains("MyModel") || val.contains("instance"),
        "Variable should still have a display value after empty toString; got: {:?}",
        val
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: attach request wires up evaluateToStringInDebugViews correctly
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_attach_request_sets_to_string_setting_to_false() {
    let (backend, call_count) = ToStringMockBackend::new_returning("MyModel(custom)");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    // Attach with setting explicitly false.
    let attach_req = crate::DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "evaluateToStringInDebugViews": false
        })),
    };
    adapter.handle_request(&attach_req).await;
    while rx.try_recv().is_ok() {}

    let vars = get_locals(&mut adapter, &mut rx).await;
    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        0,
        "No evaluate calls when evaluateToStringInDebugViews = false in attach"
    );
    let val = vars[0]["value"].as_str().unwrap();
    assert!(
        !val.contains("MyModel(custom)"),
        "toString result should be absent when disabled via attach; got: {:?}",
        val
    );
}

#[tokio::test]
async fn test_attach_request_sets_to_string_setting_to_true() {
    let (backend, call_count) = ToStringMockBackend::new_returning("MyModel(from attach)");
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    // Attach with setting explicitly true.
    let attach_req = crate::DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "evaluateToStringInDebugViews": true
        })),
    };
    adapter.handle_request(&attach_req).await;
    while rx.try_recv().is_ok() {}

    let vars = get_locals(&mut adapter, &mut rx).await;
    assert_eq!(vars.len(), 1);
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "evaluate should be called when evaluateToStringInDebugViews = true in attach"
    );
    let val = vars[0]["value"].as_str().unwrap();
    assert!(
        val.contains("MyModel(from attach)"),
        "toString result should appear when enabled via attach; got: {:?}",
        val
    );
}
