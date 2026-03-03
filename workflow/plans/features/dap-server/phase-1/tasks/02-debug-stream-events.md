## Task: Subscribe to Debug + Isolate Streams and Forward Events

**Objective**: Update the VM Service client to subscribe to `Debug` and `Isolate` streams on connect/reconnect, and forward parsed events through the existing `VmClientEvent` pipeline.

**Depends on**: None (can start in parallel with task 01 — this task modifies stream subscription and event forwarding plumbing, while task 01 defines the types)

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs` — Add `"Debug"` and `"Isolate"` to `RESUBSCRIBE_STREAMS`
- `crates/fdemon-daemon/src/vm_service/protocol.rs` — Add discriminants for Debug/Isolate stream IDs to event routing

### Details

#### 1. Add streams to subscription list

In `client.rs`, update the `RESUBSCRIBE_STREAMS` constant at line 274:

```rust
// Before:
const RESUBSCRIBE_STREAMS: &[&str] = &["Extension", "Logging", "GC"];

// After:
const RESUBSCRIBE_STREAMS: &[&str] = &["Extension", "Logging", "GC", "Debug", "Isolate"];
```

This single change ensures both initial connection and reconnection (via `resubscribe_streams()` at line 935) subscribe to the new streams. The existing `streamListen` loop handles arbitrary stream names — no other transport changes needed.

#### 2. Verify event forwarding path

Debug and Isolate stream events already flow through the existing pipeline:

```
WebSocket frame
  → run_io_loop() parses VmServiceMessage::Event(VmServiceEvent)
  → StreamEvent { stream_id, kind, data, ... } is extracted
  → event_tx.send(VmClientEvent::StreamEvent(event))
  → arrives at mpsc::Receiver<VmClientEvent> in fdemon-app
```

The new streams produce `StreamEvent` objects with:
- `stream_id = "Debug"` and `kind` = `"PauseBreakpoint"`, `"PauseException"`, `"Resume"`, etc.
- `stream_id = "Isolate"` and `kind` = `"IsolateStart"`, `"IsolateRunnable"`, etc.

These will arrive in `fdemon-app` as `VmClientEvent::StreamEvent` and be routed by `stream_id` in the event processing code (task 05 handles the app-level routing).

#### 3. Add stream ID constants (optional but recommended)

In `protocol.rs`, add constants for stream identification:

```rust
/// VM Service stream identifiers.
pub mod stream_id {
    pub const EXTENSION: &str = "Extension";
    pub const LOGGING: &str = "Logging";
    pub const GC: &str = "GC";
    pub const DEBUG: &str = "Debug";
    pub const ISOLATE: &str = "Isolate";
}
```

Then update `RESUBSCRIBE_STREAMS` to use these constants:

```rust
const RESUBSCRIBE_STREAMS: &[&str] = &[
    stream_id::EXTENSION,
    stream_id::LOGGING,
    stream_id::GC,
    stream_id::DEBUG,
    stream_id::ISOLATE,
];
```

And update existing `stream_id` string comparisons in the codebase to use these constants.

### Acceptance Criteria

1. `RESUBSCRIBE_STREAMS` includes `"Debug"` and `"Isolate"`
2. On VM Service connect, `streamListen("Debug")` and `streamListen("Isolate")` are called
3. On reconnect, both streams are re-subscribed (via existing `resubscribe_streams()` function)
4. Debug/Isolate stream events arrive as `VmClientEvent::StreamEvent` at the receiver
5. No regression in existing Extension/Logging/GC stream handling
6. Stream ID constants defined and used consistently
7. `cargo check -p fdemon-daemon` passes
8. `cargo test -p fdemon-daemon` passes (no regressions)

### Testing

Since stream subscription is tested by the existing `resubscribe_streams` tests, add a focused test:

```rust
#[test]
fn test_resubscribe_streams_includes_debug_and_isolate() {
    assert!(RESUBSCRIBE_STREAMS.contains(&"Debug"));
    assert!(RESUBSCRIBE_STREAMS.contains(&"Isolate"));
}

#[test]
fn test_stream_id_constants() {
    assert_eq!(stream_id::DEBUG, "Debug");
    assert_eq!(stream_id::ISOLATE, "Isolate");
}
```

Integration-level verification (that `streamListen` is actually called) is covered by the existing reconnection tests. The stream subscription mechanism is generic — adding strings to the constant array is all that's needed.

### Notes

- This is the smallest task in Phase 1 — it's a ~5-line change to `RESUBSCRIBE_STREAMS` plus optional stream ID constants.
- The event *parsing* (turning raw `StreamEvent` data into typed `DebugEvent`/`IsolateEvent`) happens at the app layer (task 05), not here.
- Debug stream events can be high-frequency during stepping. The existing `mpsc::channel` buffer handles this — no capacity concerns at this stage.
- The `Debug` stream may generate events even when no DAP client is connected. This is fine — the events will be received and routed to the handler, which will check if debugging is active before processing.

---

## Completion Summary

**Status:** Not started
