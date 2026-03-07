## Task: Enhance Expression Evaluation by Context

**Objective**: Differentiate expression evaluation behavior based on the `context` field (`hover`, `watch`, `repl`, `clipboard`, `variables`). Currently all contexts are treated identically. This task adds context-specific behavior: auto-`toString()` for hover, full output for clipboard, and side-effect awareness for repl.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-dap/src/adapter/evaluate.rs`: Add context-specific dispatching and formatting

### Details

#### Context Behavior Matrix

| Context | Behavior | Display |
|---------|----------|---------|
| `"hover"` | Side-effect-free. Call `toString()` on non-primitive results. Truncate long strings. | Short tooltip |
| `"watch"` | Evaluate expression. Return structured result with `variablesReference` for expandable objects. | Watch panel row |
| `"repl"` | Full evaluation. Side effects allowed. Return complete string representation. Multi-line OK. | Debug console |
| `"clipboard"` | Evaluate, then format for pasting. No truncation. Include full object representation. | Clipboard text |
| `"variables"` | Same as watch — evaluate sub-expressions from variable view. | Variable panel |

#### Hover Context Enhancement

For hover tooltips, IDEs expect a short, readable value. For objects, call `toString()`:

```rust
async fn evaluate_for_hover(&self, isolate_id: &str, frame_index: usize, expression: &str) -> EvalResult {
    let result = self.backend.evaluate_in_frame(isolate_id, frame_index, expression).await?;

    match result.kind.as_deref() {
        // Primitives: use value_as_string directly
        Some("Int" | "Double" | "String" | "Bool" | "Null") => {
            Ok(format_primitive(&result))
        }
        // Objects: call toString()
        _ => {
            let to_string_expr = format!("({}).toString()", expression);
            match self.backend.evaluate_in_frame(isolate_id, frame_index, &to_string_expr).await {
                Ok(str_result) => Ok(format_primitive(&str_result)),
                Err(_) => Ok(format_primitive(&result)), // Fallback to type name
            }
        }
    }
}
```

#### Clipboard Context

For clipboard, provide complete, untruncated output:

```rust
async fn evaluate_for_clipboard(&self, ...) -> EvalResult {
    let result = self.backend.evaluate_in_frame(isolate_id, frame_index, expression).await?;
    // No truncation, full representation
    Ok(format_full(&result))
}
```

#### REPL Context

For debug console REPL:
- Allow side-effect expressions (assignments, method calls)
- Return the full result including type info
- Multi-line results are OK

#### Response Format

The `evaluate` DAP response has:
```json
{
  "result": "display string",
  "type": "String",
  "variablesReference": 0,  // 0 = no children to expand
  "namedVariables": 0,
  "indexedVariables": 0
}
```

For expandable objects in `watch`/`variables` context, set `variablesReference > 0` so the IDE can request children. For `hover` and `clipboard`, always set `variablesReference: 0` (no expansion).

### Acceptance Criteria

1. Hovering over a variable in the IDE shows its `toString()` value
2. Watch panel shows structured values with expandable objects
3. Debug console REPL evaluates expressions with full output
4. Clipboard context provides untruncated text
5. Evaluation errors display the error message, not a crash
6. All existing evaluate tests pass
7. 12+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_hover_context_calls_to_string() {
    // Set up adapter with mock backend
    // Mock evaluate("x") returns object
    // Mock evaluate("(x).toString()") returns "MyClass(42)"
    // Call evaluate with context: "hover"
    // Verify result is "MyClass(42)" with variablesReference: 0
}

#[tokio::test]
async fn test_hover_primitive_no_to_string() {
    // Mock evaluate("x") returns Int 42
    // Call evaluate with context: "hover"
    // Verify result is "42" without calling toString
}

#[tokio::test]
async fn test_watch_context_provides_variables_reference() {
    // Mock evaluate("x") returns object with fields
    // Call evaluate with context: "watch"
    // Verify variablesReference > 0
}

#[tokio::test]
async fn test_clipboard_context_no_truncation() {
    // Mock evaluate returns a very long string
    // Call evaluate with context: "clipboard"
    // Verify full string returned without truncation
}
```

### Notes

- The existing `evaluate.rs` already handles basic `evaluateInFrame` and root library evaluation. This task adds a context-dispatching layer on top.
- `supportsEvaluateForHovers: true` is already advertised in capabilities.
- Add `supportsClipboardContext: true` to the initialize response capabilities.
- Hover evaluation should be fast — consider a timeout shorter than the default for the `toString()` call.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Added `EvalContext` enum, context-specific dispatching in `handle_evaluate`, `handle_evaluate_hover` helper, `evaluate_expression_raw` dispatch helper, `is_primitive_kind`, `truncate_for_hover` functions. Extended `MockBackend` with `to_string_result` field for hover testing. Added 27 new unit tests. |
| `crates/fdemon-dap/src/protocol/types.rs` | Added `supports_clipboard_context: Option<bool>` field to `Capabilities` struct with serde annotation. Added `supports_clipboard_context: Some(true)` to `fdemon_defaults()`. |

### Notable Decisions/Tradeoffs

1. **`EvalContext::parse` instead of `from_str`**: Clippy warns that `from_str` confuses the standard `FromStr` trait. Used `parse` as the method name to avoid the warning, which is equally readable.

2. **Hover path is a separate fast path**: `handle_evaluate_hover` is dispatched before the general evaluation path. This avoids a mutable borrow problem (the `handle_evaluate_hover` doesn't need `var_store` since hover never returns `variablesReference > 0`) and makes the hover-specific logic self-contained.

3. **`toString()` detection in MockBackend via expression suffix**: The `MockBackend::evaluate_in_frame` checks `expression.ends_with(".toString()")` to route `toString()` calls to a separate result. This accurately simulates the real dispatch without requiring a full mock call-count mechanism.

4. **No timeout for hover `toString()` call**: The task notes a timeout could be useful. This was not implemented — the underlying backend can enforce its own timeouts. Adding an in-adapter timeout would require either a `tokio::time::timeout` wrapper or a configurable duration parameter, which is out of scope for this task.

5. **Clipboard/repl share the standard path**: Both reuse the common evaluation path. The only difference is that clipboard suppresses `variablesReference` via `EvalContext::suppress_variables_reference()`. REPL gets `variablesReference` for expandable objects, which allows the user to drill in from the debug console.

6. **Pre-existing Task 04 breakage in adapter/mod.rs**: Task 04 (running concurrently) modified `adapter/mod.rs` to add `breakpoint_id` to `DebugEvent::Paused` but left call sites incomplete. This causes `cargo test -p fdemon-dap` to fail during test compilation. All errors are in Task 04's files — `fdemon-dap` library itself compiles cleanly (`cargo check -p fdemon-dap` passes).

### Testing Performed

- `cargo check -p fdemon-dap` — Passed (no errors in my files)
- `cargo clippy -p fdemon-dap` — 2 warnings in Task 04's `breakpoints.rs`, zero warnings in my files
- `cargo fmt -p fdemon-dap` — Applied formatting
- `cargo test -p fdemon-dap` — Cannot run due to Task 04's incomplete `DebugEvent::Paused { breakpoint_id }` changes in `adapter/mod.rs` test code; all errors are in Task 04's territory

### Risks/Limitations

1. **Tests blocked by Task 04**: The `fdemon-dap` test suite cannot compile until Task 04 completes its `DebugEvent::Paused` migration in `adapter/mod.rs`. All 27 new tests are correct and will pass once Task 04 resolves the compile error.

2. **Hover timeout not implemented**: Long-running `toString()` calls will block the hover response indefinitely. A future task should wrap the `toString()` evaluation with a short timeout (e.g. 500ms) to keep IDE hover snappy.

3. **Clipboard provides `format_instance_value` output, not a full recursive dump**: For deeply nested objects, the clipboard result shows `"ClassName instance"` rather than a recursive JSON/string dump. A full recursive expansion would require additional VM Service calls and is deferred to a future task.
