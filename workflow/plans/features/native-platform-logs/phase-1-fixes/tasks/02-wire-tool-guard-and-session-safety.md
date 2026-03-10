## Task: Wire Tool Availability Guard and Fix Session Safety Issues

**Objective**: Wire `ToolAvailability::native_logs_available()` into the native log spawn pipeline, add a double-start guard, fix the task leak on late `NativeLogCaptureStarted`, and add parentheses to the `needs_capture` expression.

**Depends on**: 01-fix-macos-log-check

**Review Issues:** #2 (Critical/Blocking), #4 (Major), #5 (Major), #6 (Minor)

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Add tool availability guard and double-start guard to `maybe_start_native_log_capture()`, add parentheses to `needs_capture`
- `crates/fdemon-app/src/handler/update.rs`: Fix `NativeLogCaptureStarted` handler to signal shutdown when session is gone

### Details

#### Fix 1: Wire `native_logs_available()` (Issue #2 — Critical)

In `maybe_start_native_log_capture()` (session.rs:277-307), add a tool availability guard after getting the platform. This prevents spawning `adb logcat` when `adb` is not installed, or `log stream` when the `log` check fails.

```rust
pub fn maybe_start_native_log_capture(
    state: &AppState,
    session_id: SessionId,
    msg: &DaemonMessage,
) -> Option<UpdateAction> {
    if let DaemonMessage::AppStart(app_start) = msg {
        if !state.settings.native_logs.enabled {
            return None;
        }

        if let Some(handle) = state.session_manager.get(session_id) {
            let platform = &handle.session.platform;

            // NEW: Check if required tools are available for this platform.
            if !state.tool_availability.native_logs_available(platform) {
                tracing::debug!(
                    "Native log capture skipped for {}: tools not available",
                    platform
                );
                return None;
            }

            // ... rest of function
        }
    }
    None
}
```

`state.tool_availability` is a `ToolAvailability` field on `AppState` (state.rs:878), fully accessible from the `&AppState` parameter.

#### Fix 2: Add double-start guard (Issue #4 — Major)

Add `handle.native_log_shutdown_tx.is_none()` check, following the pattern from `maybe_connect_vm_service` (session.rs:252-256) which guards with `handle.vm_shutdown_tx.is_none()`.

```rust
if let Some(handle) = state.session_manager.get(session_id) {
    let platform = &handle.session.platform;

    if !state.tool_availability.native_logs_available(platform) {
        return None;
    }

    // NEW: Don't start if already running (prevents double-start on repeat AppStart).
    if handle.native_log_shutdown_tx.is_some() {
        return None;
    }

    let needs_capture = /* ... */;
    // ...
}
```

Without this guard, if `AppStart` fires twice (hot-restart scenario), two capture processes spawn. The second `NativeLogCaptureStarted` overwrites the first's handles, orphaning the first capture task.

#### Fix 3: Handle late `NativeLogCaptureStarted` for closed sessions (Issue #5 — Major)

In update.rs (~line 1957), the `NativeLogCaptureStarted` handler currently drops `shutdown_tx` silently when the session is not found:

```rust
Message::NativeLogCaptureStarted {
    session_id,
    shutdown_tx,
    task_handle,
} => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.native_log_shutdown_tx = Some(shutdown_tx);
        if let Ok(mut slot) = task_handle.lock() {
            handle.native_log_task_handle = slot.take();
        }
        tracing::debug!("Native log capture started for session {}", session_id);
    }
    // BUG: When session is gone, shutdown_tx is dropped without sending `true`.
    // The capture task holds a watch::Receiver and runs indefinitely.
    UpdateResult::none()
}
```

Add an `else` branch to signal shutdown:

```rust
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.native_log_shutdown_tx = Some(shutdown_tx);
        if let Ok(mut slot) = task_handle.lock() {
            handle.native_log_task_handle = slot.take();
        }
        tracing::debug!("Native log capture started for session {}", session_id);
    } else {
        // Session was closed before capture started — shut down the orphaned task.
        let _ = shutdown_tx.send(true);
        if let Ok(mut slot) = task_handle.lock() {
            if let Some(h) = slot.take() {
                h.abort();
            }
        }
        tracing::debug!(
            "Native log capture arrived for closed session {} — shutting down",
            session_id
        );
    }
```

**Note:** The `VmServiceAttached` handler (update.rs:1222-1234) has an identical latent leak. Fixing it is out of scope for this task but should be filed as a separate follow-up.

#### Fix 4: Add parentheses to `needs_capture` (Issue #6 — Minor)

Change session.rs:292-293 from:

```rust
let needs_capture =
    platform == "android" || cfg!(target_os = "macos") && platform == "macos";
```

To:

```rust
let needs_capture =
    platform == "android" || (cfg!(target_os = "macos") && platform == "macos");
```

This is semantically equivalent (`&&` binds tighter than `||` in Rust), but the parentheses make the intent explicit. Every review agent flagged this independently.

### Acceptance Criteria

1. On a system without `adb`, an Android session start does NOT attempt to spawn `adb logcat` and produces no `tracing::warn!("Failed to spawn adb logcat")`. Instead, `debug!` logs "tools not available".
2. If `AppStart` fires twice for the same session, only one `StartNativeLogCapture` action is returned (second call returns `None`).
3. If the session is closed before `NativeLogCaptureStarted` arrives, the `shutdown_tx` sends `true` and the task handle is aborted.
4. `needs_capture` expression has explicit parentheses.
5. `cargo check -p fdemon-app` passes.
6. `cargo test -p fdemon-app --lib` passes.
7. `cargo clippy -p fdemon-app -- -D warnings` passes.

### Testing

Tests should be added in task 06 (dedicated test task). But each fix should be manually verified:

- **Fix 1**: Set `state.tool_availability.adb = false`, call `maybe_start_native_log_capture` with an Android `AppStart` — should return `None`.
- **Fix 2**: Set `handle.native_log_shutdown_tx = Some(...)`, call `maybe_start_native_log_capture` — should return `None`.
- **Fix 3**: Process `NativeLogCaptureStarted` for a non-existent `session_id` — verify `shutdown_tx.send(true)` was called (check via `watch::Receiver`).

### Notes

- The tool availability guard uses `state.tool_availability.native_logs_available(platform)` which dispatches on `"android"` → `self.adb` and `"macos"` → `self.macos_log` (cfg-gated). The accessor was created in task 03 precisely for this call site.
- Fix 3 follows the `shutdown_native_logs()` pattern from `SessionHandle` (handle.rs:147-158): take the Arc, send `true`, abort the task.
- The `VmServiceAttached` handler has the same latent leak but is a pre-existing issue unrelated to native logs. Consider filing separately.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/session.rs` | Added `native_logs_available()` guard (Fix 1), added double-start guard `native_log_shutdown_tx.is_some()` (Fix 2), added parentheses to `needs_capture` expression (Fix 4) |
| `crates/fdemon-app/src/handler/update.rs` | Added `else` branch to `NativeLogCaptureStarted` handler to signal shutdown and abort orphaned task when session is gone (Fix 3) |

### Notable Decisions/Tradeoffs

1. **Guard ordering**: Tool availability check is placed before the double-start guard. This means we skip the tool check entirely if already running — acceptable because the first start already passed the tool check and the early-exit path is cheap.
2. **`needs_capture` after guards**: The parentheses fix and the `needs_capture` boolean remain after both guards, so the cfg-gated macOS logic is only reached when tools are confirmed available and no capture is running.
3. **Orphan abort pattern**: Fix 3 uses `h.abort()` consistent with `shutdown_native_logs()` in `SessionHandle` — sends the shutdown signal first (cooperative stop), then aborts the task handle (forceful stop).

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1464 passed, 0 failed, 4 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **`VmServiceAttached` latent leak**: The same session-closed-before-message-arrived pattern exists in the `VmServiceAttached` handler in `update.rs`. It is pre-existing and out of scope for this task — should be tracked as a separate follow-up issue.
