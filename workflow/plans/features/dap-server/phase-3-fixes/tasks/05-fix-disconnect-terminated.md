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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/server/session.rs` | `handle_disconnect` now emits `terminated` event before the response; updated all unit tests and integration tests that expected 1 message from disconnect to expect 2; added new `test_disconnect_sends_terminated_event_before_response` test |
| `crates/fdemon-dap/src/adapter/mod.rs` | Removed `"disconnect" => self.handle_disconnect(request).await` arm from `handle_request`; removed `handle_disconnect` method entirely; removed two now-dead adapter tests (`test_handle_disconnect_returns_success`, `test_handle_disconnect_sends_terminated_event`) |
| `crates/fdemon-dap/src/server/mod.rs` | Updated `test_client_full_handshake_over_tcp` to read terminated event before disconnect response |
| `crates/fdemon-dap/src/transport/stdio.rs` | Updated `test_run_on_full_handshake_initialize_configure_disconnect` and `test_run_on_multiple_requests_over_single_stream` to consume terminated event before disconnect response |

### Notable Decisions/Tradeoffs

1. **Adapter tests removed, not updated**: The two adapter tests (`test_handle_disconnect_returns_success`, `test_handle_disconnect_sends_terminated_event`) tested dead code (`DapAdapter::handle_disconnect`). Since that method was fully removed, the tests were deleted rather than updated — there is nothing to test in the adapter for disconnect anymore.

2. **`test_run_on_session_lifecycle_connect_initialize_disconnect` not changed**: This test uses `let _ = read_message(...)` to discard the disconnect response. With 2 messages now, one is still discarded, but the test only cares about the `ClientDisconnected` server event (not the message count). The extra buffered message does not cause the server to stall, so the test still passes without modification.

3. **`test_disconnect_returns_success_response` renamed**: Renamed to `test_disconnect_returns_terminated_event_then_success_response` to accurately describe the new two-message behaviour.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test --workspace` — Passed (all tests across all crates, including 62 ignored integration tests)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Wire-level behaviour change**: Any real DAP client connected to fdemon will now receive a `terminated` event before the disconnect response. This is spec-correct and IDEs (VS Code, Zed, Helix) expect this ordering, so this is strictly an improvement.
