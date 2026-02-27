## Task: Capture Actual Process Exit Code via Dedicated Wait Task

**Objective**: Replace the always-`None` exit code in `DaemonEvent::Exited` with the actual process exit code by spawning a dedicated `child.wait()` task that owns the `Child` handle.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/process.rs`: Major refactor — extract `Child` into a wait task, change `FlutterProcess` fields
- `crates/fdemon-core/src/events.rs`: No changes (already has `code: Option<i32>`)

### Details

#### Problem

Today, `DaemonEvent::Exited { code: None }` is always emitted with `code: None` (line 137 of `process.rs`). The `stdout_reader` task detects stdout EOF but has no access to the `Child` handle, so it cannot call `try_wait()` or `wait()`. The real exit code is only captured inside `FlutterProcess::shutdown()` (line 247-250) where it's logged but never sent back through the event channel.

This means `handle_session_exited` always falls into the `None` branch, showing "Flutter process exited" rather than "exited normally" (code 0) or "exited with code N" (non-zero).

#### Approach: Dedicated `wait_for_exit` Task

Spawn a separate async task at process creation time that:
1. Takes ownership of `Child` via an `Arc<Mutex<Child>>`
2. Calls `child.wait().await` (blocks until process exits)
3. Sends `DaemonEvent::Exited { code: Some(exit_code) }` through the event channel

The `stdout_reader` STOPS emitting `DaemonEvent::Exited`. Instead, it just logs EOF and returns. The `wait_for_exit` task becomes the sole source of `Exited` events.

#### Implementation

**Step 1: Change `FlutterProcess` to use `Arc<Mutex<Child>>`**

```rust
pub struct FlutterProcess {
    child: Arc<tokio::sync::Mutex<Child>>,  // shared with wait_for_exit task
    stdin_tx: mpsc::Sender<String>,
    pid: Option<u32>,
}
```

Using `tokio::sync::Mutex` (not `std::sync::Mutex`) because `child.wait()` is async and we need to hold the lock across an `.await` point.

**Step 2: Spawn `wait_for_exit` task in `spawn_internal`**

After spawning stdout/stderr/stdin readers, spawn the waiter:

```rust
let child = Arc::new(tokio::sync::Mutex::new(child));
let child_for_waiter = Arc::clone(&child);
let event_tx_for_waiter = event_tx.clone();

tokio::spawn(async move {
    let mut child_guard = child_for_waiter.lock().await;
    match child_guard.wait().await {
        Ok(status) => {
            let code = status.code(); // Option<i32> — None on signal kill (Unix)
            info!("Flutter process exited with status: {:?}", status);
            let _ = event_tx_for_waiter
                .send(DaemonEvent::Exited { code })
                .await;
        }
        Err(e) => {
            error!("Error waiting for Flutter process: {}", e);
            let _ = event_tx_for_waiter
                .send(DaemonEvent::Exited { code: None })
                .await;
        }
    }
});
```

**Step 3: Remove `Exited` emission from `stdout_reader`**

Change line 137:
```rust
// Before:
let _ = tx.send(DaemonEvent::Exited { code: None }).await;

// After:
// Exited event is now emitted by the wait_for_exit task.
// stdout EOF just means the pipe closed — the process may still be shutting down.
```

**Step 4: Update `has_exited()` and `is_running()`**

These methods call `self.child.try_wait()` which requires `&mut Child`. With `Arc<Mutex<Child>>`, they need to acquire the lock:

```rust
pub async fn has_exited(&self) -> bool {
    let mut child = self.child.lock().await;
    matches!(child.try_wait(), Ok(Some(_)))
}

pub async fn is_running(&self) -> bool {
    let mut child = self.child.lock().await;
    matches!(child.try_wait(), Ok(None))
}
```

**Note**: These become `async` methods, which changes callers:
- `shutdown()` in `process.rs` — calls `self.has_exited()` → add `.await`
- `spawn_session` watchdog (task 01) — `process.has_exited()` → add `.await`
- `Drop` impl — cannot be async

**Step 5: Update `shutdown()`**

```rust
pub async fn shutdown(...) -> Result<()> {
    if self.has_exited().await {
        return Ok(());
    }
    // ... send stop commands ...
    let mut child = self.child.lock().await;
    match timeout(Duration::from_secs(2), child.wait()).await {
        Ok(Ok(status)) => { info!("..."); Ok(()) }
        Ok(Err(e)) => { self.force_kill_locked(&mut child).await }
        Err(_) => { self.force_kill_locked(&mut child).await }
    }
}

async fn force_kill_locked(child: &mut Child) -> Result<()> {
    child.kill().await.map_err(|e| Error::process(format!("Failed to kill: {}", e)))
}
```

**Step 6: Update `Drop` impl**

`Drop` cannot be async. Replace the `try_wait` check with a synchronous approach:

```rust
impl Drop for FlutterProcess {
    fn drop(&mut self) {
        // Cannot await the lock in Drop. If the process is still alive,
        // kill_on_drop(true) on the Child handles cleanup.
        // Just log that we're being dropped.
        debug!("FlutterProcess dropped");
    }
}
```

Since `kill_on_drop(true)` is set on the `Child`, the OS will SIGKILL the process when the `Arc<Mutex<Child>>` is fully dropped (all references gone). This is the same safety net as before.

#### Race Condition: `wait_for_exit` vs `stdout_reader`

With this change, the ordering becomes:
1. `stdout_reader` detects EOF → exits (no `Exited` event emitted)
2. `wait_for_exit` detects process exit → sends `DaemonEvent::Exited { code: Some(N) }`

The `stdout_reader` may exit before or after `wait_for_exit`. Since only `wait_for_exit` emits `Exited`, there's no duplicate. The `daemon_rx.recv()` in `spawn_session` will receive exactly one `Exited` event.

**Edge case**: If the process is killed and the event channel is already closed (session cleaned up), the `Exited` event is silently dropped (`let _ = tx.send(...).await`). This is acceptable — the session is already gone.

#### Race Condition: `try_wait()` vs `wait()`

On Unix/macOS, if `try_wait()` reaps the process (returns `Some(ExitStatus)`), a subsequent `wait()` call may return an error or block indefinitely. The `wait_for_exit` task holds the `child` lock for the duration of `wait()`, so `try_wait()` (called by `has_exited()`) can only run when `wait()` is not running. Since `wait()` is called immediately when the task starts and holds the lock until the process exits, `try_wait()` and `wait()` never race.

**But wait**: The `wait_for_exit` task acquires the lock at startup and holds it for the lifetime of the process. This means `has_exited()` and `shutdown()` will block on the lock until the process exits. This is problematic.

**Revised approach**: Don't hold the lock for the entire `wait()`. Instead, use `tokio::process::Child::wait()` which is implemented with a background driver — the child's wait handle is registered with the runtime. We can instead use a oneshot channel:

```rust
// In spawn_internal:
let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();

tokio::spawn(async move {
    let mut child_guard = child_for_waiter.lock().await;
    let result = child_guard.wait().await;
    drop(child_guard); // release lock
    let code = result.ok().and_then(|s| s.code());
    let _ = exit_tx.send(code);
});
```

Actually, this still has the lock problem. Let me reconsider.

**Better approach: Move `Child` out entirely, replace with status channel**

```rust
pub struct FlutterProcess {
    stdin_tx: mpsc::Sender<String>,
    pid: Option<u32>,
    exit_rx: oneshot::Receiver<Option<i32>>,   // receives exit code from wait task
    kill_tx: Option<oneshot::Sender<()>>,       // signals wait task to kill
}
```

The `wait_for_exit` task owns the `Child` outright:

```rust
tokio::spawn(async move {
    tokio::select! {
        result = child.wait() => {
            let code = result.ok().and_then(|s| s.code());
            let _ = event_tx_clone.send(DaemonEvent::Exited { code }).await;
            let _ = exit_tx.send(code);
        }
        _ = kill_rx => {
            // Explicit kill requested
            let _ = child.kill().await;
            let _ = event_tx_clone.send(DaemonEvent::Exited { code: None }).await;
            let _ = exit_tx.send(None);
        }
    }
});
```

`has_exited()` and `is_running()` can be removed or changed to check the oneshot status. `shutdown()` sends `kill_tx` and waits on `exit_rx`.

**This is a moderate refactor.** The benefit is clean separation: the wait task has exclusive ownership of `Child` and the main `FlutterProcess` API stays synchronous (no mutex).

### Acceptance Criteria

1. `DaemonEvent::Exited { code }` carries the real `Option<i32>` exit code when available
2. `stdout_reader` no longer emits `DaemonEvent::Exited`
3. A dedicated `wait_for_exit` task calls `child.wait()` and emits the `Exited` event
4. Normal exit (code 0) shows "Flutter process exited normally" in session log
5. Non-zero exit shows "Flutter process exited with code N"
6. Signal kill (code = None on Unix) shows "Flutter process exited"
7. `shutdown()` still works — can kill the process via the wait task
8. `has_exited()` / `is_running()` still work (or are replaced with equivalent API)
9. No race between `try_wait()` and `wait()` on the same `Child`
10. `cargo check --workspace` passes
11. `cargo clippy --workspace -- -D warnings` clean
12. All existing tests pass (`cargo test --workspace`)

### Testing

Update existing tests in `crates/fdemon-daemon/src/process.rs`:

```rust
#[tokio::test]
async fn test_exit_code_captured_on_normal_exit() {
    // Spawn a process that exits with code 0
    // Verify DaemonEvent::Exited { code: Some(0) } is received
}

#[tokio::test]
async fn test_exit_code_captured_on_error_exit() {
    // Spawn a process that exits with non-zero code
    // Verify DaemonEvent::Exited { code: Some(N) } is received
}

#[tokio::test]
async fn test_shutdown_kills_process() {
    // Spawn a long-running process
    // Call shutdown()
    // Verify process is killed and Exited event is received
}
```

Also update `crates/fdemon-app/src/handler/tests.rs` to verify `handle_session_exited` correctly formats exit messages for `Some(0)`, `Some(1)`, and `None` — these code paths already exist but were never exercised with non-`None` values.

### Notes

- This is the largest and most complex task in Phase 3. Consider implementing the simpler `Arc<Mutex<Child>>` approach first if the full refactor is too risky.
- The `kill_on_drop(true)` flag on `Child` remains the last-resort safety net.
- On Unix, a process killed by a signal has `ExitStatus::code() == None` but `ExitStatus::signal() == Some(N)`. Consider logging the signal number too.
- If task 01 (process watchdog) is implemented first, it will emit `code: None`. After this task, the watchdog becomes redundant for normal cases (the `wait_for_exit` task is faster), but the watchdog still serves as a backup if the wait task somehow fails.
- **Interaction with task 01**: After this task, the watchdog (task 01) should check `exit_rx` status instead of calling `process.has_exited()`, or be removed entirely since `wait_for_exit` is strictly more capable.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/process.rs` | Major refactor: moved `Child` ownership into `wait_for_exit` task; added `kill_tx`, `exited` (AtomicBool), `exit_notify` (Notify) fields; `has_exited()` and `is_running()` changed from `&mut self` to `&self` (synchronous atomic reads); `shutdown()` uses `exit_notify.notified()` with race-free pattern; `stdout_reader` no longer emits `DaemonEvent::Exited`; `Drop` sends kill signal; 5 new tests added. |

### Notable Decisions/Tradeoffs

1. **Clean approach chosen (oneshot + AtomicBool + Notify)**: Rather than the simpler `Arc<Mutex<Child>>` option, the `Child` is moved into the `wait_for_exit` task entirely. The `FlutterProcess` struct retains three small primitives: a `kill_tx: Option<oneshot::Sender<()>>` to signal force-kill, an `Arc<AtomicBool>` for synchronous `has_exited()` / `is_running()` checks (no lock, no `.await`), and an `Arc<Notify>` for `shutdown()` to await graceful exit. This avoids all lock contention and eliminates the `try_wait()` / `wait()` race.

2. **`has_exited()` stays synchronous**: The watchdog in `actions.rs` calls `process.has_exited()` inside a `tokio::select!`. By using an `AtomicBool`, the method stays synchronous (`&self`, no `.await`), so no change to the watchdog call site was needed.

3. **Race-free `shutdown()` wait pattern**: Before calling `exit_notify.notified()`, a `Notify` future is created first, then `has_exited()` is checked. This ensures no notification is missed between the check and the await. If the process exits after `notified()` is created but before `await`, the notification is captured correctly.

4. **`stdout_reader` no longer emits `Exited`**: The old code emitted `DaemonEvent::Exited { code: None }` on stdout EOF. This was removed. The `wait_for_exit` task is now the sole source of `Exited` events, preventing duplicates and providing the real exit code.

5. **`force_kill()` becomes synchronous**: With `Child` owned by the wait task, force-kill is implemented by sending on `kill_tx`. The wait task then calls `child.kill().await`. `force_kill()` is now synchronous (`fn` not `async fn`), which simplifies `shutdown()`.

6. **`Drop` improvement**: The new `Drop` impl sends `kill_tx` if the process hasn't exited yet, giving the wait task a chance to clean up before `kill_on_drop(true)` on the `Child` activates.

7. **5 new tests cover the acceptance criteria**: `test_exit_code_captured_on_normal_exit`, `test_exit_code_captured_on_error_exit`, `test_stdout_reader_does_not_emit_exited_event`, `test_has_exited_becomes_true_after_exit`, `test_shutdown_kills_long_running_process`.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)
- `cargo test --workspace` - Passed (2,765 tests: 0 failed, 74 ignored)

### Risks/Limitations

1. **`test_shutdown_kills_long_running_process` requires `sh`**: The new process-level tests use `sh -c "..."` as a stand-in for a Flutter process, which is standard on macOS/Linux but not Windows. Since the project targets macOS/Linux for Flutter development, this is acceptable.

2. **`test_has_exited_becomes_true_after_exit` uses a 200ms timeout**: If the test environment is extremely slow, this could flake. The timeout could be increased if needed.

3. **Watchdog in `actions.rs` is now redundant for normal exit cases**: The `wait_for_exit` task emits `DaemonEvent::Exited` before the watchdog interval (5s) would fire, making the watchdog a pure backup for SIGKILL-without-EOF scenarios. This is acceptable and per the task notes.
