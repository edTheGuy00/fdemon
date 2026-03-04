## Task: Handle Toggle During Transitional States

**Objective**: Make `ToggleDap` a no-op when `DapStatus` is `Starting` or `Stopping`, preventing surprising re-spawn or re-stop behavior during transitions.

**Depends on**: merge (post-merge improvement)

**Priority**: MEDIUM

**Review Source**: REVIEW.md Issue #5 (Logic Reasoning Checker)

### Scope

- `crates/fdemon-app/src/handler/dap.rs`: Update `handle_toggle` logic, add tests

### Background

`handle_toggle` at `dap.rs:46-52` currently has two branches:

```rust
fn handle_toggle(state: &mut AppState) -> UpdateResult {
    if state.dap_status.is_running() {
        handle_stop(state)
    } else {
        handle_start(state)
    }
}
```

Since `is_running()` only returns `true` for `Running`:
- `Starting` -> falls into `else` -> calls `handle_start` -> re-emits `SpawnDapServer` (after Task 02 fix, this becomes a no-op, but the intent is still unclear)
- `Stopping` -> falls into `else` -> calls `handle_start` -> sets status to `Starting` and emits `SpawnDapServer` while the previous stop is still in progress

Neither behavior is intuitive for a toggle. A toggle during a transition should be ignored — the user pressed the key, but the system is already processing a state change.

### Details

Change `handle_toggle` to:

```rust
fn handle_toggle(state: &mut AppState) -> UpdateResult {
    match state.dap_status {
        DapStatus::Running { .. } => handle_stop(state),
        DapStatus::Off => handle_start(state),
        DapStatus::Starting | DapStatus::Stopping => UpdateResult::none(),
    }
}
```

This is more explicit than the `if/else` and handles all four states clearly.

### Acceptance Criteria

1. `ToggleDap` when `Off` -> starts server (existing behavior, unchanged)
2. `ToggleDap` when `Running` -> stops server (existing behavior, unchanged)
3. `ToggleDap` when `Starting` -> no-op (returns `UpdateResult::none()`)
4. `ToggleDap` when `Stopping` -> no-op (returns `UpdateResult::none()`)
5. Existing toggle tests pass (update `test_toggle_when_off_starts` and `test_toggle_when_running_stops`)
6. New tests for `Starting` and `Stopping` states

### Testing

Add to the inline test module in `dap.rs`:

```rust
#[test]
fn test_toggle_when_starting_is_noop() {
    let mut state = test_state();
    state.dap_status = DapStatus::Starting;
    let result = handle_dap_message(&mut state, &Message::ToggleDap);
    assert_eq!(state.dap_status, DapStatus::Starting);
    assert!(result.action.is_none());
}

#[test]
fn test_toggle_when_stopping_is_noop() {
    let mut state = test_state();
    state.dap_status = DapStatus::Stopping;
    let result = handle_dap_message(&mut state, &Message::ToggleDap);
    assert_eq!(state.dap_status, DapStatus::Stopping);
    assert!(result.action.is_none());
}
```

### Notes

- This task pairs well with Task 02 (guard-start-starting-state). After both are applied, the double-start path is fully closed: both `StartDapServer` and `ToggleDap` are no-ops during `Starting`.
- The explicit `match` is preferred over `if/else` per CODE_STANDARDS.md: "Exhaustive matches, avoid catch-all `_` when variants matter."

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/dap.rs` | Replaced `if/else` in `handle_toggle` with an exhaustive `match` on all four `DapStatus` variants; added `test_toggle_when_starting_is_noop` and `test_toggle_when_stopping_is_noop` |

### Notable Decisions/Tradeoffs

1. **Exact task specification followed**: The `match` arms are in the order specified by the task (`Running { .. } => stop`, `Off => start`, `Starting | Stopping => none()`), which reads naturally: active state triggers the opposite action, transitional states are silently ignored.
2. **No test updates needed for existing tests**: `test_toggle_when_off_starts` and `test_toggle_when_running_stops` were already correct and continued to pass unchanged — the new `match` preserves the same behavior for those two cases.

### Testing Performed

- `cargo test -p fdemon-app` - Passed (1264 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: This is a pure behavior fix with no side effects. The two transitional states (`Starting`, `Stopping`) were previously mishandled; now they are explicitly no-ops.
