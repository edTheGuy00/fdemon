## Task: Guard handle_start Against Starting State

**Objective**: Prevent double `StartDapServer` from spawning orphaned server tasks by extending the guard in `handle_start` to also reject the `Starting` state.

**Depends on**: None

**Priority**: MEDIUM (pre-merge)

**Review Source**: REVIEW.md Issue #2 (Logic Reasoning Checker, Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-app/src/handler/dap.rs`: Fix guard in `handle_start`, update test

### Background

`handle_start` at `dap.rs:28-36` guards against re-starting with:

```rust
if state.dap_status.is_running() {
    return UpdateResult::none();
}
```

`is_running()` only returns `true` for `DapStatus::Running { .. }` (see `state.rs:787-789`). When the status is `Starting`, `is_running()` returns `false`, so a second `StartDapServer` message will:

1. Set status to `Starting` again (no-op, already `Starting`)
2. Return `UpdateAction::SpawnDapServer` — spawning a **second** server task
3. The first server's handle gets overwritten when `DapServerStarted` arrives, orphaning the first task

The existing test `test_start_when_starting_transitions_to_starting_with_action` (line 168) documents this behavior but does not prevent it.

### Details

#### 1. Extend the Guard

Change `handle_start` (line 29) from:

```rust
if state.dap_status.is_running() {
    return UpdateResult::none();
}
```

to:

```rust
if state.dap_status.is_running() || state.dap_status == DapStatus::Starting {
    return UpdateResult::none();
}
```

#### 2. Update Existing Test

The test `test_start_when_starting_transitions_to_starting_with_action` (line 168) currently asserts that `StartDapServer` while `Starting` produces a `SpawnDapServer` action. Update it to assert the correct new behavior: no action is returned.

Rename the test to `test_start_when_starting_is_noop` and change assertions:

```rust
#[test]
fn test_start_when_starting_is_noop() {
    let mut state = test_state();
    state.dap_status = DapStatus::Starting;
    let result = handle_dap_message(&mut state, &Message::StartDapServer);
    assert_eq!(state.dap_status, DapStatus::Starting);
    assert!(result.action.is_none(), "Should not spawn a second server");
}
```

### Acceptance Criteria

1. `StartDapServer` when `DapStatus::Starting` returns `UpdateResult::none()` (no action)
2. `StartDapServer` when `DapStatus::Running` still returns `UpdateResult::none()` (unchanged)
3. `StartDapServer` when `DapStatus::Off` still transitions to `Starting` with `SpawnDapServer` action
4. All existing tests pass (23 handler tests)
5. `cargo test -p fdemon-app` passes

### Testing

Update the existing test and verify the other start/stop tests still pass. No new test file needed — all tests are inline in `dap.rs`.

### Notes

- This is a one-line fix plus a test update.
- The `Stopping` state is not guarded here because `StartDapServer` while `Stopping` is a less likely race (the stop flow is initiated by the user and completes quickly). If desired, Task 06 (toggle-transitional-states) addresses this more comprehensively post-merge.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/dap.rs` | Extended guard in `handle_start` to also reject `DapStatus::Starting`; renamed and updated test `test_start_when_starting_transitions_to_starting_with_action` to `test_start_when_starting_is_noop` asserting no action is returned |

### Notable Decisions/Tradeoffs

1. **Guard condition uses equality check**: `state.dap_status == DapStatus::Starting` is the direct, readable approach. `DapStatus` derives `PartialEq` so this compiles cleanly without any additional infrastructure.

### Testing Performed

- `cargo test -p fdemon-app` - Passed (1262 unit tests + 1 doc test)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Stopping state not guarded**: Per task notes, `StartDapServer` while `Stopping` is not guarded here — addressed by Task 06 if needed.
