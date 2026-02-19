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

**Status:** Not Started
