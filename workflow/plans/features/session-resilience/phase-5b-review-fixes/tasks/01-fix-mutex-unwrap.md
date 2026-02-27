## Task: Fix Mutex Lock `unwrap()` in Action Dispatcher

**Objective**: Replace the panicking `.lock().unwrap()` on the session task map mutex in `mod.rs` with defensive `match` handling, consistent with the pattern already used in `session.rs`.

**Depends on**: None

**Review Issue**: #1 (Major — all 4 review agents flagged this)

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Line 160 — replace `.lock().unwrap()` with `match .lock()`

### Details

The `ConnectVmService` action arm at `mod.rs:158-161` panics if the mutex is poisoned:

```rust
UpdateAction::ConnectVmService { session_id, ws_uri } => {
    let handle = vm_service::spawn_vm_service_connection(session_id, ws_uri, msg_tx);
    session_tasks.lock().unwrap().insert(session_id, handle);
}
```

This is inconsistent with `session.rs`, which handles the same mutex defensively in two places:
- `session.rs:221-228` — `if let Ok(mut guard) = session_tasks_clone.lock()` (cleanup path)
- `session.rs:233-248` — `match session_tasks.lock()` (registration path)

**Replace with:**

```rust
UpdateAction::ConnectVmService { session_id, ws_uri } => {
    let handle = vm_service::spawn_vm_service_connection(session_id, ws_uri, msg_tx);
    match session_tasks.lock() {
        Ok(mut guard) => {
            guard.insert(session_id, handle);
        }
        Err(e) => {
            warn!(
                "ConnectVmService: could not track VM task for session {} \
                 (poisoned lock): {}",
                session_id, e
            );
        }
    }
}
```

### Acceptance Criteria

1. No `.lock().unwrap()` calls anywhere in the `actions/` directory
2. The `ConnectVmService` arm uses `match session_tasks.lock()` with a `warn!` on `Err`
3. Pattern matches `session.rs:233-248` style
4. `cargo clippy --workspace -- -D warnings` clean
5. `cargo test -p fdemon-app` passes

### Testing

No new tests needed — this is a defensive error-handling change. The mutex poisoning scenario requires a thread panic, which is not practical to unit test. The behavioral correctness is that the application logs a warning instead of panicking.

### Notes

- This is the only `unwrap()` on a mutex lock in the entire `actions/` directory. All other lock sites already use defensive patterns.
- The `warn!` log level matches the existing pattern in `session.rs:226`.
- The `tracing::warn` macro is already imported in `mod.rs`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/mod.rs` | Replaced `.lock().unwrap()` with `match session_tasks.lock()` in the `ConnectVmService` arm; `Ok` branch inserts the handle, `Err` branch emits a `warn!` log. |

### Notable Decisions/Tradeoffs

1. **Exact pattern from task spec**: The replacement matches the spec verbatim, including the warning message text and continuation backslash line-break, which also mirrors the style in `session.rs:233-248`.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Passed
- `cargo test -p fdemon-app` - Passed (1163 unit tests, 1 doctest, 0 failures)

### Risks/Limitations

1. **Mutex poisoning is not unit-tested**: As noted in the task, inducing a poisoned mutex requires a panicking thread, which is impractical to test in isolation. The defensive branch exists to prevent a cascade panic; its correctness is verified by code review against the established pattern in `session.rs`.
