## Task: Emit `terminated` Event on Client-Initiated Disconnect

**Objective**: Ensure DAP clients receive a `terminated` event before the disconnect response when they send a `disconnect` request. Currently, the session intercepts `disconnect` before the adapter, so the adapter's `terminated` event emission is dead code.

**Depends on**: None

**Estimated Time**: 1–2 hours

**Severity**: MAJOR — IDEs may not properly clean up their debug UI without the `terminated` event.

### Scope

- `crates/fdemon-dap/src/server/session.rs`: Add `terminated` event to `handle_disconnect`
- `crates/fdemon-dap/src/adapter/mod.rs`: Remove dead `disconnect` arm from adapter's `handle_request`

### Details

#### Current Flow

```
Client sends "disconnect" request
  └── session.handle_request() matches "disconnect" at session.rs:518
        └── session.handle_disconnect() at session.rs:622-626
              ├── Sets state = SessionState::Disconnecting
              └── Returns [Response(success)]  ← NO terminated event

Adapter's handle_request() at adapter/mod.rs:391-409 also matches "disconnect"
  └── adapter.handle_disconnect() at adapter/mod.rs:1584-1588
        ├── Sends terminated event  ← NEVER REACHED (dead code)
        └── Returns success response
```

The session intercepts `"disconnect"` at line 518 and never falls through to the wildcard branch that delegates to the adapter. The adapter's `handle_disconnect` is dead code.

#### Fix

**Step 1**: Emit `terminated` event in session's `handle_disconnect`:

```rust
fn handle_disconnect(&mut self, request: &DapRequest) -> Vec<DapMessage> {
    self.state = SessionState::Disconnecting;

    // Emit terminated event before the disconnect response (DAP spec)
    let terminated = self.make_event(DapEvent::terminated());
    let resp = self.make_response(DapResponse::success(request, None));

    vec![
        DapMessage::Event(terminated),  // terminated first
        DapMessage::Response(resp),     // then disconnect response
    ]
}
```

**Step 2**: Remove the dead `"disconnect"` arm from adapter's `handle_request` at `adapter/mod.rs:406`:

```rust
// Remove this line:
"disconnect" => self.handle_disconnect(request).await,
```

**Step 3**: Remove or mark `adapter.handle_disconnect()` at `adapter/mod.rs:1584-1588`. Since the session now handles it, the adapter method is unused. Remove it entirely.

#### DAP Spec Reference

The DAP specification says: "The `terminated` event indicates that debugging of the debuggee has terminated. This does **not** mean that the debuggee itself has exited." Clients use this event to transition their UI out of debug mode. It should be sent before the disconnect response.

### Acceptance Criteria

1. Client-initiated `disconnect` produces: `terminated` event → `disconnect` response (in that order)
2. Server-initiated shutdown still produces `terminated` event (existing behavior, verify preserved)
3. No dead code: adapter's `handle_disconnect` is removed
4. No `#[allow(dead_code)]` needed on any disconnect-related method
5. All existing tests pass

### Testing

```rust
#[tokio::test]
async fn test_disconnect_sends_terminated_event() {
    // Set up session, complete initialization + attach
    // Send disconnect request
    // Verify response contains:
    //   1. Event with event="terminated"
    //   2. Response with command="disconnect", success=true
    // Verify terminated event seq < disconnect response seq
}
```

### Notes

- The `DapEvent::terminated()` constructor should already exist (used in the shutdown arm of the select loop). Verify it's available.
- Check if any tests assert the exact response count from `handle_disconnect` — they may need updating to expect 2 messages instead of 1.
