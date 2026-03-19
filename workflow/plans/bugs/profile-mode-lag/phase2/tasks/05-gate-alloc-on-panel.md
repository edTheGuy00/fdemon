## Task: Gate Allocation Profiling on Performance Panel Visibility

**Objective**: Only run the `getAllocationProfile` timer when the Performance panel is actually visible, eliminating the most expensive RPC (~1-second heap walk) when users are viewing logs or other panels.

**Depends on**: 04-scale-intervals-by-mode

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/performance.rs`: Add a `watch` channel receiver for alloc-pause state; skip alloc tick when paused; fire one immediate alloc poll on unpause
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Send pause/unpause signals when switching panels; unpause when entering DevTools with Performance as default
- `crates/fdemon-app/src/session/handle.rs`: Add `alloc_pause_tx: Option<Arc<watch::Sender<bool>>>` field to `SessionHandle`
- `crates/fdemon-app/src/message.rs`: Add `alloc_pause_tx` to `VmServicePerformanceMonitoringStarted` message variant

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: `DevToolsPanel` enum, `DevToolsViewState` struct, `UiMode`
- `crates/fdemon-app/src/handler/update.rs`: Where `VmServicePerformanceMonitoringStarted` is handled (~line 1641)

### Details

#### Current State

- The `alloc_tick` arm in the performance polling loop (`actions/performance.rs:183-225`) runs unconditionally whenever performance monitoring is active
- `getAllocationProfile` forces the Dart VM to walk the entire heap — this is the single most expensive RPC
- Performance monitoring starts on `VmServiceConnected` regardless of active panel or UI mode
- There is no mechanism to pause/resume the allocation timer based on panel visibility

#### Design: Watch Channel for Alloc Pause

Use the same `tokio::sync::watch` pattern already established for shutdown channels. A `watch::channel<bool>` where:
- `true` = allocation polling is **paused** (Performance panel not visible)
- `false` = allocation polling is **active** (Performance panel visible)

Initial value: `true` (paused) — allocation polling starts paused since performance monitoring begins on VM connect, often before the user opens DevTools.

#### Implementation Steps

**Step 1: Create the pause channel in `spawn_performance_polling`**

In `actions/performance.rs`, alongside the existing `perf_shutdown_tx/rx` pair:

```rust
// Initial: paused (true) — unpause when Performance panel is entered
let (alloc_pause_tx, mut alloc_pause_rx) = tokio::sync::watch::channel(true);
let alloc_pause_tx = Arc::new(alloc_pause_tx);
```

Include `alloc_pause_tx` in the `VmServicePerformanceMonitoringStarted` message so it gets stored on the session handle.

**Step 2: Gate the alloc tick arm**

In the `tokio::select!` loop, modify the `alloc_tick` arm:

```rust
_ = alloc_tick.tick() => {
    // Skip if allocation polling is paused (Performance panel not visible)
    if *alloc_pause_rx.borrow() {
        continue;
    }
    // ... existing getAllocationProfile logic ...
}
```

**Step 3: Fire immediate poll on unpause**

Add a new arm to the `select!` loop that watches for unpause transitions:

```rust
_ = alloc_pause_rx.changed() => {
    if !*alloc_pause_rx.borrow() {
        // Unpaused — fire one immediate allocation profile fetch
        // so the user sees fresh data when they open the panel.
        // ... same getAllocationProfile logic as the tick arm ...
    }
}
```

This ensures the allocation table is populated immediately when the user switches to the Performance panel, without waiting for the next tick.

**Step 4: Store `alloc_pause_tx` on `SessionHandle`**

In `session/handle.rs`, add:

```rust
pub alloc_pause_tx: Option<Arc<watch::Sender<bool>>>,
```

In `message.rs`, add the field to `VmServicePerformanceMonitoringStarted`:

```rust
Message::VmServicePerformanceMonitoringStarted {
    session_id: SessionId,
    perf_shutdown_tx: Arc<watch::Sender<bool>>,
    perf_task_handle: SharedTaskHandle,
    alloc_pause_tx: Arc<watch::Sender<bool>>,  // NEW
}
```

In `handler/update.rs` where `VmServicePerformanceMonitoringStarted` is handled (~line 1641-1653), store `alloc_pause_tx` on the session handle.

**Step 5: Send pause/unpause signals from panel switching**

In `handler/devtools/mod.rs`, in `handle_switch_panel`:

```rust
// When switching TO Performance panel: unpause allocation polling
DevToolsPanel::Performance => {
    if let Some(handle) = state.session_manager.current_mut() {
        if let Some(tx) = &handle.alloc_pause_tx {
            let _ = tx.send(false); // unpause
        }
    }
}

// When switching AWAY from Performance panel: pause allocation polling
// (applies to Inspector and Network branches)
// Before the match on the new panel, pause alloc if the OLD panel was Performance:
if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
    if let Some(handle) = state.session_manager.current_mut() {
        if let Some(tx) = &handle.alloc_pause_tx {
            let _ = tx.send(true); // pause
        }
    }
}
```

Also in `handle_enter_devtools_mode`: if the default panel is Performance, send unpause.

In `handle_exit_devtools_mode`: send pause (user left DevTools entirely).

**Step 6: Clean up on disconnect**

In `VmServiceDisconnected` handler, clear `alloc_pause_tx` (set to `None`). The polling task's `alloc_pause_rx` will see the sender drop and `changed()` will return an error, which the shutdown arm already handles.

#### State Diagram

```
                         VmServiceConnected
                               │
                               ▼
                    alloc_pause = true (PAUSED)
                               │
                  ┌────────────┴────────────┐
                  │                         │
           Enter DevTools              Stay in logs
           (Performance)               (no change)
                  │                         │
                  ▼                         ▼
        alloc_pause = false         alloc_pause = true
           (ACTIVE)                    (PAUSED)
                  │                         │
          Switch to                  Open DevTools
          Inspector/Network         (non-Performance)
                  │                         │
                  ▼                         ▼
        alloc_pause = true          alloc_pause = true
           (PAUSED)                    (PAUSED)
                  │
          Switch back to
          Performance
                  │
                  ▼
        alloc_pause = false
           (ACTIVE)
```

### Acceptance Criteria

1. `getAllocationProfile` does NOT fire when the Performance panel is not visible
2. `getAllocationProfile` fires normally (at the configured interval) when the Performance panel IS visible
3. Switching to the Performance panel triggers one immediate `getAllocationProfile` fetch (no stale data)
4. Switching away from the Performance panel pauses allocation polling within one tick
5. Exiting DevTools mode pauses allocation polling
6. Entering DevTools mode with `default_panel = "performance"` unpauses allocation polling
7. The memory tick arm (`getMemoryUsage` + `getIsolate`) is NOT affected by the alloc pause — memory monitoring continues regardless of panel visibility (it's lightweight)
8. `alloc_pause_tx` is stored on `SessionHandle` and cleared on disconnect
9. All existing tests pass: `cargo test --workspace`
10. New tests verify pause/unpause behavior

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_pause_tx_stored_on_session_handle() {
        // After VmServicePerformanceMonitoringStarted is handled,
        // handle.alloc_pause_tx should be Some(...)
    }

    #[test]
    fn test_switch_to_performance_sends_unpause() {
        // SwitchDevToolsPanel(Performance) should send false on alloc_pause_tx
    }

    #[test]
    fn test_switch_away_from_performance_sends_pause() {
        // SwitchDevToolsPanel(Inspector) when current panel is Performance
        // should send true on alloc_pause_tx
    }

    #[test]
    fn test_exit_devtools_sends_pause() {
        // handle_exit_devtools_mode should send true on alloc_pause_tx
    }

    #[test]
    fn test_enter_devtools_with_performance_default_sends_unpause() {
        // handle_enter_devtools_mode with default_panel = "performance"
        // should send false on alloc_pause_tx
    }

    #[test]
    fn test_alloc_pause_cleared_on_disconnect() {
        // VmServiceDisconnected should set alloc_pause_tx = None
    }
}
```

### Notes

- This task only gates the **allocation profile** timer. The memory snapshot/sample timer continues running unconditionally because `getMemoryUsage` is lightweight (no heap walk). Phase 3 will add full performance monitoring pause/resume for non-DevTools mode.
- The `watch::channel` approach is consistent with the existing `perf_shutdown_tx` / `network_shutdown_tx` pattern used throughout the session handle.
- The immediate-fetch on unpause prevents stale data when users open the Performance panel. Without it, users might see allocation data that's up to 5 seconds old (in profile mode with scaled intervals).
- The `alloc_pause_rx.changed()` arm in the select loop adds a fourth arm alongside `memory_tick`, `alloc_tick`, and `perf_shutdown_rx`. Tokio's `select!` handles this efficiently.
- Edge case: if the user rapidly toggles panels, the `watch` channel coalesces — only the final value matters, so rapid toggles don't create burst fetches.
- `alloc_pause_tx` starts as `None` on `SessionHandle::new()` and is populated by the `VmServicePerformanceMonitoringStarted` handler — consistent with `perf_shutdown_tx` lifecycle.

---

## Completion Summary

**Status:** Not Started
