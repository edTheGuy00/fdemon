## Task: Add Idempotency Guard to handle_session_exited

**Objective**: Add an early return to `handle_session_exited` when the session is already `Stopped`, preventing duplicate log entries if the handler is called twice for the same session.

**Depends on**: Task 02 (the idempotency guard is defense-in-depth for the duplicate exit race fix)

**Review Reference**: Phase-3 Review Issues #5, #7

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Add early return (~line 96)
- `crates/fdemon-app/src/handler/tests.rs`: Add double-exit idempotency test

### Details

#### Problem

`handle_session_exited` (session.rs:95-152) has no check for whether the session is already in `AppPhase::Stopped`. While most operations inside the handler are safe to repeat (`.take()` on Options returns `None` on the second call, `phase = Stopped` is idempotent), `handle.session.add_log(...)` is **not** idempotent — calling the handler twice produces two "Flutter process exited" log entries.

Even after task 02 fixes the watchdog race, this guard is valuable as defense-in-depth: if any future code path accidentally sends a duplicate `Exited` event, the handler gracefully ignores it.

#### Current code (session.rs:95-98)

```rust
pub fn handle_session_exited(state: &mut AppState, session_id: SessionId, code: Option<i32>) {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let (level, message) = match code {
            // ...
```

#### Fix

Add an early return after obtaining the session handle:

```rust
pub fn handle_session_exited(state: &mut AppState, session_id: SessionId, code: Option<i32>) {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Guard: ignore duplicate exit events — the session is already stopped.
        if handle.session.phase == AppPhase::Stopped {
            return;
        }

        let (level, message) = match code {
            // ... rest unchanged
```

#### Double-exit test

Add a test that sends two `DaemonEvent::Exited` events to the same session and verifies:
1. No panic
2. Only one "exited" log entry exists
3. Session phase is `Stopped` after both

```rust
#[test]
fn test_handle_session_exited_duplicate_exit_is_idempotent() {
    let mut state = AppState::new();
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // First exit: should process normally
    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(0) },
        },
    );

    // Second exit: should be silently ignored
    update(
        &mut state,
        Message::SessionDaemon {
            session_id,
            event: DaemonEvent::Exited { code: Some(1) },
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    assert_eq!(handle.session.phase, AppPhase::Stopped);

    // Only one exit log entry should exist (from the first exit, not the second)
    let exit_logs: Vec<_> = handle
        .session
        .logs
        .iter()
        .filter(|e| e.message.contains("exited"))
        .collect();
    assert_eq!(
        exit_logs.len(),
        1,
        "duplicate exit should not add a second log entry"
    );
    assert!(
        exit_logs[0].message.contains("exited normally"),
        "the first exit (code 0) log should be preserved, not overwritten by code 1"
    );
}
```

### Acceptance Criteria

1. `handle_session_exited` returns early when `handle.session.phase == AppPhase::Stopped`
2. A test sends two `Exited` events and verifies only one log entry
3. The first exit's code is preserved (not overwritten by the second)
4. `cargo check --workspace` passes
5. `cargo clippy --workspace -- -D warnings` clean
6. `cargo test -p fdemon-app` passes

### Notes

- This is a common "at-most-once" handler pattern. The `AppPhase::Stopped` check is the canonical guard since the handler's purpose is to transition the session to `Stopped`
- The `.take()` pattern on `vm_shutdown_tx`, `perf_task_handle`, etc. provides partial idempotency but not for the log entry
- Consider whether `debug!()` logging on the early return path is useful for diagnostics (optional)
