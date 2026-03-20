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
