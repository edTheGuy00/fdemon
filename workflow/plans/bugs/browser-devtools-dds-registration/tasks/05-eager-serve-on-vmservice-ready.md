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
