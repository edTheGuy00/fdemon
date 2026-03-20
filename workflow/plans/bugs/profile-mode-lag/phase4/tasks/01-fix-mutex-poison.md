## Task: Fix Silent Mutex Poison Handling on Transfer-Slot Reads

**Objective**: Replace `.lock().ok().and_then(|mut g| g.take())` with explicit `match` that logs a warning and recovers via `into_inner()` on poison, preventing silent JoinHandle loss.

**Depends on**: None

**Estimated Time**: 0.5 hours

**PR Review Comments**: #1 (update.rs:1712), #2 (network.rs:121)

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs`: Fix `perf_task_handle.lock().ok()` at ~line 1712
- `crates/fdemon-app/src/handler/devtools/network.rs`: Fix `task_handle.lock().ok()` at ~line 121

**Files Read (Dependencies):**
- `crates/fdemon-app/src/actions/mod.rs`: Reference pattern — uses `match .lock() { Ok(...), Err(e) => warn!(...) }` at lines 180-188, 465-472, 546-552

### Details

#### Current State

Both sites use the terse pattern:
```rust
handle.perf_task_handle = perf_task_handle.lock().ok().and_then(|mut g| g.take());
```

If the `std::sync::Mutex` is poisoned, `.ok()` converts the error to `None`, `.and_then` short-circuits, and the `JoinHandle` is silently lost. The monitoring task becomes a zombie — it runs forever but cannot be aborted on session close.

#### Existing Codebase Pattern

The codebase already has 8+ sites that handle poison explicitly with `match` and `warn!` (see `actions/mod.rs:180-188`, `engine.rs:559-565`, `actions/session.rs:237`). These two sites are inconsistent outliers.

#### Fix

Replace both sites with a `match` that recovers the handle via `into_inner()`:

```rust
// Before:
handle.perf_task_handle = perf_task_handle.lock().ok().and_then(|mut g| g.take());

// After:
handle.perf_task_handle = match perf_task_handle.lock() {
    Ok(mut guard) => guard.take(),
    Err(e) => {
        warn!("perf task handle mutex poisoned: {e}");
        e.into_inner().take()
    }
};
```

Same transformation for `network.rs:121`:

```rust
// Before:
handle.network_task_handle = task_handle.lock().ok().and_then(|mut g| g.take());

// After:
handle.network_task_handle = match task_handle.lock() {
    Ok(mut guard) => guard.take(),
    Err(e) => {
        warn!("network task handle mutex poisoned: {e}");
        e.into_inner().take()
    }
};
```

Using `into_inner()` is strictly better than `.ok()` — it both logs the anomaly AND recovers the JoinHandle so cleanup can still abort/await it.

### Acceptance Criteria

1. Both sites use `match` with explicit `Err` arm
2. Poison errors are logged via `warn!`
3. `into_inner().take()` recovers the JoinHandle instead of losing it
4. `cargo test --workspace` passes
5. `cargo clippy --workspace -- -D warnings` passes

### Testing

No new tests needed — this is a defensive error-handling improvement. The poison scenario requires a panic while holding the mutex guard, which is not testable without unsafe contrivance. Verify with existing test suite.

### Notes

- `tracing::warn!` is already imported in both files (used elsewhere in the same modules).
- The `into_inner()` approach is used elsewhere in the Rust ecosystem for poison recovery and is the recommended pattern when the data is still valid despite the panic that caused poisoning.
