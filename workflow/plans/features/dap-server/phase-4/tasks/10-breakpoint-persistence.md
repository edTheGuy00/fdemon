## Task: Persist Breakpoints Across Hot Restart

**Objective**: When a hot restart occurs (which creates a new Dart isolate with new IDs), automatically re-apply all DAP breakpoints to the new isolate. Without this, all breakpoints are lost after hot restart and the user must re-set them manually.

**Depends on**: 02-hot-reload-restart-dap

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-dap/src/adapter/breakpoints.rs`: Store the "desired" breakpoint state independently of VM breakpoint IDs
- `crates/fdemon-dap/src/adapter/mod.rs`: On `IsolateStart`/`IsolateRunnable` after hot restart, re-apply all breakpoints
- `crates/fdemon-dap/src/adapter/mod.rs`: Invalidate variable references and frame IDs on restart

### Details

#### The Problem

1. IDE sets breakpoints via `setBreakpoints` → adapter calls `addBreakpointWithScriptUri` → VM assigns breakpoint IDs
2. User triggers hot restart → old isolate destroyed, new isolate created
3. All VM breakpoint IDs from the old isolate are now invalid
4. IDE does NOT re-send `setBreakpoints` — it expects the adapter to handle persistence

#### Desired Breakpoint State

Maintain a "desired" breakpoint list separate from the "active" (VM-tracked) breakpoints:

```rust
pub struct BreakpointManager {
    /// What the IDE wants — survives hot restart
    desired: HashMap<String, Vec<DesiredBreakpoint>>,  // file_uri → breakpoints

    /// What's currently set in the VM — invalidated on hot restart
    active: HashMap<String, Vec<ActiveBreakpoint>>,    // file_uri → breakpoints

    /// Maps DAP breakpoint ID → VM breakpoint ID (invalidated on restart)
    dap_to_vm: HashMap<i64, String>,
}

struct DesiredBreakpoint {
    dap_id: i64,
    line: i32,
    column: Option<i32>,
    condition: Option<String>,
    hit_condition: Option<String>,
    log_message: Option<String>,
}

struct ActiveBreakpoint {
    dap_id: i64,
    vm_id: String,
    verified: bool,
}
```

#### Re-Application Flow

```
Hot restart triggered
  │
  ├── Old isolate exits → DebugEvent::IsolateExit
  │   └── Clear active breakpoints and dap_to_vm mapping
  │   └── Send "breakpoint changed" events with verified: false for all desired breakpoints
  │   └── Invalidate variable/frame reference stores (on_resume)
  │
  ├── New isolate starts → DebugEvent::IsolateStart
  │
  └── New isolate runnable → DebugEvent::IsolateRunnable (or PauseStart)
      │
      ├── For each file in desired breakpoints:
      │   ├── For each breakpoint:
      │   │   ├── addBreakpointWithScriptUri(new_isolate_id, uri, line, column?)
      │   │   ├── Map new VM breakpoint ID → existing DAP breakpoint ID
      │   │   └── Store in active breakpoints
      │   └── Emit "breakpoint changed" events with verified: true
      │
      ├── Re-apply exception pause mode
      │
      └── Resume isolate if it was paused at start
```

#### Exception Pause Mode Persistence

The exception pause mode (`All`, `Unhandled`, `None`) must also be re-applied to the new isolate:

```rust
// After re-applying breakpoints:
if let Some(mode) = &self.exception_mode {
    self.backend.set_exception_pause_mode(&new_isolate_id, mode).await.ok();
}
```

#### Breakpoint Changed Events

While breakpoints are being re-applied, send `breakpoint` events to keep the IDE's verification status accurate:

```json
// During restart — mark unverified:
{ "event": "breakpoint", "body": { "reason": "changed", "breakpoint": { "id": 1, "verified": false } } }

// After re-application — mark verified:
{ "event": "breakpoint", "body": { "reason": "changed", "breakpoint": { "id": 1, "verified": true, "line": 25 } } }
```

This gives the IDE visual feedback (breakpoint dots may briefly gray out during restart, then light up again).

### Acceptance Criteria

1. Breakpoints remain visible and active after hot restart
2. Breakpoint verification status updates during restart (unverified → verified)
3. Variable references are invalidated (old object IDs don't produce errors)
4. Exception pause mode is re-applied to new isolate
5. Conditional breakpoints and logpoints preserve their conditions through restart
6. No duplicate breakpoints after multiple restarts
7. 12+ new unit tests

### Testing

```rust
#[test]
fn test_desired_breakpoints_survive_restart() {
    let mut mgr = BreakpointManager::new();
    mgr.set_desired("file:///main.dart", vec![bp(1, 25), bp(2, 30)]);
    mgr.clear_active(); // Simulates hot restart
    assert_eq!(mgr.desired_for("file:///main.dart").len(), 2);
}

#[tokio::test]
async fn test_breakpoints_reapplied_on_new_isolate() {
    // Set up adapter with mock backend
    // Set breakpoints
    // Simulate IsolateExit → IsolateStart
    // Verify addBreakpointWithScriptUri called for each desired breakpoint
    // Verify new VM IDs mapped to existing DAP IDs
}

#[tokio::test]
async fn test_exception_mode_reapplied() {
    // Set exception mode to "All"
    // Simulate restart
    // Verify setExceptionPauseMode called on new isolate
}

#[test]
fn test_breakpoint_events_during_restart() {
    // Verify "changed" events sent with verified: false then verified: true
}
```

### Notes

- The `IsolateRunnable` event (not `IsolateStart`) is the correct trigger for re-applying breakpoints. The isolate must be fully initialized before breakpoints can be set.
- `addBreakpointWithScriptUri` may resolve to a different line than requested (due to recompilation). The `BreakpointResolved` event updates the actual line — forward this as a `breakpoint changed` event.
- Hot reload (not restart) does NOT invalidate breakpoints — VM breakpoint IDs survive hot reload. Only hot restart needs re-application.
- Variable reference invalidation (`on_resume()`) should clear frame stores, variable stores, AND source reference stores.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/breakpoints.rs` | Added `DesiredBreakpoint` struct, `DesiredBreakpointSpec` type alias, `BreakpointManager` struct with desired/active separation; added `drain_all` and `insert_with_id` methods to `BreakpointState`; 13 new tests |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `IsolateRunnable` variant to `DebugEvent`; added `desired_breakpoints` field to `DapAdapter`; updated `handle_set_breakpoints` to record desired state; updated `IsolateExit` handler to clear active and emit unverified events; added `IsolateRunnable` handler for re-application; updated `on_hot_restart` to also drain active breakpoints; exported new types; 9 new integration tests |

### Notable Decisions/Tradeoffs

1. **Kept `breakpoint_state: BreakpointState` alongside `desired_breakpoints`**: Instead of replacing `breakpoint_state` with a full `BreakpointManager`, I kept the existing `BreakpointState` field (used by ~20 call sites in tests and handlers) and added a separate `desired_breakpoints: HashMap<String, Vec<DesiredBreakpoint>>` field. This minimized churn to existing code while adding the desired/active separation. `BreakpointManager` is available as a standalone struct for callers who want the cleaner API.

2. **Step 0 + Step 3 two-pass desired sync in `handle_set_breakpoints`**: The desired breakpoints are first recorded with placeholder IDs in Step 0 before the active state is built, then re-synced with actual DAP IDs in Step 3 once the active state is populated. This ensures the desired state always has real stable IDs.

3. **`IsolateRunnable` added to `DebugEvent`**: This is the correct trigger per the task notes. The VM Service emits `IsolateRunnable` when the isolate is fully initialized and can receive breakpoint RPCs. Added the variant before `BreakpointResolved` in the match block.

4. **`on_hot_restart` also clears active breakpoints**: Since hot restart creates a new isolate, active breakpoints are invalid. The `IsolateExit` event handles clearing during normal restart flow; `on_hot_restart` is a safety net for cases where the adapter is reset without going through the event sequence.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (561 tests, 23 new)
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)
- `cargo fmt --all` - Passed

### Risks/Limitations

1. **Desired state uses second `set_desired` call for same URI replaces IDs**: When `handle_set_breakpoints` is called twice for the same URI, new DAP IDs are allocated. This is correct per DAP spec (setBreakpoints replaces the full set), but callers who cache DAP IDs will see new ones. This matches standard IDE behavior.

2. **`IsolateRunnable` must be forwarded by fdemon-app**: The `IsolateRunnable` variant is now defined in `DebugEvent` but the Engine integration layer in `fdemon-app` must forward the VM Service `IsolateRunnable` event through this channel. This wiring is outside the scope of this task.
