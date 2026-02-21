## Task: Fix Duplicate Polling Tasks on Repeated Panel Switches

**Objective**: Prevent spawning duplicate network monitoring polling tasks when the user switches to the Network panel multiple times. Currently, pressing `n` → `i` → `n` spawns a second polling task while the first continues, orphaning the first task permanently.

**Depends on**: None
**Severity**: CRITICAL
**Review ref**: REVIEW.md Issue #1

### Scope

- `crates/fdemon-app/src/handler/devtools/mod.rs`: Add idempotency guard in `handle_switch_panel`
- `crates/fdemon-app/src/handler/devtools/network.rs`: Add defensive cleanup in `handle_network_monitoring_started`
- `crates/fdemon-app/src/handler/tests.rs`: Add test for duplicate spawn prevention

### Root Cause

`handle_switch_panel` for `DevToolsPanel::Network` (line ~170-186) unconditionally returns `StartNetworkMonitoring` when `vm_connected && !extensions_unavailable`. There is no check for whether a monitoring task is already running.

Compare to the Inspector panel (line ~152-167) which guards with `inspector.root.is_none() && !inspector.loading`.

When a second task starts, `handle_network_monitoring_started` (line ~79-90) overwrites `handle.network_shutdown_tx` and `handle.network_task_handle` without signalling or aborting the old task. The old task becomes a zombie.

### Fix

#### Part 1 — Idempotency guard in `handle_switch_panel`

In `crates/fdemon-app/src/handler/devtools/mod.rs`, add a guard checking `handle.network_shutdown_tx.is_some()`:

```rust
DevToolsPanel::Network => {
    if let Some(handle) = state.session_manager.selected() {
        let session_id = handle.session.id;
        let vm_connected = handle.session.vm_connected;
        let extensions_unavailable =
            handle.session.network.extensions_available == Some(false);
        let already_running = handle.network_shutdown_tx.is_some();  // ADD
        if vm_connected && !extensions_unavailable && !already_running {  // ADD guard
            return UpdateResult::action(UpdateAction::StartNetworkMonitoring {
                session_id,
                handle: None,
                poll_interval_ms: 1000,
            });
        }
    }
}
```

#### Part 2 — Defensive cleanup in `handle_network_monitoring_started`

In `crates/fdemon-app/src/handler/devtools/network.rs` (~line 85), before overwriting handles, abort/signal the old task:

```rust
if let Some(handle) = state.session_manager.get_mut(session_id) {
    // Belt-and-suspenders: stop any previously running task before replacing handles.
    if let Some(h) = handle.network_task_handle.take() {
        h.abort();
    }
    if let Some(ref tx) = handle.network_shutdown_tx {
        let _ = tx.send(true);
    }

    handle.network_shutdown_tx = Some(shutdown_tx);
    handle.network_task_handle = task_handle.lock().ok().and_then(|mut g| g.take());
}
```

This mirrors the teardown pattern from `VmServiceDisconnected` handler (update.rs ~line 1283-1290).

### Tests

Add a test in handler tests that:
1. Sets up a session with `network_shutdown_tx = Some(...)` (already running)
2. Sends `SwitchDevToolsPanel(Network)`
3. Asserts the returned action is `None` (no duplicate spawn)

Add a test that:
1. Sends `SwitchDevToolsPanel(Network)` with `network_shutdown_tx = None`
2. Asserts `StartNetworkMonitoring` is returned (normal case still works)

### Verification

```bash
cargo test -p fdemon-app -- duplicate_polling
cargo test -p fdemon-app -- switch_panel
cargo clippy -p fdemon-app
```
