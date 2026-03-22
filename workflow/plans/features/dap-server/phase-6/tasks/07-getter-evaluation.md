## Task: Implement Getter Evaluation in Variables Panel

**Objective**: When expanding a `PlainInstance` object in the variables panel, evaluate getter methods and display their results alongside regular fields. This is controlled by the `evaluateGettersInDebugViews` setting — when true, getters are eagerly evaluated; when false, they appear as lazy expandable items.

**Depends on**: 03-globals-scope

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Extend `expand_object` to traverse class hierarchy, collect getters, and evaluate them
- `crates/fdemon-dap/src/adapter/backend.rs`: No new methods needed — uses existing `evaluate` and `get_object`

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/mod.rs`: `DapAdapter` struct for settings storage

### Details

#### Getter evaluation flow:

When `expand_object` is called for a `PlainInstance`:

1. After processing `fields`, fetch getter methods from the class hierarchy:
   - Read `obj.class.id` to get the class ID
   - Call `backend.get_object(isolate_id, class_id, None, None)` to get the full `Class` object
   - Read `class.functions` — filter for `kind == "ImplicitGetter"` or `kind == "Getter"` that are not `static`
   - Exclude internal getters: `_identityHashCode`, `hashCode` (for primitives), `runtimeType`
   - Traverse superclass chain: `class.super.id` → `get_object` → repeat, until `super` is null or `Object`

2. For each getter:
   - If `evaluate_getters_in_debug_views == true`:
     - Call `backend.evaluate(isolate_id, obj_id, getter_name)` with 1s timeout
     - On success: convert result to `DapVariable` with `presentationHint.attributes: ["hasSideEffects"]`
     - On error: show `"<error: {message}>"` as the value, `variablesReference: 0`
     - On timeout: show `"<timed out>"` as the value
   - If `evaluate_getters_in_debug_views == false`:
     - Show getter as a lazy variable with `presentationHint.lazy: true`
     - `value: ""` (empty — IDE will show a "click to evaluate" placeholder)
     - `variablesReference > 0` pointing to a new `VariableRef::GetterEval` variant

3. Add `VariableRef::GetterEval` variant:
   ```rust
   pub enum VariableRef {
       Scope { ... },
       Object { ... },
       GetterEval { isolate_id: String, instance_id: String, getter_name: String },
   }
   ```

4. Handle `GetterEval` in `handle_variables`:
   - Call `backend.evaluate(isolate_id, instance_id, getter_name)` with 1s timeout
   - Return single variable with the evaluation result

#### Settings:

Add `evaluate_getters_in_debug_views: bool` to `DapAdapter` state. Default: `true`.
This should be settable from:
- The `attach` request args (as `evaluateGettersInDebugViews`)
- The `updateDebugOptions` custom request (Task 13)

#### Getter collection limits:

- Max 50 getter evaluations per object (prevent extremely large class hierarchies from hanging)
- Stop traversing superclass chain at depth 10 (prevent infinite loops in malformed class hierarchies)
- Evaluate getters sequentially, not in parallel (avoid overwhelming the VM Service)

### Acceptance Criteria

1. Expanding a `PlainInstance` shows both fields and evaluated getters
2. Getters have `presentationHint.attributes: ["hasSideEffects"]`
3. Getter errors show `"<error: message>"` without crashing
4. Getter timeouts show `"<timed out>"` after 1 second
5. `_identityHashCode` and similar internal getters are filtered out
6. When `evaluateGettersInDebugViews == false`, getters appear as lazy items
7. Lazy getters evaluate on explicit expansion
8. 15+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_expand_object_includes_getters() {
    // MockBackend: get_object(class_id) returns Class with functions: [Getter "name", Getter "age"]
    // MockBackend: evaluate(obj_id, "name") returns "Alice"
    // MockBackend: evaluate(obj_id, "age") returns 30
    // Verify expand_object returns fields + getter variables
}

#[tokio::test]
async fn test_getter_error_shows_error_string() {
    // MockBackend: evaluate returns error
    // Verify variable value is "<error: ...>"
}

#[tokio::test]
async fn test_internal_getters_filtered() {
    // Class has _identityHashCode, hashCode, runtimeType getters
    // Verify they are NOT included in results
}

#[tokio::test]
async fn test_lazy_getters_when_setting_false() {
    // evaluateGettersInDebugViews = false
    // Verify getters have lazy: true presentation hint
}
```

### Notes

- The 1s timeout for getter evaluation matches the Dart DDS adapter's behavior. Use `tokio::time::timeout(Duration::from_secs(1), ...)`.
- Getters can have side effects — the `hasSideEffects` attribute tells the IDE to show a warning icon. The `disableBreakpoints: true` parameter on the evaluate call prevents recursive pause.
- Class hierarchy traversal is expensive but bounded. Cache the getter list per class ID within a single pause (clear on resume with `var_store`).

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/variables.rs` | Added `GetterEval` handling in `handle_variables`, new `evaluate_lazy_getter` and `collect_getters_from_class` helpers, extended `expand_object` PlainInstance path with getter collection and evaluation, added module-level constants (`MAX_GETTER_EVALUATIONS`, `MAX_SUPERCLASS_DEPTH`, `GETTER_EVAL_TIMEOUT`, `FILTERED_GETTER_NAMES`), added `DapVariablePresentationHint` import |
| `crates/fdemon-dap/src/adapter/stack.rs` | Added `GetterEval` variant to `VariableRef` enum |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `evaluate_getters_in_debug_views: bool` field to `DapAdapter`, initialized to `true` in `new_with_tx` |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Updated `handle_attach` to read `evaluateGettersInDebugViews` from attach args and apply to adapter |
| `crates/fdemon-dap/src/protocol/types.rs` | Added `lazy: Option<bool>` to `DapVariablePresentationHint`, added `evaluate_getters_in_debug_views: Option<bool>` to `AttachRequestArguments`, fixed existing struct initializations to include new fields |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Added `getter_evaluation` to test module list |
| `crates/fdemon-dap/src/adapter/tests/getter_evaluation.rs` | New file with 16 unit tests |

### Notable Decisions/Tradeoffs

1. **`PlainInstance`-only getter evaluation**: Only `PlainInstance` objects get getter evaluation — not `Closure`, `RegExp`, etc. These types could technically have getters but they rarely appear in variable panels and adding getters to them could be confusing. The task spec focuses on `PlainInstance`.

2. **Explicit `PlainInstance` arm in `expand_object`**: Added a new explicit `"PlainInstance"` match arm before the catch-all `_` arm. This is cleaner than modifying the catch-all to special-case PlainInstance and avoids affecting other kinds like Closure, RegExp, etc.

3. **`collect_getters_from_class` is `async fn` on `&self`** (not `&mut self`): Getter collection only reads from the backend — it doesn't modify adapter state (no new variable references are allocated). This allows it to run before the mutable borrow needed for `evaluate` and `instance_ref_to_variable` in the eager path.

4. **No caching**: The task notes suggest caching getter lists per class ID. This was deferred — the `var_store` is already cleared on resume, and the 50-getter limit + 10-depth limit prevent performance issues. Adding a class-getter cache would require another `HashMap` field in `DapAdapter`.

5. **Timeout of exactly 1 second**: Matches the Dart DDS adapter's behavior as specified. Uses `tokio::time::timeout` which requires the multi-thread test flavor for tests where the timer actually fires.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (672 tests total, 16 new getter evaluation tests)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **No getter caching**: Each `variables` request for an expanded PlainInstance will re-traverse the class hierarchy. For large hierarchies with many getters, this means many `get_object` calls per expansion. Acceptable for the current scope; can be addressed in a follow-up.

2. **Sequential getter evaluation**: Getters are evaluated one by one as specified. For objects with many getters (up to 50), this can be slow on high-latency devices. Parallel evaluation could be added but was explicitly excluded from this task's scope.
