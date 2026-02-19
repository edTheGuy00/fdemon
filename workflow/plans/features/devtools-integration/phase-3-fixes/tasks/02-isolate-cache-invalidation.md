## Task: Fix Isolate ID Cache Invalidation on Hot Restart

**Objective**: Add an `invalidate_isolate_cache()` method to `VmRequestHandle` and call it on hot restart so that performance RPCs target the new isolate instead of the dead one.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs`: Add `invalidate_isolate_cache()` to `VmRequestHandle`
- `crates/fdemon-app/src/handler/update.rs`: Call invalidation on `SessionRestartCompleted`

### Details

#### Root Cause

`VmRequestHandle` caches the main isolate ID in `isolate_id_cache: Arc<Mutex<Option<String>>>` (Tokio async mutex). The cache is populated on first call to `main_isolate_id()` (slow path: calls `getVM` RPC) and reused on subsequent calls (fast path: returns cached clone).

The **only** existing invalidation is in `run_client_task()` at `client.rs:588-594`, triggered on WebSocket **reconnection**. But hot restart does NOT disconnect the WebSocket — it sends `DaemonCommand::Restart` over the Flutter daemon's JSON-RPC channel. The WebSocket stays connected, so the cache is never cleared.

After hot restart, the Dart VM creates a new isolate with a different ID. The stale cached ID points to the dead pre-restart isolate. `spawn_performance_polling` calls `handle.main_isolate_id().await` every 2 seconds, gets the stale ID, and `getMemoryUsage` silently fails with a protocol error (logged at debug level).

#### Fix

**1. Add public invalidation method** to `VmRequestHandle` (in `client.rs`):

```rust
/// Clear the cached main isolate ID.
///
/// Call this after events that create a new isolate (hot restart)
/// so the next `main_isolate_id()` call re-fetches from the VM.
pub async fn invalidate_isolate_cache(&self) {
    let mut cache = self.isolate_id_cache.lock().await;
    *cache = None;
}
```

This uses the existing `Arc<Mutex<Option<String>>>` — the method just locks and sets to `None`. Thread-safe because all clones share the same `Arc`.

**2. Call on hot restart completion** in `handler/update.rs`:

The `SessionRestartCompleted` handler is at `update.rs:202-210`. After `handle.session.complete_reload()`, add:

```rust
// Invalidate isolate ID cache — hot restart creates a new isolate
if let Some(ref vm_handle) = handle.vm_request_handle {
    // We can't call async from the sync update function.
    // Use try_lock or a sync invalidation method instead.
}
```

**Important**: `handler::update()` is a **synchronous** function (TEA purity). We cannot call `.await` here. Options:

**(a) Use `std::sync::Mutex` instead of `tokio::Mutex`** for `isolate_id_cache`. The lock is held only briefly (read or clear), so contention risk is negligible. This allows `invalidate_isolate_cache()` to be sync:

```rust
pub fn invalidate_isolate_cache(&self) {
    if let Ok(mut cache) = self.isolate_id_cache.lock() {
        *cache = None;
    }
}
```

This requires changing the type from `Arc<tokio::sync::Mutex<Option<String>>>` to `Arc<std::sync::Mutex<Option<String>>>` and updating `main_isolate_id()` accordingly (use `.lock().unwrap()` instead of `.lock().await`).

**(b) Keep `tokio::Mutex`, add a sync invalidation via `try_lock()`:**

```rust
pub fn invalidate_isolate_cache(&self) {
    if let Ok(mut cache) = self.isolate_id_cache.try_lock() {
        *cache = None;
    }
}
```

This could fail under contention (if `main_isolate_id()` is executing concurrently), but contention is extremely unlikely since the polling task sleeps 2 seconds between calls.

**Recommended: Option (a)** — switch to `std::sync::Mutex`. The lock is never held across `.await` points (the RPC call in `main_isolate_id()` releases the lock before sending the request and re-acquires to store the result), so `std::sync::Mutex` is safe. **However**, looking at the code more carefully, `main_isolate_id()` acquires the lock, checks if it's `Some`, and if not, makes an async RPC call while holding the lock. This means it IS held across an await point, which means we must keep `tokio::Mutex` for `main_isolate_id()` but can use `try_lock()` for invalidation.

**Recommended: Option (b)** — use `try_lock()` for the sync invalidation path. The polling loop sleeps 2 seconds, so the lock is almost never held.

**3. Also call on `HotReload` completion** (optional but good hygiene):

Hot reload (`SessionReloadCompleted` at `update.rs:183-199`) doesn't create a new isolate, so invalidation is technically unnecessary. However, it's harmless and adds resilience. Consider whether to include.

### Acceptance Criteria

1. `VmRequestHandle` has a public `invalidate_isolate_cache()` method
2. `SessionRestartCompleted` handler calls `invalidate_isolate_cache()`
3. After hot restart, the next `main_isolate_id()` call re-fetches via `getVM` RPC
4. Existing tests still pass — no behavioral change except cache clearing
5. New test: `invalidate_isolate_cache` clears the cached value
6. New test: after invalidation, `main_isolate_id()` refetches (uses mock)
7. `cargo test -p fdemon-daemon -p fdemon-app` passes
8. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
// In fdemon-daemon tests
#[tokio::test]
async fn test_invalidate_isolate_cache_clears_cached_value() {
    // Setup: create VmRequestHandle with pre-populated cache
    // Action: call invalidate_isolate_cache()
    // Assert: next main_isolate_id() call sends getVM request
}

// In fdemon-app handler tests
#[test]
fn test_restart_completed_invalidates_isolate_cache() {
    // Setup: session with vm_request_handle set
    // Action: process SessionRestartCompleted message
    // Assert: invalidate_isolate_cache was called
}
```

### Notes

- The `run_client_task()` reconnection path (`client.rs:588-594`) already clears the cache — this fix adds a second invalidation trigger for hot restart
- The performance polling task at `actions.rs:509-513` calls `main_isolate_id()` every 2 seconds, so after invalidation the cache is repopulated within 2 seconds automatically
- `enable_frame_tracking()` at `actions.rs:609-614` is called only once at startup, so a stale cache there would require re-calling it after restart — out of scope for this fix, but worth noting

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `invalidate_isolate_cache()` method using `try_lock()` on `VmRequestHandle`; added `new_for_test()` and `cached_isolate_id()` test helpers under `#[cfg(any(test, feature = "test-helpers"))]`; added 3 new unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Added cache invalidation call in `SessionRestartCompleted` handler after `complete_reload()` |
| `crates/fdemon-app/src/handler/tests.rs` | Added 3 new handler tests: `test_restart_completed_invalidates_isolate_cache`, `test_restart_completed_without_vm_handle_does_not_panic`, `test_reload_completed_does_not_invalidate_isolate_cache` |

### Notable Decisions/Tradeoffs

1. **`try_lock()` over `std::sync::Mutex`**: Used option (b) from the task — `try_lock()` on the existing `tokio::Mutex`. The task correctly identifies that `main_isolate_id()` holds the lock across an await point (the `getVM` RPC call), making `std::sync::Mutex` unsafe. The `try_lock()` approach is safe and handles the edge case by logging a debug message and silently skipping — contention is essentially impossible given the 2-second polling interval.

2. **Test helpers behind `#[cfg(any(test, feature = "test-helpers"))]`**: Added `new_for_test()` and `cached_isolate_id()` to `VmRequestHandle` matching the established pattern from `CommandSender::new_for_test()`. The `fdemon-app` dev-dependencies already declare `fdemon-daemon` with `features = ["test-helpers"]`, so these are immediately usable in handler tests.

3. **Hot reload does NOT invalidate cache**: The `SessionReloadCompleted` handler is intentionally left unchanged — hot reload preserves the Dart isolate (same ID), so invalidating there would cause an unnecessary `getVM` RPC on the next poll. A test (`test_reload_completed_does_not_invalidate_isolate_cache`) asserts this behavior explicitly.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test -p fdemon-daemon -p fdemon-app` — Passed (340 + 803 tests, 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed (no formatting issues)
- New `fdemon-daemon` tests (3): `test_invalidate_isolate_cache_clears_cached_value`, `test_invalidate_isolate_cache_is_idempotent_when_already_empty`, `test_invalidate_isolate_cache_shared_across_clones` — all passed
- New `fdemon-app` handler tests (3): `test_restart_completed_invalidates_isolate_cache`, `test_restart_completed_without_vm_handle_does_not_panic`, `test_reload_completed_does_not_invalidate_isolate_cache` — all passed

### Risks/Limitations

1. **`try_lock()` miss under extreme contention**: If `main_isolate_id()` happens to be executing exactly when `invalidate_isolate_cache()` is called, the invalidation is silently skipped. The probability is negligible (polling sleeps 2 seconds; invalidation is synchronous and instantaneous), and even if it occurred, the cache would be repopulated with a new valid ID anyway since `getVM` would have been called.

2. **`enable_frame_tracking()` not re-called after restart**: The frame tracking extension registration (`ext.dart.developer.timeline.enable`) is set up once at startup. After hot restart, the new isolate needs it re-enabled. This is noted as out of scope per the task description.
