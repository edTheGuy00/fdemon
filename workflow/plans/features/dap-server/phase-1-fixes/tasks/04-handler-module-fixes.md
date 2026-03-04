## Task: Fix handler module visibility, BreakpointRemoved handler, and add intent comments

**Objective**: Fix the `debug` module's visibility to match peer modules, add `untrack_breakpoint` to the `BreakpointRemoved` handler, and add intent comments on no-op match arms.

**Depends on**: None

**Review Issues**: #4 (module visibility), #6 (BreakpointRemoved handler), #8 (no-op comments)

### Scope

- `crates/fdemon-app/src/handler/devtools/mod.rs`:
  - Change `pub mod debug;` to `pub(crate) mod debug;` to match `network` and `performance`

- `crates/fdemon-app/src/handler/devtools/debug.rs`:
  - In the `BreakpointRemoved` arm (around line 96-98): add `handle.session.debug.untrack_breakpoint(&breakpoint.id);`
  - Add intent comments to the `BreakpointRemoved` and `BreakpointUpdated` no-op arms, matching the comment style of `BreakpointAdded`

### Details

**Issue #4 — Module visibility:**

Current state in `handler/devtools/mod.rs`:
```rust
pub mod debug;           // ← inconsistent
pub mod inspector;
pub(crate) mod network;
pub(crate) mod performance;
```

Fix:
```rust
pub(crate) mod debug;    // ← matches network and performance
pub mod inspector;
pub(crate) mod network;
pub(crate) mod performance;
```

**Issue #6 — BreakpointRemoved handler:**

Current code in `debug.rs`:
```rust
DebugEvent::BreakpointRemoved { breakpoint, .. } => {
    tracing::debug!("Breakpoint removed: {}", breakpoint.id);
}
```

Fixed:
```rust
DebugEvent::BreakpointRemoved { breakpoint, .. } => {
    // Remove from tracked breakpoints so DebugState stays consistent
    // with the VM. This covers VM-initiated removals (e.g., hot restart
    // clearing breakpoints) in addition to user-initiated ones.
    handle.session.debug.untrack_breakpoint(&breakpoint.id);
    tracing::debug!("Breakpoint removed: {}", breakpoint.id);
}
```

**Issue #8 — Intent comments on no-op arms:**

Add brief comments explaining why each arm is intentionally a no-op (or now has minimal logic), matching the style used for `BreakpointAdded`:
```rust
DebugEvent::BreakpointUpdated { breakpoint, .. } => {
    // Breakpoint metadata updates (e.g., resolved location) are informational.
    // Full breakpoint sync will be implemented in Phase 3 (DAP adapter).
    tracing::debug!("Breakpoint updated: {}", breakpoint.id);
}
```

### Acceptance Criteria

1. `debug` module visibility is `pub(crate)` in `handler/devtools/mod.rs`
2. `BreakpointRemoved` arm calls `handle.session.debug.untrack_breakpoint(&breakpoint.id)`
3. `BreakpointRemoved` and `BreakpointUpdated` arms have intent comments
4. `cargo check --workspace` passes (no visibility errors)
5. `cargo test --workspace` passes

### Testing

- Add a unit test in `debug.rs` tests that verifies `BreakpointRemoved` event causes the breakpoint to be removed from `DebugState`:

```rust
#[test]
fn test_breakpoint_removed_untracks() {
    // Setup: track a breakpoint in DebugState
    // Dispatch BreakpointRemoved event
    // Assert: breakpoint is no longer in DebugState
}
```

### Notes

- The `untrack_breakpoint` method already exists and is tested in `debug_state.rs`. This task just wires it into the event handler.
- `inspector` is `pub` — this may or may not be intentional. This task only fixes `debug` to match `network`/`performance`. If `inspector` should also be `pub(crate)`, that's a separate concern.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Changed `pub mod debug;` to `pub(crate) mod debug;` to match `network` and `performance` |
| `crates/fdemon-app/src/handler/devtools/debug.rs` | Added `handle.session.debug.untrack_breakpoint(&breakpoint.id)` in `BreakpointRemoved` arm; added intent comments on `BreakpointRemoved` and `BreakpointUpdated` arms; added `test_breakpoint_removed_untracks` unit test |

### Notable Decisions/Tradeoffs

1. **Pre-existing compilation breakage from Task 01**: The `parse_debug_event` / `parse_isolate_event` function signatures were already updated to `&StreamEvent` (partial Task 01 work) but the tests in `debugger_types.rs` still used the old 2-arg form. Since Task 04 requires `cargo test --workspace` to pass, and the `debugger_types.rs` tests were discovered to have already been updated (the file was modified by the linter/system), the workspace compiled and tested cleanly. The `actions/vm_service.rs` call sites were also already updated.

2. **`BreakpointRemoved` now calls `untrack_breakpoint`**: The untrack call is placed before the tracing log to ensure the state mutation happens even if there's a future early return added. This matches the pattern used in `mark_breakpoint_verified` for `BreakpointResolved`.

3. **Intent comments match existing style**: The comments on `BreakpointUpdated` follow the same pattern as the pre-existing comment on `BreakpointAdded` — explaining why the handler is limited for now and referencing the future phase that will complete the implementation.

### Testing Performed

- `cargo fmt --all` - Passed (no changes needed)
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all tests pass)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Task 01 partial state**: The `debugger_types.rs` tests were already updated to use `StreamEvent` before this task ran (the system/linter had applied those changes). This means the Task 01 acceptance criterion for test updates is partially done, but the full Task 01 (integration tests, `parse_isolate_ref` removal) still needs completion.
