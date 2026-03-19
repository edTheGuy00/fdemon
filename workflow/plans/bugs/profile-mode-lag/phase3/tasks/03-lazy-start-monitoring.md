## Task: Lazy-Start Performance Monitoring

**Objective**: Defer performance monitoring startup from `VmServiceConnected` to the first time the user enters DevTools, so that sessions which never open DevTools incur zero polling overhead. Combined with Tasks 01 and 02, this achieves the Phase 3 goal of zero VM Service polling RPCs when viewing logs.

**Depends on**: 01-pause-perf-when-not-devtools (uses `perf_pause_tx` channel for resume behavior)

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs`: Remove `StartPerformanceMonitoring` from `VmServiceConnected` and `VmServiceReconnected` handlers; add conditional start when DevTools is already active during (re)connect
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Add `StartPerformanceMonitoring` dispatch to `handle_enter_devtools_mode` when VM is connected and no perf task is running
- `crates/fdemon-app/src/actions/performance.rs`: Change initial `perf_pause` value handling — when started from DevTools entry, monitoring should start active (not paused)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: `UiMode`, `DevToolsViewState`, `VmConnectionStatus`
- `crates/fdemon-app/src/session/handle.rs`: `perf_shutdown_tx` (sentinel for whether perf task is running), `perf_pause_tx`
- `crates/fdemon-app/src/handler/mod.rs`: `UpdateAction::StartPerformanceMonitoring` variant definition
- `crates/fdemon-app/src/config/types.rs`: `Settings` (performance_refresh_ms, allocation_profile_interval_ms)

### Details

#### Current State

- `VmServiceConnected` unconditionally emits `UpdateAction::StartPerformanceMonitoring` (update.rs:~1407)
- `VmServiceReconnected` unconditionally emits `UpdateAction::StartPerformanceMonitoring` (update.rs:~1477)
- The performance task starts with `perf_pause = true` (from Task 01), so it's paused but still alive — consuming a tokio task slot and maintaining timer state even if the user never opens DevTools
- After Task 01, entering DevTools sends `perf_pause = false` to unpause the existing task

#### Design: Deferred Start with On-Demand Resume

Remove the eager start from `VmServiceConnected` / `VmServiceReconnected`. Instead:

1. **First DevTools entry**: `handle_enter_devtools_mode` checks if VM is connected and no perf task is running → dispatches `StartPerformanceMonitoring`
2. **Subsequent entries**: perf task already exists, so just unpause via `perf_pause_tx.send(false)` (existing Task 01 behavior)
3. **VM (re)connects while in DevTools**: the handler checks if DevTools is active and starts monitoring immediately

#### Implementation Steps

**Step 1: Remove eager start from `VmServiceConnected`**

In `handler/update.rs`, in the `VmServiceConnected` handler (~line 1407):

Before (current):
```rust
// Always returns StartPerformanceMonitoring
UpdateResult::with_action(UpdateAction::StartPerformanceMonitoring { ... })
```

After:
```rust
// Only start monitoring if DevTools is currently active
if state.ui_mode == UiMode::DevTools {
    UpdateResult::with_action(UpdateAction::StartPerformanceMonitoring { ... })
} else {
    UpdateResult::default() // or whatever the "no action" result is
}
```

Keep the teardown logic (abort old task, clear old senders) — that cleanup is still needed on reconnect regardless.

**Step 2: Same change for `VmServiceReconnected`**

In `handler/update.rs`, in the `VmServiceReconnected` handler (~line 1477):

Apply the same conditional: only start monitoring if DevTools is active.

**Step 3: Add start-on-demand in `handle_enter_devtools_mode`**

In `handler/devtools/mod.rs`, in `handle_enter_devtools_mode`:

After the existing logic (set active_panel, enter DevTools mode), check if monitoring needs to start:

```rust
// If VM is connected and no performance monitoring task is running, start it now
let needs_start = if let Some(handle) = state.session_manager.current() {
    handle.perf_shutdown_tx.is_none()
        && state.devtools_view_state.connection_status == VmConnectionStatus::Connected
} else {
    false
};

if needs_start {
    // Build StartPerformanceMonitoring action
    let session_id = state.session_manager.selected_id().unwrap();
    return UpdateResult::with_action(UpdateAction::StartPerformanceMonitoring {
        session_id,
        handle: None, // hydrated by process.rs
        performance_refresh_ms: state.settings.devtools.performance_refresh_ms,
        allocation_profile_interval_ms: state.settings.devtools.allocation_profile_interval_ms,
        mode: /* get FlutterMode from session's launch config, default Debug */,
    });
}
```

Note: this action also needs the existing `perf_pause_tx.send(false)` and `alloc_pause_tx.send(false)` signals. But there's a timing subtlety — see Step 4.

**Step 4: Handle the timing issue for initial pause values**

When monitoring starts from `handle_enter_devtools_mode`:
1. `StartPerformanceMonitoring` action is dispatched
2. `spawn_performance_polling` creates `perf_pause_tx` with initial value `true` (paused)
3. `VmServicePerformanceMonitoringStarted` message fires
4. Handler stores `perf_pause_tx` on SessionHandle
5. But `handle_enter_devtools_mode` already returned — it can't send unpause

**Solution**: In the `VmServicePerformanceMonitoringStarted` handler (update.rs:~1667), after storing the senders, check the current UI state and adjust:

```rust
// If DevTools is currently active, unpause performance monitoring immediately
// (handles the case where monitoring was just lazy-started from DevTools entry)
if state.ui_mode == UiMode::DevTools {
    let _ = perf_pause_tx.send(false);
}

// If the active panel is Performance, also unpause allocation polling
if state.ui_mode == UiMode::DevTools
    && state.devtools_view_state.active_panel == DevToolsPanel::Performance
{
    let _ = alloc_pause_tx.send(false);
}
```

This is the simplest solution — the handler has access to the freshly created senders and the current UI state. No changes to the action enum or spawn function signature needed.

**Step 5: Verify frame timing is unaffected**

Frame timing comes from the Dart VM's extension stream events, not from the performance polling task. The VM Service connection itself (established in `VmServiceConnected`) continues to receive push events regardless of whether performance polling is active. No changes needed.

**Step 6: Handle edge case — session switch while in DevTools**

When the user switches sessions (1-9 keys) while in DevTools, the new session may not have a perf task running yet. The existing `handle_switch_panel` or a follow-up message should trigger monitoring for the new session.

Check: does session switching re-trigger `handle_enter_devtools_mode`? If not, add a check in the session switch handler (Message::SelectSessionByIndex) that starts monitoring if DevTools is active and the new session has no perf task.

#### State Machine

```
┌─────────────────────────┐
│     VM Not Connected    │
│   (no monitoring at all)│
└────────────┬────────────┘
             │ VmServiceConnected
             ▼
┌─────────────────────────┐
│   VM Connected,         │
│   DevTools NOT active   │──── No monitoring task spawned
│   (viewing logs)        │     (zero overhead)
└────────────┬────────────┘
             │ User presses 'd' (EnterDevToolsMode)
             ▼
┌─────────────────────────┐
│   VM Connected,         │
│   DevTools active,      │──── Monitoring started (lazy)
│   perf_pause = false    │     perf_pause adjusted by handler
└────────────┬────────────┘
             │ User presses Esc (ExitDevToolsMode)
             ▼
┌─────────────────────────┐
│   VM Connected,         │
│   DevTools NOT active   │──── Monitoring PAUSED (task alive, zero RPCs)
│   perf_pause = true     │
└────────────┬────────────┘
             │ User re-enters DevTools
             ▼
┌─────────────────────────┐
│   VM Connected,         │
│   DevTools active,      │──── Monitoring UNPAUSED (immediate fetch)
│   perf_pause = false    │     Task already running, just unpause
└─────────────────────────┘
```

Edge case: VM disconnects while in DevTools → task is cleaned up. VM reconnects while still in DevTools → new task started immediately (Step 1/2 check `ui_mode == DevTools`).

### Acceptance Criteria

1. No performance monitoring task is spawned when `VmServiceConnected` fires and DevTools is not active
2. Performance monitoring starts when the user first enters DevTools (presses 'd')
3. VM (re)connecting while DevTools is already active starts monitoring immediately
4. The `perf_pause_tx` starts with the correct initial value based on current UI state (active if DevTools is open)
5. The `alloc_pause_tx` starts with the correct initial value based on current panel (active if Performance panel is visible)
6. Subsequent DevTools exits/entries use pause/unpause (don't restart the task)
7. Frame timing (push events from VM extension stream) is completely unaffected
8. Session switching while in DevTools starts monitoring for the new session if needed
9. All existing tests pass: `cargo test --workspace`
10. New tests cover lazy-start trigger, VM reconnect during DevTools, and session switch scenarios

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_connect_without_devtools_does_not_start_monitoring() {
        // VmServiceConnected when ui_mode != DevTools
        // should NOT return StartPerformanceMonitoring action
    }

    #[test]
    fn test_vm_connect_with_devtools_active_starts_monitoring() {
        // VmServiceConnected when ui_mode == DevTools
        // SHOULD return StartPerformanceMonitoring action
    }

    #[test]
    fn test_enter_devtools_starts_monitoring_when_vm_connected() {
        // handle_enter_devtools_mode when VM connected and no perf task
        // should return StartPerformanceMonitoring action
    }

    #[test]
    fn test_enter_devtools_does_not_start_when_vm_disconnected() {
        // handle_enter_devtools_mode when VM not connected
        // should NOT return StartPerformanceMonitoring
    }

    #[test]
    fn test_enter_devtools_does_not_restart_existing_task() {
        // handle_enter_devtools_mode when perf_shutdown_tx is Some
        // (task already running) should just unpause, not restart
    }

    #[test]
    fn test_monitoring_started_handler_adjusts_pause_for_active_devtools() {
        // VmServicePerformanceMonitoringStarted when ui_mode == DevTools
        // should send false (unpause) on perf_pause_tx
    }

    #[test]
    fn test_monitoring_started_handler_adjusts_alloc_for_performance_panel() {
        // VmServicePerformanceMonitoringStarted when DevTools active
        // AND active_panel == Performance should send false on alloc_pause_tx
    }

    #[test]
    fn test_session_switch_in_devtools_starts_monitoring_for_new_session() {
        // When switching sessions while in DevTools, if the new session
        // has VM connected but no perf task, monitoring should start
    }
}
```

### Notes

- This task depends on Task 01 (`perf_pause_tx` channel) but NOT on Task 02 (network pause). Network monitoring is already demand-started and doesn't need this change.
- The timing issue (Step 4) is the trickiest part. The `VmServicePerformanceMonitoringStarted` handler is the right place to adjust initial pause state because it has access to both the freshly created senders and the current UI state. This avoids adding params to `StartPerformanceMonitoring` or changing `spawn_performance_polling`'s signature.
- **Backwards compatibility tradeoff**: Users who relied on memory history being populated before opening DevTools will now see an empty Performance panel until they enter DevTools. The BUG.md suggests a mitigation (slow background poll or config flag), but the simplest approach is to populate on first visit. The immediate-fetch-on-unpause from Task 01 ensures data appears within one RPC round-trip.
- Frame timing is completely separate from the performance polling task. It's driven by `Dart Developer Service Extension` stream events, which are received over the VM Service WebSocket connection. The WebSocket connection is established in `VmServiceConnected` and stays open regardless of monitoring state.
- The perf state reset in `VmServiceConnected` (clearing ring buffers, etc.) should remain unconditional — it's about clearing stale data, not about monitoring lifecycle.

---

## Completion Summary

**Status:** Done
**Branch:** fix/profile-mode-lag-25

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | Gated `StartPerformanceMonitoring` in `VmServiceConnected` and `VmServiceReconnected` on `ui_mode == DevTools`; added pause adjustment in `VmServicePerformanceMonitoringStarted` handler |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added lazy-start logic in `handle_enter_devtools_mode`: detects missing perf task, dispatches `StartPerformanceMonitoring` on first DevTools entry |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | Added `maybe_start_monitoring_for_selected_session` helper; called from `handle_select_session_by_index`, `handle_next_session`, `handle_previous_session` |
| `crates/fdemon-app/src/handler/tests.rs` | Updated 8 existing tests for new behavior; added 13 new tests covering lazy-start, VM reconnect during DevTools, and session switch scenarios |

### Notable Decisions/Tradeoffs

1. **`session.vm_connected` instead of `connection_status`**: The `devtools_view_state.connection_status` is reset to `Disconnected` by `DevToolsViewState::reset()` during session switching, making it unreliable for the "should we start monitoring?" check. Used `session.vm_connected` (the authoritative per-session flag) throughout.

2. **Timing fix in `VmServicePerformanceMonitoringStarted`**: When monitoring is lazy-started from `handle_enter_devtools_mode`, the task starts with `perf_pause = true` (the initial channel value). The handler now checks `ui_mode` and `active_panel` immediately after storing the senders and sends the correct unpause signals, avoiding a window where monitoring is alive but paused while the user is staring at DevTools.

3. **Session switch**: `DevToolsViewState::reset()` is called during session switches but does not affect `session.vm_connected`. The `maybe_start_monitoring_for_selected_session` helper is a clean DRY extraction used by all three session switch handlers.

4. **Performance state reset remains unconditional**: The ring buffer clearing in `VmServiceConnected` was intentionally left unconditional — it clears stale data on reconnect regardless of monitoring lifecycle.

### Testing Performed

- `cargo test -p fdemon-app` - PASS (1850 tests)
- `cargo test --workspace` - PASS (all crates)
- `cargo clippy --workspace` - PASS (no warnings)
- `cargo fmt --all -- --check` - PASS

### Risks/Limitations

1. **Empty Performance panel on first visit**: Users who were accustomed to memory data being pre-populated before opening DevTools will now see an empty panel on first open. The immediate-fetch-on-unpause from Task 01 means data appears within one polling cycle (~2 seconds), which is acceptable per the task notes.
