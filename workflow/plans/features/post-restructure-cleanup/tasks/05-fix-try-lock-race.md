## Task: Fix try_lock() race in session task tracking

**Objective**: Replace `try_lock()` with a reliable lock acquisition in `spawn_session()` to prevent silent loss of session task handles during concurrent session spawns.

**Review Issue**: #7 (MINOR) - Session task tracking uses try_lock()

**Depends on**: None

### Scope

- `crates/fdemon-app/src/actions.rs`: Fix lines 324-332 (try_lock), line 24 (type definition)

### Details

#### The Problem

`spawn_session()` (line 158) is a **non-async** function. After spawning a tokio task, it stores the `JoinHandle` in a `SessionTaskMap` (a `tokio::sync::Mutex<HashMap<SessionId, JoinHandle<()>>>`). Because it's non-async, it cannot use `.lock().await` and instead uses `try_lock()`:

```rust
// Current code (actions.rs:324-332)
if let Ok(mut guard) = session_tasks.try_lock() {
    guard.insert(session_id, handle);
    info!("Session {} task added to tracking (total: {})", session_id, guard.len());
}
// No else branch -- handle silently dropped if lock is held
```

If the lock is contended (e.g., another session's cleanup task holds it at line ~320 where `.lock().await.remove(&session_id)` runs), the `JoinHandle` is dropped. The task continues running but cannot be tracked for shutdown cleanup.

#### The Fix

Change `SessionTaskMap` from `tokio::sync::Mutex` to `std::sync::Mutex`. This allows blocking `.lock()` in non-async code. The critical section is a single `HashMap::insert` (~nanoseconds), so blocking is negligible.

**Step 1: Change the type alias (line 24):**
```rust
// Before:
pub type SessionTaskMap = Arc<tokio::sync::Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;

// After:
pub type SessionTaskMap = Arc<std::sync::Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;
```

**Step 2: Update the storage code (lines 324-332):**
```rust
// Before:
if let Ok(mut guard) = session_tasks.try_lock() {
    guard.insert(session_id, handle);
    info!(...);
}

// After:
match session_tasks.lock() {
    Ok(mut guard) => {
        guard.insert(session_id, handle);
        info!(
            "Session {} task added to tracking (total: {})",
            session_id,
            guard.len()
        );
    }
    Err(e) => {
        warn!("Session {} task handle could not be tracked (poisoned lock): {}", session_id, e);
    }
}
```

**Step 3: Update all other `.lock().await` calls on `session_tasks` to `.lock().unwrap()`:**

Find all usages of `session_tasks` (or its clones) that call `.lock().await` and change them to `.lock().unwrap()` or `.lock().expect("session_tasks lock")`. These are likely in async code inside the spawned task (around line 320):

```rust
// Before:
session_tasks_clone.lock().await.remove(&session_id);

// After:
if let Ok(mut guard) = session_tasks_clone.lock() {
    guard.remove(&session_id);
}
```

**Step 4: Update the import (line 7):**
If `tokio::sync::Mutex` was imported only for `SessionTaskMap`, remove it from the import. If other code in the file uses it, leave it.

### Acceptance Criteria

1. `SessionTaskMap` uses `std::sync::Mutex` instead of `tokio::sync::Mutex`
2. No `try_lock()` calls remain for session task tracking
3. Lock failures (poisoning) are logged with `warn!`, not silently ignored
4. All `.lock().await` calls on `SessionTaskMap` are updated to `.lock()` (blocking)
5. `cargo test -p fdemon-app` passes
6. `cargo clippy -p fdemon-app -- -D warnings` passes
7. No deadlock risk (verify critical sections are trivial)

### Testing

Existing session management tests in `fdemon-app` should continue to pass. The behavioral change is:
- **Before**: Task handle silently lost under contention
- **After**: Task handle always stored (blocking briefly if needed)

No new tests are strictly necessary, but consider adding a test that spawns two sessions concurrently and verifies both handles are tracked.

### Notes

- `std::sync::Mutex` should NOT be held across `.await` points. Verify that no code path does `let guard = session_tasks.lock().unwrap(); some_async_fn().await; drop(guard);` -- this would panic or deadlock. The current code only uses the lock for brief HashMap operations, so this should not be an issue.
- An alternative approach would be to make `spawn_session()` async, but this propagates up to `handle_action()` which is currently sync. The `std::sync::Mutex` approach is less invasive.
- Per tokio docs: "the rule of thumb is to use the `std` mutex if the lock is held for a very short time, or if it is never held across `.await` points."

---

## Completion Summary

**Status:** Blocked

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/crates/fdemon-app/src/actions.rs` | Changed `SessionTaskMap` type alias from `tokio::sync::Mutex` to `std::sync::Mutex` (line 23), replaced `try_lock()` with `lock()` and added poisoning error handling (lines 331-346), updated cleanup lock usage (lines 319-327), removed `Mutex` from tokio::sync imports (line 7) |
| `/Users/ed/Dev/zabin/flutter-demon/crates/fdemon-app/src/engine.rs` | Updated `Mutex::new()` to `std::sync::Mutex::new()` (line 148), removed `.await` from `lock()` call and added poisoning error handling in shutdown (lines 330-336), removed `Mutex` from tokio::sync imports (line 11) |
| `/Users/ed/Dev/zabin/flutter-demon/crates/fdemon-app/src/process.rs` | Updated `SessionTaskMap` parameter type signature to use `std::sync::Mutex` (line 27), removed `Mutex` from tokio::sync imports (line 10) |

### Notable Decisions/Tradeoffs

1. **Blocking mutex**: Changed from `tokio::sync::Mutex` to `std::sync::Mutex` because critical sections are trivial HashMap operations (insert/remove/drain). This eliminates the race condition where `try_lock()` could silently drop task handles under contention.

2. **Poison handling**: All `.lock()` calls now use `match` or `if let Ok()` to handle potential lock poisoning. Poisoning is logged with `warn!` instead of panicking, which is appropriate since this would only occur if a previous thread panicked while holding the lock.

3. **No `.await` points**: Verified that no code holds the std::sync::Mutex guard across `.await` points, which would cause panics or deadlocks. All usage is for brief HashMap operations only.

4. **Consistent error handling**: Used `warn!` for all lock failures with descriptive messages including session_id context.

### Testing Performed

**BLOCKED**: Cannot run tests due to unrelated uncommitted changes in the repository that break compilation.

The codebase contains uncommitted work-in-progress changes that introduce compilation errors:
- New file `crates/fdemon-app/src/input_key.rs` (not committed)
- Partial refactoring of `crates/fdemon-app/src/handler/keys.rs` to use `InputKey` instead of `KeyEvent`
- Many other files modified (see git status output)

These changes are NOT related to this task (fixing try_lock race in session task tracking) but prevent the build from completing.

**My specific changes are correct per the task specification:**
- Changed `SessionTaskMap` from `tokio::sync::Mutex` to `std::sync::Mutex`
- Replaced `try_lock()` with reliable `.lock()`
- Added proper poison error handling with `warn!` logging
- Updated all `.lock().await` to `.lock()` (blocking)
- Removed unnecessary `Mutex` imports from tokio::sync

### Verification Commands (when build is fixed)

```bash
cargo test -p fdemon-app
cargo clippy -p fdemon-app -- -D warnings
```

### Risks/Limitations

1. **Lock poisoning**: If a thread panics while holding the session_tasks lock, subsequent lock acquisitions will return `Err(PoisonError)`. This is handled by logging warnings and proceeding without tracking/cleanup. The poisoned state persists for the lifetime of the Arc, but this should be rare (only on panic).

2. **Build blocked**: Cannot verify tests pass due to unrelated uncommitted changes. The changes follow the task specification exactly but need a clean codebase to verify.

3. **No deadlock risk**: Verified that std::sync::Mutex is never held across `.await` points. All critical sections are trivial HashMap operations.
