## Task: Create VM Service WebSocket Client

**Objective**: Implement `VmServiceClient` — an async WebSocket client that connects to the Dart VM Service, sends JSON-RPC requests, receives responses/events, and handles reconnection.

**Depends on**: 01-websocket-deps, 03-vm-protocol

**Estimated Time**: 5-7 hours

### Scope

- **NEW** `crates/fdemon-daemon/src/vm_service/client.rs` — WebSocket client implementation
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Export client types

### Details

#### Architecture

The client runs as a background task with a channel-based API:

```
┌─────────────────────────────────────────────────────────────┐
│                     VmServiceClient                          │
│                                                              │
│  ┌──────────────┐        ┌──────────────────────────────┐   │
│  │   Public API │        │   Background Task             │   │
│  │              │        │                                │   │
│  │  send_req()──┼──cmd──▶│  WebSocket read/write loop    │   │
│  │              │  chan   │                                │   │
│  │  events() ◀──┼──evt──◀│  Route: response → tracker    │   │
│  │              │  chan   │         event → event channel  │   │
│  └──────────────┘        └──────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  VmRequestTracker (from protocol.rs)                  │   │
│  │  Correlates request IDs with response receivers       │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

#### VmServiceClient API

```rust
pub struct VmServiceClient {
    cmd_tx: mpsc::Sender<ClientCommand>,
    event_rx: mpsc::Receiver<VmServiceEvent>,
    state: Arc<RwLock<ConnectionState>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
}

enum ClientCommand {
    SendRequest {
        method: String,
        params: Option<serde_json::Value>,
        response_tx: oneshot::Sender<Result<serde_json::Value, VmServiceError>>,
    },
    Disconnect,
}

impl VmServiceClient {
    /// Connect to VM Service at the given WebSocket URI.
    /// Spawns a background task for WebSocket I/O.
    pub async fn connect(ws_uri: &str) -> Result<Self> { ... }

    /// Send a JSON-RPC request and wait for the response.
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> { ... }

    /// Get a receiver for stream events (Extension, Logging, etc.)
    pub fn event_receiver(&mut self) -> &mut mpsc::Receiver<VmServiceEvent> { ... }

    /// Get current connection state
    pub fn connection_state(&self) -> ConnectionState { ... }

    /// Gracefully disconnect
    pub async fn disconnect(&self) { ... }

    /// Check if connected
    pub fn is_connected(&self) -> bool { ... }
}
```

#### Background Task

The background task:
1. Connects to WebSocket using `tokio-tungstenite`
2. Splits into read/write halves
3. Read loop: parses messages via `parse_vm_message()`, routes responses to tracker, events to channel
4. Write loop: receives commands from `cmd_tx`, serializes and sends
5. On disconnect: attempts reconnection with exponential backoff (1s, 2s, 4s, max 30s)
6. On shutdown: closes WebSocket gracefully

```rust
async fn run_client_task(
    ws_uri: String,
    cmd_rx: mpsc::Receiver<ClientCommand>,
    event_tx: mpsc::Sender<VmServiceEvent>,
    state: Arc<RwLock<ConnectionState>>,
) {
    // 1. Connect to WebSocket
    // 2. Split into read/write
    // 3. Select on: cmd_rx (send requests), ws_read (receive messages)
    // 4. On ws message: parse → route to tracker or event channel
    // 5. On disconnect: reconnect with backoff
    // 6. On Disconnect command: close gracefully
}
```

#### Reconnection Strategy

```rust
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
```

On unexpected disconnect:
1. Set state to `Reconnecting { attempt: 1 }`
2. Wait `INITIAL_BACKOFF * 2^attempt` (capped at `MAX_BACKOFF`)
3. Attempt reconnection
4. If successful: re-subscribe to streams, reset attempt counter
5. If failed: increment attempt, retry up to `MAX_RECONNECT_ATTEMPTS`
6. After max attempts: set state to `Disconnected`, log error

### Acceptance Criteria

1. `VmServiceClient::connect(ws_uri)` establishes WebSocket connection
2. `request()` sends JSON-RPC and returns parsed response
3. Stream events (Extension, Logging) arrive via `event_receiver()`
4. `disconnect()` closes WebSocket gracefully
5. Connection state is tracked and queryable
6. Reconnection with exponential backoff works on unexpected disconnect
7. All errors use the project's `Error` enum (no panics)
8. Background task cleans up on drop

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_connection_state_transitions() {
        // Test Disconnected → Connecting → Connected → Disconnected
    }

    #[test]
    fn test_reconnection_backoff_calculation() {
        // Test exponential backoff: 1s, 2s, 4s, 8s, ..., 30s cap
    }

    #[test]
    fn test_client_command_serialization() {
        // Test that ClientCommand produces valid JSON-RPC
    }

    // Integration tests would need a mock WebSocket server
    // Defer those to Task 08 (session integration) or Phase 5
}
```

### Notes

- Use `tokio::select!` for multiplexing read/write in the background task
- The `futures-util` crate provides `SplitSink`/`SplitStream` for WebSocket halves
- Channel buffer sizes: `cmd_tx` = 32 (bounded), `event_tx` = 256 (bounded, events can be bursty)
- The client does NOT handle stream subscriptions — that's Task 05's responsibility
- Keep error messages user-friendly (will show in logs if connection fails)
- Consider using `tokio_tungstenite::connect_async` for the initial connection

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/client.rs` | **NEW** — Full VmServiceClient implementation with ConnectionState, ClientCommand, background task, reconnection logic, and 14 unit tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `pub mod client` and re-exported `ConnectionState` and `VmServiceClient` |

### Notable Decisions/Tradeoffs

1. **`blocking_read()` for synchronous state accessors**: `connection_state()` and `is_connected()` use `state.blocking_read()` so they can be `fn` (not `async fn`). This matches the task's API spec and avoids forcing callers to await simple queries. The lock is only held momentarily by the background task on state transitions, so contention risk is negligible.

2. **Initial connection before spawning background task**: `VmServiceClient::connect()` establishes the first WebSocket connection before returning, rather than connecting in the background. This makes the API ergonomic — callers get an immediate error if the URI is unreachable — and avoids a race where `request()` is called before the socket is open.

3. **`tokio::spawn` for response forwarding**: After `ws_sink.send()` succeeds, a small Tokio task is spawned to await the oneshot receiver and forward the response to the caller. This avoids blocking the I/O loop while waiting for a response, which could deadlock (the I/O loop also needs to run to _receive_ the response).

4. **`try_send` for events**: Stream events are forwarded with `try_send` (non-blocking). If the buffer is full a warning is logged and the event is dropped. Blocking on the I/O loop for event delivery could cause backpressure that stalls request/response processing.

5. **`VmServiceError` unused import suppressed**: The `VmServiceError` type is used in `vm_error_to_error` and in test bodies, so it's a genuine import not a dead one. Clippy accepted this with no warnings.

6. **Backoff uses `checked_shl`**: `u64::saturating_shl` doesn't exist in stable Rust. Used `checked_shl` which returns `None` on overflow (shift count >= 64), falling back to `u64::MAX` before capping at `MAX_BACKOFF.as_secs()`.

### Testing Performed

- `cargo check --workspace` — Passed (0 errors, 0 warnings)
- `cargo test -p fdemon-daemon` — Passed (174 tests: 14 new client tests + 160 pre-existing; 3 ignored integration tests)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)
- `cargo fmt --all` — Applied minor line-wrap adjustments, output is clean

### Risks/Limitations

1. **No integration test with real WebSocket server**: The acceptance criteria and task notes defer mock-server tests to Task 08. All async behavior (reconnection, request/response correlation) is tested indirectly through unit tests of pure functions (backoff calculation, response conversion).

2. **Pending-request orphaning on reconnect**: When the connection drops unexpectedly, in-flight requests registered in `VmRequestTracker` are orphaned — their oneshot senders remain in the tracker but the WebSocket socket that would have delivered the responses is gone. Callers will eventually see `ChannelClosed` when the tracker is dropped on reconnect. This is acceptable for the current scope; Task 05 (stream subscriptions) can add explicit cleanup on reconnect.

3. **`blocking_read()` in async context**: Tokio's documentation warns that `blocking_read()` can deadlock if the write lock is held by the same async task. In practice this can't happen here because the write lock is only acquired by the background Tokio task, never by the same task that calls `connection_state()` or `is_connected()`. Safe in the current design.
