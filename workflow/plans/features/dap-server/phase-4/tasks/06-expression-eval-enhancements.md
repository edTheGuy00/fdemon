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
