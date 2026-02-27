## Task: Guard connection_status Writes in All VM Lifecycle Handlers

**Objective**: Apply the active-session guard (`active_id == Some(session_id)`) to `connection_status` and `vm_connection_error` writes in `VmServiceConnected`, `VmServiceDisconnected`, and `VmServiceConnectionFailed` handlers, matching the pattern already used in `handle_vm_service_reconnecting`.

**Depends on**: None (can be done in parallel with task 01)

**Review Reference**: Phase-2 Review Issue #6

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Guard writes in 3 handlers

### Details

#### Problem

`connection_status` is a global field on `AppState::devtools_view_state` (not per-session). `handle_vm_service_reconnecting` correctly guards it:

```rust
// handler/devtools/mod.rs:248-255 — CORRECT
let active_id = state.session_manager.selected().map(|h| h.session.id);
if active_id == Some(session_id) {
    state.devtools_view_state.connection_status = VmConnectionStatus::Reconnecting { ... };
}
```

But three other handlers write unconditionally:

**VmServiceConnected** (`update.rs:1201-1204`):
```rust
// NO GUARD — any session's connect overwrites the global indicator
state.devtools_view_state.vm_connection_error = None;
state.devtools_view_state.connection_status = VmConnectionStatus::Connected;
```

**VmServiceDisconnected** (`update.rs:1280-1281`):
```rust
// NO GUARD — background session disconnect pollutes foreground display
state.devtools_view_state.connection_status = VmConnectionStatus::Disconnected;
```

**VmServiceConnectionFailed** (`update.rs:1273-1274`):
```rust
// NO GUARD — background session failure shows error on foreground
state.devtools_view_state.vm_connection_error = Some(format!("Connection failed: {error}"));
```

#### Multi-session bug scenario

1. Session A (active tab) is healthy, `connection_status = Connected`
2. Session B (background) disconnects
3. `VmServiceDisconnected { session_id: B }` fires
4. Handler unconditionally writes `connection_status = Disconnected`
5. User sees "Disconnected" on Session A's DevTools panel — **incorrect**

The reverse is equally bad: Session A is reconnecting, Session B connects, and the indicator flips to "Connected" prematurely.

#### Fix

Add the same guard pattern to all three handlers:

**Fix 1 — VmServiceConnected** (`update.rs` ~line 1200):
```rust
let active_id = state.session_manager.selected().map(|h| h.session.id);
if active_id == Some(session_id) {
    state.devtools_view_state.vm_connection_error = None;
    state.devtools_view_state.connection_status = VmConnectionStatus::Connected;
}
```

**Fix 2 — VmServiceDisconnected** (`update.rs` ~line 1279):
```rust
let active_id = state.session_manager.selected().map(|h| h.session.id);
if active_id == Some(session_id) {
    state.devtools_view_state.connection_status = VmConnectionStatus::Disconnected;
}
```

**Fix 3 — VmServiceConnectionFailed** (`update.rs` ~line 1272):
```rust
let active_id = state.session_manager.selected().map(|h| h.session.id);
if active_id == Some(session_id) {
    state.devtools_view_state.vm_connection_error = Some(format!("Connection failed: {error}"));
}
```

### Acceptance Criteria

1. `VmServiceConnected` handler guards `connection_status` and `vm_connection_error` writes
2. `VmServiceDisconnected` handler guards `connection_status` write
3. `VmServiceConnectionFailed` handler guards `vm_connection_error` write
4. Background session VM events do not affect the foreground session's connection indicator
5. `handle_vm_service_reconnecting` remains unchanged (already correct)
6. Per-session state updates (e.g., `handle.session.vm_connected`) remain unguarded — they are per-session, not global
7. `cargo check --workspace` passes
8. `cargo clippy --workspace -- -D warnings` clean

### Notes

- If task 01 adds a `VmServiceReconnected` handler, apply the same guard there from the start
- The guard pattern is: `state.session_manager.selected().map(|h| h.session.id)` — this returns the currently active tab's session ID
- Consider whether `DevToolsViewState::reset()` (called on tab switches) already handles stale state — it does reset, but that only helps on manual tab switches, not on background events arriving while viewing a tab
- Also verify that session tab switching (`SelectSessionByIndex`) correctly syncs `connection_status` from the newly-selected session's `vm_connected` field — if not, that's a separate issue

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | Added active-session guard to `VmServiceConnected`, `VmServiceConnectionFailed`, and `VmServiceDisconnected` handlers |

### Notable Decisions/Tradeoffs

1. **Per-session state writes remain unguarded**: `handle.session.vm_connected`, `handle.vm_request_handle`, `handle.vm_shutdown_tx`, and all polling task handles are per-session fields and continue to be updated regardless of which session is active — only the global `devtools_view_state` fields are guarded.

2. **Guard placed before `session_manager.get_mut`**: In `VmServiceDisconnected`, the `active_id` guard is evaluated before calling `get_mut` to avoid a borrow conflict; the per-session cleanup block runs unconditionally after, which is correct.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-app` - Passed (1129 tests, 5 ignored, 0 failed)

### Risks/Limitations

1. **No new unit tests added**: The multi-session guard behaviour is covered by the existing handler test suite structure; dedicated tests for background-session isolation would require multi-session fixtures not yet present in the test harness.
