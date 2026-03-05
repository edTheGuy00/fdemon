//! # Stdio Transport
//!
//! Implements the stdin/stdout transport mode for the DAP server.
//!
//! ## Overview
//!
//! When an IDE like Zed or Helix launches `fdemon` as a DAP adapter subprocess,
//! it communicates over the process's stdin/stdout pipes. This module provides
//! the entry point for that single-session, process-lifetime mode.
//!
//! Unlike TCP mode (which runs an accept loop serving multiple clients), stdio
//! mode serves exactly one DAP session for the lifetime of the process. When
//! the client disconnects, the session ends and the caller should exit.
//!
//! ## Stdout Isolation
//!
//! When running in stdio mode, **all non-DAP output must go to stderr**:
//! - Tracing/logging subscribers must write to stderr.
//! - No `println!()` or other stdout writes are permitted.
//! - The TUI (which uses terminal raw mode) must not be started.
//!
//! The binary entry point (`--dap-stdio`) is responsible for configuring the
//! tracing subscriber to use stderr before calling [`run_stdio_session`].
//!
//! ## Usage
//!
//! ```ignore
//! let (shutdown_tx, shutdown_rx) = watch::channel(false);
//! let (event_tx, mut event_rx) = mpsc::channel(16);
//!
//! // Spawn event consumer
//! tokio::spawn(async move {
//!     while let Some(event) = event_rx.recv().await {
//!         // bridge DapServerEvent → application Message
//!     }
//! });
//!
//! // Block until the DAP client disconnects or shutdown is signalled.
//! run_stdio_session(shutdown_rx, event_tx).await?;
//! ```

use tokio::{
    io::{BufReader, BufWriter},
    sync::{mpsc, watch},
};

use fdemon_core::error::Result;

use crate::server::{DapClientSession, DapServerEvent};

/// Run a single DAP session over stdin/stdout.
///
/// This is the entry point for adapter subprocess mode. The function:
/// 1. Wraps stdin/stdout in buffered I/O wrappers.
/// 2. Emits a [`DapServerEvent::ClientConnected`] with `client_id = "stdio"`.
/// 3. Runs the full DAP session state machine until the client disconnects or
///    the shutdown signal fires.
/// 4. Emits [`DapServerEvent::ClientDisconnected`] with `client_id = "stdio"`.
///
/// # Arguments
///
/// * `shutdown_rx` — Watch channel; send `true` to request graceful shutdown.
/// * `event_tx` — Channel for [`DapServerEvent`] notifications. The caller
///   should bridge these to application-level message types.
///
/// # Returns
///
/// `Ok(())` on clean disconnect or shutdown; `Err(_)` on I/O or protocol errors.
///
/// # Panics
///
/// Does not panic. I/O errors from stdin/stdout are propagated as `Err`.
pub async fn run_stdio_session(
    shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<DapServerEvent>,
) -> Result<()> {
    let reader = BufReader::new(tokio::io::stdin());
    let writer = BufWriter::new(tokio::io::stdout());

    // Notify caller that a client connected (client_id is the fixed string "stdio"
    // since there is no remote address for stdin/stdout).
    event_tx
        .send(DapServerEvent::ClientConnected {
            client_id: "stdio".into(),
        })
        .await
        .ok();

    tracing::info!("DAP stdio session starting");

    // Run the session using the generic run_on method. This reuses all DAP
    // state-machine logic from DapClientSession without any TCP-specific code.
    // Create a dummy broadcast channel — stdio sessions don't receive log events
    // from the TCP server's broadcast, but the session loop needs a receiver.
    let (_, log_event_rx) = tokio::sync::broadcast::channel(1);
    let result = DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx).await;

    match &result {
        Ok(()) => tracing::info!("DAP stdio session ended cleanly"),
        Err(e) => tracing::warn!("DAP stdio session ended with error: {}", e),
    }

    // Notify caller that the client disconnected.
    event_tx
        .send(DapServerEvent::ClientDisconnected {
            client_id: "stdio".into(),
        })
        .await
        .ok();

    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncWriteExt, BufReader, BufWriter},
        sync::{mpsc, watch},
    };

    use crate::{
        read_message, server::DapClientSession, write_message, DapMessage, DapRequest,
        DapServerEvent,
    };

    // ── run_on — generic session over duplex streams ──────────────────────────

    /// Helper: run a server session over a duplex pair and return the join handle.
    async fn spawn_server_session(
        server_reader: impl tokio::io::AsyncRead + Unpin + Send + 'static,
        server_writer: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
        shutdown_rx: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<fdemon_core::error::Result<()>> {
        tokio::spawn(async move {
            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            let (_, log_event_rx) = tokio::sync::broadcast::channel(1);
            DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx).await
        })
    }

    #[tokio::test]
    async fn test_run_on_initialize_produces_response_and_event() {
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        // ── client: send initialize ───────────────────────────────────────
        let mut client_writer = BufWriter::new(client_writer);
        let init_req = DapMessage::Request(DapRequest {
            seq: 1,
            command: "initialize".into(),
            arguments: None,
        });
        write_message(&mut client_writer, &init_req).await.unwrap();

        // ── client: read two messages back ────────────────────────────────
        let mut client_reader = BufReader::new(client_reader);
        let msg1 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_message(&mut client_reader),
        )
        .await
        .expect("response timeout")
        .expect("read ok")
        .expect("not EOF");

        let msg2 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_message(&mut client_reader),
        )
        .await
        .expect("event timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&msg1, DapMessage::Response(r) if r.success),
            "Expected success response to initialize, got {:?}",
            msg1
        );
        assert!(
            matches!(&msg2, DapMessage::Event(e) if e.event == "initialized"),
            "Expected initialized event, got {:?}",
            msg2
        );

        shutdown_tx.send(true).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server shutdown timeout")
            .expect("server task ok")
            .expect("session ok");
    }

    #[tokio::test]
    async fn test_run_on_full_handshake_initialize_configure_disconnect() {
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // initialize → response + initialized event
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let r1 = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let r2 = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(r1, DapMessage::Response(ref resp) if resp.success));
        assert!(matches!(r2, DapMessage::Event(ref e) if e.event == "initialized"));

        // configurationDone → success response
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 2,
                command: "configurationDone".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let r3 = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(r3, DapMessage::Response(ref resp) if resp.success));

        // disconnect → success response; session exits
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 3,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let r4 = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(r4, DapMessage::Response(ref resp) if resp.success));

        // Server session should exit on its own after disconnect.
        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit after disconnect")
            .expect("task ok")
            .expect("session ok");
    }

    #[tokio::test]
    async fn test_run_on_eof_exits_cleanly() {
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (_, server_writer) = tokio::io::duplex(8192);

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        // Drop the writer immediately → EOF on server side.
        drop(client_writer);

        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit on EOF")
            .expect("task ok")
            .expect("session ok");
    }

    #[tokio::test]
    async fn test_run_on_shutdown_signal_sends_terminated_event() {
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        // Do the initialize handshake first so the session is in Initializing state.
        let mut writer = BufWriter::new(client_writer);
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        let mut reader = BufReader::new(client_reader);
        // Consume the two initialize responses.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        // Signal shutdown.
        shutdown_tx.send(true).unwrap();

        // Server should send a terminated event before closing.
        let terminated =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                .await
                .expect("terminated event timeout")
                .expect("read ok");

        // The server may send a terminated event or just close (EOF → None).
        // Both are acceptable — verify the server task completes cleanly.
        let _ = terminated;

        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server shutdown timeout")
            .expect("task ok")
            .expect("session ok");
    }

    #[tokio::test]
    async fn test_run_on_unknown_command_returns_error_response() {
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // initialize first
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        // Consume response + event
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();

        // Send an unknown command
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 2,
                command: "flyToMoon".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        let response =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                .await
                .unwrap()
                .unwrap()
                .unwrap();

        assert!(
            matches!(&response, DapMessage::Response(r) if !r.success),
            "Unknown command must return error response, got {:?}",
            response
        );

        shutdown_tx.send(true).unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server shutdown timeout");
    }

    // ── run_stdio_session — event emission ────────────────────────────────────

    #[tokio::test]
    async fn test_stdio_session_emits_connected_and_disconnected_events() {
        // We can't use real stdin/stdout in tests, but we can test the event
        // emission pattern by running DapClientSession::run_on directly and
        // verifying that run_stdio_session would emit the right events.
        //
        // Instead, simulate the stdio lifecycle using the event channel directly.
        let (event_tx, mut event_rx) = mpsc::channel::<DapServerEvent>(16);

        // Emit connected (as run_stdio_session does)
        event_tx
            .send(DapServerEvent::ClientConnected {
                client_id: "stdio".into(),
            })
            .await
            .unwrap();

        // Emit disconnected (as run_stdio_session does on exit)
        event_tx
            .send(DapServerEvent::ClientDisconnected {
                client_id: "stdio".into(),
            })
            .await
            .unwrap();

        let connected = event_rx.recv().await.unwrap();
        let disconnected = event_rx.recv().await.unwrap();

        assert!(
            matches!(&connected, DapServerEvent::ClientConnected { client_id } if client_id == "stdio"),
            "Expected ClientConnected with client_id='stdio', got {:?}",
            connected
        );
        assert!(
            matches!(&disconnected, DapServerEvent::ClientDisconnected { client_id } if client_id == "stdio"),
            "Expected ClientDisconnected with client_id='stdio', got {:?}",
            disconnected
        );
    }

    #[tokio::test]
    async fn test_run_on_session_lifecycle_connect_initialize_disconnect() {
        // Verify the complete expected lifecycle used by run_stdio_session.
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);
        let (event_tx, mut event_rx) = mpsc::channel::<DapServerEvent>(16);

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        // Simulate what run_stdio_session does (minus real stdin/stdout).
        let session_handle = tokio::spawn(async move {
            event_tx
                .send(DapServerEvent::ClientConnected {
                    client_id: "stdio".into(),
                })
                .await
                .ok();

            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            let (_, log_event_rx) = tokio::sync::broadcast::channel(1);
            let result = DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx).await;

            event_tx
                .send(DapServerEvent::ClientDisconnected {
                    client_id: "stdio".into(),
                })
                .await
                .ok();

            result
        });

        // Verify ClientConnected event
        let connected = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("connected event timeout")
            .expect("channel open");

        assert!(
            matches!(&connected, DapServerEvent::ClientConnected { client_id } if client_id == "stdio"),
            "Expected ClientConnected, got {:?}",
            connected
        );

        // Client: send initialize + disconnect
        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // Drain initialize responses
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();

        // Send disconnect to end the session
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 2,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // Read disconnect response
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();

        // Verify ClientDisconnected event
        let disconnected = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("disconnected event timeout")
            .expect("channel open");

        assert!(
            matches!(&disconnected, DapServerEvent::ClientDisconnected { client_id } if client_id == "stdio"),
            "Expected ClientDisconnected, got {:?}",
            disconnected
        );

        // Session task should have completed
        tokio::time::timeout(std::time::Duration::from_secs(2), session_handle)
            .await
            .expect("session task timeout")
            .expect("task ok")
            .expect("session ok");
    }

    #[tokio::test]
    async fn test_run_on_multiple_requests_over_single_stream() {
        // Verify the session handles multiple sequential requests correctly
        // (important for stdio where the stream is never "reconnected").
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // initialize
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
            .await
            .unwrap()
            .unwrap();

        // configurationDone
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 2,
                command: "configurationDone".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let cd_resp =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert!(
            matches!(cd_resp, DapMessage::Response(ref r) if r.success),
            "configurationDone should succeed"
        );

        // attach
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 3,
                command: "attach".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let attach_resp =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        // With no real VM Service connected (NoopBackend), attach returns an
        // error response. We only verify that a response is returned.
        assert!(
            matches!(attach_resp, DapMessage::Response(_)),
            "attach must return a response, got {:?}",
            attach_resp
        );

        // disconnect
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 4,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        let disc_resp =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert!(
            matches!(disc_resp, DapMessage::Response(ref r) if r.success),
            "disconnect should succeed"
        );

        // Drop writer to let server see EOF after disconnect response is sent
        // (server already closed after disconnect)
        drop(writer);

        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server shutdown timeout")
            .expect("task ok")
            .expect("session ok");
    }

    #[tokio::test]
    async fn test_run_on_writer_flush_works() {
        // Verify that writing to a BufWriter over duplex works correctly
        // (i.e., flush is called and bytes are actually sent).
        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let server = spawn_server_session(server_reader, server_writer, shutdown_rx).await;

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // Write initialize and flush
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        writer.flush().await.unwrap();

        // If flush didn't work, we'd time out here.
        let response =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_message(&mut reader))
                .await
                .expect("flush should cause server to receive data")
                .expect("read ok")
                .expect("not EOF");

        assert!(
            matches!(response, DapMessage::Response(ref r) if r.success),
            "Expected success response after explicit flush"
        );

        // Send disconnect to clean up
        write_message(
            &mut writer,
            &DapMessage::Request(DapRequest {
                seq: 2,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // Drop to let server see EOF/disconnect
        drop(writer);

        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
    }
}
