//! # DapService — DAP server lifecycle management
//!
//! Provides a thin wrapper around [`server::start`] that manages the
//! DAP server lifecycle (start and stop). The event bridge from
//! [`DapServerEvent`] to the host application's message type is handled
//! by the caller (e.g., `fdemon-app`), which maps events to `Message`
//! variants without creating a circular dependency.
//!
//! ## Usage
//!
//! ```ignore
//! // In fdemon-app (which depends on fdemon-dap):
//! let (event_tx, event_rx) = mpsc::channel(32);
//! let handle = DapService::start(port, bind_addr, event_tx).await?;
//!
//! // Bridge events in the app layer:
//! tokio::spawn(async move {
//!     while let Some(event) = event_rx.recv().await {
//!         // map event → Message and send to engine
//!     }
//! });
//!
//! // On shutdown:
//! DapService::stop(handle).await;
//! ```

use tokio::sync::mpsc;

use fdemon_core::error::Result;

use crate::server::{DapServerConfig, DapServerEvent, DapServerHandle};

/// Manages the DAP server lifecycle.
///
/// This is a stateless helper — all lifecycle state lives in the
/// [`DapServerHandle`] returned by [`DapService::start`]. The caller
/// (Engine integration layer) stores the handle and passes it to
/// [`DapService::stop`] on shutdown.
///
/// ## Cross-crate boundary
///
/// `DapService` does **not** reference `fdemon-app` types such as `Message`.
/// Instead, it accepts an `mpsc::Sender<DapServerEvent>` so the caller can
/// bridge events to application-specific message types without creating a
/// circular dependency.
pub struct DapService;

impl DapService {
    /// Start the DAP TCP server and begin forwarding events to the caller.
    ///
    /// Binds the server to `bind_addr:port` and returns a [`DapServerHandle`]
    /// for lifecycle management. Events are sent over `event_tx` so the caller
    /// can map them to application-specific messages (e.g., `Message` variants)
    /// without creating a crate dependency from `fdemon-dap` to `fdemon-app`.
    ///
    /// # Arguments
    ///
    /// * `port` — TCP port to bind on. Use `0` to let the OS assign an
    ///   ephemeral port; the actual port is in [`DapServerHandle::port`].
    /// * `bind_addr` — Bind address string (e.g. `"127.0.0.1"`).
    /// * `event_tx` — Channel for [`DapServerEvent`] notifications. The
    ///   caller is responsible for draining this channel.
    ///
    /// # Returns
    ///
    /// A [`DapServerHandle`] on success, or an [`Error`] if the port is
    /// unavailable.
    ///
    /// [`Error`]: fdemon_core::error::Error
    pub async fn start(
        port: u16,
        bind_addr: String,
        event_tx: mpsc::Sender<DapServerEvent>,
    ) -> Result<DapServerHandle> {
        let config = DapServerConfig { port, bind_addr };
        crate::server::start(config, event_tx).await
    }

    /// Stop a running DAP server and wait for it to finish.
    ///
    /// Signals the server task to shut down, then awaits the task with a
    /// 5-second timeout. If the task does not complete within the timeout
    /// it is abandoned (not cancelled forcefully).
    ///
    /// # Arguments
    ///
    /// * `handle` — The [`DapServerHandle`] returned by [`DapService::start`].
    pub async fn stop(handle: DapServerHandle) {
        // Signal shutdown. If the receiver is already gone the server has
        // already stopped — that is fine, so we ignore the error.
        let _ = handle.shutdown_tx.send(true);

        // Wait for the accept-loop task to finish with a generous timeout.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle.task).await;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    /// Helper: start a server on port 0 (OS-assigned) and return the handle.
    async fn start_server() -> (DapServerHandle, mpsc::Receiver<DapServerEvent>) {
        let (event_tx, event_rx) = mpsc::channel(16);
        let handle = DapService::start(0, "127.0.0.1".to_string(), event_tx)
            .await
            .expect("DapService::start should succeed on port 0");
        (handle, event_rx)
    }

    #[tokio::test]
    async fn test_dap_service_start_returns_valid_port() {
        let (handle, _rx) = start_server().await;
        assert!(handle.port > 0, "OS-assigned port must be nonzero");
        DapService::stop(handle).await;
    }

    #[tokio::test]
    async fn test_dap_service_stop_completes_cleanly() {
        let (handle, _rx) = start_server().await;
        // Should not panic or hang.
        DapService::stop(handle).await;
    }

    #[tokio::test]
    async fn test_dap_service_start_fails_on_port_in_use() {
        let (handle, _rx) = start_server().await;
        let occupied_port = handle.port;

        let (event_tx2, _rx2) = mpsc::channel(16);
        let result = DapService::start(occupied_port, "127.0.0.1".to_string(), event_tx2).await;

        assert!(
            result.is_err(),
            "Starting a second server on an occupied port must fail"
        );

        DapService::stop(handle).await;
    }

    #[tokio::test]
    async fn test_dap_service_events_forwarded() {
        use tokio::net::TcpStream;

        let (handle, mut event_rx) = start_server().await;
        let port = handle.port;

        // Connect a client so we get a ClientConnected event.
        let _stream = TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .expect("client should connect");

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("event should arrive within 2 seconds")
            .expect("channel should not be closed");

        assert!(
            matches!(event, DapServerEvent::ClientConnected { .. }),
            "Expected ClientConnected, got {:?}",
            event
        );

        DapService::stop(handle).await;
    }
}
