## Task: Flush Batched Logs in `SharedSourceStopped` Handler

**Objective**: Fix the `SharedSourceStopped` handler to check the `queue_log()` return value and call `flush_batched_logs()` when needed, matching the pattern used at every other log-queueing site in the handler layer.

**Depends on**: None

**Severity**: MINOR

**Review Reference**: [REVIEW.md](../../../../reviews/features/pre-app-custom-sources-phase-2/REVIEW.md) — "MINOR — `SharedSourceStopped` does not flush batched logs"

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Fix flush in `SharedSourceStopped` match arm (~line 2361)
- `crates/fdemon-app/src/handler/tests.rs`: Update `test_shared_source_stopped_removes_handle_and_warns` (~line 8670)

### Context

There are three `queue_log()` call sites in `update.rs`:

| Line | Handler | Checks return? | Calls flush? |
|------|---------|----------------|--------------|
| 2007-2008 | `NativeLog` | YES | YES |
| 2316-2317 | `SharedSourceLog` | YES | YES |
| **2367** | **`SharedSourceStopped`** | **NO** | **NO** |

The `SharedSourceStopped` handler is the only site that discards the boolean return value. The warning log entry is buffered in the `LogBatcher` and only appears in `session.logs` when the engine's next `flush_pending_logs()` call runs (before rendering). While not a correctness bug (the log does eventually appear), it breaks the established pattern and could delay the warning by one tick.

### Details

**Code change in `update.rs`** — replace the `queue_log` call inside the for-loop of the `SharedSourceStopped` arm:

Current code (~line 2367):
```rust
handle.session.queue_log(entry);
```

New code:
```rust
if handle.session.queue_log(entry) {
    handle.session.flush_batched_logs();
}
```

This matches the exact pattern used by `NativeLog` (line 2007-2008) and `SharedSourceLog` (line 2316-2317).

**Test update in `tests.rs`** — the existing test `test_shared_source_stopped_removes_handle_and_warns` (line ~8670) currently calls `state.session_manager.flush_all_pending_logs()` at line 8711 before asserting. This manual flush masks the bug. Update the test:

1. **Remove** the `state.session_manager.flush_all_pending_logs()` call at line 8711.
2. The assertions on `logs_a` and `logs_b` should now pass **without** the manual flush, because the handler itself flushes eagerly.
3. This proves the handler correctly flushes on its own.

### Acceptance Criteria

1. `SharedSourceStopped` handler checks the `queue_log()` return value and calls `flush_batched_logs()` when it returns `true`.
2. The existing test `test_shared_source_stopped_removes_handle_and_warns` passes **without** calling `flush_all_pending_logs()` before asserting.
3. All existing tests continue to pass.

### Testing

The existing test at `tests.rs:8670` is the primary verification. After removing the manual flush call, it should still find the warning log entries in both sessions' logs. If it fails, the code change is incorrect. No new test file needed.

### Notes

- The `LogBatcher` uses a 100-entry threshold (`BATCH_MAX_SIZE`) and a 16ms time threshold (`BATCH_FLUSH_INTERVAL`). Since the `SharedSourceStopped` handler typically queues only one entry per session, the threshold won't trigger on its own in most cases — making the explicit `flush_batched_logs()` call the only reliable way to make the log immediately visible.
- In tests, the `LogBatcher` time threshold is almost certainly met (since tests run instantly relative to 16ms), so the `should_flush()` check returns `true` and the test should pass after removing the manual flush. If timing sensitivity is a concern, the test can call `queue_log` directly with a fresh batcher that has an elapsed interval.

---

## Completion Summary

**Status:** Not Started
