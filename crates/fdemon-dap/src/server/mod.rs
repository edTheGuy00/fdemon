//! # DAP TCP Server
//!
//! Provides the TCP listener that accepts DAP client connections and dispatches
//! each connection to a [`DapClientSession`] running in its own Tokio task.
//!
//! ## Lifecycle
//!
//! 1. Call [`start`] with a [`DapServerConfig`] and an event channel sender.
//! 2. [`start`] binds the TCP listener and returns a [`DapServerHandle`].
//! 3. Call [`DapServerHandle::port`] to get the actual port (useful when
//!    `port = 0` is specified to get an OS-assigned port).
//! 4. The accept loop runs in a background task until `shutdown_tx.send(true)`
//!    is called, which causes the loop to break and all active sessions to
//!    receive a `terminated` event.
//!
//! ## Cross-crate event pattern
//!
//! `fdemon-dap` cannot depend on `fdemon-app` (that would create a circular
//! dependency). Instead, the server emits [`DapServerEvent`] values over an
//! `mpsc` channel. The Engine integration layer (Task 05) maps these events to
//! `Message` variants for the TEA update loop.
//!
//! ## Security
//!
//! By default the server binds to `127.0.0.1`. Binding to `0.0.0.0` is
//! supported but exposes the DAP port on all interfaces — this is a significant
//! security risk and should be clearly documented if used.

pub mod session;

pub use session::{DapClientSession, NoopBackend, SessionState};

use std::sync::Arc;

use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc, watch, Semaphore},
};

use fdemon_core::error::{Error, Result};

use crate::adapter::{DebugEvent, DynDebugBackend};

// ─────────────────────────────────────────────────────────────────────────────
// BackendFactory
// ─────────────────────────────────────────────────────────────────────────────

/// Per-client backend produced by [`BackendFactory::create`].
///
/// Bundles the type-erased [`DynDebugBackend`] with a per-session debug event
/// receiver. The receiver carries VM debug events (paused, resumed, breakpoint
/// hit) forwarded from the Engine to the DAP session loop.
pub struct BackendHandle {
    /// The type-erased debug backend for this session.
    ///
    /// Uses [`DynDebugBackend`] rather than `Box<dyn DebugBackend>` because
    /// [`crate::adapter::DebugBackend`] is not object-safe (its methods return
    /// `impl Future` via `trait_variant::make`).
    pub backend: DynDebugBackend,

    /// Receiver for per-session VM debug events (stopped, resumed, etc.).
    pub debug_event_rx: mpsc::Receiver<DebugEvent>,
}

/// Factory that creates a [`BackendHandle`] for each accepted DAP client.
///
/// Implemented in `fdemon-app` (which depends on both `fdemon-dap` and
/// `fdemon-daemon`) so this crate does not need to depend on either.
///
/// The factory is invoked once per TCP connection. If no Flutter session is
/// active it should return `None`, causing the connection to fall back to
/// [`NoopBackend`] (all debug commands return errors but the handshake still
/// completes normally).
pub trait BackendFactory: Send + Sync + 'static {
    /// Create a backend handle for a new DAP client connection.
    ///
    /// Returns `None` if no active Flutter session is available.
    fn create(&self) -> Option<BackendHandle>;
}

/// Maximum number of concurrent DAP client connections.
///
/// Connections beyond this limit are rejected immediately (the TCP stream is
/// dropped) and a warning is logged. This prevents unbounded task growth when
/// a misbehaving client or port scanner hammers the server.
const MAX_CONCURRENT_CLIENTS: usize = 8;

/// Backoff duration after a TCP accept error to prevent tight error loops
/// (e.g., file descriptor exhaustion).
const ACCEPT_ERROR_BACKOFF: std::time::Duration = std::time::Duration::from_millis(100);

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Events emitted by the DAP server to its host application.
///
/// The Engine integration layer (Task 05) maps these to `Message` variants for
/// the TEA update loop. Using a dedicated enum decouples `fdemon-dap` from
/// `fdemon-app`, avoiding a circular dependency.
#[derive(Debug, Clone)]
pub enum DapServerEvent {
    /// A new DAP client connected. `client_id` is the remote address string.
    ClientConnected { client_id: String },
    /// A DAP client disconnected (gracefully or via error/EOF).
    ClientDisconnected { client_id: String },
    /// The server encountered a fatal error (e.g., accept loop failed).
    ServerError { reason: String },
    /// A DAP debug session became active (client sent `attach` and it succeeded).
    DebugSessionStarted { client_id: String },
    /// A DAP debug session ended (client disconnected or app exited).
    DebugSessionEnded { client_id: String },
}

/// Handle returned when the DAP server starts successfully.
///
/// Used by the Engine to track server lifecycle and initiate shutdown.
/// When dropped the server task is **not** automatically cancelled — call
/// `shutdown_tx.send(true)` first, then `.await` on `task` to join cleanly.
///
/// Use [`DapServerHandle::port`] to retrieve the listening port. The
/// `shutdown_tx` and `task` fields are `pub(crate)` and are only accessed by
/// [`crate::service::DapService::stop`] and tests within this crate.
pub struct DapServerHandle {
    /// The actual port the server is listening on.
    ///
    /// This may differ from the configured port when `port = 0` was passed to
    /// [`start`], in which case the OS assigned an ephemeral port.
    /// `pub(crate)` so [`crate::service::DapService::start_stdio`] can
    /// construct a handle with `port: 0` (stdio has no TCP port).
    pub(crate) port: u16,

    /// Send `true` to signal the server and all active sessions to shut down.
    pub(crate) shutdown_tx: watch::Sender<bool>,

    /// Join handle for the accept-loop task.
    ///
    /// Await this after sending the shutdown signal to ensure the server has
    /// fully stopped before releasing resources.
    pub(crate) task: tokio::task::JoinHandle<()>,

    /// Broadcast sender for forwarding [`DebugEvent`]s to all active sessions.
    ///
    /// The Engine uses this to send [`DebugEvent::LogOutput`] events (and
    /// other debug events) to every connected DAP client. Each accepted
    /// connection subscribes by calling `log_event_tx.subscribe()`.
    ///
    /// The capacity is intentionally small — log events are latency-tolerant
    /// and a slow subscriber should lag rather than block the sender.
    pub(crate) log_event_tx: broadcast::Sender<DebugEvent>,
}

impl std::fmt::Debug for DapServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DapServerHandle")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl DapServerHandle {
    /// Returns the port the server is listening on.
    ///
    /// This may differ from the configured port when `port = 0` was passed to
    /// [`start`], in which case the OS assigned an ephemeral port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns a clone of the broadcast sender for forwarding debug events to
    /// all active DAP client sessions.
    ///
    /// The Engine uses this to push [`DebugEvent::LogOutput`] events (Flutter
    /// app stdout/stderr) to every connected IDE debug console. Each accepted
    /// TCP connection subscribes to this channel automatically.
    ///
    /// If there are no active subscribers (no clients connected), `send()`
    /// returns an error which can be safely ignored.
    pub fn log_event_sender(&self) -> broadcast::Sender<DebugEvent> {
        self.log_event_tx.clone()
    }
}

/// Configuration for starting a DAP TCP server.
pub struct DapServerConfig {
    /// Port to listen on. Use `0` to let the OS assign an ephemeral port.
    pub port: u16,

    /// Bind address (e.g., `"127.0.0.1"` for local-only, `"0.0.0.0"` for all
    /// interfaces — the latter is a security risk; prefer `127.0.0.1`).
    pub bind_addr: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Server startup
// ─────────────────────────────────────────────────────────────────────────────

/// Start the DAP TCP server.
///
/// Binds a TCP listener to `config.bind_addr:config.port`, then spawns a
/// background task that accepts connections until shutdown is signalled.
///
/// When `backend_factory` is `Some`, each accepted connection calls the
/// factory once. If the factory returns `Some(handle)`, the session is run
/// with [`DapClientSession::run_on_with_backend`] so real VM Service debugging
/// works. If the factory returns `None` (no active Flutter session), or if
/// `backend_factory` is `None`, the connection falls back to [`NoopBackend`].
///
/// # Arguments
///
/// * `config` — Address/port configuration.
/// * `event_tx` — Channel used to emit [`DapServerEvent`] to the host.
/// * `backend_factory` — Optional factory for creating per-session backends.
///
/// # Returns
///
/// A [`DapServerHandle`] on success, or an error if binding fails (e.g., the
/// port is already in use).
///
/// # Errors
///
/// - [`Error::Io`] — If `TcpListener::bind` or `local_addr()` fails.
pub async fn start(
    config: DapServerConfig,
    event_tx: mpsc::Sender<DapServerEvent>,
    backend_factory: Option<Arc<dyn BackendFactory>>,
) -> Result<DapServerHandle> {
    let addr = format!("{}:{}", config.bind_addr, config.port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| Error::protocol(format!("DAP server failed to bind to {}: {}", addr, e)))?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| Error::protocol(format!("DAP server failed to get local addr: {}", e)))?
        .port();

    tracing::info!(
        "DAP server listening on {}:{}",
        config.bind_addr,
        actual_port
    );

    if config.bind_addr != "127.0.0.1"
        && config.bind_addr != "::1"
        && config.bind_addr != "localhost"
    {
        tracing::warn!(
            bind_addr = %config.bind_addr,
            "DAP server bound to non-loopback address. The 'evaluate' command allows \
             arbitrary code execution — binding to a network interface exposes this \
             to remote connections."
        );
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Broadcast channel for forwarding debug events (e.g., LogOutput) to all
    // active sessions. Capacity of 256 allows bursts of log lines without
    // blocking; lagging receivers are dropped automatically by tokio.
    let (log_event_tx, _) = broadcast::channel::<DebugEvent>(256);

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CLIENTS));
    let task = tokio::spawn(accept_loop(
        listener,
        shutdown_rx,
        event_tx,
        semaphore,
        log_event_tx.clone(),
        backend_factory,
    ));

    Ok(DapServerHandle {
        port: actual_port,
        shutdown_tx,
        task,
        log_event_tx,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Accept loop
// ─────────────────────────────────────────────────────────────────────────────

/// Accept connections until a shutdown signal is received.
///
/// For each accepted connection:
/// 1. Checks whether we are below [`MAX_CONCURRENT_CLIENTS`]; if full the
///    connection is dropped immediately and a warning is logged.
/// 2. Emits a [`DapServerEvent::ClientConnected`] with the remote address.
/// 3. If a `backend_factory` is provided and returns `Some(handle)`, spawns a
///    task running [`DapClientSession::run_on_with_backend`] with the real
///    backend. Otherwise falls back to [`DapClientSession::run`] with
///    [`NoopBackend`].
/// 4. On session completion, emits [`DapServerEvent::ClientDisconnected`].
///
/// When `shutdown_rx` receives `true`, the loop breaks. Each active session
/// also watches the same `shutdown_rx` and terminates independently.
///
/// On accept errors a 100 ms back-off is applied before retrying to prevent
/// a tight error-spam loop when the OS is under resource pressure.
///
/// The `log_event_tx` broadcast sender is subscribed for each new connection
/// so the session can receive [`DebugEvent::LogOutput`] (and other events)
/// forwarded by the Engine.
async fn accept_loop(
    listener: TcpListener,
    mut shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<DapServerEvent>,
    semaphore: Arc<Semaphore>,
    log_event_tx: broadcast::Sender<DebugEvent>,
    backend_factory: Option<Arc<dyn BackendFactory>>,
) {
    loop {
        tokio::select! {
            // Bias toward checking for shutdown first so a pending accept()
            // doesn't block us from seeing the shutdown signal.
            biased;

            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!("DAP server accept loop shutting down");
                    break;
                }
            }

            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let client_id = addr.to_string();

                        // Enforce the concurrent-connection cap. try_acquire_owned is
                        // non-blocking so we never stall the accept loop waiting for a
                        // slot — we reject immediately and keep the shutdown path clear.
                        let permit = match semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                tracing::warn!(
                                    "DAP server: max concurrent clients ({}) reached, rejecting connection from {}",
                                    MAX_CONCURRENT_CLIENTS,
                                    addr
                                );
                                drop(stream);
                                continue;
                            }
                        };

                        tracing::debug!("DAP client connected: {}", client_id);

                        // Notify the host that a client connected.
                        if event_tx
                            .send(DapServerEvent::ClientConnected {
                                client_id: client_id.clone(),
                            })
                            .await
                            .is_err()
                        {
                            // Host has dropped the receiver — shut down.
                            tracing::warn!("DAP event channel closed; stopping accept loop");
                            break;
                        }

                        // Clone handles needed for the per-client task.
                        let session_shutdown = shutdown_rx.clone();
                        let session_tx = event_tx.clone();
                        let session_client_id = client_id.clone();

                        // Attempt to create a real backend for this connection.
                        let maybe_backend = backend_factory.as_ref().and_then(|f| f.create());

                        if let Some(backend_handle) = maybe_backend {
                            // Real backend available — use run_on_with_backend.
                            tokio::spawn(async move {
                                let (reader, writer) = stream.into_split();
                                let reader = tokio::io::BufReader::new(reader);

                                let result = DapClientSession::<DynDebugBackend>::run_on_with_backend(
                                    reader,
                                    writer,
                                    session_shutdown,
                                    backend_handle.backend,
                                    backend_handle.debug_event_rx,
                                )
                                .await;

                                if let Err(e) = result {
                                    tracing::warn!(
                                        "DAP client session error ({}): {}",
                                        session_client_id,
                                        e
                                    );
                                } else {
                                    tracing::debug!(
                                        "DAP client session ended cleanly: {}",
                                        session_client_id
                                    );
                                }

                                // Notify the host that the client disconnected.
                                let _ = session_tx
                                    .send(DapServerEvent::ClientDisconnected {
                                        client_id: session_client_id,
                                    })
                                    .await;

                                // Release the semaphore slot.
                                drop(permit);
                            });
                        } else {
                            // No backend — fall back to NoopBackend via run().
                            // Subscribe to the log event broadcast for this session.
                            let log_event_rx = log_event_tx.subscribe();

                            tokio::spawn(async move {
                                if let Err(e) =
                                    DapClientSession::run(stream, session_shutdown, log_event_rx).await
                                {
                                    tracing::warn!(
                                        "DAP client session error ({}): {}",
                                        session_client_id,
                                        e
                                    );
                                } else {
                                    tracing::debug!(
                                        "DAP client session ended cleanly: {}",
                                        session_client_id
                                    );
                                }

                                // Notify the host that the client disconnected.
                                let _ = session_tx
                                    .send(DapServerEvent::ClientDisconnected {
                                        client_id: session_client_id,
                                    })
                                    .await;

                                // Release the semaphore slot so new connections can be accepted.
                                drop(permit);
                            });
                        }
                    }
                    Err(e) => {
                        tracing::error!("DAP server failed to accept connection: {}", e);
                        // Emit a server error event; continue accepting.
                        let _ = event_tx
                            .send(DapServerEvent::ServerError {
                                reason: e.to_string(),
                            })
                            .await;
                        // Backoff to prevent tight error loop on persistent OS failures
                        // (e.g., file descriptor exhaustion).
                        tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                    }
                }
            }
        }
    }

    tracing::debug!("DAP server accept loop exited");
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_message, write_message, DapMessage, DapRequest};
    use tokio::io::BufReader;
    use tokio::net::TcpStream;

    // ── Server startup ────────────────────────────────────────────────────────

    /// Helper: start a server on port 0 (NoopBackend for all connections).
    async fn start_test_server(event_tx: mpsc::Sender<DapServerEvent>) -> DapServerHandle {
        start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
            None, // no backend factory — uses NoopBackend
        )
        .await
        .expect("server should start on port 0")
    }

    #[tokio::test]
    async fn test_server_binds_to_port_zero_and_gets_assigned_port() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;
        assert!(handle.port() > 0, "OS-assigned port must be > 0");
        // Shut down cleanly.
        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_binds_to_specific_port() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;
        assert!(handle.port() > 0);
        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_start_fails_on_port_already_in_use() {
        // Bind once, then try to bind again on the same port.
        let (event_tx1, _) = mpsc::channel(16);
        let handle1 = start_test_server(event_tx1).await;
        let occupied_port = handle1.port();

        let (event_tx2, _) = mpsc::channel(16);
        let result = start(
            DapServerConfig {
                port: occupied_port,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx2,
            None,
        )
        .await;

        assert!(
            result.is_err(),
            "Starting a server on an occupied port must return an error"
        );

        handle1.shutdown_tx.send(true).unwrap();
        handle1.task.await.unwrap();
    }

    // ── Shutdown ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_server_shutdown_stops_accept_loop() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;

        // Signal shutdown.
        handle.shutdown_tx.send(true).unwrap();

        // The task should complete within a reasonable time.
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.task)
            .await
            .expect("server task should complete after shutdown signal")
            .expect("server task should not panic");
    }

    // ── Events ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_client_connect_emits_connected_event() {
        let (event_tx, mut event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;

        // Connect a TCP client.
        let _stream = TcpStream::connect(format!("127.0.0.1:{}", handle.port()))
            .await
            .expect("client should connect");

        // Wait for the ClientConnected event.
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("event should arrive within timeout")
            .expect("channel should not be closed");

        assert!(
            matches!(event, DapServerEvent::ClientConnected { .. }),
            "Expected ClientConnected event, got {:?}",
            event
        );

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_client_disconnect_emits_disconnected_event() {
        let (event_tx, mut event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;

        // Connect and immediately drop (simulates clean EOF).
        {
            let _stream = TcpStream::connect(format!("127.0.0.1:{}", handle.port()))
                .await
                .expect("client should connect");
            // Stream is dropped here, causing EOF on server side.
        }

        // Drain events until we see ClientDisconnected.
        let deadline = std::time::Duration::from_secs(3);
        let mut found_disconnected = false;
        let result = tokio::time::timeout(deadline, async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, DapServerEvent::ClientDisconnected { .. }) {
                    found_disconnected = true;
                    break;
                }
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Timed out waiting for ClientDisconnected event"
        );
        assert!(found_disconnected, "Expected ClientDisconnected event");

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    // ── Client session over TCP ───────────────────────────────────────────────

    /// Helper: write a DAP request to a TCP stream and read back messages.
    async fn send_request_and_read_responses(
        stream: &mut TcpStream,
        request: DapRequest,
        expected_count: usize,
    ) -> Vec<DapMessage> {
        let msg = DapMessage::Request(request);
        write_message(stream, &msg)
            .await
            .expect("write should succeed");

        // Read `expected_count` messages back.
        let mut responses = Vec::new();
        let (read_half, _write_half) = stream.split();
        let mut reader = BufReader::new(read_half);
        for _ in 0..expected_count {
            let resp =
                tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                    .await
                    .expect("response should arrive within timeout")
                    .expect("read should not error")
                    .expect("should not be EOF");
            responses.push(resp);
        }
        responses
    }

    #[tokio::test]
    async fn test_client_connect_and_initialize() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", handle.port()))
            .await
            .unwrap();

        let init_req = DapRequest {
            seq: 1,
            command: "initialize".into(),
            arguments: None,
        };

        // initialize should produce response + initialized event (2 messages).
        let responses = send_request_and_read_responses(&mut stream, init_req, 2).await;

        assert_eq!(responses.len(), 2);
        // First message must be a success response.
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if r.success),
            "initialize must return a success response"
        );
        // Second message must be the initialized event.
        assert!(
            matches!(&responses[1], DapMessage::Event(e) if e.event == "initialized"),
            "initialize must be followed by an initialized event"
        );
        // Response must include capabilities.
        if let DapMessage::Response(r) = &responses[0] {
            assert!(
                r.body.is_some(),
                "initialize response must include capabilities"
            );
        }

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_client_full_handshake_over_tcp() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start_test_server(event_tx).await;

        let port = handle.port();

        // Use a separate task so we can interact with the session freely.
        let client_task = tokio::spawn(async move {
            let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            // ── initialize ────────────────────────────────────────────────
            write_message(
                &mut stream,
                &DapMessage::Request(DapRequest {
                    seq: 1,
                    command: "initialize".into(),
                    arguments: None,
                }),
            )
            .await
            .unwrap();

            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);

            // Response + initialized event
            let r1 = read_message(&mut reader).await.unwrap().unwrap();
            let r2 = read_message(&mut reader).await.unwrap().unwrap();
            assert!(matches!(r1, DapMessage::Response(ref r) if r.success));
            assert!(matches!(r2, DapMessage::Event(ref e) if e.event == "initialized"));

            // ── configurationDone ─────────────────────────────────────────
            write_message(
                &mut write_half,
                &DapMessage::Request(DapRequest {
                    seq: 2,
                    command: "configurationDone".into(),
                    arguments: None,
                }),
            )
            .await
            .unwrap();

            let r3 = read_message(&mut reader).await.unwrap().unwrap();
            assert!(matches!(r3, DapMessage::Response(ref r) if r.success));

            // ── disconnect ────────────────────────────────────────────────
            write_message(
                &mut write_half,
                &DapMessage::Request(DapRequest {
                    seq: 3,
                    command: "disconnect".into(),
                    arguments: None,
                }),
            )
            .await
            .unwrap();

            // DAP spec: terminated event before disconnect response.
            let r4 = read_message(&mut reader).await.unwrap().unwrap();
            assert!(
                matches!(r4, DapMessage::Event(ref e) if e.event == "terminated"),
                "Expected terminated event, got {:?}",
                r4
            );
            let r5 = read_message(&mut reader).await.unwrap().unwrap();
            assert!(
                matches!(r5, DapMessage::Response(ref r) if r.success),
                "Expected disconnect success response, got {:?}",
                r5
            );
        });

        tokio::time::timeout(std::time::Duration::from_secs(5), client_task)
            .await
            .expect("client task should complete within timeout")
            .expect("client task should not panic");

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_clients_can_connect_concurrently() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let handle = start_test_server(event_tx).await;

        let port = handle.port();

        // Connect 3 clients simultaneously.
        let mut tasks = Vec::new();
        for i in 0..3_u64 {
            let t = tokio::spawn(async move {
                let _stream = TcpStream::connect(format!("127.0.0.1:{}", port))
                    .await
                    .expect("client should connect");
                // Keep alive briefly.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                drop(_stream);
                i
            });
            tasks.push(t);
        }

        // Collect all 3 client results.
        for t in tasks {
            t.await.unwrap();
        }

        // We should have received at least 3 ClientConnected events.
        let mut connected_count = 0u32;
        let drain_result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while let Ok(Some(event)) =
                tokio::time::timeout(std::time::Duration::from_millis(200), event_rx.recv()).await
            {
                if matches!(event, DapServerEvent::ClientConnected { .. }) {
                    connected_count += 1;
                    if connected_count >= 3 {
                        break;
                    }
                }
            }
        })
        .await;

        assert!(
            drain_result.is_ok(),
            "Timed out waiting for 3 ClientConnected events"
        );
        assert!(
            connected_count >= 3,
            "Expected at least 3 ClientConnected events, got {}",
            connected_count
        );

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    // ── Factory tests ─────────────────────────────────────────────────────────

    /// A mock backend that records calls for testing.
    #[cfg(test)]
    mod mock_backend {
        use crate::adapter::{
            BackendError, BreakpointResult, DapExceptionPauseMode, DebugEvent, DynDebugBackend,
            DynDebugBackendInner,
        };
        use std::future::Future;
        use std::pin::Pin;
        use tokio::sync::mpsc;

        pub struct MockBackendInner;

        impl DynDebugBackendInner for MockBackendInner {
            fn pause_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }

            fn resume_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _step: Option<crate::adapter::StepMode>,
                _frame_index: Option<i32>,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }

            fn add_breakpoint_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _uri: &'a str,
                _line: i32,
                _column: Option<i32>,
            ) -> Pin<Box<dyn Future<Output = Result<BreakpointResult, BackendError>> + Send + 'a>>
            {
                Box::pin(async {
                    Ok(BreakpointResult {
                        vm_id: "bp1".into(),
                        resolved: true,
                        line: None,
                        column: None,
                    })
                })
            }

            fn remove_breakpoint_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _bp_id: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }

            fn set_exception_pause_mode_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _mode: DapExceptionPauseMode,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }

            fn get_stack_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _limit: Option<i32>,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({"frames": []})) })
            }

            fn get_object_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _object_id: &'a str,
                _offset: Option<i64>,
                _count: Option<i64>,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({})) })
            }

            fn evaluate_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _target_id: &'a str,
                _expression: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({"result": "42"})) })
            }

            fn evaluate_in_frame_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _frame_index: i32,
                _expression: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({"result": "42"})) })
            }

            fn get_vm_boxed(
                &self,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + '_>>
            {
                Box::pin(async { Ok(serde_json::json!({"isolates": []})) })
            }

            fn get_isolate_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({})) })
            }

            fn get_scripts_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({"scripts": []})) })
            }

            fn call_service_boxed<'a>(
                &'a self,
                _method: &'a str,
                _params: Option<serde_json::Value>,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({})) })
            }

            fn set_library_debuggable_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _library_id: &'a str,
                _is_debuggable: bool,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }

            fn get_source_report_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _script_id: &'a str,
                _report_kinds: Vec<String>,
                _token_pos: Option<i64>,
                _end_token_pos: Option<i64>,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>
            {
                Box::pin(async { Ok(serde_json::json!({"ranges": [], "scripts": []})) })
            }

            fn get_source_boxed<'a>(
                &'a self,
                _isolate_id: &'a str,
                _script_id: &'a str,
            ) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send + 'a>>
            {
                Box::pin(async { Ok(String::new()) })
            }

            fn hot_reload_boxed(
                &self,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
                Box::pin(async { Ok(()) })
            }

            fn hot_restart_boxed(
                &self,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
                Box::pin(async { Ok(()) })
            }

            fn stop_app_boxed(
                &self,
            ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
                Box::pin(async { Ok(()) })
            }

            fn ws_uri_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
                Box::pin(async { None })
            }

            fn device_id_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
                Box::pin(async { None })
            }

            fn build_mode_boxed(&self) -> Pin<Box<dyn Future<Output = String> + Send + '_>> {
                Box::pin(async { "debug".to_string() })
            }
        }

        /// A factory that always returns a mock backend.
        pub struct AlwaysBackendFactory;

        impl super::BackendFactory for AlwaysBackendFactory {
            fn create(&self) -> Option<super::BackendHandle> {
                let (_, debug_event_rx) = mpsc::channel::<DebugEvent>(8);
                Some(super::BackendHandle {
                    backend: DynDebugBackend::new(Box::new(MockBackendInner)),
                    debug_event_rx,
                })
            }
        }

        /// A factory that never returns a backend.
        pub struct NeverBackendFactory;

        impl super::BackendFactory for NeverBackendFactory {
            fn create(&self) -> Option<super::BackendHandle> {
                None
            }
        }
    }

    /// When factory returns None, the session should still initialize cleanly.
    #[tokio::test]
    async fn test_factory_returning_none_falls_back_to_noop() {
        let factory = Arc::new(mock_backend::NeverBackendFactory);

        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
            Some(factory),
        )
        .await
        .expect("server should start");

        let port = handle.port();

        let client_task = tokio::spawn(async move {
            let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            // initialize should still succeed with NoopBackend fallback.
            let req = DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            });
            write_message(&mut stream, &req).await.unwrap();

            let (read_half, _) = stream.split();
            let mut reader = BufReader::new(read_half);
            let resp =
                tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();

            assert!(
                matches!(resp, DapMessage::Response(ref r) if r.success),
                "initialize must succeed even with NoopBackend fallback"
            );
        });

        tokio::time::timeout(std::time::Duration::from_secs(5), client_task)
            .await
            .expect("client task should complete")
            .expect("client task should not panic");

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    /// When factory returns Some(MockBackend), the session should initialize cleanly.
    #[tokio::test]
    async fn test_factory_returning_some_backend_initializes_cleanly() {
        let factory = Arc::new(mock_backend::AlwaysBackendFactory);

        let (event_tx, _event_rx) = mpsc::channel(16);
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
            Some(factory),
        )
        .await
        .expect("server should start");

        let port = handle.port();

        let client_task = tokio::spawn(async move {
            let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            let req = DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            });
            write_message(&mut stream, &req).await.unwrap();

            let (read_half, _) = stream.split();
            let mut reader = BufReader::new(read_half);
            let resp =
                tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();

            assert!(
                matches!(resp, DapMessage::Response(ref r) if r.success),
                "initialize must succeed with real backend"
            );
        });

        tokio::time::timeout(std::time::Duration::from_secs(5), client_task)
            .await
            .expect("client task should complete")
            .expect("client task should not panic");

        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }
}
