## Task: Implement Conditional Breakpoints

**Objective**: Implement conditional breakpoint support so the debugger only pauses when a user-specified expression evaluates to truthy. The capability `supportsConditionalBreakpoints: true` is already advertised; this task implements the actual behavior.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 3–5 hours

### Scope

- `crates/fdemon-dap/src/adapter/breakpoints.rs`: Modify `setBreakpoints` handler to store conditions
- `crates/fdemon-dap/src/adapter/mod.rs`: On `PauseBreakpoint` event, evaluate condition before emitting `stopped`
- `crates/fdemon-dap/src/adapter/mod.rs`: Add `BreakpointCondition` struct to `TrackedBreakpoint`

### Details

#### How Conditional Breakpoints Work in DAP

1. IDE sends `setBreakpoints` with `SourceBreakpoint` entries that have `condition` and/or `hitCondition` fields
2. The adapter sets **unconditional** breakpoints in the VM (Dart VM doesn't support conditional breakpoints natively)
3. When a breakpoint is hit (`PauseBreakpoint` event), the adapter:
   a. Checks if the hit breakpoint has a condition
   b. If yes: evaluates the condition expression via `evaluateInFrame`
   c. If result is truthy: emits `stopped` event (normal breakpoint behavior)
   d. If result is falsy: silently resumes the isolate (transparent to user)
4. If no condition: normal breakpoint behavior (emit `stopped`)

#### Condition Storage

Extend `TrackedBreakpoint` (in `DebugState` or adapter's breakpoint tracking):

```rust
pub struct TrackedBreakpoint {
    pub dap_id: i64,
    pub vm_id: String,
    pub uri: String,
    pub line: i32,
    pub verified: bool,
    pub condition: Option<String>,      // NEW
    pub hit_condition: Option<String>,   // NEW
    pub hit_count: u64,                  // NEW — tracks hits for hit conditions
}
```

#### Hit Condition Evaluation

`hitCondition` is a string like `"== 5"`, `">= 3"`, `"% 2 == 0"`. The adapter:
1. Increments `hit_count` on every VM breakpoint hit
2. Evaluates the hit condition against the count
3. Supported operators: `==`, `>=`, `<=`, `>`, `<`, `%` (modulo)
4. If the hit condition passes AND the regular condition passes (if any), emit `stopped`

```rust
fn evaluate_hit_condition(hit_count: u64, condition: &str) -> bool {
    // Parse simple expressions: ">= 5", "== 3", "% 2 == 0"
    // Use regex or simple parsing — this doesn't need a full expression evaluator
}
```

#### Condition Evaluation Flow

```rust
async fn on_pause_breakpoint(&mut self, isolate_id: &str, breakpoint_id: &str) {
    if let Some(bp) = self.find_tracked_breakpoint(breakpoint_id) {
        bp.hit_count += 1;

        // Check hit condition first (cheap, no RPC)
        if let Some(hit_cond) = &bp.hit_condition {
            if !evaluate_hit_condition(bp.hit_count, hit_cond) {
                self.backend.resume(isolate_id, None).await.ok();
                return; // Don't stop
            }
        }

        // Check expression condition (requires evaluateInFrame RPC)
        if let Some(condition) = &bp.condition {
            match self.backend.evaluate_in_frame(isolate_id, 0, condition).await {
                Ok(result) if is_truthy(&result) => {
                    // Condition met — fall through to emit stopped
                }
                Ok(_) => {
                    // Condition not met — silently resume
                    self.backend.resume(isolate_id, None).await.ok();
                    return;
                }
                Err(e) => {
                    // Condition evaluation error — treat as truthy (stop and let user see the error)
                    tracing::warn!("Conditional breakpoint evaluation failed: {}", e);
                }
            }
        }
    }

    // Emit stopped event
    self.send_stopped_event(isolate_id, "breakpoint");
}
```

#### Truthiness

For Dart values, truthy means:
- `bool: true` → truthy
- `bool: false` → falsy
- `null` → falsy
- Everything else → truthy (Dart doesn't have JS-style falsy values)

```rust
fn is_truthy(result: &InstanceRef) -> bool {
    match result.kind.as_deref() {
        Some("Bool") => result.value_as_string.as_deref() == Some("true"),
        Some("Null") => false,
        _ => true, // Non-null, non-bool values are truthy
    }
}
```

### Acceptance Criteria

1. Setting a breakpoint with `condition: "x > 5"` only pauses when `x > 5` at the breakpoint
2. Setting `hitCondition: ">= 3"` pauses only on the 3rd+ hit
3. Combining `condition` and `hitCondition` requires both to be true
4. Condition evaluation errors cause the breakpoint to stop (safe default)
5. Breakpoint verification response includes condition status
6. All existing breakpoint tests pass
7. 15+ new unit tests

### Testing

```rust
#[test]
fn test_hit_condition_gte_3() {
    assert!(!evaluate_hit_condition(1, ">= 3"));
    assert!(!evaluate_hit_condition(2, ">= 3"));
    assert!(evaluate_hit_condition(3, ">= 3"));
    assert!(evaluate_hit_condition(4, ">= 3"));
}

#[test]
fn test_hit_condition_eq_5() {
    assert!(!evaluate_hit_condition(4, "== 5"));
    assert!(evaluate_hit_condition(5, "== 5"));
    assert!(!evaluate_hit_condition(6, "== 5"));
}

#[test]
fn test_hit_condition_modulo() {
    assert!(evaluate_hit_condition(2, "% 2 == 0"));
    assert!(!evaluate_hit_condition(3, "% 2 == 0"));
}

#[test]
fn test_is_truthy() {
    assert!(is_truthy(&make_bool_instance("true")));
    assert!(!is_truthy(&make_bool_instance("false")));
    assert!(!is_truthy(&make_null_instance()));
    assert!(is_truthy(&make_string_instance("hello")));
}

#[tokio::test]
async fn test_conditional_breakpoint_resumes_when_false() {
    // Set up adapter with mock backend
    // Set breakpoint with condition "x > 5"
    // Simulate PauseBreakpoint event
    // Mock evaluateInFrame to return false
    // Verify resume() was called (not stopped event)
}
```

### Notes

- The Dart VM does NOT support native conditional breakpoints. All conditions are evaluated adapter-side. This adds latency on each breakpoint hit (one evaluate RPC round-trip) but is the standard approach used by all DAP adapters.
- `evaluateInFrame` at frame index 0 evaluates in the context of the top (current) stack frame.
- `SourceBreakpoint.condition` and `.hit_condition` fields already exist in `protocol/types.rs:432-438`. The adapter's `setBreakpoints` handler just needs to read and store them.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/breakpoints.rs` | Added `BreakpointCondition` struct; extended `BreakpointEntry` with `condition`, `hit_condition`, `hit_count` fields; added `add_with_condition()` and `increment_hit_count()` methods; added `evaluate_hit_condition()` and `is_truthy()` module-level functions; 22 new unit tests |
| `crates/fdemon-dap/src/adapter/mod.rs` | Extended `DebugEvent::Paused` with `breakpoint_id: Option<String>`; updated `handle_debug_event` to evaluate conditions on `Paused + Breakpoint`; updated `handle_set_breakpoints` to store conditions via `add_with_condition()`; exported `BreakpointCondition`; updated 7 existing test calls; added 10 new integration tests |
| `crates/fdemon-app/src/handler/devtools/debug.rs` | Updated `PauseBreakpoint` handler to extract and pass breakpoint ID to `DapDebugEvent::Paused`; added `breakpoint_id: None` to all non-breakpoint pause events; updated 6 pattern matches to use `..` for forward-compatibility |

### Notable Decisions/Tradeoffs

1. **`BreakpointCondition` struct instead of extra args**: The `add_with_condition()` function would have exceeded clippy's 7-argument limit (`too_many_arguments`). Introducing `BreakpointCondition { condition, hit_condition }` keeps the API clean and matches the task's mention of a `BreakpointCondition` struct.

2. **Extended `DebugEvent::Paused` with `breakpoint_id`**: To evaluate conditions adapter-side, we need to know which VM breakpoint was hit. This required updating `fdemon-app/debug.rs` to pass through the breakpoint ID from the `PauseBreakpoint` VM event. All existing pattern matches were updated to use `..` for forward-compatibility.

3. **Condition evaluation only when `breakpoint_id` is Some**: If no `breakpoint_id` is supplied (e.g., from older code or non-breakpoint pauses), the adapter skips condition checking and always emits `stopped`. This is the safe default.

4. **Hit count incremented before all checks**: The hit count is always incremented on every VM breakpoint hit (even if the hit condition fails). This matches standard debugger behavior — the count reflects actual hits regardless of whether the debugger paused.

5. **`is_truthy` and `evaluate_hit_condition` are public**: Both functions are pub in `breakpoints.rs` so they can be used in integration tests. The doc example in `evaluate_hit_condition` also serves as a doc-test.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (464 tests)
- `cargo test --workspace` - Passed (all crates)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied (formatter reformatted `debug.rs` pattern matches)

### Risks/Limitations

1. **`fdemon-app/debug.rs` is modified**: The task scope said "you CAN modify `breakpoints.rs` and `adapter/mod.rs`". `debug.rs` was also modified to pass the breakpoint ID through. This is necessary for the feature to work end-to-end. The change is minimal (adding `breakpoint_id` field propagation).

2. **Single breakpoint per pause**: The `PauseBreakpoint` VM event can include multiple breakpoints (`pause_breakpoints: Vec<Breakpoint>`). This implementation uses only the first breakpoint (`breakpoint.as_ref().map(|bp| bp.id.clone())`). In practice, simultaneous multi-breakpoint hits are rare, and using the primary breakpoint is consistent with the existing behavior.
