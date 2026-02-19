## Task: Replace RwLock::unwrap() with Poison-Safe Access in client.rs

**Objective**: Replace all 9 `RwLock::unwrap()` calls in `client.rs` with `unwrap_or_else(|e| e.into_inner())` to prevent panics on poisoned locks.

**Depends on**: None (independent of the extensions split)

**Estimated Time**: 0.5-1 hour

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs` — 9 occurrences of `RwLock::unwrap()`

### Details

#### Problem

The `state: Arc<std::sync::RwLock<ConnectionState>>` field in `VmServiceClient` uses `.unwrap()` on all lock acquisitions. A `std::sync::RwLock` becomes poisoned if a thread panics while holding the lock. Once poisoned, every subsequent `.unwrap()` panics, cascading the failure.

The stored type is `ConnectionState` — a simple `#[derive(Debug, Clone, PartialEq)]` enum with no `Drop` side effects. Recovering the inner value from a poisoned lock is safe and correct.

#### All 9 Occurrences

| Line | Access | Context |
|------|--------|---------|
| 162 | `.write().unwrap()` | `connect()` — sets Connected after first WS connect |
| 227 | `.read().unwrap()` | `connection_state()` — public getter |
| 243 | `.read().unwrap()` | `is_connected()` — public getter |
| 424 | `.write().unwrap()` | `run_client_task()` — sets Disconnected on clean exit |
| 437 | `.write().unwrap()` | `run_client_task()` — sets Disconnected after max retries |
| 443 | `.write().unwrap()` | `run_client_task()` — sets Reconnecting at backoff start |
| 457 | `.write().unwrap()` | `run_client_task()` — sets Disconnected on channel close |
| 466 | `.write().unwrap()` | `run_client_task()` — sets Connected after reconnect |
| 483 | `.write().unwrap()` | `run_client_task()` — sets Disconnected after reconnect loop |

#### Fix

Replace each occurrence:

```rust
// Before (read):
self.state.read().unwrap().clone()

// After (read):
self.state.read().unwrap_or_else(|e| e.into_inner()).clone()

// Before (write):
let mut guard = state.write().unwrap();

// After (write):
let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
```

#### What NOT to Change

- **`isolate_id_cache`** uses `tokio::sync::Mutex` which has no poison concept. Its `.lock().await` calls are already correct.
- **`unwrap_or()` calls** (lines 314, 613) are not lock-related and are already safe.
- **Test code `unwrap()` calls** (lines 855, 871, 944, 987, 1003, 1066) are acceptable in tests.

### Acceptance Criteria

1. Zero `RwLock::unwrap()` calls remain in production code in `client.rs`
2. All 9 occurrences replaced with `unwrap_or_else(|e| e.into_inner())`
3. No changes to `isolate_id_cache` (tokio::sync::Mutex) — already correct
4. No changes to test code `unwrap()` calls
5. All existing tests pass
6. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — this is a defensive improvement. The existing test suite validates behavior is unchanged.

```bash
cargo fmt --all && cargo check --workspace && cargo test --lib && cargo clippy --workspace -- -D warnings
```

### Notes

- This is a mechanical find-and-replace task. Each replacement is identical in form.
- The `unwrap_or_else(|e| e.into_inner())` pattern returns a `MutexGuard`/`RwLockReadGuard`/`RwLockWriteGuard` to the inner value, clearing the poison state. For a simple enum like `ConnectionState`, this is always safe.
- This task can run in parallel with task 01 since it only touches `client.rs`, which is not affected by the extensions split.
