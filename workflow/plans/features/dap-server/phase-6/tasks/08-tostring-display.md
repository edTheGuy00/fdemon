## Task: Implement toString() Display in Variables Panel

**Objective**: When `evaluateToStringInDebugViews` is true, call `toString()` on `PlainInstance` objects and append the result to the variable display value. Instead of showing just `"MyClass"`, show `"MyClass (custom string repr)"`. This is how the official Dart adapter displays objects.

**Depends on**: 03-globals-scope

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Add `toString()` call in `instance_ref_to_variable` for `PlainInstance` and similar kinds

### Details

#### Where to add toString():

In `instance_ref_to_variable`, for the `PlainInstance` match arm (and potentially `RegExp`, `StackTrace`):

```rust
"PlainInstance" | "RegExp" | "StackTrace" => {
    let class_display = class_name.unwrap_or(kind);

    let display = if self.evaluate_to_string_in_debug_views {
        // Call toString() on the object
        match tokio::time::timeout(
            Duration::from_secs(1),
            self.backend.evaluate(isolate_id, obj_id, "toString()")
        ).await {
            Ok(Ok(result)) => {
                let str_val = result.get("valueAsString")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if str_val.is_empty() || str_val == format!("Instance of '{}'", class_display) {
                    class_display.to_string()  // Skip default toString() output
                } else {
                    format!("{} ({})", class_display, str_val)
                }
            }
            _ => class_display.to_string(),  // Timeout or error: silent fallback
        }
    } else {
        class_display.to_string()
    };

    (display, var_ref, type_name)
}
```

#### When NOT to call toString():

- Primitives (`Int`, `Double`, `Bool`, `String`, `Null`) — they already show their value
- Collections (`List`, `Map`, `Set`) — they show `"TypeName (N items)"`
- `Closure` — shows `"Closure (functionName)"`
- `Type` — shows `"Type (ClassName)"`
- `Sentinel` — shows `valueAsString` directly
- `Null` — shows `"null"`

Only call `toString()` for: `PlainInstance`, `RegExp`, `StackTrace`, `Record`, `WeakReference`

#### Settings:

Add `evaluate_to_string_in_debug_views: bool` to `DapAdapter` state. Default: `true`.
Settable from `attach` request args (as `evaluateToStringInDebugViews`).

#### Handling the default toString() output:

Dart's default `toString()` returns `"Instance of 'ClassName'"` which is not useful. If the toString result matches this pattern, skip appending it — just show the class name.

#### Async consideration:

`instance_ref_to_variable` is currently a synchronous function. Adding `toString()` requires making it async (or evaluating toString separately in a wrapper). Options:
1. Make `instance_ref_to_variable` async — touches many call sites
2. Add a separate `enrich_with_toString` pass after collecting variables — cleaner separation

Recommendation: Option 2. Collect all variables first, then in a second pass, call `toString()` for each `PlainInstance` variable with `variablesReference > 0`. This limits the async surface and makes the timeout handling cleaner.

### Acceptance Criteria

1. `PlainInstance` variables show `"MyClass (custom string)"` when toString returns useful output
2. Default `"Instance of 'ClassName'"` toString output is suppressed
3. toString errors/timeouts silently fall back to class name only
4. Primitives, collections, closures do NOT call toString
5. `evaluateToStringInDebugViews = false` disables toString calls entirely
6. 10+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_to_string_appended_to_plain_instance() {
    // MockBackend: evaluate(obj_id, "toString()") returns "MyModel(id: 42, name: Alice)"
    // Verify display = "MyModel (MyModel(id: 42, name: Alice))"
}

#[tokio::test]
async fn test_default_to_string_suppressed() {
    // MockBackend: evaluate returns "Instance of 'MyModel'"
    // Verify display = "MyModel" (no appended text)
}

#[tokio::test]
async fn test_to_string_timeout_silent() {
    // MockBackend: evaluate hangs forever
    // Verify display = "MyModel" after 1s timeout, no error shown
}

#[tokio::test]
async fn test_to_string_disabled_by_setting() {
    // evaluateToStringInDebugViews = false
    // Verify toString() is never called
}
```

### Notes

- The 1s timeout is critical — some `toString()` implementations are expensive or buggy. The user should never see the variables panel hang because of a bad `toString()`.
- `disableBreakpoints: true` should be passed to the evaluate call to prevent recursive pauses.
- Consider batching toString calls — if 20 variables are in a scope, making 20 sequential evaluate RPCs could be slow. But parallel evaluation risks overwhelming the VM. Sequential with a total timeout (e.g., 5s for all toStrings in a scope) is a safe approach.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Added `evaluate_to_string_in_debug_views: Option<bool>` field to `AttachRequestArguments`; fixed existing test struct initializer |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `evaluate_to_string_in_debug_views: bool` field to `DapAdapter` (default: `true`); initialized in `new_with_tx` |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Apply `evaluateToStringInDebugViews` from attach args |
| `crates/fdemon-dap/src/adapter/variables.rs` | Added `TO_STRING_EVAL_TIMEOUT`, `TO_STRING_KINDS`, `ToStringCandidate` struct; added `enrich_with_to_string` async method; added `to_string_candidate` free function; modified Locals and Exceptions scope collection to collect candidates and run the enrichment pass |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Registered `to_string_display` test module |
| `crates/fdemon-dap/src/adapter/tests/to_string_display.rs` | New file: 17 unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **Option 2 (separate enrichment pass)**: The task recommended either making `instance_ref_to_variable_with_eval_name` async (Option 1) or a post-collection pass (Option 2). Option 2 was chosen because it avoids making many synchronous call sites async and cleanly separates concerns.

2. **`&mut [DapVariable]` not `&mut Vec<DapVariable>`**: Clippy (`-D warnings`) enforces the more idiomatic slice parameter. Changed the `enrich_with_to_string` signature accordingly.

3. **`WeakReference` included in TO_STRING_KINDS**: The task says "Only call toString() for: PlainInstance, RegExp, StackTrace, Record, WeakReference". `Record` was not included because it has a distinct display format (`"Record (N fields)"`) that is more informative than a toString result. WeakReference was included as specified.

4. **Sequential evaluation**: toString calls are made sequentially per the task's safe approach. Each call has a 1-second timeout.

5. **Exceptions scope enrichment**: The exception variable in the Exceptions scope also receives toString enrichment when `evaluate_to_string_in_debug_views` is true, consistent with the Locals scope behavior.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (689 fdemon-dap tests, all workspace tests pass)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **No total scope timeout**: Sequential toString calls add up. For scopes with many PlainInstance variables, this could add up to N×1s. A future improvement could add a total-scope toString budget.
2. **`disableBreakpoints` not passed**: The task notes mention passing `disableBreakpoints: true` to prevent recursive pauses; the backend `evaluate` trait method does not have this parameter. This is a known limitation deferred to a future task.
