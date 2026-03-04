## Task: Implement TCP Server and Client Session State Machine

**Objective**: Build the DAP TCP server that accepts client connections and manages per-client session state machines. Each session handles the DAP initialization handshake (initialize → configurationDone → attach). This is the core networking and protocol layer of the DAP server.

**Depends on**: 01 (needs protocol types and codec from fdemon-dap)

### Scope

- `crates/fdemon-dap/src/server/mod.rs` — **NEW**: TCP listener, connection acceptance, shutdown
- `crates/fdemon-dap/src/server/session.rs` — **NEW**: Per-client session state machine
- `crates/fdemon-dap/src/lib.rs` — Add `pub mod server;`

### Details

#### 1. Server Module (`server/mod.rs`)

The DAP server listens on a TCP port, accepts connections, and spawns a task per client:

```rust
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};

/// Handle returned when the DAP server starts successfully.
/// Used by the Engine to track and stop the server.
pub struct DapServerHandle {
    /// The actual port the server is listening on (may differ from requested if port=0).
    pub port: u16,
    /// Send `true` to shut down the server and all client sessions.
    pub shutdown_tx: watch::Sender<bool>,
    /// Join handle for the server task.
    pub task: tokio::task::JoinHandle<()>,
}

/// Configuration for starting a DAP server.
pub struct DapServerConfig {
    /// Port to listen on (0 = auto-assign).
    pub port: u16,
    /// Bind address (e.g., "127.0.0.1").
    pub bind_addr: String,
    /// Channel to send Messages back to the Engine (DapClientConnected, etc.).
    pub msg_tx: mpsc::Sender<crate::Message>,
}
```

Note: `crate::Message` here refers to a re-exported type. Since `fdemon-dap` does NOT depend on `fdemon-app`, the `msg_tx` channel type needs to be generic or use a trait. See the "Cross-crate messaging" note below.

**Cross-crate messaging pattern:**

Since `fdemon-dap` cannot depend on `fdemon-app` (that would create a circular dependency), the server uses a callback trait or generic message sender:

```rust
/// Trait for sending DAP lifecycle events back to the host application.
/// Implemented by the Engine integration layer (Task 05).
pub trait DapEventSink: Send + Sync + 'static {
    /// Called when a client connects.
    fn on_client_connected(&self, client_id: String);
    /// Called when a client disconnects.
    fn on_client_disconnected(&self, client_id: String);
    /// Called when the server encounters a fatal error.
    fn on_server_error(&self, error: String);
}
```

Alternatively, use `mpsc::Sender<DapServerEvent>` where `DapServerEvent` is defined in `fdemon-dap`:

```rust
/// Events emitted by the DAP server to its host.
#[derive(Debug, Clone)]
pub enum DapServerEvent {
    ClientConnected { client_id: String },
    ClientDisconnected { client_id: String },
    ServerError { reason: String },
}
```

The Engine integration (Task 05) maps `DapServerEvent` → `Message` variants.

**Server startup:**

```rust
/// Start the DAP TCP server.
///
/// Binds to the configured address/port, then spawns a background task
/// that accepts connections until shutdown is signaled.
///
/// Returns a handle for lifecycle management, or an error if binding fails.
pub async fn start(
    config: DapServerConfig,
    event_tx: mpsc::Sender<DapServerEvent>,
) -> Result<DapServerHandle> {
    let addr = format!("{}:{}", config.bind_addr, config.port);
    let listener = TcpListener::bind(&addr).await?;
    let actual_port = listener.local_addr()?.port();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let task = tokio::spawn(accept_loop(listener, shutdown_rx, event_tx));

    Ok(DapServerHandle {
        port: actual_port,
        shutdown_tx,
        task,
    })
}

async fn accept_loop(
    listener: TcpListener,
    mut shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<DapServerEvent>,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let client_id = addr.to_string();
                        let _ = event_tx.send(DapServerEvent::ClientConnected {
                            client_id: client_id.clone(),
                        }).await;

                        let shutdown = shutdown_rx.clone();
                        let tx = event_tx.clone();
                        tokio::spawn(async move {
                            let result = DapClientSession::run(stream, shutdown).await;
                            if let Err(e) = &result {
                                tracing::warn!("DAP client session error: {}", e);
                            }
                            let _ = tx.send(DapServerEvent::ClientDisconnected {
                                client_id,
                            }).await;
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept DAP connection: {}", e);
                    }
                }
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!("DAP server shutting down");
                    break;
                }
            }
        }
    }
}
```

**Graceful shutdown:**

When `shutdown_tx.send(true)` is called:
1. The accept loop breaks
2. Each client session also watches `shutdown_rx` and terminates
3. The `DapServerHandle.task` completes

#### 2. Client Session State Machine (`server/session.rs`)

Each connected client goes through a state machine:

```
Uninitialized → Initializing → Configured → Disconnecting
                                    ↓
                              (attach/disconnect)
```

For Phase 2, the session handles only the initialization handshake — no debugging features.

```rust
/// State of a DAP client session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Client connected but has not sent `initialize`.
    Uninitialized,
    /// `initialize` received, waiting for `configurationDone`.
    Initializing,
    /// `configurationDone` received, ready for `attach` or debugging.
    Configured,
    /// Client is disconnecting.
    Disconnecting,
}

/// Manages a single DAP client connection.
pub struct DapClientSession {
    state: SessionState,
    /// Monotonic sequence number for server-sent messages.
    next_seq: i64,
    /// Client's reported capabilities (from initialize request).
    client_info: Option<InitializeRequestArguments>,
}

impl DapClientSession {
    /// Run the session until the client disconnects or shutdown is signaled.
    pub async fn run(
        stream: tokio::net::TcpStream,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let (reader, writer) = stream.into_split();
        let mut reader = tokio::io::BufReader::new(reader);
        let mut writer = writer;
        let mut session = Self::new();

        loop {
            tokio::select! {
                result = read_message(&mut reader) => {
                    match result {
                        Ok(Some(DapMessage::Request(req))) => {
                            let responses = session.handle_request(&req);
                            for msg in responses {
                                write_message(&mut writer, &msg).await?;
                            }
                            if session.state == SessionState::Disconnecting {
                                break;
                            }
                        }
                        Ok(Some(_)) => {
                            // Ignore non-request messages from client
                            tracing::debug!("Ignoring non-request DAP message from client");
                        }
                        Ok(None) => {
                            // Clean EOF — client disconnected
                            break;
                        }
                        Err(e) => {
                            tracing::warn!("Error reading DAP message: {}", e);
                            break;
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        // Server shutting down — send terminated event
                        let event = DapEvent::terminated();
                        let msg = DapMessage::Event(session.assign_seq(event));
                        let _ = write_message(&mut writer, &msg).await;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn new() -> Self {
        Self {
            state: SessionState::Uninitialized,
            next_seq: 1,
            client_info: None,
        }
    }

    /// Assign a monotonic seq number to an outgoing event.
    fn assign_seq(&mut self, mut event: DapEvent) -> DapEvent {
        event.seq = self.next_seq;
        self.next_seq += 1;
        event
    }

    /// Handle an incoming request and return response(s) + events to send.
    fn handle_request(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        match request.command.as_str() {
            "initialize" => self.handle_initialize(request),
            "configurationDone" => self.handle_configuration_done(request),
            "attach" => self.handle_attach(request),
            "disconnect" => self.handle_disconnect(request),
            _ => self.handle_unknown(request),
        }
    }
}
```

**Initialize handler:**

```rust
fn handle_initialize(&mut self, request: &DapRequest) -> Vec<DapMessage> {
    if self.state != SessionState::Uninitialized {
        return vec![DapMessage::Response(
            DapResponse::error(request, "initialize already called"),
        )];
    }

    // Parse client arguments
    if let Some(args) = &request.arguments {
        self.client_info = serde_json::from_value(args.clone()).ok();
    }

    self.state = SessionState::Initializing;

    // Build capabilities response
    let capabilities = Capabilities::fdemon_defaults();
    let body = serde_json::to_value(&capabilities).unwrap_or_default();
    let response = DapResponse::success(request, Some(body));

    // Send response + initialized event
    let mut initialized = DapEvent::initialized();
    initialized.seq = self.next_seq;
    self.next_seq += 1;

    vec![
        DapMessage::Response(response),
        DapMessage::Event(initialized),
    ]
}
```

**ConfigurationDone handler:**

```rust
fn handle_configuration_done(&mut self, request: &DapRequest) -> Vec<DapMessage> {
    if self.state != SessionState::Initializing {
        return vec![DapMessage::Response(
            DapResponse::error(request, "unexpected configurationDone"),
        )];
    }
    self.state = SessionState::Configured;
    vec![DapMessage::Response(DapResponse::success(request, None))]
}
```

**Attach handler (stub for Phase 2):**

```rust
fn handle_attach(&mut self, request: &DapRequest) -> Vec<DapMessage> {
    if self.state != SessionState::Configured {
        return vec![DapMessage::Response(
            DapResponse::error(request, "must call configurationDone before attach"),
        )];
    }
    // Phase 2: Accept attach but don't connect to VM Service yet
    // Phase 3 will wire this to VmRequestHandle
    tracing::info!("DAP client attached (debugging not yet implemented)");
    vec![DapMessage::Response(DapResponse::success(request, None))]
}
```

**Disconnect handler:**

```rust
fn handle_disconnect(&mut self, request: &DapRequest) -> Vec<DapMessage> {
    self.state = SessionState::Disconnecting;
    vec![DapMessage::Response(DapResponse::success(request, None))]
}
```

**Unknown command handler:**

```rust
fn handle_unknown(&mut self, request: &DapRequest) -> Vec<DapMessage> {
    tracing::debug!("Unhandled DAP command: {}", request.command);
    vec![DapMessage::Response(
        DapResponse::error(request, format!("unsupported command: {}", request.command)),
    )]
}
```

#### 3. Module Structure

```
crates/fdemon-dap/src/
├── lib.rs              # pub mod protocol; pub mod server;
├── protocol/
│   ├── mod.rs
│   ├── types.rs
│   └── codec.rs
└── server/
    ├── mod.rs           # DapServerHandle, DapServerConfig, DapServerEvent, start()
    └── session.rs       # DapClientSession, SessionState
```

### Acceptance Criteria

1. `DapServerHandle` contains `port: u16`, `shutdown_tx`, and `task: JoinHandle`
2. `start()` binds to the configured address/port and returns the actual port (handles port=0)
3. `start()` returns an error if the port is already in use
4. The accept loop spawns a new task per client connection
5. The accept loop terminates when `shutdown_tx.send(true)` is called
6. Client sessions handle `initialize` → respond with Capabilities + `initialized` event
7. Client sessions handle `configurationDone` → respond with success
8. Client sessions handle `attach` → respond with success (stub)
9. Client sessions handle `disconnect` → respond with success, session terminates
10. Client sessions reject out-of-order requests (e.g., `configurationDone` before `initialize`)
11. Client sessions handle unknown commands with error response (not crash)
12. `DapServerEvent::ClientConnected` and `ClientDisconnected` are emitted correctly
13. Seq numbers on server-sent messages are monotonically increasing
14. Client sessions terminate cleanly on shutdown signal (send `terminated` event)
15. Client sessions terminate cleanly on client TCP disconnect (EOF)
16. `cargo check -p fdemon-dap` passes
17. `cargo test -p fdemon-dap` passes
18. `cargo clippy -p fdemon-dap -- -D warnings` clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === Session State Machine Tests ===

    #[test]
    fn test_session_starts_uninitialized() {
        let session = DapClientSession::new();
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    #[test]
    fn test_initialize_transitions_to_initializing() {
        let mut session = DapClientSession::new();
        let req = DapRequest { seq: 1, command: "initialize".into(), arguments: None };
        let responses = session.handle_request(&req);
        assert_eq!(session.state, SessionState::Initializing);
        assert_eq!(responses.len(), 2); // response + initialized event
    }

    #[test]
    fn test_double_initialize_returns_error() {
        let mut session = DapClientSession::new();
        let req = DapRequest { seq: 1, command: "initialize".into(), arguments: None };
        session.handle_request(&req);
        let req2 = DapRequest { seq: 2, command: "initialize".into(), arguments: None };
        let responses = session.handle_request(&req2);
        assert!(!matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_configuration_done_after_initialize() {
        let mut session = DapClientSession::new();
        session.handle_request(&DapRequest { seq: 1, command: "initialize".into(), arguments: None });
        let req = DapRequest { seq: 2, command: "configurationDone".into(), arguments: None };
        let responses = session.handle_request(&req);
        assert_eq!(session.state, SessionState::Configured);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_configuration_done_before_initialize_returns_error() {
        let mut session = DapClientSession::new();
        let req = DapRequest { seq: 1, command: "configurationDone".into(), arguments: None };
        let responses = session.handle_request(&req);
        assert!(!matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_disconnect_transitions_to_disconnecting() {
        let mut session = DapClientSession::new();
        session.handle_request(&DapRequest { seq: 1, command: "initialize".into(), arguments: None });
        let req = DapRequest { seq: 2, command: "disconnect".into(), arguments: None };
        session.handle_request(&req);
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    #[test]
    fn test_unknown_command_returns_error_response() {
        let mut session = DapClientSession::new();
        session.handle_request(&DapRequest { seq: 1, command: "initialize".into(), arguments: None });
        let req = DapRequest { seq: 2, command: "flyToMoon".into(), arguments: None };
        let responses = session.handle_request(&req);
        assert!(!matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_seq_numbers_are_monotonic() {
        let mut session = DapClientSession::new();
        let req = DapRequest { seq: 1, command: "initialize".into(), arguments: None };
        let responses = session.handle_request(&req);
        // initialized event should have seq=1
        if let DapMessage::Event(e) = &responses[1] {
            assert_eq!(e.seq, 1);
        }
    }

    // === TCP Server Tests (async) ===

    #[tokio::test]
    async fn test_server_binds_to_port_zero() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let config = DapServerConfig {
            port: 0,
            bind_addr: "127.0.0.1".to_string(),
            msg_tx: event_tx,
        };
        let handle = start(config, /* ... */).await.unwrap();
        assert!(handle.port > 0);
        handle.shutdown_tx.send(true).unwrap();
    }

    #[tokio::test]
    async fn test_server_shutdown_stops_accept_loop() {
        // Start server, send shutdown, verify task completes
    }

    #[tokio::test]
    async fn test_client_connect_and_initialize() {
        // Start server, connect TCP client, send initialize request,
        // verify response contains capabilities
    }
}
```

### Notes

- The `DapEventSink` trait / `DapServerEvent` enum pattern decouples `fdemon-dap` from `fdemon-app`. The Engine integration (Task 05) bridges the gap by mapping `DapServerEvent` → `Message` variants.
- Phase 2 handles only the handshake. The `attach` handler is a stub that responds with success but doesn't connect to any VM Service. Phase 3 will add the `DapAdapter` that bridges DAP requests to VM Service RPCs.
- The session state machine is intentionally simple for Phase 2 (4 states). Phase 3 will add an `Attached`/`Debugging` state for active debugging.
- Client sessions are fully independent — they share no mutable state. Each has its own seq counter and state. Phase 4 may add shared state for multi-client coordination.
- The server binds to `127.0.0.1` by default for security. Binding to `0.0.0.0` (all interfaces) is supported but should be clearly documented as a security risk.
- TCP keep-alive and connection timeouts are deferred to Phase 4 (production hardening).
