## Task: Fix IsolateRunnable event mistranslation

**Objective**: Fix the bug where `IsolateEvent::IsolateRunnable` is translated to `DapDebugEvent::IsolateStart` instead of `DapDebugEvent::IsolateRunnable`, which makes breakpoint re-application after hot restart non-functional.

**Depends on**: None

**Severity**: Critical (blocking merge)

### Scope

- `crates/fdemon-app/src/handler/devtools/debug.rs`: Fix match arm at line ~263

### Details

In `handle_isolate_event`, the match on `IsolateEvent::IsolateRunnable` currently produces the wrong `DapDebugEvent` variant:

**Current (broken):**
```rust
IsolateEvent::IsolateRunnable { isolate } => Some(DapDebugEvent::IsolateStart {
    isolate_id: isolate.id.clone(),
    name: isolate.name.clone().unwrap_or_default(),
}),
```

**Fixed:**
```rust
IsolateEvent::IsolateRunnable { isolate } => Some(DapDebugEvent::IsolateRunnable {
    isolate_id: isolate.id.clone(),
}),
```

Note: `DapDebugEvent::IsolateRunnable` only carries `isolate_id` (no `name` field), unlike `IsolateStart` which carries both. The `DapDebugEvent::IsolateRunnable` variant is defined at `adapter/mod.rs:583`.

**Why this matters:** The adapter's breakpoint re-application handler (`adapter/mod.rs:1127-1231`) matches on `DebugEvent::IsolateRunnable`. Because this variant is never produced by the upstream translator, the handler is dead code — breakpoints are silently lost after every hot restart.

### Acceptance Criteria

1. `IsolateEvent::IsolateRunnable` produces `DapDebugEvent::IsolateRunnable { isolate_id }` (not `IsolateStart`)
2. Existing tests in `debug.rs` still pass
3. Add a test that verifies `IsolateRunnable` events are forwarded with the correct variant (not as `IsolateStart`)
4. `cargo test -p fdemon-app` — all tests pass

### Testing

```rust
#[test]
fn test_isolate_runnable_produces_correct_dap_event() {
    // Send an IsolateEvent::IsolateRunnable through handle_isolate_event
    // Verify the forwarded event is DapDebugEvent::IsolateRunnable (not IsolateStart)
}
```

### Notes

- This is a one-line fix but has high impact — it unblocks Task 10 (breakpoint persistence) from Phase 4
- The `DapDebugEvent::IsolateRunnable` variant exists and is well-documented in the adapter
