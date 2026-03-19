## Task: Pause Performance Monitoring When Not in DevTools

**Objective**: Add a `perf_pause_tx` watch channel that gates the entire performance polling loop (both `memory_tick` and `alloc_tick` arms) when the user is not in DevTools mode, eliminating all performance-related VM Service RPCs when viewing logs.

**Depends on**: Phase 2 task 05-gate-alloc-on-panel

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/performance.rs`: Create `perf_pause_tx/rx` watch channel; gate both `memory_tick` and `alloc_tick` arms on perf_pause; add `perf_pause_rx.changed()` arm for immediate fetch on unpause
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Send `false` (unpause) in `handle_enter_devtools_mode`; send `true` (pause) in `handle_exit_devtools_mode`
- `crates/fdemon-app/src/session/handle.rs`: Add `perf_pause_tx: Option<Arc<watch::Sender<bool>>>` field
- `crates/fdemon-app/src/message.rs`: Add `perf_pause_tx` to `VmServicePerformanceMonitoringStarted` message variant
- `crates/fdemon-app/src/handler/update.rs`: Store `perf_pause_tx` in `VmServicePerformanceMonitoringStarted` handler; clear in `VmServiceDisconnected`, `VmServiceConnected`, `VmServiceReconnected`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: `UiMode` enum (to check if DevTools is active)
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Existing `alloc_pause_tx` pattern to replicate

### Details

#### Current State

- Performance monitoring starts unconditionally on `VmServiceConnected` (update.rs:~1407)
- The `memory_tick` arm (`getMemoryUsage` + `getIsolate`) runs every `memory_interval_ms` regardless of which panel is visible or whether DevTools is even open
- Phase 2 added `alloc_pause_tx` to gate only the `getAllocationProfile` arm on Performance panel visibility
- There is no mechanism to pause the entire polling loop when the user is viewing logs

#### Design: Layered Pause Channels

After this task, the performance polling loop has two independent pause channels:

| Channel | Gates | Paused When |
|---------|-------|-------------|
| `perf_pause_tx` (NEW) | Both `memory_tick` and `alloc_tick` | User is not in DevTools mode |
| `alloc_pause_tx` (existing) | Only `alloc_tick` | Performance panel is not visible |

The `alloc_tick` arm checks BOTH channels — it only fires when DevTools is active AND the Performance panel is visible. The `memory_tick` arm checks only `perf_pause_tx`.

#### Implementation Steps

**Step 1: Create the pause channel in `spawn_performance_polling`**

In `actions/performance.rs`, alongside the existing `alloc_pause_tx/rx` pair:

```rust
// Initial: paused (true) — monitoring starts at VM connect, before DevTools is opened
let (perf_pause_tx, mut perf_pause_rx) = tokio::sync::watch::channel(true);
let perf_pause_tx = std::sync::Arc::new(perf_pause_tx);
```

Include `perf_pause_tx` in the `VmServicePerformanceMonitoringStarted` message.

**Step 2: Gate both tick arms**

In the `tokio::select!` loop:

```rust
_ = memory_tick.tick() => {
    // Skip if performance monitoring is paused (not in DevTools)
    if *perf_pause_rx.borrow() {
        continue;
    }
    // ... existing getMemoryUsage + getIsolate logic ...
}

_ = alloc_tick.tick() => {
    // Skip if perf paused (not in DevTools) OR alloc paused (not on Performance panel)
    if *perf_pause_rx.borrow() || *alloc_pause_rx.borrow() {
        continue;
    }
    // ... existing getAllocationProfile logic ...
}
```

**Step 3: Add unpause arm for immediate memory fetch**

Add a new arm to the `select!` loop:

```rust
Ok(()) = perf_pause_rx.changed() => {
    if !*perf_pause_rx.borrow() {
        // Unpaused — user entered DevTools. Fire one immediate memory fetch
        // so the Performance panel shows current data without waiting for the next tick.
        // ... same getMemoryUsage + MemorySnapshot + MemorySample logic as memory_tick ...
    }
}
```

**Step 4: Store `perf_pause_tx` on `SessionHandle`**

In `session/handle.rs`:

```rust
/// Pause channel for entire performance monitoring loop.
/// `true` = paused (user not in DevTools), `false` = active.
pub perf_pause_tx: Option<Arc<watch::Sender<bool>>>,
```

Initialize to `None` in `SessionHandle::new()`.

In `message.rs`, add the field to `VmServicePerformanceMonitoringStarted`:

```rust
Message::VmServicePerformanceMonitoringStarted {
    session_id: SessionId,
    perf_shutdown_tx: Arc<watch::Sender<bool>>,
    perf_task_handle: SharedTaskHandle,
    alloc_pause_tx: Arc<watch::Sender<bool>>,
    perf_pause_tx: Arc<watch::Sender<bool>>,  // NEW
}
```

**Step 5: Store and clear in update.rs handlers**

In `VmServicePerformanceMonitoringStarted` handler (~line 1667):
- Store `perf_pause_tx` on the session handle

In `VmServiceDisconnected` handler (~line 1512):
- Set `handle.perf_pause_tx = None`

In `VmServiceConnected` handler (~line 1307):
- Clear old `handle.perf_pause_tx = None` during teardown

In `VmServiceReconnected` handler (~line 1417):
- Clear old `handle.perf_pause_tx = None` during teardown

**Step 6: Send pause/unpause signals from DevTools mode entry/exit**

In `handler/devtools/mod.rs`:

In `handle_enter_devtools_mode` (after setting ui_mode):
```rust
// Unpause performance monitoring (memory tick + alloc tick)
if let Some(handle) = state.session_manager.current_mut() {
    if let Some(tx) = &handle.perf_pause_tx {
        let _ = tx.send(false); // unpause
    }
}
```

In `handle_exit_devtools_mode` (before or after setting ui_mode):
```rust
// Pause performance monitoring
if let Some(handle) = state.session_manager.current_mut() {
    if let Some(tx) = &handle.perf_pause_tx {
        let _ = tx.send(true); // pause
    }
}
```

Note: `alloc_pause_tx` signals remain as-is — `handle_exit_devtools_mode` already sends `true` on `alloc_pause_tx`. The `perf_pause_tx` is a higher-level gate; the `alloc_pause_tx` is a finer-grained gate within Performance-specific polling.

#### Channel State Diagram

```
                      VmServiceConnected
                            │
                            ▼
                perf_pause = true (PAUSED)
                alloc_pause = true (PAUSED)
                            │
              ┌─────────────┴─────────────┐
              │                           │
       Enter DevTools                Stay in logs
              │                      (no change)
              ▼                           │
    perf_pause = false              perf_pause = true
    (alloc_pause depends                  │
     on active panel)               Zero RPCs
              │
       ┌──────┴──────┐
       │              │
    Exit DevTools   Switch panels
       │              │
       ▼              ▼
  perf_pause = true   perf_pause unchanged
  alloc_pause = true  (alloc_pause toggled
                       per existing logic)
```

### Acceptance Criteria

1. `getMemoryUsage` and `getIsolate` do NOT fire when the user is viewing logs (not in DevTools mode)
2. `getAllocationProfile` does NOT fire when the user is viewing logs (already true via alloc_pause, now also gated via perf_pause)
3. Entering DevTools mode unpauses the performance polling loop
4. Exiting DevTools mode pauses the performance polling loop
5. On unpause (entering DevTools), one immediate memory fetch fires so the Performance panel shows current data
6. The existing `alloc_pause_tx` behavior is preserved — alloc is additionally gated on Performance panel visibility
7. `perf_pause_tx` is stored on `SessionHandle` and cleared on disconnect/reconnect
8. Frame timing (event-driven, push from VM extension stream) is NOT affected
9. All existing tests pass: `cargo test --workspace`
10. New tests verify perf_pause behavior for enter/exit DevTools

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_pause_tx_stored_on_session_handle() {
        // After VmServicePerformanceMonitoringStarted is handled,
        // handle.perf_pause_tx should be Some(...)
    }

    #[test]
    fn test_enter_devtools_sends_perf_unpause() {
        // handle_enter_devtools_mode should send false on perf_pause_tx
    }

    #[test]
    fn test_exit_devtools_sends_perf_pause() {
        // handle_exit_devtools_mode should send true on perf_pause_tx
    }

    #[test]
    fn test_perf_pause_cleared_on_disconnect() {
        // VmServiceDisconnected should set perf_pause_tx = None
    }

    #[test]
    fn test_panel_switch_does_not_affect_perf_pause() {
        // SwitchDevToolsPanel should NOT change perf_pause_tx
        // (it only affects alloc_pause_tx)
    }
}
```

### Notes

- This task only adds the perf_pause channel and hooks it into DevTools entry/exit. It does NOT change when monitoring starts (that's Task 03).
- The `perf_pause` channel follows the exact same pattern as `alloc_pause`: `watch::channel<bool>`, `Arc`-wrapped sender on `SessionHandle`, `changed()` arm in the select loop.
- Convention: `true` = paused, `false` = active. This is consistent with `alloc_pause_tx`.
- Initial value is `true` (paused) because monitoring currently starts at VM connect time, well before the user opens DevTools. Task 03 will change when monitoring starts, but the pause channel remains relevant for re-entry.
- The `memory_tick` continues to tick even when paused (timers are cheap). The `continue` on the borrow check skips the actual VM calls. This is simpler than pausing/resuming the timer itself and consistent with the alloc_pause approach.
- When both `perf_pause` and `alloc_pause` are false (DevTools active, Performance panel visible), all three RPCs fire normally. When `perf_pause` is true, nothing fires regardless of `alloc_pause` state.

---

## Completion Summary

**Status:** Done
**Branch:** fix/profile-mode-lag-25

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/performance.rs` | Added `perf_pause_tx/rx` channel; gated `memory_tick` on `perf_pause_rx`; gated `alloc_tick` on both channels; added `perf_pause_rx.changed()` arm with immediate memory fetch on unpause |
| `crates/fdemon-app/src/message.rs` | Added `perf_pause_tx` field to `VmServicePerformanceMonitoringStarted` |
| `crates/fdemon-app/src/session/handle.rs` | Added `perf_pause_tx` field; initialized to `None` in `new()`; updated `Debug` impl |
| `crates/fdemon-app/src/handler/update.rs` | Store `perf_pause_tx` in `VmServicePerformanceMonitoringStarted` handler; clear on `VmServiceConnected`, `VmServiceReconnected`, `VmServiceDisconnected` |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Send `false` (unpause) in `handle_enter_devtools_mode`; send `true` (pause) in `handle_exit_devtools_mode`; added 5 new tests |
| `crates/fdemon-app/src/handler/tests.rs` | Added `perf_pause_tx` field to `VmServicePerformanceMonitoringStarted` message construction in existing test |

### Notable Decisions/Tradeoffs

1. **Immediate memory fetch on unpause**: The `perf_pause_rx.changed()` arm duplicates the memory fetch logic from `memory_tick` rather than factoring it into a shared helper. This is consistent with the existing `alloc_pause_rx.changed()` pattern, and the duplicated code is straightforward enough to not warrant a shared helper at this stage.

2. **`handle_exit_devtools_mode` accesses `session_manager.selected()` twice**: One for `perf_pause_tx` and one for `alloc_pause_tx`. The existing code already accessed it once for `alloc_pause_tx`; adding a second access is the simplest correct approach given the TEA model (no intermediate mutable borrows to thread through).

3. **`perf_pause_tx` is sent before `alloc_pause_tx` in `handle_enter_devtools_mode`**: This order doesn't matter semantically (both are async watch channels), but sends the higher-level gate before the finer-grained one as a readability convention.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app -- handler::devtools` - Passed (175 tests)
- `cargo test --workspace` - Passed (all test suites, 0 failures)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` + `--check` - Passed

### Risks/Limitations

1. **Initial value true vs. DevTools-open scenario**: If somehow the user is in DevTools when a VM reconnect fires (e.g., hot restart), `perf_pause_tx` starts `true` (paused). The user must exit and re-enter DevTools to unpause. Task 03 (change when monitoring starts) may address this, but it is out of scope here.
