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

**Status:** Not Started
