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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/protocol.rs` | Added `pub mod stream_id` with five named constants (`EXTENSION`, `LOGGING`, `GC`, `DEBUG`, `ISOLATE`) and three corresponding tests |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `stream_id` to the `use super::protocol` import; updated `RESUBSCRIBE_STREAMS` to use the constants and include `DEBUG`/`ISOLATE`; updated `subscribe_flutter_streams` to subscribe to `Debug` and `Isolate` streams via constants; added three new tests for `RESUBSCRIBE_STREAMS` coverage |

### Notable Decisions/Tradeoffs

1. **Constants in `protocol.rs`, used from `client.rs`**: The `stream_id` module lives in `protocol.rs` alongside the other VM Service protocol types. This follows the existing pattern where protocol-level constants and types are co-located. `client.rs` imports them via `use super::protocol::stream_id`, which keeps the layer boundary clean.

2. **`subscribe_flutter_streams` updated alongside `RESUBSCRIBE_STREAMS`**: Both the initial-connect path (`subscribe_flutter_streams`) and the reconnect path (`RESUBSCRIBE_STREAMS`/`resubscribe_streams`) were updated together to ensure they stay in sync. This satisfies acceptance criteria 2 and 3.

3. **String literals in tests left unchanged**: Test fixtures in the test modules still use raw string literals (e.g. `"Extension"` in JSON payloads) where they are part of test data rather than code routing. The constants are used in all production routing/subscription code.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (389 unit tests, 0 failed, 3 ignored)
  - New tests: `test_resubscribe_streams_includes_debug_and_isolate`, `test_resubscribe_streams_retains_existing_streams`, `test_resubscribe_streams_uses_correct_stream_id_values`
  - New tests in protocol: `test_stream_id_constants_match_vm_service_protocol`, `test_stream_id_debug_constant`, `test_stream_id_isolate_constant`
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Debug stream event volume**: As noted in the task, the `Debug` stream can be high-frequency during stepping. The existing `mpsc::channel` with `EVENT_CHANNEL_CAPACITY = 256` handles bursts; no capacity concerns at this phase since app-level routing (task 05) is not yet implemented.
