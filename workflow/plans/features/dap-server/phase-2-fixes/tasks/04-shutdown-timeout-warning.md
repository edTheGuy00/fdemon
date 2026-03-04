## Task: Log Warning and Abort on Shutdown Timeout

**Objective**: Make the DAP server shutdown timeout observable by logging a warning and aborting the stuck task instead of silently abandoning it.

**Depends on**: None

**Priority**: MEDIUM (pre-merge)

**Review Source**: REVIEW.md Issue #6 (Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-dap/src/service.rs`: Improve `DapService::stop` timeout handling

### Background

`DapService::stop` at `service.rs:88-95` currently discards the timeout result:

```rust
pub async fn stop(handle: DapServerHandle) {
    let _ = handle.shutdown_tx.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle.task).await;
}
```

If the 5-second timeout fires:
1. The `JoinHandle` future is dropped but the spawned task continues running (tokio does not cancel tasks on `JoinHandle` drop)
2. No diagnostic is logged — the timeout is completely invisible
3. The server task leaks until the process exits

### Details

Replace the `stop` method body with:

```rust
pub async fn stop(handle: DapServerHandle) {
    // Signal shutdown. If the receiver is already gone — fine, ignore error.
    let _ = handle.shutdown_tx.send(true);

    // Wait for the accept-loop task to finish with a generous timeout.
    match tokio::time::timeout(std::time::Duration::from_secs(5), handle.task).await {
        Ok(Ok(())) => {
            // Task completed normally.
        }
        Ok(Err(join_err)) => {
            tracing::warn!("DAP server task panicked during shutdown: {}", join_err);
        }
        Err(_elapsed) => {
            tracing::warn!("DAP server task did not complete within 5s shutdown timeout");
            handle.task.abort();
        }
    }
}
```

Wait — `handle.task` is moved into the `timeout` future and consumed, so we can't call `abort()` on it after the timeout. The `timeout` returns the inner `JoinHandle`'s result on success, but on `Err(_elapsed)` the `JoinHandle` is dropped (which does NOT abort the task).

To abort on timeout, we need to restructure slightly:

```rust
pub async fn stop(handle: DapServerHandle) {
    let _ = handle.shutdown_tx.send(true);

    let timeout_result =
        tokio::time::timeout(std::time::Duration::from_secs(5), &mut handle.task).await;

    match timeout_result {
        Ok(Ok(())) => {}
        Ok(Err(join_err)) => {
            tracing::warn!("DAP server task panicked during shutdown: {}", join_err);
        }
        Err(_elapsed) => {
            tracing::warn!("DAP server task did not complete within 5s shutdown timeout");
            handle.task.abort();
        }
    }
}
```

Actually, `tokio::time::timeout` takes ownership of the future. Since `JoinHandle` implements `Future`, we need to pin it. A simpler approach: just log the warning without abort. The task will be cleaned up when the process exits, and the warning makes the timeout observable for debugging.

**Simplest correct approach:**

```rust
pub async fn stop(handle: DapServerHandle) {
    let _ = handle.shutdown_tx.send(true);

    if tokio::time::timeout(
        std::time::Duration::from_secs(5),
        handle.task,
    )
    .await
    .is_err()
    {
        tracing::warn!("DAP server task did not complete within 5s shutdown timeout");
    }
}
```

This logs the warning when the timeout fires. The dropped `JoinHandle` does not abort the task, but the shutdown signal was already sent via `shutdown_tx` — if the task didn't respond within 5s, it's likely stuck in an OS-level accept/read and will be cleaned up at process exit.

### Acceptance Criteria

1. `DapService::stop` logs a `tracing::warn!` when the 5s timeout fires
2. Normal shutdown (task completes within timeout) produces no warning
3. Existing tests pass — the `test_stop_sends_shutdown` test should still work
4. `cargo test -p fdemon-dap` passes
5. `cargo clippy -p fdemon-dap -- -D warnings` clean

### Testing

The existing `test_stop_sends_shutdown` test in `service.rs` verifies that `stop()` completes without error for a well-behaved server. No new test is needed for the timeout path — it would require a server that deliberately hangs, which is fragile in CI. The warning is a diagnostic aid, not a correctness requirement.

### Notes

- The `let _ =` on `shutdown_tx.send()` is correct and should remain — the receiver may already be dropped if the server exited early.
- A future enhancement (Task 07) could use `tokio::select!` with an `abort()` call, but the simple warning is sufficient for Phase 2.
