## Task: Add VM Service Debugging RPC Wrappers

**Objective**: Create async functions that wrap all VM Service debugging RPCs using the existing `VmRequestHandle::request()` method. These functions translate typed Rust parameters into JSON-RPC calls and parse the responses into typed return values.

**Depends on**: 01-debug-types (uses `StepOption`, `ExceptionPauseMode`, `Breakpoint`, `Stack`, `InstanceRef`, `ScriptList`, `ScriptRef`, `SourceLocation`)

### Scope

- `crates/fdemon-daemon/src/vm_service/debugger.rs` — **NEW FILE**: All debugging RPC wrapper functions
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Add `pub mod debugger;` and re-export public functions

### Details

Create `debugger.rs` following the established pattern from `performance.rs` and `network.rs`: free async functions taking `handle: &VmRequestHandle` as the first parameter.

#### RPC functions to implement

**1. `pause`**

```rust
/// Pauses execution of the given isolate.
pub async fn pause(handle: &VmRequestHandle, isolate_id: &str) -> Result<()> {
    let params = serde_json::json!({ "isolateId": isolate_id });
    handle.request("pause", Some(params)).await?;
    Ok(())
}
```

**2. `resume`**

```rust
/// Resumes execution of the given isolate, optionally with a step option.
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
```

**3. `add_breakpoint_with_script_uri`**

```rust
/// Adds a breakpoint at the given line in a script identified by URI.
/// Preferred over addBreakpoint — handles deferred libraries correctly.
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
    let result = handle.request("addBreakpointWithScriptUri", Some(params)).await?;
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse breakpoint: {e}")))
}
```

**4. `remove_breakpoint`**

```rust
/// Removes a breakpoint by its VM Service ID.
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
```

**5. `get_stack`**

```rust
/// Gets the current stack trace for a paused isolate.
/// Use `limit` to cap the number of frames returned.
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
```

**6. `get_object`**

```rust
/// Gets a VM object by its ID. Returns raw JSON since objects are highly polymorphic.
/// Used for expanding variables, inspecting instances, fetching script source, etc.
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
```

**7. `evaluate`**

```rust
/// Evaluates an expression in the context of a target object (library, class, or instance).
/// Returns InstanceRef on success, or Error variant on evaluation failure.
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
    // Check if the result is an error response from the VM
    if result.get("type").and_then(|t| t.as_str()) == Some("@Error") {
        let message = result.get("message").and_then(|m| m.as_str()).unwrap_or("evaluation failed");
        return Err(Error::vm_service(format!("evaluate: {message}")));
    }
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse instance: {e}")))
}
```

**8. `evaluate_in_frame`**

```rust
/// Evaluates an expression in the context of a specific stack frame.
/// The isolate must be paused.
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
    if result.get("type").and_then(|t| t.as_str()) == Some("@Error") {
        let message = result.get("message").and_then(|m| m.as_str()).unwrap_or("evaluation failed");
        return Err(Error::vm_service(format!("evaluateInFrame: {message}")));
    }
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse instance: {e}")))
}
```

**9. `set_isolate_pause_mode`**

```rust
/// Sets the exception pause mode for an isolate.
/// Uses setIsolatePauseMode (not the deprecated setExceptionPauseMode).
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
```

**10. `get_scripts`**

```rust
/// Gets the list of scripts loaded in the isolate.
pub async fn get_scripts(
    handle: &VmRequestHandle,
    isolate_id: &str,
) -> Result<ScriptList> {
    let params = serde_json::json!({ "isolateId": isolate_id });
    let result = handle.request("getScripts", Some(params)).await?;
    serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse scripts: {e}")))
}
```

**11. `get_source_report`**

```rust
/// Gets a source report for the given script(s) in an isolate.
/// Returns raw JSON due to the complex SourceReport structure.
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
```

#### Module registration

In `mod.rs`, add:
```rust
pub mod debugger;
```

Re-export all public functions:
```rust
pub use debugger::{
    pause, resume, add_breakpoint_with_script_uri, remove_breakpoint,
    get_stack, get_object, evaluate, evaluate_in_frame,
    set_isolate_pause_mode, get_scripts, get_source_report,
};
```

### Acceptance Criteria

1. All 11 RPC functions compile and have correct signatures
2. Functions use the correct JSON-RPC method names from the Dart VM Service spec
3. Optional parameters are only included in the JSON when `Some`
4. `evaluate` and `evaluate_in_frame` detect VM error responses (`"type": "@Error"`) and return `Err`
5. Response parsing uses `serde_json::from_value` with proper error context
6. `get_object` and `get_source_report` return raw `Value` (polymorphic responses)
7. All functions are re-exported from `vm_service/mod.rs`
8. Each function has a doc comment explaining what it does and any important caveats
9. `cargo check -p fdemon-daemon` passes
10. `cargo clippy -p fdemon-daemon` clean

### Testing

Use `VmRequestHandle::new_for_test()` for unit tests. Mock the `request()` call to verify correct parameter serialization and response parsing.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resume_without_step() {
        let handle = VmRequestHandle::new_for_test(Some("isolates/1".to_string()));
        // Verify the request is constructed correctly
        // (test implementation depends on how the test handle works)
    }

    #[tokio::test]
    async fn test_resume_with_step_over() {
        // Verify "step": "Over" is included in params
    }

    #[tokio::test]
    async fn test_add_breakpoint_with_column() {
        // Verify column is included when Some
    }

    #[tokio::test]
    async fn test_add_breakpoint_without_column() {
        // Verify column is omitted when None
    }

    #[tokio::test]
    async fn test_evaluate_vm_error_response() {
        // Verify @Error responses are converted to Err
    }

    #[test]
    fn test_step_option_serialization() {
        assert_eq!(StepOption::Into.as_str(), "Into");
        assert_eq!(StepOption::Over.as_str(), "Over");
        assert_eq!(StepOption::Out.as_str(), "Out");
    }
}
```

### Notes

- Follow the existing pattern from `performance.rs:40-44` — `get_memory_usage(handle, isolate_id)` is the structural template.
- Error handling: use `Error::vm_service(msg)` from `fdemon-core` for all VM Service errors — this is the existing pattern (see `crates/fdemon-core/src/error.rs`).
- `get_object` returns raw `Value` intentionally — Dart VM objects are highly polymorphic (Instance, Script, Library, Class, etc.). The DAP adapter layer (Phase 3) will handle type discrimination.
- `get_source_report` also returns raw `Value` for the same reason.
- The `evaluate`/`evaluate_in_frame` functions need to check for `"type": "@Error"` in the response, which indicates a compile error or runtime exception during evaluation (different from a transport-level error).
- Use `setIsolatePauseMode` not `setExceptionPauseMode` — the latter is deprecated.

---

## Completion Summary

**Status:** Not started
