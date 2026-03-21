## Task: Fix Expression Injection in Hover Evaluate

**Objective**: Eliminate the expression injection vulnerability in hover evaluation by calling `toString()` on the result object reference ID instead of re-composing a Dart expression string containing the raw user input.

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/evaluate.rs`: Fix `handle_evaluate_hover` toString wrapping

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Reference `enrich_with_to_string` pattern (line 537–571) — this is the correct approach
- `crates/fdemon-dap/src/adapter/backend.rs`: `evaluate` method signature

### Details

#### The Problem (evaluate.rs:278)

```rust
let to_string_expr = format!("({}).toString()", args.expression);
```

`args.expression` is the raw string from the DAP client. A crafted expression like `a) + sideEffect(` produces `(a) + sideEffect(.toString())` — arbitrary Dart code execution.

#### The Fix

Follow the same pattern as `enrich_with_to_string` in `variables.rs:544–546`:

```rust
self.backend.evaluate(&candidate.isolate_id, &candidate.object_id, "toString()")
```

This calls `toString()` on the **object reference ID** returned by the initial evaluation, not on a re-composed expression string.

The hover evaluate flow should become:

1. Evaluate `args.expression` to get an instance ref (already done at line 264)
2. Extract the object ID from the result (e.g., `instance["id"]`)
3. Call `backend.evaluate(isolate_id, &object_id, "toString()")` on the object reference
4. Use the toString result as the display value, falling back to the original value on error

```rust
// Step 1: Already done — evaluate_expression_raw returns the instance
let instance = evaluate_expression_raw(backend, isolate_id, frame_ref.as_ref(), &args).await?;

// Step 2: Extract object ID from the result
let display_value = if let Some(object_id) = instance.get("id").and_then(|v| v.as_str()) {
    // Step 3: Call toString() on the object reference, not on a re-composed expression
    match tokio::time::timeout(
        TO_STRING_EVAL_TIMEOUT,
        backend.evaluate(isolate_id, object_id, "toString()"),
    ).await {
        Ok(Ok(str_result)) => format_instance_value(&str_result),
        _ => format_instance_value(&instance), // Fallback on error/timeout
    }
} else {
    format_instance_value(&instance)
};
```

**Important**: Import `TO_STRING_EVAL_TIMEOUT` from the appropriate location (it's used in `variables.rs` already). If it lives in `variables.rs`, consider moving the constant to `types.rs` so both modules can reference it.

### Acceptance Criteria

1. Hover evaluate never constructs a Dart expression string containing raw user-supplied text
2. `toString()` is called on the VM object reference ID, not via string interpolation
3. Hover evaluate still shows toString() values for object types (functional parity)
4. Hover evaluate with timeout/error still falls back to the raw instance value
5. Existing evaluate tests pass: `cargo test -p fdemon-dap`
6. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[tokio::test]
async fn test_hover_evaluate_does_not_embed_expression_in_tostring() {
    // Mock backend that records all evaluate calls
    // Send hover evaluate with expression "myVar"
    // Assert backend.evaluate was called with (isolate_id, object_id, "toString()")
    // NOT with "(myVar).toString()"
}

#[tokio::test]
async fn test_hover_evaluate_tostring_fallback_on_no_object_id() {
    // Mock backend returns instance with no "id" field
    // Assert display falls back to format_instance_value without toString call
}
```

### Notes

- The first evaluate call at line 264 (`evaluate_expression_raw` with `args.expression`) is fine — the DAP spec explicitly allows arbitrary expression evaluation in the debug console. The injection is only a concern for the *secondary* toString wrapping, which should be read-only.
- `TO_STRING_EVAL_TIMEOUT` is 1 second per call, matching the pattern in `variables.rs`.
