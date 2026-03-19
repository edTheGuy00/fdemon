## Task: Pause Network Monitoring When Leaving Network Tab

**Objective**: Add a `network_pause_tx` watch channel that pauses the network polling loop when the user switches away from the Network tab, and resumes when they return. This eliminates `getHttpProfile` RPCs when the user is viewing other DevTools panels or logs.

**Depends on**: 01-pause-perf-when-not-devtools (file overlap on `devtools/mod.rs`, `session/handle.rs`, `message.rs`)

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/network.rs`: Create `network_pause_tx/rx` watch channel; gate `poll_tick` arm on network_pause; add `network_pause_rx.changed()` arm for immediate fetch on unpause
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Send `true` (pause) when switching away from Network; send `false` (unpause) when switching to Network; send `true` (pause) in `handle_exit_devtools_mode`
- `crates/fdemon-app/src/handler/devtools/network.rs`: Store `network_pause_tx` from `VmServiceNetworkMonitoringStarted`
- `crates/fdemon-app/src/session/handle.rs`: Add `network_pause_tx: Option<Arc<watch::Sender<bool>>>` field
- `crates/fdemon-app/src/message.rs`: Add `network_pause_tx` to `VmServiceNetworkMonitoringStarted` message variant

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: `DevToolsPanel` enum, `UiMode`
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Existing `alloc_pause_tx` and `perf_pause_tx` patterns (from Task 01)

### Details

#### Current State

- Network monitoring is demand-started: it only begins when the user first switches to the Network panel (`handle_switch_panel` in `devtools/mod.rs:~221-249`)
- An idempotency guard (`network_shutdown_tx.is_some()`) prevents duplicate task spawns
- Once started, the network task runs forever until VM disconnect — there is no pause mechanism
- The polling loop has only two `select!` arms: `poll_tick` and `network_shutdown_rx.changed()`

#### Design: Network Pause Channel

Add a `network_pause_tx: watch::Sender<bool>` channel (same pattern as `alloc_pause_tx` and `perf_pause_tx`):

| Value | Meaning |
|-------|---------|
| `true` | Paused — skip `getHttpProfile` calls |
| `false` | Active — poll normally |

**Initial value: `false` (active)** — unlike `perf_pause` and `alloc_pause` which start paused, network monitoring starts active because it is only spawned when the user is already on the Network tab.

#### Implementation Steps

**Step 1: Create the pause channel in `spawn_network_monitoring`**

In `actions/network.rs`, alongside the existing `network_shutdown_tx/rx` pair:

```rust
// Initial: active (false) — task starts when user is already on Network tab
let (network_pause_tx, mut network_pause_rx) = tokio::sync::watch::channel(false);
let network_pause_tx = std::sync::Arc::new(network_pause_tx);
```

Include `network_pause_tx` in the `VmServiceNetworkMonitoringStarted` message.

**Step 2: Gate the poll tick arm**

In the `tokio::select!` loop:

```rust
_ = poll_tick.tick() => {
    // Skip if network monitoring is paused (not on Network tab)
    if *network_pause_rx.borrow() {
        continue;
    }
    // ... existing getHttpProfile logic ...
}
```

**Step 3: Add unpause arm for immediate fetch**

Add a new arm to the `select!` loop:

```rust
Ok(()) = network_pause_rx.changed() => {
    if !*network_pause_rx.borrow() {
        // Unpaused — user switched back to Network tab.
        // Fire one immediate getHttpProfile fetch so the table shows
        // any requests that occurred while the tab was hidden.
        // ... same getHttpProfile + VmServiceHttpProfileReceived logic as poll_tick ...
    }
}
```

**Step 4: Store `network_pause_tx` on `SessionHandle`**

In `session/handle.rs`:

```rust
/// Pause channel for network monitoring polling.
/// `true` = paused (not on Network tab), `false` = active.
pub network_pause_tx: Option<Arc<watch::Sender<bool>>>,
```

Initialize to `None` in `SessionHandle::new()`.

In `message.rs`, add the field to `VmServiceNetworkMonitoringStarted`:

```rust
Message::VmServiceNetworkMonitoringStarted {
    session_id: SessionId,
    network_shutdown_tx: Arc<watch::Sender<bool>>,
    network_task_handle: SharedTaskHandle,
    network_pause_tx: Arc<watch::Sender<bool>>,  // NEW
}
```

**Step 5: Store in `handle_network_monitoring_started`**

In `handler/devtools/network.rs`, in `handle_network_monitoring_started`, store `network_pause_tx` on the session handle alongside the existing `network_shutdown_tx` and `network_task_handle`.

**Step 6: Send pause/unpause signals from panel switching**

In `handler/devtools/mod.rs`:

In `handle_switch_panel`, when leaving the Network panel (before the switch):
```rust
if old_panel == DevToolsPanel::Network {
    if let Some(handle) = state.session_manager.current_mut() {
        if let Some(tx) = &handle.network_pause_tx {
            let _ = tx.send(true); // pause network polling
        }
    }
}
```

In `handle_switch_panel`, in the `Network` match arm (after the switch):
```rust
DevToolsPanel::Network => {
    // Unpause network polling if task is running
    if let Some(handle) = state.session_manager.current_mut() {
        if let Some(tx) = &handle.network_pause_tx {
            let _ = tx.send(false); // unpause
        }
    }
    // ... existing StartNetworkMonitoring dispatch for first-time start ...
}
```

In `handle_exit_devtools_mode`:
```rust
// Pause network monitoring (if running)
if let Some(handle) = state.session_manager.current_mut() {
    if let Some(tx) = &handle.network_pause_tx {
        let _ = tx.send(true); // pause
    }
}
```

In `handle_enter_devtools_mode`, if default panel is Network:
```rust
// Unpause network monitoring if task is already running
if default_panel == DevToolsPanel::Network {
    if let Some(handle) = state.session_manager.current_mut() {
        if let Some(tx) = &handle.network_pause_tx {
            let _ = tx.send(false); // unpause
        }
    }
}
```

**Step 7: Clear on disconnect**

In `VmServiceDisconnected` handler, set `handle.network_pause_tx = None` alongside the existing `network_shutdown_tx = None`.

#### Channel State Diagram

```
                User switches to Network tab
                        │
                        ▼
            ┌───────────────────────────┐
            │ StartNetworkMonitoring    │
            │ network_pause = false     │  (first visit)
            │ (ACTIVE — polls normally) │
            └───────────┬───────────────┘
                        │
          ┌─────────────┴─────────────┐
          │                           │
    Switch to other panel       Stay on Network
          │                     (no change)
          ▼
    network_pause = true
    (PAUSED — no RPCs)
          │
    Switch back to Network
          │
          ▼
    network_pause = false
    (ACTIVE + immediate fetch)
          │
    Exit DevTools entirely
          │
          ▼
    network_pause = true
    (PAUSED — no RPCs)
          │
    Re-enter DevTools (default=Network)
          │
          ▼
    network_pause = false
    (ACTIVE + immediate fetch)
```

### Acceptance Criteria

1. `getHttpProfile` does NOT fire when the user is on a non-Network DevTools panel
2. `getHttpProfile` does NOT fire when the user has exited DevTools to view logs
3. Switching to the Network tab unpauses polling and triggers one immediate fetch
4. Switching away from the Network tab pauses polling within one tick
5. Entering DevTools with `default_panel = "network"` unpauses the network task if already running
6. Network data captured during pause (by the VM) is retrieved on the next unpause (immediate fetch)
7. `network_pause_tx` is stored on `SessionHandle` and cleared on disconnect
8. The existing `network_shutdown_tx` idempotency guard continues to work (only starts task once)
9. All existing tests pass: `cargo test --workspace`
10. New tests verify network_pause behavior for panel switching and DevTools exit

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_pause_tx_stored_on_session_handle() {
        // After VmServiceNetworkMonitoringStarted is handled,
        // handle.network_pause_tx should be Some(...)
    }

    #[test]
    fn test_switch_away_from_network_sends_pause() {
        // SwitchDevToolsPanel(Performance) when current panel is Network
        // should send true on network_pause_tx
    }

    #[test]
    fn test_switch_to_network_sends_unpause() {
        // SwitchDevToolsPanel(Network) should send false on network_pause_tx
    }

    #[test]
    fn test_exit_devtools_pauses_network() {
        // handle_exit_devtools_mode should send true on network_pause_tx
    }

    #[test]
    fn test_enter_devtools_with_network_default_unpauses() {
        // handle_enter_devtools_mode with default_panel = "network"
        // should send false on network_pause_tx (if task is running)
    }

    #[test]
    fn test_network_pause_cleared_on_disconnect() {
        // VmServiceDisconnected should set network_pause_tx = None
    }
}
```

### Notes

- Initial value is `false` (active), NOT `true` (paused) — this differs from `perf_pause` and `alloc_pause`. The reason: network monitoring only starts when the user is already on the Network tab, so it should poll immediately.
- The `network_shutdown_tx.is_some()` idempotency guard in `handle_switch_panel` prevents re-spawning the task. The pause channel works alongside this — once the task is spawned, subsequent Network tab visits unpause instead of re-spawning.
- The `getHttpProfile` VM Service call returns all HTTP requests since the last call. Network data captured during a pause is not lost — it accumulates on the VM side and is retrieved in full on the next unpause fetch.
- The `watch::channel` coalesces rapid updates. If the user rapidly switches panels, only the final pause/unpause state matters, preventing burst fetches.
- Edge case: if the user enters DevTools on the Network tab for the first time, the task hasn't started yet, so `network_pause_tx` is `None`. The `handle_enter_devtools_mode` unpause send safely does nothing (checks `is_some()`). The task will be started by the `StartNetworkMonitoring` action in `handle_switch_panel`, which the enter handler delegates to.
