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

**Status:** Not Started
