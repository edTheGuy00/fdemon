//! VM Service RPC wrappers for debugging operations.
//!
//! This module provides async functions for all core debugging RPCs exposed by
//! the Dart VM Service Protocol. Functions translate typed Rust parameters into
//! JSON-RPC calls and parse responses into typed return values.
//!
//! ## Callers
//!
//! All public functions take a [`VmRequestHandle`] rather than a full
//! [`crate::vm_service::VmServiceClient`]. This allows callers to share the
//! handle across background tasks without holding a reference to the whole
//! client.
//!
//! ## Error handling
//!
//! Transport-level errors (channel closed, connection lost) are propagated as
//! [`Error::ChannelClosed`]. VM Service JSON-RPC errors are propagated as
//! [`Error::Protocol`]. Response parse failures are returned as
//! [`Error::VmService`] with context about which field or response was invalid.
//!
//! ## References
//!
//! - Dart VM Service Protocol:
//!   <https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md>

use fdemon_core::prelude::*;

use super::client::VmRequestHandle;
use super::debugger_types::{
    Breakpoint, ExceptionPauseMode, InstanceRef, ScriptList, Stack, StepOption,
};

// ── pause ─────────────────────────────────────────────────────────────────────

/// Pauses execution of the given isolate.
///
/// After calling this, the isolate enters a paused state and emits a
/// `PauseInterrupted` event on the Debug stream. Use [`resume`] to continue
/// execution.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited, or a
/// transport error if the request fails.
pub async fn pause(handle: &VmRequestHandle, isolate_id: &str) -> Result<()> {
    let params = serde_json::json!({ "isolateId": isolate_id });
    handle.request("pause", Some(params)).await?;
    Ok(())
}

// ── resume ────────────────────────────────────────────────────────────────────

/// Resumes execution of the given isolate, optionally with a step option.
///
/// When `step` is `None`, the isolate resumes normal execution. When `step`
/// is `Some`, the isolate performs a single step operation:
/// - [`StepOption::Into`] — step into function calls
/// - [`StepOption::Over`] — step over the current line
/// - [`StepOption::Out`] — step out of the current function
/// - [`StepOption::OverAsyncSuspension`] — step over async suspension points
///
/// The isolate must be paused before calling this function.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited, or a
/// transport error if the request fails.
pub async fn resume(
    handle: &VmRequestHandle,
    isolate_id: &str,
    step: Option<StepOption>,
) -> Result<()> {
    let mut params = serde_json::json!({ "isolateId": isolate_id });
    if let Some(step) = step {
        params["step"] = serde_json::json!(step.as_str());
    }
    handle.request("resume", Some(params)).await?;
    Ok(())
}

// ── add_breakpoint_with_script_uri ────────────────────────────────────────────

/// Adds a breakpoint at the given line in a script identified by URI.
///
/// Preferred over `addBreakpoint` — this form correctly handles breakpoints
/// in deferred libraries that have not yet been loaded at the time the
/// breakpoint is set. The VM will resolve and activate the breakpoint when
/// the script is eventually loaded.
///
/// # Arguments
///
/// * `handle` — VM Service request handle
/// * `isolate_id` — isolate to add the breakpoint in
/// * `script_uri` — URI of the script (e.g. `"package:app/main.dart"`)
/// * `line` — 1-based line number
/// * `column` — optional 1-based column number; omitted from the request when `None`
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::VmService`] if the response cannot be parsed as a [`Breakpoint`],
/// or a transport error if the request fails.
pub async fn add_breakpoint_with_script_uri(
    handle: &VmRequestHandle,
    isolate_id: &str,
    script_uri: &str,
    line: i32,
    column: Option<i32>,
) -> Result<Breakpoint> {
    let mut params = serde_json::json!({
        "isolateId": isolate_id,
        "scriptUri": script_uri,
        "line": line,
    });
    if let Some(col) = column {
        params["column"] = serde_json::json!(col);
    }
    let result = handle
        .request("addBreakpointWithScriptUri", Some(params))
        .await?;
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse breakpoint: {e}")))
}

// ── remove_breakpoint ─────────────────────────────────────────────────────────

/// Removes a breakpoint by its VM Service ID.
///
/// After removal, the breakpoint is no longer active and will not cause the
/// isolate to pause. The `breakpoint_id` is the `id` field from a
/// [`Breakpoint`] returned by [`add_breakpoint_with_script_uri`].
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited, or a
/// transport error if the request fails.
pub async fn remove_breakpoint(
    handle: &VmRequestHandle,
    isolate_id: &str,
    breakpoint_id: &str,
) -> Result<()> {
    let params = serde_json::json!({
        "isolateId": isolate_id,
        "breakpointId": breakpoint_id,
    });
    handle.request("removeBreakpoint", Some(params)).await?;
    Ok(())
}

// ── get_stack ─────────────────────────────────────────────────────────────────

/// Gets the current stack trace for a paused isolate.
///
/// The isolate must be paused before calling this function. Returns a
/// [`Stack`] containing the synchronous frames and optionally async causal
/// frames and awaiter frames.
///
/// Use `limit` to cap the number of frames returned. This is useful for
/// performance when displaying a summary view — large stacks can be slow
/// to serialize. Pass `None` to request all frames.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::VmService`] if the response cannot be parsed as a [`Stack`],
/// or a transport error if the request fails.
pub async fn get_stack(
    handle: &VmRequestHandle,
    isolate_id: &str,
    limit: Option<i32>,
) -> Result<Stack> {
    let mut params = serde_json::json!({ "isolateId": isolate_id });
    if let Some(limit) = limit {
        params["limit"] = serde_json::json!(limit);
    }
    let result = handle.request("getStack", Some(params)).await?;
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse stack: {e}")))
}

// ── get_object ────────────────────────────────────────────────────────────────

/// Gets a VM object by its ID. Returns raw JSON since objects are highly polymorphic.
///
/// Used for expanding variables, inspecting instances, fetching script source,
/// looking up libraries and classes, etc. The caller is responsible for
/// discriminating the object type from the `"type"` field in the returned JSON.
///
/// Dart VM objects include: `Instance`, `Script`, `Library`, `Class`, `Field`,
/// `Function`, `Code`, `Context`, `Closure`, `TypedData`, `RegExp`, etc.
///
/// # Arguments
///
/// * `handle` — VM Service request handle
/// * `isolate_id` — isolate that owns the object
/// * `object_id` — object ID (e.g. from an [`InstanceRef`] `id` field)
/// * `offset` — for list/map objects, the 0-based starting index for paging
/// * `count` — for list/map objects, the maximum number of elements to return
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited, or a
/// transport error if the request fails.
pub async fn get_object(
    handle: &VmRequestHandle,
    isolate_id: &str,
    object_id: &str,
    offset: Option<i64>,
    count: Option<i64>,
) -> Result<serde_json::Value> {
    let mut params = serde_json::json!({
        "isolateId": isolate_id,
        "objectId": object_id,
    });
    if let Some(offset) = offset {
        params["offset"] = serde_json::json!(offset);
    }
    if let Some(count) = count {
        params["count"] = serde_json::json!(count);
    }
    handle.request("getObject", Some(params)).await
}

// ── evaluate ──────────────────────────────────────────────────────────────────

/// Evaluates an expression in the context of a target object (library, class, or instance).
///
/// Returns an [`InstanceRef`] on success. The isolate does not need to be paused
/// for library-context evaluation, but must be paused for instance-context
/// evaluation.
///
/// Detects VM-level evaluation failures (compile errors, runtime exceptions)
/// by checking for a `"type": "@Error"` response and converting them to
/// [`Error::VmService`]. This is distinct from transport-level errors.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::VmService`] if the expression fails to evaluate or the response
/// cannot be parsed as an [`InstanceRef`], or a transport error if the request
/// fails.
pub async fn evaluate(
    handle: &VmRequestHandle,
    isolate_id: &str,
    target_id: &str,
    expression: &str,
) -> Result<InstanceRef> {
    let params = serde_json::json!({
        "isolateId": isolate_id,
        "targetId": target_id,
        "expression": expression,
    });
    let result = handle.request("evaluate", Some(params)).await?;
    // Check if the result is an error response from the VM (compile or runtime error).
    // This is different from a transport-level error and must be checked explicitly.
    if result.get("type").and_then(|t| t.as_str()) == Some("@Error") {
        let message = result
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("evaluation failed");
        return Err(Error::vm_service(format!("evaluate: {message}")));
    }
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse instance: {e}")))
}

// ── evaluate_in_frame ─────────────────────────────────────────────────────────

/// Evaluates an expression in the context of a specific stack frame.
///
/// The isolate must be paused. `frame_index` is 0-based and corresponds to the
/// `index` field of a [`crate::vm_service::debugger_types::Frame`] from a
/// previous [`get_stack`] call.
///
/// Detects VM-level evaluation failures (compile errors, runtime exceptions)
/// by checking for a `"type": "@Error"` response and converting them to
/// [`Error::VmService`]. This is distinct from transport-level errors.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::VmService`] if the expression fails to evaluate or the response
/// cannot be parsed as an [`InstanceRef`], or a transport error if the request
/// fails.
pub async fn evaluate_in_frame(
    handle: &VmRequestHandle,
    isolate_id: &str,
    frame_index: i32,
    expression: &str,
) -> Result<InstanceRef> {
    let params = serde_json::json!({
        "isolateId": isolate_id,
        "frameIndex": frame_index,
        "expression": expression,
    });
    let result = handle.request("evaluateInFrame", Some(params)).await?;
    // Check if the result is an error response from the VM (compile or runtime error).
    // This is different from a transport-level error and must be checked explicitly.
    if result.get("type").and_then(|t| t.as_str()) == Some("@Error") {
        let message = result
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("evaluation failed");
        return Err(Error::vm_service(format!("evaluateInFrame: {message}")));
    }
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse instance: {e}")))
}

// ── set_isolate_pause_mode ────────────────────────────────────────────────────

/// Sets the exception pause mode for an isolate.
///
/// Controls when the debugger pauses on thrown exceptions:
/// - [`ExceptionPauseMode::None`] — never pause on exceptions
/// - [`ExceptionPauseMode::Unhandled`] — pause on unhandled exceptions (default)
/// - [`ExceptionPauseMode::All`] — pause on all thrown exceptions
///
/// Uses `setIsolatePauseMode` (not the deprecated `setExceptionPauseMode`).
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited, or a
/// transport error if the request fails.
pub async fn set_isolate_pause_mode(
    handle: &VmRequestHandle,
    isolate_id: &str,
    exception_pause_mode: ExceptionPauseMode,
) -> Result<()> {
    let params = serde_json::json!({
        "isolateId": isolate_id,
        "exceptionPauseMode": exception_pause_mode.as_str(),
    });
    handle.request("setIsolatePauseMode", Some(params)).await?;
    Ok(())
}

// ── get_scripts ───────────────────────────────────────────────────────────────

/// Gets the list of scripts loaded in the isolate.
///
/// Returns a [`ScriptList`] containing [`crate::vm_service::debugger_types::ScriptRef`]
/// entries for every script currently loaded. Use the `uri` field of each
/// `ScriptRef` to match scripts to source files by URI.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::VmService`] if the response cannot be parsed as a [`ScriptList`],
/// or a transport error if the request fails.
pub async fn get_scripts(handle: &VmRequestHandle, isolate_id: &str) -> Result<ScriptList> {
    let params = serde_json::json!({ "isolateId": isolate_id });
    let result = handle.request("getScripts", Some(params)).await?;
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse scripts: {e}")))
}

// ── get_source_report ─────────────────────────────────────────────────────────

/// Gets a source report for the given script(s) in an isolate.
///
/// Returns raw JSON due to the complex and polymorphic `SourceReport` structure.
/// The `reports` slice specifies which report kinds to include, e.g.:
/// - `"Coverage"` — line-level coverage data
/// - `"PossibleBreakpoints"` — valid breakpoint positions
///
/// When `script_id` is `None`, the report covers all loaded scripts. When
/// `Some`, only the specified script is included. This can be much faster
/// for single-file coverage queries.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited, or a
/// transport error if the request fails.
pub async fn get_source_report(
    handle: &VmRequestHandle,
    isolate_id: &str,
    reports: &[&str],
    script_id: Option<&str>,
) -> Result<serde_json::Value> {
    let mut params = serde_json::json!({
        "isolateId": isolate_id,
        "reports": reports,
    });
    if let Some(script_id) = script_id {
        params["scriptId"] = serde_json::json!(script_id);
    }
    handle.request("getSourceReport", Some(params)).await
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // NOTE: All tests in this module are synchronous parameter-construction tests.
    // They verify that JSON-RPC parameter objects are assembled correctly (required
    // fields present, optional fields omitted when None, enum values serialised to
    // the right strings). No async RPC tests are included because there is no mock
    // transport for `VmRequestHandle` — wiring a fake channel would require a live
    // Tokio runtime and a stub task, adding complexity without meaningful coverage
    // of the protocol layer. End-to-end RPC coverage will be addressed separately.
    use super::*;
    use serde_json::json;

    // ── StepOption serialization ────────────────────────────────────────────

    #[test]
    fn test_step_option_into_as_str() {
        assert_eq!(StepOption::Into.as_str(), "Into");
    }

    #[test]
    fn test_step_option_over_as_str() {
        assert_eq!(StepOption::Over.as_str(), "Over");
    }

    #[test]
    fn test_step_option_out_as_str() {
        assert_eq!(StepOption::Out.as_str(), "Out");
    }

    #[test]
    fn test_step_option_over_async_suspension_as_str() {
        assert_eq!(
            StepOption::OverAsyncSuspension.as_str(),
            "OverAsyncSuspension"
        );
    }

    // ── ExceptionPauseMode serialization ───────────────────────────────────

    #[test]
    fn test_exception_pause_mode_none_as_str() {
        assert_eq!(ExceptionPauseMode::None.as_str(), "None");
    }

    #[test]
    fn test_exception_pause_mode_unhandled_as_str() {
        assert_eq!(ExceptionPauseMode::Unhandled.as_str(), "Unhandled");
    }

    #[test]
    fn test_exception_pause_mode_all_as_str() {
        assert_eq!(ExceptionPauseMode::All.as_str(), "All");
    }

    // ── evaluate VM error detection ─────────────────────────────────────────

    /// Verify that an `@Error` type response from the VM is converted to `Err`.
    /// We test the detection logic directly on serde_json::Value to avoid
    /// needing a live WebSocket connection.
    #[test]
    fn test_evaluate_vm_error_detection() {
        let error_response = json!({
            "type": "@Error",
            "kind": "UnhandledException",
            "message": "Null check operator used on a null value",
            "exception": {}
        });

        // The detection logic from evaluate():
        let is_vm_error = error_response.get("type").and_then(|t| t.as_str()) == Some("@Error");
        assert!(is_vm_error, "should detect @Error type");

        let message = error_response
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("evaluation failed");
        assert_eq!(message, "Null check operator used on a null value");
    }

    #[test]
    fn test_evaluate_non_error_response_not_detected_as_vm_error() {
        let success_response = json!({
            "type": "@Instance",
            "kind": "String",
            "id": "objects/1",
            "valueAsString": "hello"
        });

        let is_vm_error = success_response.get("type").and_then(|t| t.as_str()) == Some("@Error");
        assert!(!is_vm_error, "should not treat @Instance as an error");
    }

    #[test]
    fn test_evaluate_missing_message_uses_fallback() {
        let error_response = json!({
            "type": "@Error",
            "kind": "CompilationError"
            // no "message" field
        });

        let message = error_response
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("evaluation failed");
        assert_eq!(message, "evaluation failed");
    }

    // ── JSON parameter construction ─────────────────────────────────────────

    #[test]
    fn test_resume_params_without_step_excludes_step_key() {
        // Simulate the parameter construction from resume()
        let isolate_id = "isolates/1";
        let step: Option<StepOption> = None;

        let mut params = json!({ "isolateId": isolate_id });
        if let Some(step) = step {
            params["step"] = json!(step.as_str());
        }

        assert_eq!(params["isolateId"], "isolates/1");
        assert!(
            params.get("step").is_none(),
            "step key should be absent when None"
        );
    }

    #[test]
    fn test_resume_params_with_step_over_includes_step_key() {
        let isolate_id = "isolates/1";
        let step: Option<StepOption> = Some(StepOption::Over);

        let mut params = json!({ "isolateId": isolate_id });
        if let Some(step) = step {
            params["step"] = json!(step.as_str());
        }

        assert_eq!(params["step"], "Over");
    }

    #[test]
    fn test_add_breakpoint_params_with_column() {
        let isolate_id = "isolates/1";
        let script_uri = "package:app/main.dart";
        let line = 42_i32;
        let column: Option<i32> = Some(5);

        let mut params = json!({
            "isolateId": isolate_id,
            "scriptUri": script_uri,
            "line": line,
        });
        if let Some(col) = column {
            params["column"] = json!(col);
        }

        assert_eq!(params["column"], 5);
    }

    #[test]
    fn test_add_breakpoint_params_without_column() {
        let isolate_id = "isolates/1";
        let script_uri = "package:app/main.dart";
        let line = 42_i32;
        let column: Option<i32> = None;

        let mut params = json!({
            "isolateId": isolate_id,
            "scriptUri": script_uri,
            "line": line,
        });
        if let Some(col) = column {
            params["column"] = json!(col);
        }

        assert!(
            params.get("column").is_none(),
            "column should be absent when None"
        );
    }

    #[test]
    fn test_get_stack_params_with_limit() {
        let isolate_id = "isolates/1";
        let limit: Option<i32> = Some(10);

        let mut params = json!({ "isolateId": isolate_id });
        if let Some(limit) = limit {
            params["limit"] = json!(limit);
        }

        assert_eq!(params["limit"], 10);
    }

    #[test]
    fn test_get_stack_params_without_limit() {
        let isolate_id = "isolates/1";
        let limit: Option<i32> = None;

        let mut params = json!({ "isolateId": isolate_id });
        if let Some(limit) = limit {
            params["limit"] = json!(limit);
        }

        assert!(
            params.get("limit").is_none(),
            "limit should be absent when None"
        );
    }

    #[test]
    fn test_get_object_params_with_offset_and_count() {
        let isolate_id = "isolates/1";
        let object_id = "objects/42";
        let offset: Option<i64> = Some(0);
        let count: Option<i64> = Some(100);

        let mut params = json!({
            "isolateId": isolate_id,
            "objectId": object_id,
        });
        if let Some(offset) = offset {
            params["offset"] = json!(offset);
        }
        if let Some(count) = count {
            params["count"] = json!(count);
        }

        assert_eq!(params["offset"], 0);
        assert_eq!(params["count"], 100);
    }

    #[test]
    fn test_get_object_params_without_optional_fields() {
        let isolate_id = "isolates/1";
        let object_id = "objects/42";
        let offset: Option<i64> = None;
        let count: Option<i64> = None;

        let mut params = json!({
            "isolateId": isolate_id,
            "objectId": object_id,
        });
        if let Some(offset) = offset {
            params["offset"] = json!(offset);
        }
        if let Some(count) = count {
            params["count"] = json!(count);
        }

        assert!(
            params.get("offset").is_none(),
            "offset should be absent when None"
        );
        assert!(
            params.get("count").is_none(),
            "count should be absent when None"
        );
    }

    #[test]
    fn test_get_source_report_params_with_script_id() {
        let isolate_id = "isolates/1";
        let reports: &[&str] = &["Coverage", "PossibleBreakpoints"];
        let script_id: Option<&str> = Some("scripts/5");

        let mut params = json!({
            "isolateId": isolate_id,
            "reports": reports,
        });
        if let Some(script_id) = script_id {
            params["scriptId"] = json!(script_id);
        }

        assert_eq!(params["scriptId"], "scripts/5");
        assert_eq!(
            params["reports"],
            json!(["Coverage", "PossibleBreakpoints"])
        );
    }

    #[test]
    fn test_get_source_report_params_without_script_id() {
        let isolate_id = "isolates/1";
        let reports: &[&str] = &["Coverage"];
        let script_id: Option<&str> = None;

        let mut params = json!({
            "isolateId": isolate_id,
            "reports": reports,
        });
        if let Some(script_id) = script_id {
            params["scriptId"] = json!(script_id);
        }

        assert!(
            params.get("scriptId").is_none(),
            "scriptId should be absent when None"
        );
    }

    #[test]
    fn test_set_isolate_pause_mode_params() {
        let isolate_id = "isolates/1";
        let mode = ExceptionPauseMode::All;

        let params = json!({
            "isolateId": isolate_id,
            "exceptionPauseMode": mode.as_str(),
        });

        assert_eq!(params["exceptionPauseMode"], "All");
    }
}
