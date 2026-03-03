## Task: Add VmClientEvent enum and update daemon channel types

**Objective**: Introduce a `VmClientEvent` wrapper enum that extends the VM Service event channel to carry both stream notifications and connection lifecycle events. Refactor channel plumbing throughout the daemon crate.

**Depends on**: None (Phase 1 complete)

### Scope

- `crates/fdemon-daemon/src/vm_service/protocol.rs`: Add `VmClientEvent` enum
- `crates/fdemon-daemon/src/vm_service/client.rs`: Change channel type, wrap sends, update API
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-export new type

### Details

**Current state:** The event channel is `mpsc::channel<VmServiceEvent>` (capacity 256). `VmServiceEvent` is a struct representing raw `streamNotify` JSON-RPC notifications. No lifecycle events (reconnecting, reconnected, disconnected) are sent through this channel.

**Change:** Add a `VmClientEvent` enum that wraps `VmServiceEvent` and adds lifecycle variants. Change the channel type. Update all senders and receivers.

#### Step 1: Define VmClientEvent in protocol.rs

Add the enum **after** the existing `VmServiceEvent` struct definition (after line ~106):

```rust
/// Events emitted by the VM Service client through the event channel.
///
/// Wraps raw stream notifications (`VmServiceEvent`) with connection lifecycle
/// events so consumers can react to reconnection status changes.
#[derive(Debug)]
pub enum VmClientEvent {
    /// A stream notification from the VM Service (e.g., Extension, Logging, GC).
    StreamEvent(VmServiceEvent),
    /// The client is attempting to reconnect after a connection loss.
    Reconnecting {
        /// Current attempt number (1-based).
        attempt: u32,
        /// Maximum attempts before giving up.
        max_attempts: u32,
    },
    /// The client successfully reconnected after a connection loss.
    Reconnected,
    /// All reconnection attempts exhausted; the client has given up.
    PermanentlyDisconnected,
}
```

**Note:** `VmClientEvent` derives `Debug` but NOT `Deserialize` — it is constructed in Rust code, not parsed from JSON. `VmServiceEvent` continues to derive `Debug, Deserialize` as before.

#### Step 2: Update channel type in client.rs

1. **Import** `VmClientEvent` from `super::protocol` (add to the existing import at line 46):
   ```rust
   use super::protocol::{..., VmClientEvent, ...};
   ```

2. **Change channel creation** in `VmServiceClient::connect()` (line 345):
   ```rust
   // Before:
   let (event_tx, event_rx) = mpsc::channel::<VmServiceEvent>(EVENT_CHANNEL_CAPACITY);
   // After:
   let (event_tx, event_rx) = mpsc::channel::<VmClientEvent>(EVENT_CHANNEL_CAPACITY);
   ```

3. **Update `VmServiceClient` struct** field type (line 331):
   ```rust
   // Before:
   event_rx: mpsc::Receiver<VmServiceEvent>,
   // After:
   event_rx: mpsc::Receiver<VmClientEvent>,
   ```

4. **Update `event_receiver()` return type** (line 415):
   ```rust
   // Before:
   pub fn event_receiver(&mut self) -> &mut mpsc::Receiver<VmServiceEvent>
   // After:
   pub fn event_receiver(&mut self) -> &mut mpsc::Receiver<VmClientEvent>
   ```

5. **Update function signatures** for internal helpers that take `event_tx`:
   - `run_client_task` (line 586): `event_tx: mpsc::Sender<VmClientEvent>`
   - `run_io_loop` (line 687): `event_tx: &mpsc::Sender<VmClientEvent>`
   - `handle_ws_text` (line 795): `event_tx: &mpsc::Sender<VmClientEvent>`

6. **Wrap the event send** in `handle_ws_text` (lines 808-814):
   ```rust
   // Before:
   VmServiceMessage::Event(event) => {
       if let Err(err) = event_tx.try_send(event) {
           warn!("VM Service: event channel full or closed, dropping event: {}", err);
       }
   }
   // After:
   VmServiceMessage::Event(event) => {
       if let Err(err) = event_tx.try_send(VmClientEvent::StreamEvent(event)) {
           warn!("VM Service: event channel full or closed, dropping event: {}", err);
       }
   }
   ```

#### Step 3: Export MAX_RECONNECT_ATTEMPTS

In `client.rs`, change the visibility of `MAX_RECONNECT_ATTEMPTS` (line ~248) from `const` to `pub const`:

```rust
// Before:
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
// After:
pub const MAX_RECONNECT_ATTEMPTS: u32 = 10;
```

#### Step 4: Update re-exports in mod.rs

Add `VmClientEvent` and `MAX_RECONNECT_ATTEMPTS` to the re-exports (line 66 and 90):

```rust
// Line 66: add VmClientEvent to the client re-export line
pub use client::{ConnectionState, VmRequestHandle, VmServiceClient, MAX_RECONNECT_ATTEMPTS};

// Line 90: add VmClientEvent to the protocol re-export line
pub use protocol::{
    ..., VmClientEvent, VmServiceEvent, ...
};
```

**Note:** `MAX_RECONNECT_ATTEMPTS` is defined in `client.rs`, so re-export it from the `client` use line, not `protocol`.

### Acceptance Criteria

1. `VmClientEvent` enum is defined in `protocol.rs` with 4 variants: `StreamEvent`, `Reconnecting`, `Reconnected`, `PermanentlyDisconnected`
2. Event channel type is `mpsc::channel<VmClientEvent>` (not `VmServiceEvent`)
3. `event_receiver()` returns `&mut mpsc::Receiver<VmClientEvent>`
4. Stream events are wrapped in `VmClientEvent::StreamEvent()` before sending
5. `MAX_RECONNECT_ATTEMPTS` is `pub const` and re-exported from `vm_service` module
6. `VmClientEvent` is re-exported from `vm_service` module
7. `cargo check -p fdemon-daemon` passes
8. `cargo test -p fdemon-daemon` passes — all existing tests still pass

### Testing

No new tests in this task. This is a pure refactoring task — behavior is unchanged. The only events that flow through the channel are still `StreamEvent(VmServiceEvent)`. Lifecycle variants are defined but not sent yet (Task 05 adds emission).

Existing daemon tests should continue to pass because they test the VmServiceClient API, and the event_receiver() change is backward-compatible at the consumption site (recv() still returns an Option).

### Notes

- `VmServiceEvent` is NOT renamed or removed — it continues to exist as a struct inside `VmClientEvent::StreamEvent`
- The `actions.rs` consumer in `fdemon-app` will NOT compile after this change because it accesses `event.params.event` directly — Task 06 fixes that. Use `cargo check -p fdemon-daemon` (not workspace) for verification.
- `try_send` (non-blocking) continues to be used for stream events. Lifecycle events (Task 05) should use the same `try_send` for consistency — the channel capacity (256) is more than sufficient for the rare lifecycle events.
- No daemon tests reference `VmServiceEvent` by name, so the type change is transparent to tests.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/protocol.rs` | Added `VmClientEvent` enum with 4 variants after `VmServiceEvent` struct |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Changed `MAX_RECONNECT_ATTEMPTS` to `pub const`; updated import to add `VmClientEvent` and remove unused `VmServiceEvent`; changed `event_rx` field type; changed `event_tx`/`event_rx` channel type in `connect()`; updated `event_receiver()` return type; updated `run_client_task`, `run_io_loop`, `handle_ws_text` function signatures; wrapped event send with `VmClientEvent::StreamEvent()` |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `MAX_RECONNECT_ATTEMPTS` to client re-export; added `VmClientEvent` to protocol re-export |

### Notable Decisions/Tradeoffs

1. **Removed `VmServiceEvent` from client.rs import**: After wrapping event sends with `VmClientEvent::StreamEvent(event)`, `VmServiceEvent` is no longer used directly in `client.rs`. Removed it to avoid the unused import warning that would fail the `-D warnings` clippy check.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed (no warnings)
- `cargo test -p fdemon-daemon` - Passed (375 passed, 0 failed, 3 ignored)
- `cargo fmt --all --check` - Passed (no formatting issues)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **App crate broken**: As expected and documented in the task, `fdemon-app` (specifically `actions.rs`) still accesses `VmServiceEvent` fields directly and will not compile until Task 06 updates the consumer. Verified by using `-p fdemon-daemon` only as instructed.
