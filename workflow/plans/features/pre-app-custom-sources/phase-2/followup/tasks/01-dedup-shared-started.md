## Task: Add Deduplication Guard to `SharedSourceStarted` Handler

**Objective**: Close the TOCTOU race window that allows duplicate shared source processes by adding a duplicate-name check inside the `SharedSourceStarted` handler. If a shared source with the same name is already registered, the incoming duplicate must be shut down immediately.

**Depends on**: None

**Severity**: MAJOR

**Review Reference**: [REVIEW.md](../../../../reviews/features/pre-app-custom-sources-phase-2/REVIEW.md) — "MAJOR — No deduplication guard on `SharedSourceStarted` handler"

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Add dedup guard in `SharedSourceStarted` match arm (~line 2328)
- `crates/fdemon-app/src/handler/tests.rs`: Add test `test_shared_source_started_duplicate_is_rejected`

### Context

The `SharedSourceStarted` handler at `update.rs:2323–2350` unconditionally pushes a new `SharedSourceHandle` onto `state.shared_source_handles`. The spawn-side guard (`running_shared_names.contains()` in `native_logs.rs:516–533`) reduces the TOCTOU window but cannot eliminate it because the check-and-spawn happen asynchronously:

```
Session A: snapshot running_shared_names = []  → SpawnPreAppSources
Session B: snapshot running_shared_names = []  → SpawnPreAppSources  (stale — A hasn't reported yet)
Coordinator A: spawns "logcat" → SharedSourceStarted { name: "logcat" }  → push (len=1)
Coordinator B: spawns "logcat" → SharedSourceStarted { name: "logcat" }  → push (len=2, DUPLICATE)
```

The method `state.is_shared_source_running(&name)` already exists at `state.rs:1265–1268` and does the right check. It is currently used only on the spawn-gating side, not in the handler itself.

### Details

In the `Message::SharedSourceStarted` match arm (`update.rs` ~line 2328), add a dedup guard **before** the `task_handle.lock()` extraction and the `.push()`:

```rust
Message::SharedSourceStarted {
    name,
    shutdown_tx,
    task_handle,
    start_before_app,
} => {
    // ── Dedup guard: close TOCTOU window ──────────────────────────
    // The spawn-side check (running_shared_names snapshot) reduces but
    // cannot eliminate duplicate spawns.  Since update() is single-
    // threaded, this handler-side check is the authoritative gate.
    if state.is_shared_source_running(&name) {
        tracing::warn!(
            "Duplicate SharedSourceStarted for '{}' — shutting down extra process",
            name
        );
        let _ = shutdown_tx.send(true);
        if let Some(task) = task_handle.lock().ok().and_then(|mut s| s.take()) {
            task.abort();
        }
        return UpdateResult::none();
    }

    // (existing code below — extract JoinHandle, push, log info)
    let extracted = task_handle.lock().ok().and_then(|mut slot| slot.take());
    state
        .shared_source_handles
        .push(crate::session::SharedSourceHandle {
            name: name.clone(),
            shutdown_tx,
            task_handle: extracted,
            start_before_app,
        });

    tracing::info!("Shared source '{}' started", name);
    UpdateResult::none()
}
```

Key implementation notes:
- The guard must come **before** extracting the `JoinHandle` from `task_handle`, because the extraction takes the `Option` via `.take()` — doing this first would leave nothing to abort in the rejection path.
- `shutdown_tx.send(true)` signals the duplicate process to shut down gracefully.
- `task.abort()` is a belt-and-suspenders kill in case the process ignores the shutdown signal.
- `return UpdateResult::none()` skips the push entirely.

### Acceptance Criteria

1. If `SharedSourceStarted` arrives for a name already in `state.shared_source_handles`, the handler:
   - Does NOT push a second handle
   - Sends `true` on the incoming `shutdown_tx`
   - Aborts the incoming `task_handle`
   - Logs a `tracing::warn!`
   - Returns `UpdateResult::none()`
2. If the name is not already running, behavior is unchanged (push, log info).
3. New test `test_shared_source_started_duplicate_is_rejected` passes.

### Testing

Add a test in `handler/tests.rs` following the existing `test_shared_source_started_stores_handle` pattern (line ~8613). The test needs a tokio runtime because it creates `JoinHandle`s.

```rust
#[test]
fn test_shared_source_started_duplicate_is_rejected() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut state = make_test_state();
        // ... set up state with sessions as needed ...

        // First SharedSourceStarted — should succeed
        let (msg1, _shutdown_rx1) = make_shared_source_started("my-source", tokio::spawn(async {}));
        let result1 = update(&mut state, msg1);
        assert!(result1.action.is_none());
        assert_eq!(state.shared_source_handles.len(), 1);

        // Second SharedSourceStarted with the SAME name — should be rejected
        let (shutdown_tx2, mut shutdown_rx2) = tokio::sync::watch::channel(false);
        let task2 = tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(60)).await });
        let task_slot2 = std::sync::Arc::new(std::sync::Mutex::new(Some(task2)));
        let msg2 = Message::SharedSourceStarted {
            name: "my-source".to_string(),
            shutdown_tx: std::sync::Arc::new(shutdown_tx2),
            task_handle: task_slot2,
            start_before_app: true,
        };
        let result2 = update(&mut state, msg2);

        // Dedup guard should have fired
        assert!(result2.action.is_none());
        assert_eq!(state.shared_source_handles.len(), 1); // still 1, not 2
        assert_eq!(state.shared_source_handles[0].name, "my-source");

        // The duplicate's shutdown channel should have received `true`
        assert_eq!(*shutdown_rx2.borrow(), true);
    });
}
```

Adapt the helper usage and shutdown_rx assertion to match the actual test infrastructure in `tests.rs`. The existing `make_shared_source_started` helper (line ~8400) may need to be extended or bypassed to get access to the `shutdown_rx` for assertion.

### Notes

- The `update()` function is single-threaded (TEA model), so the `is_shared_source_running` check inside the handler is race-free — this is the authoritative dedup point.
- The spawn-side `running_shared_names` guard remains valuable as an optimization (avoids spawning a process that would immediately be killed), but it is not sufficient on its own.

---

## Completion Summary

**Status:** Not Started
