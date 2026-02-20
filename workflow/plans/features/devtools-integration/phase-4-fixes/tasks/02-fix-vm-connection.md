## Task: Fix VM Service Connection Lifecycle and Surface Failures

**Objective**: Fix the `vm_shutdown_tx` leak that permanently blocks VM reconnection after disconnect, and surface connection failure messages in DevTools panels so users see actionable errors instead of "VM Service not connected."

**Depends on**: 01-fix-loading-stuck

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Clear `vm_shutdown_tx` in `VmServiceDisconnected`, set DevTools error in `VmServiceConnectionFailed`
- `crates/fdemon-app/src/state.rs`: Add `vm_connection_error: Option<String>` to `DevToolsViewState`
- `crates/fdemon-tui/src/widgets/devtools/performance.rs`: Display connection error when present
- `crates/fdemon-app/src/handler/tests.rs`: Add regression tests

### Details

#### Bug 1: `vm_shutdown_tx` Leak on Natural Disconnect

**Root Cause**: `VmServiceDisconnected` handler (update.rs:1197-1215) clears `vm_request_handle`, aborts perf tasks, and sets `vm_connected = false`, but does NOT clear `vm_shutdown_tx`. After the WebSocket closes naturally (Flutter app terminated, VM exited), `vm_shutdown_tx` stays `Some(...)`.

The `maybe_connect_vm_service` guard (session.rs:218-228) checks `handle.vm_shutdown_tx.is_none()` — if it's `Some`, no new connection is attempted, permanently blocking reconnection.

**Lifecycle state table showing the bug:**

| Event | `vm_shutdown_tx` | `vm_connected` |
|---|---|---|
| `VmServiceAttached` | `Some(...)` | `false` |
| `VmServiceConnected` | `Some(...)` | `true` |
| `VmServiceDisconnected` | **`Some(...)` LEAKED** | `false` |
| Next `AppDebugPort` → `maybe_connect_vm_service` | **guard fails** | stays `false` |

**Fix**: Add `handle.vm_shutdown_tx = None;` to the `VmServiceDisconnected` handler. No need to `.send(true)` — by the time this message arrives, the `forward_vm_events` task has already exited (it sends `VmServiceDisconnected` as its final act before returning).

```rust
Message::VmServiceDisconnected { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.vm_connected = false;
        handle.vm_request_handle = None;
        handle.vm_shutdown_tx = None;  // ADD THIS LINE
        if let Some(h) = handle.perf_task_handle.take() { h.abort(); }
        if let Some(ref tx) = handle.perf_shutdown_tx { let _ = tx.send(true); }
        handle.perf_shutdown_tx = None;
        handle.session.performance.monitoring_active = false;
    }
    UpdateResult::none()
}
```

#### Bug 2: Connection Failures Invisible in DevTools Mode

**Root Cause**: `VmServiceConnectionFailed` (update.rs:1179-1195) adds a Warning to the session log, but DevTools panels don't show session logs. The Performance panel checks `session.vm_connected` and shows "VM Service not connected" with no explanation of why.

**Fix**: Add a `vm_connection_error: Option<String>` field to `DevToolsViewState` in `state.rs`. Set it on `VmServiceConnectionFailed`. Clear it on `VmServiceConnected`. Display it in the Performance panel's disconnected state view.

**State change** (state.rs):

```rust
#[derive(Debug, Clone, Default)]
pub struct DevToolsViewState {
    pub active_panel: DevToolsPanel,
    pub inspector: InspectorState,
    pub layout_explorer: LayoutExplorerState,
    pub overlay_repaint_rainbow: bool,
    pub overlay_debug_paint: bool,
    pub overlay_performance: bool,
    pub vm_connection_error: Option<String>,  // NEW
}
```

**Handler changes** (update.rs):

In `VmServiceConnectionFailed`:
```rust
state.devtools_view_state.vm_connection_error =
    Some(format!("Connection failed: {error}"));
```

In `VmServiceConnected`:
```rust
state.devtools_view_state.vm_connection_error = None;
```

**TUI change** (performance.rs): In the disconnected state rendering, check for `vm_connection_error` and display the specific error message instead of the generic "VM Service not connected."

### Acceptance Criteria

1. After VM disconnects (natural WebSocket close), `vm_shutdown_tx` is `None`
2. `maybe_connect_vm_service` succeeds on the next `AppDebugPort` after disconnect
3. When VM connection fails (timeout, refused), the Performance panel shows "Connection failed: <reason>"
4. When VM connects successfully, the error message is cleared
5. After hot restart (AppStop → AppStart → AppDebugPort), VM reconnects automatically
6. All existing tests pass + new regression tests added

### Testing

```rust
#[test]
fn test_vm_disconnected_clears_shutdown_tx() {
    // Set up state with vm_shutdown_tx = Some(...)
    // Send VmServiceDisconnected
    // Assert: handle.vm_shutdown_tx.is_none()
}

#[test]
fn test_vm_connection_failed_sets_devtools_error() {
    // Send VmServiceConnectionFailed with error message
    // Assert: state.devtools_view_state.vm_connection_error == Some("Connection failed: ...")
}

#[test]
fn test_vm_connected_clears_devtools_error() {
    // Set vm_connection_error = Some(...)
    // Send VmServiceConnected
    // Assert: state.devtools_view_state.vm_connection_error.is_none()
}

#[test]
fn test_maybe_connect_succeeds_after_disconnect() {
    // Set up connected state
    // Process VmServiceDisconnected
    // Process new AppDebugPort
    // Assert: ConnectVmService action returned
}
```

### Notes

- The `vm_shutdown_tx = None` line is safe because `VmServiceDisconnected` is only sent by `forward_vm_events` as its last act — the task has already exited, so dropping the sender has no effect.
- The `vm_connection_error` field should be reset in the `DevToolsViewState::reset()` method (added in task 04).
- Consider: should `VmServiceDisconnected` also clear `devtools_view_state.vm_connection_error`? Probably not — a disconnect after a successful connection is not an error state worth surfacing (the user will see "VM Service not connected" which is accurate).
- The Performance panel widget already has a disconnected state view (performance.rs). Modify it to check for `vm_connection_error` and show the specific message.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `vm_connection_error: Option<String>` field to `DevToolsViewState` with doc comment |
| `crates/fdemon-app/src/handler/update.rs` | (1) `VmServiceDisconnected`: added `handle.vm_shutdown_tx = None` with explanatory comment. (2) `VmServiceConnectionFailed`: sets `state.devtools_view_state.vm_connection_error = Some(format!("Connection failed: {error}"))`. (3) `VmServiceConnected`: clears `state.devtools_view_state.vm_connection_error = None` |
| `crates/fdemon-tui/src/widgets/devtools/performance.rs` | Added `vm_connection_error: Option<&'a str>` field to `PerformancePanel`; added `with_connection_error()` builder method; updated `render_disconnected()` to show the specific error when present; added 2 new tests |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Updated `PerformancePanel::new(...)` call to chain `.with_connection_error(self.state.vm_connection_error.as_deref())` |
| `crates/fdemon-app/src/handler/tests.rs` | Added 4 regression tests: `test_vm_disconnected_clears_shutdown_tx`, `test_vm_connection_failed_sets_devtools_error`, `test_vm_connected_clears_devtools_error`, `test_maybe_connect_succeeds_after_disconnect` |

### Notable Decisions/Tradeoffs

1. **Builder pattern for `PerformancePanel`**: Added `with_connection_error()` instead of changing the `new()` signature. This keeps all existing call sites unchanged and makes the error opt-in, matching the optional nature of the field.

2. **`error_owned` binding in `render_disconnected`**: The `vm_connection_error` is `Option<&'a str>`, so converting to a `&str` for the `message` variable requires a local owned `String` for the formatted case. This avoids lifetime issues without unnecessary clones in the non-error path.

3. **`vm_connection_error` not cleared on `VmServiceDisconnected`**: Per the task notes, a natural disconnect (WebSocket closed cleanly) is not an error state — the user will see "VM Service not connected" which is accurate. Only `VmServiceConnectionFailed` sets the error; `VmServiceConnected` clears it.

4. **`vm_shutdown_tx = None` is safe**: The `forward_vm_events` task sends `VmServiceDisconnected` as its final act before returning, so the background task has already exited by the time this handler runs. Dropping the `Arc<Sender>` here has no effect other than allowing the guard in `maybe_connect_vm_service` to pass.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (no errors)
- `cargo test -p fdemon-app` - Passed (837 tests including 4 new regression tests)
- `cargo test -p fdemon-tui` - Passed (519 tests including 2 new widget tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **`vm_connection_error` is global state**: It lives on `AppState.devtools_view_state` and is not scoped to a session. If multiple sessions have different connection states, only the most recent failure/success is shown. Task 04 will add a `DevToolsViewState::reset()` method to clear this field on session switch.

2. **No `VmServiceDisconnected` clearing of `vm_connection_error`**: A natural disconnect does not clear the error. If a session previously had a failed connection, then connected successfully (clearing the error), then disconnected cleanly, the DevTools panel will show "VM Service not connected" (generic) rather than a stale error — which is the correct behavior.
