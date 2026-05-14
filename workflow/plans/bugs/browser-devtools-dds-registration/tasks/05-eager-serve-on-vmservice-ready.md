## Task: Eagerly Serve DevTools When VM Service Becomes Ready

**Objective**: When a session reaches VM-service-ready state, fire `DaemonCommand::ServeDevTools` automatically. Handle the eventual response (or failure) by populating `Session.devtools_endpoint`.

**Depends on**: 04-session-stores-devtools-url

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/session.rs`: In `handle_app_started` or `handle_vm_service_ready` (whichever fires after `ws_uri` is known) — emit a follow-up `UpdateAction::SendDaemonCommand(DaemonCommand::ServeDevTools { request_id: Some(format!("devtools-serve-{session_id}")) })` and set `session.devtools_serve_pending = true`.
- `crates/fdemon-app/src/handler/update.rs`: Route `Message::DevToolsServed` (from `DaemonMessage::DevToolsServed` lifting in the daemon-event-to-Message bridge) into a new handler `handle_devtools_served(state, session_id, host, port)`:
  - Set `session.devtools_endpoint = Some(DevToolsEndpoint { host, port, served_at: Instant::now() })`.
  - Set `session.devtools_serve_pending = false`.
  - `info!("DevTools served at {host}:{port} for session {session_id}")`.
- `crates/fdemon-app/src/handler/update.rs`: Route `Message::DevToolsServeFailed` into `handle_devtools_serve_failed(state, session_id, reason)`:
  - Set `session.devtools_serve_pending = false`.
  - Leave `devtools_endpoint = None`.
  - `warn!(session_id, reason, "DevTools serve failed; falling back to legacy URL")`.
- `crates/fdemon-app/src/actions/mod.rs`: If `UpdateAction::SendDaemonCommand` doesn't already exist, add it and route it to the appropriate `FlutterProcess` stdin writer. (Likely it does — verify and use.)
- `crates/fdemon-app/src/handler/daemon_event.rs` (or wherever `DaemonMessage` is lifted into `Message`): Add the bridge for `DaemonMessage::DevToolsServed` → `Message::DevToolsServed { session_id, host, port }` and `DevToolsServeFailed` → `Message::DevToolsServeFailed { session_id, reason }`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/commands.rs`: For `DaemonCommand` shape.
- `crates/fdemon-app/src/session/session.rs`: For `Session` field access.

### Details

The trigger point matters: `app.debugPort` is the earliest event carrying `ws_uri`. Some flows already trigger `VmServiceReady` from this — fire `ServeDevTools` at the same time.

Be careful to avoid duplicates: only fire once per session. If `session.devtools_serve_pending` is already `true`, skip. If `session.devtools_endpoint` is already populated, skip.

```rust
// handler/session.rs — after ws_uri populated
if session.devtools_endpoint.is_none() && !session.devtools_serve_pending {
    session.devtools_serve_pending = true;
    return UpdateResult::with_action(UpdateAction::SendDaemonCommand(
        DaemonCommand::ServeDevTools {
            request_id: Some(format!("devtools-serve-{}", session.id)),
        },
    ));
}
```

### Acceptance Criteria

1. When a session reaches the ready state, `ServeDevTools` is fired exactly once.
2. On `DevToolsServed`, `session.devtools_endpoint` is populated; `devtools_serve_pending = false`.
3. On `DevToolsServeFailed`, `devtools_serve_pending = false`; `devtools_endpoint` stays `None`; warning logged.
4. Multiple sessions get independent `ServeDevTools` requests with distinct request ids.
5. Unit tests cover: dispatch trigger, success handler, failure handler, idempotence (no duplicate dispatch).
6. `cargo test --workspace` passes; `cargo clippy --workspace -- -D warnings` passes.

### Testing

```rust
#[test]
fn vm_service_ready_triggers_serve_devtools() {
    let mut state = AppState::test_default();
    state.add_test_session(/* ... */);
    let result = handle_app_started(&mut state, /* session_id, ws_uri */);
    match result.action.unwrap() {
        UpdateAction::SendDaemonCommand(DaemonCommand::ServeDevTools { request_id }) => {
            assert!(request_id.unwrap().starts_with("devtools-serve-"));
        }
        _ => panic!("expected ServeDevTools dispatch"),
    }
    let session = active_session(&state);
    assert!(session.devtools_serve_pending);
    assert!(session.devtools_endpoint.is_none());
}

#[test]
fn devtools_served_populates_endpoint() {
    let mut state = AppState::test_default();
    let sid = state.add_test_session(/* ... */);
    handle_devtools_served(&mut state, sid, "127.0.0.1".into(), 9100);
    let session = state.session_manager.get(sid).unwrap();
    assert_eq!(session.devtools_endpoint.as_ref().unwrap().port, 9100);
    assert!(!session.devtools_serve_pending);
}

#[test]
fn devtools_serve_failed_clears_pending() { /* ... */ }

#[test]
fn idempotent_dispatch() { /* call twice, expect one action */ }
```

### Notes

- If `UpdateAction::SendDaemonCommand` doesn't accept a `DaemonCommand` directly, plumb the new variant through. Avoid silently dropping unknown commands at the dispatch layer.
- The bridge from `DaemonMessage` to `Message` happens at the daemon-reader/engine boundary — find the existing pattern and follow it.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `UpdateAction::SendDaemonCommand { session_id, command, cmd_sender }` variant |
| `crates/fdemon-app/src/handler/session.rs` | Added `maybe_serve_devtools()` helper; added 5 unit tests for dispatch trigger, idempotence, multi-session, no-cmd_sender guard |
| `crates/fdemon-app/src/handler/daemon.rs` | Added bridge for `DaemonMessage::DevToolsServed` → `Message::DevToolsServed` (primary `app.devTools` path) and `DevToolsServeFailed` bridge, in both `Stdout` and `Message` arms |
| `crates/fdemon-app/src/handler/update.rs` | Added `info!` log to `DevToolsServed` handler; improved `warn!` to `DevToolsServeFailed`; calls `maybe_serve_devtools` in `VmServiceConnected` handler (non-DevTools mode) |
| `crates/fdemon-app/src/process.rs` | Added `hydrate_send_daemon_command()` to fill `cmd_sender` from session before dispatch |
| `crates/fdemon-app/src/actions/mod.rs` | Added `SendDaemonCommand` arm in `handle_action` (fire-and-forget via `send_fire_and_forget`) |
| `crates/fdemon-app/src/handler/tests.rs` | Added `devtools_served_handler` test module with 5 tests covering success, failure, overwrite, DDS URL, unknown-session noop |

### Notable Decisions/Tradeoffs

1. **`VmServiceConnected` as the fallback trigger instead of `AppDebugPort`**: `AppDebugPort` fires alongside `ConnectVmService` which already consumes the single `UpdateAction` slot. Using `VmServiceConnected` (which fires after the VM WebSocket connects) avoids the action conflict. In DevTools mode, `StartPerformanceMonitoring` takes priority; the `app.devTools` primary path fires before VM connection in modern Flutter, making the fallback a no-op in the common case.

2. **`cmd_sender` guard in `maybe_serve_devtools`**: Added an early return when `cmd_sender` is None (process not yet attached). This prevents setting `devtools_serve_pending = true` prematurely and avoids hydration-and-discard in `process.rs`. Existing tests (which create sessions without cmd_senders) continue to assert `result.action.is_none()` unchanged.

3. **Deferred action when both `app.devTools` event and action conflict**: In the rare case where both a devtools bridge message and a regular action are present in the `Stdout` arm, the action takes priority. The `app.devTools` event fires separately from `AppDebugPort`, so in practice only one is non-None per daemon event.

4. **`SendDaemonCommand` uses `send_fire_and_forget`**: The `devtools.serve` RPC is fire-and-forget because the response comes back as a daemon event (`app.devTools` or a JSON-RPC response) that is already handled by the bridge. No need for request-response correlation in the action.

### Testing Performed

- `cargo test -p fdemon-app` — 2142 passed; 0 failed (includes 10 new tests)
- `cargo test --workspace --lib` — all crates pass
- `cargo clippy --workspace -- -D warnings` — PASS
- `cargo fmt --all -- --check` — PASS

### Risks/Limitations

1. **Fallback only fires in Normal mode**: `maybe_serve_devtools` is only called in the non-DevTools `VmServiceConnected` branch. If the user is in DevTools mode when the VM connects, the fallback is skipped. Mitigated by the primary `app.devTools` event path which fires regardless of UI mode.

2. **Edge case: `app.devTools` and `SendDaemonCommand` race**: If `app.devTools` arrives between `VmServiceConnected` and the dispatch of `SendDaemonCommand`, `devtools_endpoint` is set but `devtools_serve_pending` remains true until the response arrives. The response handler clears it. This is benign.
