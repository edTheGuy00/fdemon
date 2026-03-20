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
//! - `"variables"` — Sub-expression evaluation from the variable panel
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
//! ## Context Dispatching
//!
//! After evaluation, the result is post-processed based on the request context:
//!
//! | Context | Behavior |
//! |---------|----------|
//! | `"hover"` | Calls `toString()` on non-primitives, truncates long strings, `variablesReference: 0` |
//! | `"watch"` | Structured result with `variablesReference > 0` for expandable objects |
//! | `"variables"` | Same as watch — sub-expressions from the variable view |
//! | `"repl"` | Full evaluation, multi-line OK, type info included |
//! | `"clipboard"` | No truncation, full representation, `variablesReference: 0` |
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

use crate::adapter::{BackendError, DebugBackend, FrameStore, VariableRef, VariableStore};
use crate::protocol::types::{EvaluateArguments, EvaluateResponseBody};
use crate::{DapRequest, DapResponse};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum string length for hover tooltips.
///
/// Hover tooltips should be short and readable. Long strings are truncated
/// to this length with a `…` suffix so the IDE tooltip remains compact.
const HOVER_MAX_LEN: usize = 100;

// ─────────────────────────────────────────────────────────────────────────────
// Evaluation context
// ─────────────────────────────────────────────────────────────────────────────

/// The context in which an `evaluate` request was made.
///
/// Parsed from the `context` field of [`EvaluateArguments`]. Controls how
/// the result is formatted and whether `variablesReference` is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalContext {
    /// Hovering over an expression in the editor — short, readable, no expansion.
    Hover,
    /// Watch panel — structured result with expandable references.
    Watch,
    /// Variable panel sub-expression — same behaviour as `Watch`.
    Variables,
    /// Debug console (REPL) — full output, side effects allowed.
    Repl,
    /// Clipboard copy — full representation, no truncation, no expansion.
    Clipboard,
    /// No context provided (default) — behaves like `Watch`.
    Unknown,
}

impl EvalContext {
    /// Parse a DAP context string into an [`EvalContext`].
    pub fn parse(s: &str) -> Self {
        match s {
            "hover" => EvalContext::Hover,
            "watch" => EvalContext::Watch,
            "variables" => EvalContext::Variables,
            "repl" => EvalContext::Repl,
            "clipboard" => EvalContext::Clipboard,
            _ => EvalContext::Unknown,
        }
    }

    /// Returns `true` for contexts where `variablesReference` should always be 0
    /// (i.e. the IDE must not try to expand children).
    pub fn suppress_variables_reference(self) -> bool {
        matches!(self, EvalContext::Hover | EvalContext::Clipboard)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Evaluate handler
// ─────────────────────────────────────────────────────────────────────────────

/// Handle an `evaluate` DAP request.
///
/// Parses the `EvaluateArguments`, resolves the isolate context, dispatches to
/// the backend, and formats the result according to the evaluation context.
///
/// # Context Behaviour
///
/// - **hover** — Calls `toString()` on non-primitive results. Truncates strings
///   longer than [`HOVER_MAX_LEN`]. Sets `variablesReference: 0`.
/// - **watch / variables** — Returns a structured result. Sets
///   `variablesReference > 0` for expandable objects so the IDE can drill in.
/// - **repl** — Full evaluation output. Multi-line strings are returned as-is.
///   `variablesReference` set for expandable objects.
/// - **clipboard** — Full representation with no truncation. `variablesReference: 0`.
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

    // Parse the evaluation context.
    let context = args
        .context
        .as_deref()
        .map(EvalContext::parse)
        .unwrap_or(EvalContext::Unknown);

    // Resolve the stack frame (if provided).
    let frame_ref = if let Some(frame_id) = args.frame_id {
        match frame_store.lookup(frame_id) {
            Some(fr) => Some(fr.clone()),
            None => return DapResponse::error(request, "Invalid frame ID"),
        }
    } else {
        None
    };

    // For hover context, use the enhanced path that calls toString() on objects.
    if context == EvalContext::Hover {
        return handle_evaluate_hover(backend, var_store, &isolate_id, frame_ref, &args, request)
            .await;
    }

    // Standard evaluation path for all other contexts.
    let result = evaluate_expression(backend, &isolate_id, frame_ref.as_ref(), &args).await;

    match result {
        Ok(instance) => {
            let value = format_instance_value(&instance);
            let type_name = instance
                .get("class")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());

            // clipboard context: no expansion even for complex types.
            // repl/watch/variables/unknown: expand complex objects.
            let var_ref = if context.suppress_variables_reference() {
                0
            } else if is_expandable(&instance) {
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

            // For clipboard context apply no truncation — use the raw formatted value.
            // For repl context also apply no truncation.
            // For watch/variables/unknown use the standard formatted value (already truncation-free
            // at the format_instance_value level; IDEs handle overflow themselves).
            let display_value = value;

            let body = EvaluateResponseBody {
                result: display_value,
                type_field: type_name,
                variables_reference: var_ref,
                named_variables: None,
                indexed_variables: None,
                presentation_hint: None,
            };
            let body_json = match serde_json::to_value(&body) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to serialize evaluate response: {}", e);
                    return DapResponse::error(request, format!("Internal error: {}", e));
                }
            };
            DapResponse::success(request, Some(body_json))
        }
        Err(e) => {
            // Evaluation errors should NOT crash the session — return as a
            // DAP error response with the error message.
            //
            // The DAP spec allows either success=false (with message) or
            // success=true (with error in result field). We use success=false
            // so that both VS Code and Zed display a clear error to the user.
            DapResponse::error(request, e.to_string())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Hover-specific evaluation
// ─────────────────────────────────────────────────────────────────────────────

/// Handle an `evaluate` request in the `"hover"` context.
///
/// For hover tooltips:
/// - Primitive types (`Int`, `Double`, `Bool`, `String`, `Null`) are formatted
///   directly from `valueAsString`.
/// - Non-primitive types trigger a secondary `toString()` evaluation to obtain
///   a readable representation. If `toString()` fails, falls back to the raw
///   `format_instance_value` result.
/// - Long strings are truncated to [`HOVER_MAX_LEN`] characters with a `…` suffix.
/// - `variablesReference` is always `0` (no expansion for hover tooltips).
async fn handle_evaluate_hover<B: DebugBackend>(
    backend: &B,
    _var_store: &mut VariableStore,
    isolate_id: &str,
    frame_ref: Option<crate::adapter::FrameRef>,
    args: &EvaluateArguments,
    request: &DapRequest,
) -> DapResponse {
    let result = evaluate_expression_raw(backend, isolate_id, frame_ref.as_ref(), args).await;

    let instance = match result {
        Ok(v) => v,
        Err(e) => return DapResponse::error(request, e.to_string()),
    };

    let kind = instance.get("kind").and_then(|k| k.as_str()).unwrap_or("");

    let display_value = if is_primitive_kind(kind) {
        // Primitives: use valueAsString directly (no toString() call needed).
        format_instance_value(&instance)
    } else {
        // Non-primitives: call toString() for a readable hover tooltip.
        let to_string_expr = format!("({}).toString()", args.expression);
        let to_string_args = EvaluateArguments {
            expression: to_string_expr,
            frame_id: args.frame_id,
            context: args.context.clone(),
        };
        match evaluate_expression_raw(backend, isolate_id, frame_ref.as_ref(), &to_string_args)
            .await
        {
            Ok(str_result) => format_instance_value(&str_result),
            // If toString() fails, fall back to the type name/valueAsString.
            Err(_) => format_instance_value(&instance),
        }
    };

    // Truncate long hover strings for IDE tooltip compactness.
    let display_value = truncate_for_hover(display_value);

    let type_name = instance
        .get("class")
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    let body = EvaluateResponseBody {
        result: display_value,
        type_field: type_name,
        // Hover never expands — variablesReference must be 0.
        variables_reference: 0,
        named_variables: None,
        indexed_variables: None,
        presentation_hint: None,
    };
    let body_json = match serde_json::to_value(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to serialize hover evaluate response: {}", e);
            return DapResponse::error(request, format!("Internal error: {}", e));
        }
    };
    DapResponse::success(request, Some(body_json))
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend dispatch helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate an expression using the appropriate backend method.
///
/// If `frame_ref` is `Some`, delegates to `evaluate_in_frame`. Otherwise
/// resolves the root library and delegates to `evaluate`.
async fn evaluate_expression<B: DebugBackend>(
    backend: &B,
    isolate_id: &str,
    frame_ref: Option<&crate::adapter::FrameRef>,
    args: &EvaluateArguments,
) -> Result<serde_json::Value, BackendError> {
    evaluate_expression_raw(backend, isolate_id, frame_ref, args).await
}

/// Inner dispatch — resolves frame vs. root library and calls backend.
async fn evaluate_expression_raw<B: DebugBackend>(
    backend: &B,
    isolate_id: &str,
    frame_ref: Option<&crate::adapter::FrameRef>,
    args: &EvaluateArguments,
) -> Result<serde_json::Value, BackendError> {
    if let Some(fr) = frame_ref {
        backend
            .evaluate_in_frame(isolate_id, fr.frame_index, &args.expression)
            .await
    } else {
        match get_root_library_id(backend, isolate_id).await {
            Ok(lib_id) => {
                backend
                    .evaluate(isolate_id, &lib_id, &args.expression)
                    .await
            }
            Err(e) => Err(BackendError::VmServiceError(e)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Root library resolution
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the root library ID for an isolate.
///
/// Calls `get_isolate()` on the backend and reads `isolate.rootLib.id`.
/// This is the reliable approach: `getIsolate` always returns the full isolate
/// object with `rootLib`, whereas the isolate refs embedded in `getVM()` may
/// omit `rootLib` depending on the Dart VM version.
pub async fn get_root_library_id<B: DebugBackend>(
    backend: &B,
    isolate_id: &str,
) -> Result<String, String> {
    let isolate = backend
        .get_isolate(isolate_id)
        .await
        .map_err(|e| e.to_string())?;

    isolate
        .get("rootLib")
        .and_then(|lib| lib.get("id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Isolate has no rootLib".to_string())
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
            | "Record"
            | "WeakReference"
    )
}

/// Check if a VM Service instance kind is a Dart primitive.
///
/// Primitives are directly displayable from `valueAsString` and do not
/// benefit from calling `toString()` during hover evaluation.
fn is_primitive_kind(kind: &str) -> bool {
    matches!(kind, "Int" | "Double" | "Bool" | "String" | "Null")
}

/// Truncate a string to [`HOVER_MAX_LEN`] characters for hover display.
///
/// If the value exceeds the limit, a `…` character is appended to signal
/// truncation. Characters are counted by Unicode scalar values (not bytes).
fn truncate_for_hover(value: String) -> String {
    let char_count = value.chars().count();
    if char_count <= HOVER_MAX_LEN {
        value
    } else {
        let truncated: String = value.chars().take(HOVER_MAX_LEN).collect();
        format!("{}…", truncated)
    }
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

    // ── EvalContext ────────────────────────────────────────────────────────

    #[test]
    fn test_eval_context_from_str_hover() {
        assert_eq!(EvalContext::parse("hover"), EvalContext::Hover);
    }

    #[test]
    fn test_eval_context_from_str_watch() {
        assert_eq!(EvalContext::parse("watch"), EvalContext::Watch);
    }

    #[test]
    fn test_eval_context_from_str_variables() {
        assert_eq!(EvalContext::parse("variables"), EvalContext::Variables);
    }

    #[test]
    fn test_eval_context_from_str_repl() {
        assert_eq!(EvalContext::parse("repl"), EvalContext::Repl);
    }

    #[test]
    fn test_eval_context_from_str_clipboard() {
        assert_eq!(EvalContext::parse("clipboard"), EvalContext::Clipboard);
    }

    #[test]
    fn test_eval_context_from_str_unknown() {
        assert_eq!(EvalContext::parse("foobar"), EvalContext::Unknown);
        assert_eq!(EvalContext::parse(""), EvalContext::Unknown);
        assert_eq!(EvalContext::parse("HOVER"), EvalContext::Unknown);
    }

    #[test]
    fn test_eval_context_suppress_variables_reference() {
        // Hover and clipboard always set variablesReference: 0.
        assert!(EvalContext::Hover.suppress_variables_reference());
        assert!(EvalContext::Clipboard.suppress_variables_reference());
        // Other contexts do not suppress.
        assert!(!EvalContext::Watch.suppress_variables_reference());
        assert!(!EvalContext::Variables.suppress_variables_reference());
        assert!(!EvalContext::Repl.suppress_variables_reference());
        assert!(!EvalContext::Unknown.suppress_variables_reference());
    }

    // ── truncate_for_hover ────────────────────────────────────────────────

    #[test]
    fn test_truncate_for_hover_short_string_unchanged() {
        let s = "hello".to_string();
        assert_eq!(truncate_for_hover(s), "hello");
    }

    #[test]
    fn test_truncate_for_hover_exactly_at_limit_unchanged() {
        let s: String = "x".repeat(HOVER_MAX_LEN);
        let result = truncate_for_hover(s.clone());
        assert_eq!(result, s);
        assert!(!result.contains('…'));
    }

    #[test]
    fn test_truncate_for_hover_over_limit_appends_ellipsis() {
        let s: String = "x".repeat(HOVER_MAX_LEN + 1);
        let result = truncate_for_hover(s);
        assert!(result.ends_with('…'));
        // The result should be HOVER_MAX_LEN chars + the ellipsis character.
        assert_eq!(result.chars().count(), HOVER_MAX_LEN + 1);
    }

    #[test]
    fn test_truncate_for_hover_empty_string_unchanged() {
        assert_eq!(truncate_for_hover(String::new()), "");
    }

    // ── is_primitive_kind ─────────────────────────────────────────────────

    #[test]
    fn test_is_primitive_kind_true() {
        assert!(is_primitive_kind("Int"));
        assert!(is_primitive_kind("Double"));
        assert!(is_primitive_kind("Bool"));
        assert!(is_primitive_kind("String"));
        assert!(is_primitive_kind("Null"));
    }

    #[test]
    fn test_is_primitive_kind_false_for_objects() {
        assert!(!is_primitive_kind("PlainInstance"));
        assert!(!is_primitive_kind("List"));
        assert!(!is_primitive_kind("Map"));
        assert!(!is_primitive_kind("Closure"));
        assert!(!is_primitive_kind(""));
    }

    // ── handle_evaluate (integration-style) ──────────────────────────────

    use crate::adapter::{
        BackendError, BreakpointResult, DapExceptionPauseMode, FrameRef, StepMode,
    };

    /// A flexible mock backend for context-dispatching tests.
    ///
    /// Supports configuring separate responses for the primary expression
    /// and a secondary `toString()` call so hover tests can verify both paths.
    struct MockBackend {
        /// Result for the primary expression evaluation.
        eval_result: Result<serde_json::Value, BackendError>,
        /// Optional result for expressions ending in `.toString()`.
        to_string_result: Option<Result<serde_json::Value, BackendError>>,
    }

    impl MockBackend {
        fn ok(val: serde_json::Value) -> Self {
            Self {
                eval_result: Ok(val),
                to_string_result: None,
            }
        }
        fn err(msg: &str) -> Self {
            Self {
                eval_result: Err(BackendError::VmServiceError(msg.to_string())),
                to_string_result: None,
            }
        }
        /// Configure a separate response for `toString()` calls.
        fn with_to_string(mut self, val: serde_json::Value) -> Self {
            self.to_string_result = Some(Ok(val));
            self
        }
        /// Configure a failing `toString()` call.
        fn with_to_string_err(mut self, msg: &str) -> Self {
            self.to_string_result = Some(Err(BackendError::VmServiceError(msg.to_string())));
            self
        }
    }

    impl crate::adapter::DebugBackend for MockBackend {
        async fn pause(&self, _: &str) -> Result<(), BackendError> {
            Ok(())
        }
        async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), BackendError> {
            Ok(())
        }
        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            line: i32,
            column: Option<i32>,
        ) -> Result<BreakpointResult, BackendError> {
            Ok(BreakpointResult {
                vm_id: format!("bp/line:{line}"),
                resolved: true,
                line: Some(line),
                column,
            })
        }
        async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), BackendError> {
            Ok(())
        }
        async fn set_exception_pause_mode(
            &self,
            _: &str,
            _: DapExceptionPauseMode,
        ) -> Result<(), BackendError> {
            Ok(())
        }
        async fn get_stack(
            &self,
            _: &str,
            _: Option<i32>,
        ) -> Result<serde_json::Value, BackendError> {
            Ok(json!({}))
        }
        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> Result<serde_json::Value, BackendError> {
            Ok(json!({}))
        }
        async fn evaluate(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<serde_json::Value, BackendError> {
            self.eval_result.clone()
        }
        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            expression: &str,
        ) -> Result<serde_json::Value, BackendError> {
            // If a to_string_result is configured and the expression ends with
            // `.toString()`, return that result instead of the primary result.
            if expression.ends_with(".toString()") {
                if let Some(ref r) = self.to_string_result {
                    return r.clone();
                }
            }
            self.eval_result.clone()
        }
        async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
            Ok(json!({
                "isolates": [
                    {
                        "id": "isolates/1",
                        "name": "main",
                    }
                ]
            }))
        }
        async fn get_isolate(&self, _: &str) -> Result<serde_json::Value, BackendError> {
            // Return a full isolate object with rootLib so get_root_library_id works.
            Ok(json!({
                "id": "isolates/1",
                "name": "main",
                "rootLib": {"id": "libraries/1"}
            }))
        }

        async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, BackendError> {
            Ok(json!({}))
        }

        async fn call_service(
            &self,
            _: &str,
            _: Option<serde_json::Value>,
        ) -> Result<serde_json::Value, BackendError> {
            Ok(json!({}))
        }

        async fn set_library_debuggable(
            &self,
            _: &str,
            _: &str,
            _: bool,
        ) -> Result<(), BackendError> {
            Ok(())
        }

        async fn get_source_report(
            &self,
            _: &str,
            _: &str,
            _: &[&str],
            _: Option<i64>,
            _: Option<i64>,
        ) -> Result<serde_json::Value, BackendError> {
            Ok(json!({}))
        }

        async fn get_source(&self, _: &str, _: &str) -> std::result::Result<String, String> {
            Ok(String::new())
        }

        async fn hot_reload(&self) -> Result<(), BackendError> {
            Ok(())
        }

        async fn hot_restart(&self) -> Result<(), BackendError> {
            Ok(())
        }

        async fn stop_app(&self) -> Result<(), BackendError> {
            Ok(())
        }

        async fn ws_uri(&self) -> Option<String> {
            None
        }

        async fn device_id(&self) -> Option<String> {
            None
        }

        async fn build_mode(&self) -> String {
            "debug".to_string()
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
        // Use a backend that returns an isolate without rootLib to test the
        // error path (get_isolate succeeds but rootLib is absent).
        struct NoRootLibBackend;
        impl crate::adapter::DebugBackend for NoRootLibBackend {
            async fn pause(&self, _: &str) -> Result<(), BackendError> {
                Ok(())
            }
            async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), BackendError> {
                Ok(())
            }
            async fn add_breakpoint(
                &self,
                _: &str,
                _: &str,
                l: i32,
                c: Option<i32>,
            ) -> Result<BreakpointResult, BackendError> {
                Ok(BreakpointResult {
                    vm_id: format!("bp/{l}"),
                    resolved: true,
                    line: Some(l),
                    column: c,
                })
            }
            async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), BackendError> {
                Ok(())
            }
            async fn set_exception_pause_mode(
                &self,
                _: &str,
                _: DapExceptionPauseMode,
            ) -> Result<(), BackendError> {
                Ok(())
            }
            async fn get_stack(
                &self,
                _: &str,
                _: Option<i32>,
            ) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn get_object(
                &self,
                _: &str,
                _: &str,
                _: Option<i64>,
                _: Option<i64>,
            ) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn evaluate(
                &self,
                _: &str,
                _: &str,
                _: &str,
            ) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn evaluate_in_frame(
                &self,
                _: &str,
                _: i32,
                _: &str,
            ) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn get_isolate(&self, _: &str) -> Result<serde_json::Value, BackendError> {
                // Return isolate without rootLib — this triggers the error path.
                Ok(json!({"id": "isolates/1", "name": "main"}))
            }
            async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn call_service(
                &self,
                _: &str,
                _: Option<serde_json::Value>,
            ) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn set_library_debuggable(
                &self,
                _: &str,
                _: &str,
                _: bool,
            ) -> Result<(), BackendError> {
                Ok(())
            }
            async fn get_source_report(
                &self,
                _: &str,
                _: &str,
                _: &[&str],
                _: Option<i64>,
                _: Option<i64>,
            ) -> Result<serde_json::Value, BackendError> {
                Ok(json!({}))
            }
            async fn get_source(&self, _: &str, _: &str) -> Result<String, String> {
                Ok(String::new())
            }
            async fn hot_reload(&self) -> Result<(), BackendError> {
                Ok(())
            }
            async fn hot_restart(&self) -> Result<(), BackendError> {
                Ok(())
            }
            async fn stop_app(&self) -> Result<(), BackendError> {
                Ok(())
            }
            async fn ws_uri(&self) -> Option<String> {
                None
            }
            async fn device_id(&self) -> Option<String> {
                None
            }
            async fn build_mode(&self) -> String {
                "debug".to_string()
            }
        }
        let backend = NoRootLibBackend;
        let result = get_root_library_id(&backend, "isolates/1").await;
        assert!(result.is_err(), "Should fail when isolate has no rootLib");
    }

    // ── Context-specific tests (Task 06) ──────────────────────────────────

    #[tokio::test]
    async fn test_hover_context_primitive_int_no_to_string() {
        // For primitive Int, hover should return the value directly.
        // The MockBackend eval_result is the primitive; the to_string_result
        // would not be called for primitives.
        let backend = MockBackend::ok(json!({"kind": "Int", "valueAsString": "42"}));
        let mut frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();
        let frame_id = frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "x", "frameId": frame_id, "context": "hover"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "hover primitive should succeed");
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "42");
        assert_eq!(body["variablesReference"], 0, "hover must not expand");
    }

    #[tokio::test]
    async fn test_hover_context_bool_primitive_no_to_string() {
        let backend = MockBackend::ok(json!({"kind": "Bool", "valueAsString": "false"}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "flag", "context": "hover"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "false");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_hover_context_object_calls_to_string() {
        // When the primary result is a PlainInstance, hover calls toString().
        let backend = MockBackend::ok(json!({
            "kind": "PlainInstance",
            "id": "objects/1",
            "class": {"name": "MyWidget"}
        }))
        .with_to_string(json!({"kind": "String", "valueAsString": "MyWidget(42)"}));

        let mut frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();
        let frame_id = frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "widget", "frameId": frame_id, "context": "hover"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(
            resp.success,
            "hover object should succeed: {:?}",
            resp.message
        );
        let body = resp.body.as_ref().unwrap();
        // The toString() result is "MyWidget(42)" — formatted as a String kind → quoted.
        assert_eq!(body["result"], "\"MyWidget(42)\"");
        assert_eq!(body["variablesReference"], 0, "hover must not expand");
    }

    #[tokio::test]
    async fn test_hover_context_to_string_failure_falls_back() {
        // When toString() fails, hover falls back to the original instance format.
        let backend = MockBackend::ok(json!({
            "kind": "PlainInstance",
            "id": "objects/2",
            "class": {"name": "BrokenClass"}
        }))
        .with_to_string_err("toString() threw");

        let mut frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();
        let frame_id = frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "broken", "frameId": frame_id, "context": "hover"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "hover fallback should succeed");
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "BrokenClass instance");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_hover_context_long_string_is_truncated() {
        // A string longer than HOVER_MAX_LEN should be truncated with '…'.
        let long_value = "a".repeat(HOVER_MAX_LEN + 50);
        let backend = MockBackend::ok(json!({
            "kind": "PlainInstance",
            "id": "objects/3",
            "class": {"name": "LongClass"}
        }))
        .with_to_string(json!({"kind": "String", "valueAsString": long_value}));

        let mut frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();
        let frame_id = frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "longObj", "frameId": frame_id, "context": "hover"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        let result = body["result"].as_str().unwrap();
        assert!(
            result.ends_with('…'),
            "Long hover result should end with ellipsis"
        );
        // The quoted string result should still be <= HOVER_MAX_LEN chars + ellipsis.
        assert!(
            result.chars().count() <= HOVER_MAX_LEN + 1 + 2, // +2 for quotes
            "Truncated result should not be excessively long, got len={}",
            result.chars().count()
        );
    }

    #[tokio::test]
    async fn test_watch_context_object_provides_variables_reference() {
        // Watch context should return variablesReference > 0 for expandable objects.
        let backend = MockBackend::ok(json!({
            "kind": "PlainInstance",
            "id": "objects/watch1",
            "class": {"name": "Config"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "config", "context": "watch"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "watch context should succeed");
        let body = resp.body.as_ref().unwrap();
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "watch context must expose variablesReference > 0 for objects"
        );
    }

    #[tokio::test]
    async fn test_variables_context_object_provides_variables_reference() {
        // variables context (sub-expression from variable panel) behaves like watch.
        let backend = MockBackend::ok(json!({
            "kind": "List",
            "id": "objects/list2",
            "length": 5,
            "class": {"name": "List<String>"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "items", "context": "variables"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "List<String> (length: 5)");
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "variables context must expose variablesReference for lists"
        );
    }

    #[tokio::test]
    async fn test_repl_context_full_output_with_variables_reference() {
        // REPL evaluates with full output; expandable objects get variablesReference.
        let backend = MockBackend::ok(json!({
            "kind": "Map",
            "id": "objects/map1",
            "length": 3,
            "class": {"name": "Map<String, dynamic>"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "myMap", "context": "repl"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "repl context should succeed");
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "Map<String, dynamic> (length: 3)");
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "repl context should expose variablesReference for expandable objects"
        );
    }

    #[tokio::test]
    async fn test_repl_context_primitive_result() {
        let backend = MockBackend::ok(json!({"kind": "Int", "valueAsString": "99"}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "1 + 98", "context": "repl"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "99");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_clipboard_context_no_variables_reference_for_object() {
        // Clipboard context: even for expandable objects, variablesReference must be 0.
        let backend = MockBackend::ok(json!({
            "kind": "PlainInstance",
            "id": "objects/clip1",
            "class": {"name": "BigObject"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "bigObj", "context": "clipboard"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success, "clipboard context should succeed");
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "BigObject instance");
        assert_eq!(
            body["variablesReference"], 0,
            "clipboard context must not expand (variablesReference: 0)"
        );
    }

    #[tokio::test]
    async fn test_clipboard_context_long_string_not_truncated() {
        // Clipboard: full representation without truncation.
        let long_str = "z".repeat(HOVER_MAX_LEN * 5);
        let backend = MockBackend::ok(json!({"kind": "String", "valueAsString": long_str.clone()}));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "bigString", "context": "clipboard"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        let result = body["result"].as_str().unwrap();
        // Should contain the full string (as quoted String).
        assert!(
            result.len() > HOVER_MAX_LEN,
            "clipboard result should not be truncated, len={}",
            result.len()
        );
        assert!(!result.contains('…'), "clipboard must not have ellipsis");
        assert_eq!(body["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_unknown_context_behaves_like_watch() {
        // When context is unrecognized, treat as watch (expandable objects get variablesReference).
        let backend = MockBackend::ok(json!({
            "kind": "List",
            "id": "objects/unknown1",
            "length": 2,
            "class": {"name": "List<int>"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "nums", "context": "unknown_context"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "unknown context should behave like watch (expandable)"
        );
    }

    #[tokio::test]
    async fn test_no_context_behaves_like_watch() {
        // When context field is absent, treat as watch.
        let backend = MockBackend::ok(json!({
            "kind": "Map",
            "id": "objects/nocontext1",
            "length": 1,
            "class": {"name": "Map"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        // No "context" field in the arguments.
        let req = make_request_with_args(1, "evaluate", json!({"expression": "m"}));
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        assert!(
            body["variablesReference"].as_i64().unwrap_or(0) > 0,
            "no context should behave like watch"
        );
    }

    #[tokio::test]
    async fn test_eval_error_in_hover_context_returns_dap_error() {
        // Backend errors in hover context should still return a DAP error response.
        let backend = MockBackend::err("Unhandled exception: null check failed");
        let mut frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();
        let frame_id = frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "badExpr", "frameId": frame_id, "context": "hover"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(
            !resp.success,
            "hover error should produce DAP error response"
        );
        assert!(
            resp.message.as_deref().unwrap_or("").contains("null check"),
            "Error message should contain the backend error, got: {:?}",
            resp.message
        );
    }

    #[tokio::test]
    async fn test_clipboard_context_list_no_expansion() {
        // Clipboard context never expands, even for list types.
        let backend = MockBackend::ok(json!({
            "kind": "List",
            "id": "objects/cliplist",
            "length": 10,
            "class": {"name": "List<double>"}
        }));
        let frame_store = FrameStore::new();
        let mut var_store = VariableStore::new();

        let req = make_request_with_args(
            1,
            "evaluate",
            json!({"expression": "data", "context": "clipboard"}),
        );
        let resp = handle_evaluate(
            &backend,
            &frame_store,
            &mut var_store,
            Some("isolates/1"),
            &req,
        )
        .await;

        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        assert_eq!(body["result"], "List<double> (length: 10)");
        assert_eq!(body["variablesReference"], 0);
    }
}
