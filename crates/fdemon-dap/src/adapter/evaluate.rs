//! # Expression Evaluation
//!
//! Implements the DAP `evaluate` request handler.
//!
//! ## Overview
//!
//! The `evaluate` request is sent by the debugger client to evaluate an
//! expression in the context of the current debug session. This handler
//! supports the following contexts:
//!
//! - `"repl"` — Debug console (REPL) evaluation
//! - `"hover"` — Tooltip evaluation when hovering over an expression
//! - `"watch"` — Watch panel evaluation
//! - `"clipboard"` — Copy value to clipboard
//!
//! ## Implementation
//!
//! Expression evaluation dispatches to either:
//!
//! - [`DebugBackend::evaluate_in_frame`] when a `frameId` is present — evaluates
//!   in the context of a specific stack frame (requires the isolate to be paused).
//! - [`DebugBackend::evaluate`] when no `frameId` is given — evaluates in the
//!   root library context of the most recently paused isolate.
//!
//! ## Result Formatting
//!
//! VM Service instance values are mapped to human-readable strings:
//!
//! - `Null` → `"null"`
//! - `Bool`, `Int`, `Double` → the value as a string
//! - `String` → the value in double quotes
//! - `List`, `Map`, `Set` → `"ClassName (length: N)"`
//! - Other objects → `"ClassName instance"` or `valueAsString` if available
//!
//! Complex objects (Lists, Maps, PlainInstances, etc.) get a non-zero
//! `variablesReference` so the client can expand them.

use crate::adapter::{DebugBackend, FrameStore, VariableRef, VariableStore};
use crate::protocol::types::{EvaluateArguments, EvaluateResponseBody};
use crate::{DapRequest, DapResponse};

// ─────────────────────────────────────────────────────────────────────────────
// Evaluate handler
// ─────────────────────────────────────────────────────────────────────────────

/// Handle an `evaluate` DAP request.
///
/// Parses the `EvaluateArguments`, resolves the isolate context, dispatches to
/// the backend, and formats the result.
///
/// # Errors
///
/// Returns a DAP error response (not a panic) for:
/// - No paused isolate available
/// - Invalid frame ID
/// - Evaluation errors from the VM Service (returned as user-visible error text)
pub async fn handle_evaluate<B: DebugBackend>(
    backend: &B,
    frame_store: &FrameStore,
    var_store: &mut VariableStore,
    most_recent_paused_isolate: Option<&str>,
    request: &DapRequest,
) -> DapResponse {
    let args: EvaluateArguments = match &request.arguments {
        Some(v) => match serde_json::from_value(v.clone()) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, format!("invalid arguments: {e}")),
        },
        None => return DapResponse::error(request, "'evaluate' request requires arguments"),
    };

    // Determine the isolate to evaluate in — use the most recently paused isolate.
    let isolate_id = match most_recent_paused_isolate {
        Some(id) => id.to_string(),
        None => return DapResponse::error(request, "No paused isolate available for evaluation"),
    };

    // Dispatch to backend depending on whether a frame context was provided.
    let result = if let Some(frame_id) = args.frame_id {
        // Evaluate in the context of a specific stack frame.
        let frame_ref = match frame_store.lookup(frame_id) {
            Some(fr) => fr.clone(),
            None => return DapResponse::error(request, "Invalid frame ID"),
        };

        backend
            .evaluate_in_frame(&isolate_id, frame_ref.frame_index, &args.expression)
            .await
    } else {
        // Evaluate in root library context (no frame).
        match get_root_library_id(backend, &isolate_id).await {
            Ok(lib_id) => {
                backend
                    .evaluate(&isolate_id, &lib_id, &args.expression)
                    .await
            }
            Err(e) => Err(e),
        }
    };

    match result {
        Ok(instance) => {
            let value = format_instance_value(&instance);
            let type_name = instance
                .get("class")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());

            // If the result is a complex object, make it expandable.
            let var_ref = if is_expandable(&instance) {
                if let Some(id) = instance.get("id").and_then(|i| i.as_str()) {
                    var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.clone(),
                        object_id: id.to_string(),
                    })
                } else {
                    0
                }
            } else {
                0
            };

            let body = EvaluateResponseBody {
                result: value,
                type_field: type_name,
                variables_reference: var_ref,
                named_variables: None,
                indexed_variables: None,
                presentation_hint: None,
            };
            let body_json = serde_json::to_value(&body).unwrap_or_default();
            DapResponse::success(request, Some(body_json))
        }
        Err(e) => {
            // Evaluation errors should NOT crash the session — return as a
            // DAP error response with the error message.
            //
            // The DAP spec allows either success=false (with message) or
            // success=true (with error in result field). We use success=false
            // so that both VS Code and Zed display a clear error to the user.
            DapResponse::error(request, e)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Root library resolution
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the root library ID for an isolate.
///
/// This calls `get_vm()` on the backend and searches the isolate list for the
/// matching isolate's `rootLib`. If the isolate is not found or has no
/// `rootLib`, returns an error.
///
/// # Note
///
/// This is a best-effort implementation for Phase 3. If `get_vm()` doesn't
/// include `rootLib` in the isolate refs, Phase 4 will add a `get_isolate()`
/// backend call that always returns the full isolate object with `rootLib`.
pub async fn get_root_library_id<B: DebugBackend>(
    backend: &B,
    isolate_id: &str,
) -> Result<String, String> {
    let vm_info = backend.get_vm().await?;
    let isolates = vm_info
        .get("isolates")
        .and_then(|i| i.as_array())
        .ok_or_else(|| "No isolates in VM info".to_string())?;

    for isolate in isolates {
        if isolate.get("id").and_then(|i| i.as_str()) == Some(isolate_id) {
            if let Some(root_lib) = isolate
                .get("rootLib")
                .and_then(|l| l.get("id"))
                .and_then(|i| i.as_str())
            {
                return Ok(root_lib.to_string());
            }
            // Found the isolate but no rootLib — fall through to error.
            break;
        }
    }
    Err("Could not find root library for isolate".to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Value formatting
// ─────────────────────────────────────────────────────────────────────────────

/// Format a VM Service instance value for DAP display.
///
/// Maps Dart VM instance kinds to human-readable strings suitable for display
/// in the debug console, hover tooltips, or watch panel.
pub fn format_instance_value(instance: &serde_json::Value) -> String {
    let kind = instance.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    let value_as_string = instance.get("valueAsString").and_then(|v| v.as_str());

    match kind {
        "Null" => "null".to_string(),
        "Bool" | "Int" | "Double" => value_as_string.unwrap_or("?").to_string(),
        "String" => {
            let s = value_as_string.unwrap_or("");
            format!("\"{}\"", s)
        }
        "List" | "Map" | "Set" => {
            let length = instance.get("length").and_then(|l| l.as_i64()).unwrap_or(0);
            let class_name = instance
                .get("class")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(kind);
            format!("{} (length: {})", class_name, length)
        }
        _ => value_as_string.map(|s| s.to_string()).unwrap_or_else(|| {
            let class_name = instance
                .get("class")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("Object");
            format!("{} instance", class_name)
        }),
    }
}

/// Check if a VM Service instance can be expanded (has children).
///
/// Returns `true` for complex types that can be drilled into via
/// `variablesRequest`. Primitive types (`Int`, `Bool`, `String`, etc.) and
/// `Null` return `false`.
pub fn is_expandable(instance: &serde_json::Value) -> bool {
    let kind = instance.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    matches!(
        kind,
        "List"
            | "Map"
            | "Set"
            | "PlainInstance"
            | "Closure"
            | "Uint8List"
            | "Uint8ClampedList"
            | "Int32List"
            | "Float64List"
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── format_instance_value ─────────────────────────────────────────────

    #[test]
    fn test_format_null() {
        let val = json!({"kind": "Null"});
        assert_eq!(format_instance_value(&val), "null");
    }

    #[test]
    fn test_format_bool_true() {
        let val = json!({"kind": "Bool", "valueAsString": "true"});
        assert_eq!(format_instance_value(&val), "true");
    }

    #[test]
    fn test_format_bool_false() {
        let val = json!({"kind": "Bool", "valueAsString": "false"});
        assert_eq!(format_instance_value(&val), "false");
    }

    #[test]
    fn test_format_int() {
        let val = json!({"kind": "Int", "valueAsString": "42"});
        assert_eq!(format_instance_value(&val), "42");
    }

    #[test]
    fn test_format_int_negative() {
        let val = json!({"kind": "Int", "valueAsString": "-7"});
        assert_eq!(format_instance_value(&val), "-7");
    }

    #[test]
    fn test_format_double() {
        let val = json!({"kind": "Double", "valueAsString": "3.14"});
        assert_eq!(format_instance_value(&val), "3.14");
    }

    #[test]
    fn test_format_string_quoted() {
        let val = json!({"kind": "String", "valueAsString": "hello"});
        assert_eq!(format_instance_value(&val), "\"hello\"");
    }

    #[test]
    fn test_format_string_empty() {
        let val = json!({"kind": "String", "valueAsString": ""});
        assert_eq!(format_instance_value(&val), "\"\"");
    }

    #[test]
    fn test_format_string_missing_value() {
        let val = json!({"kind": "String"});
        assert_eq!(format_instance_value(&val), "\"\"");
    }

    #[test]
    fn test_format_list_with_length() {
        let val = json!({"kind": "List", "length": 5, "class": {"name": "List<String>"}});
        assert_eq!(format_instance_value(&val), "List<String> (length: 5)");
    }

    #[test]
    fn test_format_list_no_class_uses_kind() {
        let val = json!({"kind": "List", "length": 3});
        assert_eq!(format_instance_value(&val), "List (length: 3)");
    }

    #[test]
    fn test_format_map_with_length() {
        let val = json!({"kind": "Map", "length": 2, "class": {"name": "Map<String, int>"}});
        assert_eq!(format_instance_value(&val), "Map<String, int> (length: 2)");
    }

    #[test]
    fn test_format_set_with_length() {
        let val = json!({"kind": "Set", "length": 4, "class": {"name": "Set<int>"}});
        assert_eq!(format_instance_value(&val), "Set<int> (length: 4)");
    }

    #[test]
    fn test_format_list_zero_length() {
        let val = json!({"kind": "List", "class": {"name": "List"}});
        assert_eq!(format_instance_value(&val), "List (length: 0)");
    }

    #[test]
    fn test_format_plain_instance_with_value_as_string() {
        let val = json!({
            "kind": "PlainInstance",
            "class": {"name": "MyClass"},
            "valueAsString": "MyClass@0x1234"
        });
        assert_eq!(format_instance_value(&val), "MyClass@0x1234");
    }

    #[test]
    fn test_format_plain_instance_no_value_as_string() {
        let val = json!({"kind": "PlainInstance", "class": {"name": "MyClass"}});
        assert_eq!(format_instance_value(&val), "MyClass instance");
    }

    #[test]
    fn test_format_unknown_kind_with_class_name() {
        let val = json!({"kind": "SomeOtherKind", "class": {"name": "FancyType"}});
        assert_eq!(format_instance_value(&val), "FancyType instance");
    }

    #[test]
    fn test_format_unknown_kind_no_class() {
        let val = json!({"kind": "SomeOtherKind"});
        assert_eq!(format_instance_value(&val), "Object instance");
    }

    #[test]
    fn test_format_missing_kind() {
        // No kind field — falls through to the wildcard arm.
        let val = json!({"class": {"name": "Something"}});
        assert_eq!(format_instance_value(&val), "Something instance");
    }

    #[test]
    fn test_format_int_missing_value_as_string() {
        let val = json!({"kind": "Int"});
        assert_eq!(format_instance_value(&val), "?");
    }

    // ── is_expandable ─────────────────────────────────────────────────────

    #[test]
    fn test_is_expandable_list() {
        assert!(is_expandable(&json!({"kind": "List"})));
    }

    #[test]
    fn test_is_expandable_map() {
        assert!(is_expandable(&json!({"kind": "Map"})));
    }

    #[test]
    fn test_is_expandable_set() {
        assert!(is_expandable(&json!({"kind": "Set"})));
    }

    #[test]
    fn test_is_expandable_plain_instance() {
        assert!(is_expandable(&json!({"kind": "PlainInstance"})));
    }

    #[test]
    fn test_is_expandable_closure() {
        assert!(is_expandable(&json!({"kind": "Closure"})));
    }

    #[test]
    fn test_is_expandable_typed_lists() {
        assert!(is_expandable(&json!({"kind": "Uint8List"})));
        assert!(is_expandable(&json!({"kind": "Uint8ClampedList"})));
        assert!(is_expandable(&json!({"kind": "Int32List"})));
        assert!(is_expandable(&json!({"kind": "Float64List"})));
    }

    #[test]
    fn test_is_expandable_primitive() {
        assert!(!is_expandable(&json!({"kind": "Int"})));
        assert!(!is_expandable(&json!({"kind": "Double"})));
        assert!(!is_expandable(&json!({"kind": "Bool"})));
        assert!(!is_expandable(&json!({"kind": "String"})));
        assert!(!is_expandable(&json!({"kind": "Null"})));
    }

    #[test]
    fn test_is_expandable_unknown_kind_returns_false() {
        assert!(!is_expandable(&json!({"kind": "WeirdThing"})));
    }

    #[test]
    fn test_is_expandable_missing_kind_returns_false() {
        assert!(!is_expandable(&json!({})));
    }

    // ── handle_evaluate (integration-style) ──────────────────────────────

    use crate::adapter::{BreakpointResult, FrameRef, StepMode};
    use crate::protocol::types::StepArguments;

    struct MockBackend {
        eval_result: Result<serde_json::Value, String>,
    }

    impl MockBackend {
        fn ok(val: serde_json::Value) -> Self {
            Self {
                eval_result: Ok(val),
            }
        }
        fn err(msg: &str) -> Self {
            Self {
                eval_result: Err(msg.to_string()),
            }
        }
    }

    impl crate::adapter::DebugBackend for MockBackend {
        async fn pause(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), String> {
            Ok(())
        }
        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            line: i32,
            column: Option<i32>,
        ) -> Result<BreakpointResult, String> {
            Ok(BreakpointResult {
                vm_id: format!("bp/line:{line}"),
                resolved: true,
                line: Some(line),
                column,
            })
        }
        async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        async fn set_exception_pause_mode(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        async fn get_stack(&self, _: &str, _: Option<i32>) -> Result<serde_json::Value, String> {
            Ok(json!({}))
        }
        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> Result<serde_json::Value, String> {
            Ok(json!({}))
        }
        async fn evaluate(&self, _: &str, _: &str, _: &str) -> Result<serde_json::Value, String> {
            self.eval_result.clone()
        }
        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> Result<serde_json::Value, String> {
            self.eval_result.clone()
        }
        async fn get_vm(&self) -> Result<serde_json::Value, String> {
            // Return an isolate with a rootLib so that frameless evaluation works.
            Ok(json!({
                "isolates": [
                    {
                        "id": "isolates/1",
                        "name": "main",
                        "rootLib": {"id": "libraries/1"}
                    }
                ]
            }))
        }
        async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, String> {
            Ok(json!({}))
        }
    }

    fn make_request_with_args(seq: i64, command: &str, args: serde_json::Value) -> DapRequest {
        DapRequest {
            seq,
            command: command.into(),
            arguments: Some(args),
        }
    }

    #[tokio::test]
    async fn test_evaluate_no_paused_isolate_returns_error() {
        let backend = MockBackend::ok(json!({"kind": "Int", "valueAsString": "1"}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "1 + 1"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            None, // no paused isolate
            &req,
        )
        .await;

        assert!(!resp.success);
        assert!(
            resp.message
                .as_deref()
                .unwrap_or("")
                .contains("No paused isolate"),
            "Expected error about no paused isolate, got: {:?}",
            resp.message
        );
    }

    #[tokio::test]
    async fn test_evaluate_invalid_frame_id_returns_error() {
        let backend = MockBackend::ok(json!({"kind": "Int", "valueAsString": "99"}));
        let frame_store = FrameStore::new(); // empty — no frames allocated
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "x", "frameId": 999}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(!resp.success);
        assert!(
            resp.message
                .as_deref()
                .unwrap_or("")
                .contains("Invalid frame ID"),
            "Expected 'Invalid frame ID' error, got: {:?}",
            resp.message
        );
    }

    #[tokio::test]
    async fn test_evaluate_primitive_no_frame_id() {
        let backend = MockBackend::ok(json!({"kind": "Int", "valueAsString": "42"}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "answer"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "Expected success, got: {:?}", resp.message);
        let body = resp.body.as_ref().expect("Expected response body");
        assert_eq!(body["result"], "42");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_evaluate_string_is_quoted() {
        let backend = MockBackend::ok(json!({"kind": "String", "valueAsString": "hello world"}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "myStr"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().expect("Expected body");
        assert_eq!(body["result"], "\"hello world\"");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_evaluate_list_is_expandable() {
        let backend = MockBackend::ok(json!({
            "kind": "List",
            "id": "objects/list1",
            "length": 3,
            "class": {"name": "List<int>"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "myList"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().expect("Expected body");
        assert_eq!(body["result"], "List<int> (length: 3)");
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "List should be expandable (variablesReference > 0)"
        );
    }

    #[tokio::test]
    async fn test_evaluate_with_frame_id() {
        let backend = MockBackend::ok(json!({"kind": "Bool", "valueAsString": "true"}));
        let mut frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        // Allocate a frame so the lookup succeeds.
        let frame_id = frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "flag", "frameId": frame_id}),
        );

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "Expected success, got: {:?}", resp.message);
        let body = resp.body.as_ref().expect("Expected body");
        assert_eq!(body["result"], "true");
    }

    #[tokio::test]
    async fn test_evaluate_error_from_backend_returns_dap_error() {
        let backend = MockBackend::err("Cannot evaluate expression: isolate not paused");
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "bad"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(!resp.success);
        assert!(
            resp.message
                .as_deref()
                .unwrap_or("")
                .contains("Cannot evaluate"),
            "Expected backend error message, got: {:?}",
            resp.message
        );
    }

    #[tokio::test]
    async fn test_evaluate_null_result() {
        let backend = MockBackend::ok(json!({"kind": "Null"}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "nothing"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().expect("Expected body");
        assert_eq!(body["result"], "null");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_evaluate_plain_instance_expandable() {
        let backend = MockBackend::ok(json!({
            "kind": "PlainInstance",
            "id": "objects/inst1",
            "class": {"name": "MyClass"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(1, "evaluate", json!({"expression": "obj"}));

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().expect("Expected body");
        assert_eq!(body["result"], "MyClass instance");
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "PlainInstance should be expandable"
        );
    }

    #[tokio::test]
    async fn test_evaluate_missing_arguments_returns_error() {
        let backend = MockBackend::ok(json!({}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = DapRequest {
            seq: 1,
            command: "evaluate".into(),
            arguments: None,
        };

        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(!resp.success);
    }

    // ── get_root_library_id ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_root_library_id_found() {
        let backend = MockBackend::ok(json!({})); // eval_result unused here
        let result = get_root_library_id(&backend, "isolates/1").await;
        assert_eq!(result, Ok("libraries/1".to_string()));
    }

    #[tokio::test]
    async fn test_get_root_library_id_not_found() {
        let backend = MockBackend::ok(json!({}));
        let result = get_root_library_id(&backend, "isolates/999").await;
        assert!(result.is_err());
    }
}
