//! # DAP TCP Server
//!
//! Provides the TCP listener that accepts DAP client connections and dispatches
//! each connection to a [`DapClientSession`] running in its own Tokio task.
//!
//! ## Lifecycle
//!
//! 1. Call [`start`] with a [`DapServerConfig`] and an event channel sender.
//! 2. [`start`] binds the TCP listener and returns a [`DapServerHandle`].
//! 3. The handle's `port` field gives the actual port (useful when `port = 0`
//!    is specified to get an OS-assigned port).
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

pub use session::{DapClientSession, SessionState};

use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
};

use fdemon_core::error::{Error, Result};

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
}

/// Handle returned when the DAP server starts successfully.
///
/// Used by the Engine to track server lifecycle and initiate shutdown.
/// When dropped the server task is **not** automatically cancelled — call
/// `shutdown_tx.send(true)` first, then `.await` on `task` to join cleanly.
pub struct DapServerHandle {
    /// The actual port the server is listening on.
    ///
    /// This may differ from the configured port when `port = 0` was passed to
    /// [`start`], in which case the OS assigned an ephemeral port.
    pub port: u16,

    /// Send `true` to signal the server and all active sessions to shut down.
    pub shutdown_tx: watch::Sender<bool>,

    /// Join handle for the accept-loop task.
    ///
    /// Await this after sending the shutdown signal to ensure the server has
    /// fully stopped before releasing resources.
    pub task: tokio::task::JoinHandle<()>,
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
/// # Arguments
///
/// * `config` — Address/port configuration.
/// * `event_tx` — Channel used to emit [`DapServerEvent`] to the host.
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

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let task = tokio::spawn(accept_loop(listener, shutdown_rx, event_tx));

    Ok(DapServerHandle {
        port: actual_port,
        shutdown_tx,
        task,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Accept loop
// ─────────────────────────────────────────────────────────────────────────────

/// Accept connections until a shutdown signal is received.
///
/// For each accepted connection:
/// 1. Emits a [`DapServerEvent::ClientConnected`] with the remote address.
/// 2. Spawns a task running [`DapClientSession::run`].
/// 3. On session completion, emits [`DapServerEvent::ClientDisconnected`].
///
/// When `shutdown_rx` receives `true`, the loop breaks. Each active session
/// also watches the same `shutdown_rx` and terminates independently.
async fn accept_loop(
    listener: TcpListener,
    mut shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<DapServerEvent>,
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

                        tokio::spawn(async move {
                            if let Err(e) = DapClientSession::run(stream, session_shutdown).await {
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
                        });
                    }
                    Err(e) => {
                        tracing::error!("DAP server failed to accept connection: {}", e);
                        // Emit a server error event; continue accepting.
                        let _ = event_tx
                            .send(DapServerEvent::ServerError {
                                reason: e.to_string(),
                            })
                            .await;
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

    #[tokio::test]
    async fn test_server_binds_to_port_zero_and_gets_assigned_port() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        let config = DapServerConfig {
            port: 0,
            bind_addr: "127.0.0.1".to_string(),
        };
        let handle = start(config, event_tx).await.expect("server should start");
        assert!(handle.port > 0, "OS-assigned port must be > 0");
        // Shut down cleanly.
        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_binds_to_specific_port() {
        let (event_tx, _event_rx) = mpsc::channel(16);
        // port=0 lets OS pick; we verify we get a valid port back.
        let config = DapServerConfig {
            port: 0,
            bind_addr: "127.0.0.1".to_string(),
        };
        let handle = start(config, event_tx).await.unwrap();
        assert!(handle.port > 0);
        handle.shutdown_tx.send(true).unwrap();
        handle.task.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_start_fails_on_port_already_in_use() {
        // Bind once, then try to bind again on the same port.
        let (event_tx1, _) = mpsc::channel(16);
        let handle1 = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx1,
        )
        .await
        .unwrap();
        let occupied_port = handle1.port;

        let (event_tx2, _) = mpsc::channel(16);
        let result = start(
            DapServerConfig {
                port: occupied_port,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx2,
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
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
        )
        .await
        .unwrap();

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
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
        )
        .await
        .unwrap();

        // Connect a TCP client.
        let _stream = TcpStream::connect(format!("127.0.0.1:{}", handle.port))
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
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
        )
        .await
        .unwrap();

        // Connect and immediately drop (simulates clean EOF).
        {
            let _stream = TcpStream::connect(format!("127.0.0.1:{}", handle.port))
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
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
        )
        .await
        .unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", handle.port))
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
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
        )
        .await
        .unwrap();

        let port = handle.port;

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

            let r4 = read_message(&mut reader).await.unwrap().unwrap();
            assert!(matches!(r4, DapMessage::Response(ref r) if r.success));
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
        let handle = start(
            DapServerConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
            },
            event_tx,
        )
        .await
        .unwrap();

        let port = handle.port;

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
}
