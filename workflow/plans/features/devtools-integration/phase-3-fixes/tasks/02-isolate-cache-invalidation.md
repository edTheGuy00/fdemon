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
