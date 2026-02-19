//! Async WebSocket client for the Dart VM Service.
//!
//! The [`VmServiceClient`] connects to the Dart VM Service over WebSocket, sends
//! JSON-RPC 2.0 requests, routes responses back to callers via oneshot channels,
//! and forwards stream events through an mpsc channel.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     VmServiceClient                          │
//! │                                                              │
//! │  ┌──────────────┐        ┌──────────────────────────────┐   │
//! │  │   Public API │        │   Background Task             │   │
//! │  │              │        │                                │   │
//! │  │  request() ──┼──cmd──▶│  WebSocket read/write loop    │   │
//! │  │              │  chan   │                                │   │
//! │  │  events()  ◀─┼──evt──◀│  Route: response → tracker    │   │
//! │  │              │  chan   │         event → event channel  │   │
//! │  └──────────────┘        └──────────────────────────────┘   │
//! │                                                              │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │  VmRequestTracker (from protocol.rs)                  │   │
//! │  │  Correlates request IDs with response receivers       │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

use fdemon_core::prelude::*;

use super::extensions::build_extension_params;
use super::protocol::{
    parse_vm_message, IsolateInfo, IsolateRef, VmInfo, VmRequestTracker, VmServiceError,
    VmServiceEvent, VmServiceMessage, VmServiceRequest,
};

// ---------------------------------------------------------------------------
// VmRequestHandle
// ---------------------------------------------------------------------------

/// A clonable handle for making VM Service RPC requests.
///
/// This shares the underlying WebSocket connection with the [`VmServiceClient`]
/// that created it. Multiple handles can make concurrent requests through
/// the same background WebSocket task.
///
/// The handle becomes inoperable when the [`VmServiceClient`] (or its background
/// task) is dropped — requests will return [`Error::ChannelClosed`].
#[derive(Clone)]
pub struct VmRequestHandle {
    cmd_tx: mpsc::Sender<ClientCommand>,
    state: Arc<std::sync::RwLock<ConnectionState>>,
    /// Cached main isolate ID. Cleared by the background task on reconnection.
    isolate_id_cache: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for VmRequestHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner()).clone();
        f.debug_struct("VmRequestHandle")
            .field("connection_state", &state)
            .finish()
    }
}

impl VmRequestHandle {
    /// Send a JSON-RPC request and wait for the response.
    ///
    /// Blocks (awaits) until the VM Service replies or the connection is
    /// lost.  Returns the `result` field of a successful response, or an
    /// error for JSON-RPC errors and transport failures.
    ///
    /// # Errors
    ///
    /// - [`Error::ChannelClosed`] if the background task has exited.
    /// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error.
    /// - [`Error::Daemon`] if the response arrived without a `result` field.
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(ClientCommand::SendRequest {
                method: method.to_string(),
                params,
                response_tx,
            })
            .await
            .map_err(|_| Error::ChannelClosed)?;

        response_rx.await.map_err(|_| Error::ChannelClosed)?
    }

    /// Return the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.state.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Return `true` if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        *self.state.read().unwrap_or_else(|e| e.into_inner()) == ConnectionState::Connected
    }

    /// Get the main isolate ID, discovering it if not yet cached.
    ///
    /// The isolate ID is cached after the first successful discovery. The
    /// cache is invalidated automatically when the WebSocket reconnects, so
    /// this method will re-discover on the next call after a reconnection.
    ///
    /// # Returns
    ///
    /// The isolate ID string (e.g., `"isolates/6010531716406367"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the VM Service call to `getVM` fails or no
    /// non-system isolate is found.
    pub async fn main_isolate_id(&self) -> Result<String> {
        // Fast path: return from cache if available.
        {
            let guard = self.isolate_id_cache.lock().await;
            if let Some(ref id) = *guard {
                return Ok(id.clone());
            }
        }

        // Slow path: call getVM and find the main isolate.
        let result = self.request("getVM", None).await?;
        let vm: VmInfo = serde_json::from_value(result)
            .map_err(|e| Error::vm_service(format!("parse getVM: {e}")))?;
        let isolate = vm
            .isolates
            .iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false))
            .ok_or_else(|| Error::vm_service("no non-system isolate found"))?;
        let id = isolate.id.clone();

        {
            let mut guard = self.isolate_id_cache.lock().await;
            *guard = Some(id.clone());
        }

        debug!("VM Service: cached main isolate ID: {}", id);
        Ok(id)
    }

    /// Create a `VmRequestHandle` backed by a disconnected dummy channel.
    ///
    /// Intended for unit tests that need a handle but do not make real RPC
    /// calls. The pre-populated `isolate_id` is written directly into the
    /// cache; pass `None` for an empty cache.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn new_for_test(isolate_id: Option<String>) -> Self {
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(1);
        Self {
            cmd_tx,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::new(Mutex::new(isolate_id)),
        }
    }

    /// Peek at the current cached isolate ID without modifying it.
    ///
    /// Returns `None` if the cache is empty or if the lock cannot be
    /// acquired immediately (extremely rare; only possible if
    /// `main_isolate_id()` is executing concurrently).
    ///
    /// Intended for unit tests that need to inspect internal state.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn cached_isolate_id(&self) -> Option<String> {
        self.isolate_id_cache
            .try_lock()
            .ok()
            .and_then(|g| g.clone())
    }

    /// Clear the cached main isolate ID.
    ///
    /// Call this after events that create a new isolate (hot restart) so
    /// the next [`main_isolate_id()`] call re-fetches from the VM via
    /// `getVM` RPC.
    ///
    /// Uses `try_lock()` on the internal `tokio::Mutex`. Under the very
    /// rare condition that `main_isolate_id()` is executing concurrently
    /// (i.e. the lock is held across the async `getVM` call), the
    /// invalidation is silently skipped. In practice this is harmless: the
    /// performance polling loop sleeps 2 seconds between calls, so
    /// contention is extremely unlikely; and even if it occurs the cache
    /// will be repopulated with the same value that was already being
    /// fetched.
    pub fn invalidate_isolate_cache(&self) {
        if let Ok(mut cache) = self.isolate_id_cache.try_lock() {
            *cache = None;
            debug!("VM Service: isolate ID cache invalidated (hot restart)");
        } else {
            debug!("VM Service: isolate ID cache lock contention during invalidation — skipped");
        }
    }

    /// Call a Flutter service extension method.
    ///
    /// Automatically includes `isolateId` in the params map. All additional
    /// parameter values must be strings (VM Service protocol requirement).
    ///
    /// # Errors
    ///
    /// - [`Error::ChannelClosed`] if the background task has exited.
    /// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error,
    ///   including error code `-32601` when the extension is not available.
    /// - [`Error::Daemon`] if the response arrived without a `result` field.
    pub async fn call_extension(
        &self,
        method: &str,
        isolate_id: &str,
        args: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value> {
        let params = build_extension_params(isolate_id, args);
        self.request(method, Some(params)).await
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Initial reconnection backoff duration.
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// Maximum reconnection backoff duration (cap).
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Maximum number of consecutive reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 10;

/// Capacity of the command channel (bounded, to apply backpressure).
const CMD_CHANNEL_CAPACITY: usize = 32;

/// Capacity of the event channel (bounded, events can be bursty).
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Stream IDs that the background task re-subscribes to after a reconnection.
/// A fresh VM Service connection has no active subscriptions, so these must be
/// re-established to keep receiving Extension, Logging, and GC events.
const RESUBSCRIBE_STREAMS: &[&str] = &["Extension", "Logging", "GC"];

/// How often to run stale request cleanup in the I/O loop.
const STALE_REQUEST_CLEANUP_INTERVAL: Duration = Duration::from_secs(30);

/// Timeout after which a pending request is considered stale and removed.
const STALE_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Current connection state of a [`VmServiceClient`].
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Not connected and not attempting to connect.
    Disconnected,
    /// Initial connection attempt in progress.
    Connecting,
    /// Connected and ready to exchange messages.
    Connected,
    /// Connection lost; background task is retrying.
    Reconnecting {
        /// The current reconnection attempt number (1-indexed).
        attempt: u32,
    },
}

// ---------------------------------------------------------------------------
// Internal command type
// ---------------------------------------------------------------------------

/// Internal messages sent from the public API to the background task.
enum ClientCommand {
    /// Send a JSON-RPC request and deliver the response to `response_tx`.
    SendRequest {
        method: String,
        params: Option<serde_json::Value>,
        response_tx: oneshot::Sender<Result<serde_json::Value>>,
    },
    /// Gracefully close the WebSocket connection and stop the background task.
    Disconnect,
}

// ---------------------------------------------------------------------------
// WebSocket type alias
// ---------------------------------------------------------------------------

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

// ---------------------------------------------------------------------------
// VmServiceClient
// ---------------------------------------------------------------------------

/// Async WebSocket client for the Dart VM Service.
///
/// Create with [`VmServiceClient::connect`], then use [`request`] to issue
/// JSON-RPC calls and [`event_receiver`] to consume stream events.
///
/// The client spawns a background Tokio task that owns the WebSocket
/// connection. The task cleans up automatically when `VmServiceClient` is
/// dropped (the command channel closes, which signals the task to exit).
///
/// Use [`VmServiceClient::request_handle`] to extract a clonable
/// [`VmRequestHandle`] that can be shared with background tasks for
/// on-demand RPC calls without blocking the event-forwarding loop.
pub struct VmServiceClient {
    /// Shared request handle — owns the cmd_tx, state, and isolate cache.
    handle: VmRequestHandle,
    /// Stream-event receiver (not clonable; owned exclusively by this client).
    event_rx: mpsc::Receiver<VmServiceEvent>,
}

impl VmServiceClient {
    /// Connect to the Dart VM Service at `ws_uri` and return a client.
    ///
    /// Spawns a background task that manages the WebSocket connection,
    /// including automatic reconnection with exponential backoff.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial connection cannot be established.
    pub async fn connect(ws_uri: &str) -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<ClientCommand>(CMD_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel::<VmServiceEvent>(EVENT_CHANNEL_CAPACITY);
        let state = Arc::new(std::sync::RwLock::new(ConnectionState::Connecting));
        let isolate_id_cache: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // Attempt the first connection before returning so callers know whether
        // the URI is reachable.
        info!("Connecting to VM Service at {}", ws_uri);
        let ws_stream = connect_ws(ws_uri).await?;

        {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            *guard = ConnectionState::Connected;
        }

        let ws_uri_owned = ws_uri.to_string();
        let state_clone = Arc::clone(&state);
        let cache_clone = Arc::clone(&isolate_id_cache);

        tokio::spawn(run_client_task(
            ws_uri_owned,
            ws_stream,
            cmd_rx,
            event_tx,
            state_clone,
            cache_clone,
        ));

        Ok(Self {
            handle: VmRequestHandle {
                cmd_tx,
                state,
                isolate_id_cache,
            },
            event_rx,
        })
    }

    /// Create a clonable request handle that shares this client's connection.
    ///
    /// The handle can make RPC requests independently of the event receiver.
    /// Multiple handles can coexist; they all route through the same background
    /// WebSocket task.
    ///
    /// The handle becomes inoperable (returns [`Error::ChannelClosed`]) when
    /// this client (or the underlying background task) is dropped.
    pub fn request_handle(&self) -> VmRequestHandle {
        self.handle.clone()
    }

    /// Send a JSON-RPC request and wait for the response.
    ///
    /// Delegates to the internal [`VmRequestHandle`].
    ///
    /// # Errors
    ///
    /// - [`Error::ChannelClosed`] if the background task has exited.
    /// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error.
    /// - [`Error::Daemon`] if the response arrived without a `result` field.
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.handle.request(method, params).await
    }

    /// Return a mutable reference to the stream-event receiver.
    ///
    /// Callers can `recv()` or `try_recv()` on this to consume VM Service
    /// stream events (Extension, Logging, GC, etc.).
    pub fn event_receiver(&mut self) -> &mut mpsc::Receiver<VmServiceEvent> {
        &mut self.event_rx
    }

    /// Return the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.handle.connection_state()
    }

    /// Gracefully close the WebSocket connection.
    ///
    /// Sends a [`ClientCommand::Disconnect`] to the background task and
    /// returns immediately.  The task will send a Close frame and then
    /// terminate.
    pub async fn disconnect(&self) {
        // Ignore the send error — if the channel is already closed the task
        // has already exited.
        let _ = self.handle.cmd_tx.send(ClientCommand::Disconnect).await;
    }

    /// Return `true` if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.handle.is_connected()
    }

    // ── VM introspection methods ──────────────────────────────────────────

    /// Call `getVM` — returns VM info with the list of running isolates.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VmService`] if the response cannot be parsed as
    /// [`VmInfo`], or a transport error if the request fails.
    pub async fn get_vm(&self) -> Result<VmInfo> {
        let result = self.request("getVM", None).await?;
        serde_json::from_value(result)
            .map_err(|e| Error::vm_service(format!("parse getVM response: {e}")))
    }

    /// Call `getIsolate` — returns full isolate details for `isolate_id`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VmService`] if the response cannot be parsed as
    /// [`IsolateInfo`], or a transport error if the request fails.
    pub async fn get_isolate(&self, isolate_id: &str) -> Result<IsolateInfo> {
        let params = serde_json::json!({ "isolateId": isolate_id });
        let result = self.request("getIsolate", Some(params)).await?;
        serde_json::from_value(result)
            .map_err(|e| Error::vm_service(format!("parse getIsolate response: {e}")))
    }

    /// Call `streamListen` — subscribe to a named VM Service stream.
    ///
    /// Common stream IDs: `"Extension"`, `"Logging"`, `"GC"`, `"Isolate"`.
    ///
    /// # Errors
    ///
    /// Returns a transport or protocol error if the request fails.
    pub async fn stream_listen(&self, stream_id: &str) -> Result<()> {
        let params = serde_json::json!({ "streamId": stream_id });
        self.request("streamListen", Some(params)).await?;
        Ok(())
    }

    /// Call `streamCancel` — unsubscribe from a named VM Service stream.
    ///
    /// # Errors
    ///
    /// Returns a transport or protocol error if the request fails.
    pub async fn stream_cancel(&self, stream_id: &str) -> Result<()> {
        let params = serde_json::json!({ "streamId": stream_id });
        self.request("streamCancel", Some(params)).await?;
        Ok(())
    }

    /// Discover the main Flutter UI isolate.
    ///
    /// Calls `getVM`, then finds the first non-system isolate. In a typical
    /// Flutter app this is the isolate named `"main"`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VmService`] if no non-system isolate is found, or a
    /// transport error if `getVM` fails.
    pub async fn discover_main_isolate(&self) -> Result<IsolateRef> {
        let vm = self.get_vm().await?;

        // Find the main isolate — the first non-system isolate.
        // In a typical Flutter app there is exactly one such isolate.
        let main_isolate = vm
            .isolates
            .iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false))
            .ok_or_else(|| Error::vm_service("no non-system isolate found"))?;

        Ok(main_isolate.clone())
    }

    /// Subscribe to Flutter streams (Extension, Logging, and GC).
    ///
    /// Returns a list of human-readable error descriptions for any streams
    /// that could not be subscribed (non-fatal — the app continues without
    /// them).
    pub async fn subscribe_flutter_streams(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Extension stream: Flutter.Error events (widget crash logs)
        if let Err(e) = self.stream_listen("Extension").await {
            errors.push(format!("Extension stream: {e}"));
        }

        // Logging stream: structured log records
        if let Err(e) = self.stream_listen("Logging").await {
            errors.push(format!("Logging stream: {e}"));
        }

        // GC stream: garbage collection events for memory monitoring
        if let Err(e) = self.stream_listen("GC").await {
            errors.push(format!("GC stream: {e}"));
        }

        errors
    }

    // ── Service extension methods ─────────────────────────────────────────

    /// Call a Flutter service extension method.
    ///
    /// Delegates to the internal [`VmRequestHandle`].
    ///
    /// # Errors
    ///
    /// - [`Error::ChannelClosed`] if the background task has exited.
    /// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error,
    ///   including error code `-32601` when the extension is not available.
    /// - [`Error::Daemon`] if the response arrived without a `result` field.
    pub async fn call_extension(
        &self,
        method: &str,
        isolate_id: &str,
        args: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value> {
        self.handle.call_extension(method, isolate_id, args).await
    }

    /// Get the main isolate ID, discovering it if not yet cached.
    ///
    /// Delegates to the internal [`VmRequestHandle`].
    ///
    /// # Errors
    ///
    /// Returns an error if the VM Service call to `getVM` fails or no
    /// non-system isolate is found.
    pub async fn main_isolate_id(&self) -> Result<String> {
        self.handle.main_isolate_id().await
    }
}

// ---------------------------------------------------------------------------
// Background task
// ---------------------------------------------------------------------------

/// Entry point for the background WebSocket I/O task.
///
/// Accepts an already-open `ws_stream` for the first connection, then manages
/// reconnection on unexpected disconnects.
async fn run_client_task(
    ws_uri: String,
    ws_stream: WsStream,
    mut cmd_rx: mpsc::Receiver<ClientCommand>,
    event_tx: mpsc::Sender<VmServiceEvent>,
    state: Arc<std::sync::RwLock<ConnectionState>>,
    isolate_id_cache: Arc<Mutex<Option<String>>>,
) {
    let mut tracker = VmRequestTracker::new();

    // Run the read/write loop with the initial connection.
    let reconnect = run_io_loop(ws_stream, &mut cmd_rx, &event_tx, &mut tracker, false).await;

    if !reconnect {
        // Either we received a Disconnect command or the cmd channel is closed.
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        *guard = ConnectionState::Disconnected;
        return;
    }

    // Connection lost unexpectedly — attempt reconnection with backoff.
    let mut attempt: u32 = 1;
    loop {
        if attempt > MAX_RECONNECT_ATTEMPTS {
            error!(
                "VM Service: exceeded {} reconnection attempts, giving up",
                MAX_RECONNECT_ATTEMPTS
            );
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            *guard = ConnectionState::Disconnected;
            break;
        }

        {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            *guard = ConnectionState::Reconnecting { attempt };
        }

        let backoff = compute_backoff(attempt);
        warn!(
            "VM Service: connection lost, retrying in {:?} (attempt {}/{})",
            backoff, attempt, MAX_RECONNECT_ATTEMPTS
        );
        tokio::time::sleep(backoff).await;

        // Check if the cmd channel has closed while we were sleeping — the
        // client was dropped, no point reconnecting.
        if cmd_rx.is_closed() {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            *guard = ConnectionState::Disconnected;
            break;
        }

        match connect_ws(&ws_uri).await {
            Ok(ws_stream) => {
                info!("VM Service: reconnected (attempt {})", attempt);
                {
                    let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                    *guard = ConnectionState::Connected;
                }

                // Invalidate the isolate ID cache — after a reconnection the
                // VM may have started a new isolate with a different ID.
                {
                    let mut cache = isolate_id_cache.lock().await;
                    *cache = None;
                    debug!("VM Service: cleared isolate ID cache after reconnection");
                }

                attempt = 1; // reset on success

                let reconnect =
                    run_io_loop(ws_stream, &mut cmd_rx, &event_tx, &mut tracker, true).await;
                if !reconnect {
                    let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                    *guard = ConnectionState::Disconnected;
                    break;
                }
                // If run_io_loop returned true again the loop continues and
                // retries.
            }
            Err(err) => {
                warn!(
                    "VM Service: reconnection attempt {} failed: {}",
                    attempt, err
                );
                attempt += 1;
            }
        }
    }

    debug!("VM Service background task exiting");
}

/// Run one connection's read/write select loop.
///
/// Returns `true` if the connection was lost unexpectedly (caller should
/// reconnect), or `false` if the task should terminate (Disconnect command or
/// channel closed).
///
/// When `resubscribe` is `true`, re-subscribes to Extension and Logging
/// streams before entering the main loop (used after reconnection).
async fn run_io_loop(
    ws_stream: WsStream,
    cmd_rx: &mut mpsc::Receiver<ClientCommand>,
    event_tx: &mpsc::Sender<VmServiceEvent>,
    tracker: &mut VmRequestTracker,
    resubscribe: bool,
) -> bool {
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    if resubscribe {
        resubscribe_streams(&mut ws_sink, tracker).await;
    }

    let mut cleanup_interval = tokio::time::interval(STALE_REQUEST_CLEANUP_INTERVAL);
    cleanup_interval.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            // ── Incoming WebSocket message ───────────────────────────────
            frame = ws_stream.next() => {
                match frame {
                    Some(Ok(WsMessage::Text(text))) => {
                        handle_ws_text(text.as_str(), tracker, event_tx).await;
                    }
                    Some(Ok(WsMessage::Close(_))) => {
                        debug!("VM Service: received Close frame");
                        return true; // reconnect
                    }
                    Some(Ok(_)) => {
                        // Ping/Pong/Binary — ignore
                    }
                    Some(Err(err)) => {
                        warn!("VM Service: WebSocket read error: {}", err);
                        return true; // reconnect
                    }
                    None => {
                        debug!("VM Service: WebSocket stream ended");
                        return true; // reconnect
                    }
                }
            }

            // ── Outgoing command from the public API ─────────────────────
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ClientCommand::SendRequest { method, params, response_tx }) => {
                        handle_send_request(
                            &method,
                            params,
                            response_tx,
                            tracker,
                            &mut ws_sink,
                        )
                        .await;
                    }
                    Some(ClientCommand::Disconnect) => {
                        send_close(&mut ws_sink).await;
                        return false; // clean shutdown
                    }
                    None => {
                        // The VmServiceClient was dropped — close gracefully.
                        debug!("VM Service: command channel closed, shutting down");
                        send_close(&mut ws_sink).await;
                        return false;
                    }
                }
            }

            // ── Periodic stale request cleanup ──────────────────────────
            _ = cleanup_interval.tick() => {
                let stale = tracker.cleanup_stale(STALE_REQUEST_TIMEOUT);
                if !stale.is_empty() {
                    debug!(
                        "VM Service: cleaned up {} stale request(s): {:?}",
                        stale.len(),
                        stale,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Establish a new WebSocket connection to `ws_uri`.
async fn connect_ws(ws_uri: &str) -> Result<WsStream> {
    let (ws_stream, _response) = connect_async(ws_uri)
        .await
        .map_err(|err| Error::daemon(format!("Failed to connect to VM Service: {err}")))?;
    Ok(ws_stream)
}

/// Compute exponential backoff duration for reconnection attempt `n`.
///
/// The formula is `INITIAL_BACKOFF * 2^(n-1)`, capped at `MAX_BACKOFF`.
fn compute_backoff(attempt: u32) -> Duration {
    // 2^(attempt-1), capped to avoid overflow.
    // checked_shl returns None if the shift amount >= 64 (or would overflow).
    let exponent = attempt.saturating_sub(1);
    let multiplier: u64 = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    let secs = INITIAL_BACKOFF.as_secs().saturating_mul(multiplier);
    Duration::from_secs(secs.min(MAX_BACKOFF.as_secs()))
}

/// Route an incoming WebSocket text frame to the tracker or event channel.
async fn handle_ws_text(
    text: &str,
    tracker: &mut VmRequestTracker,
    event_tx: &mpsc::Sender<VmServiceEvent>,
) {
    match parse_vm_message(text) {
        VmServiceMessage::Response(mut response) => {
            if let Some(id) = response.id.take() {
                if !tracker.complete(&id, response) {
                    debug!(
                        "VM Service: received response for unknown request id {}",
                        id
                    );
                }
            }
        }
        VmServiceMessage::Event(event) => {
            if let Err(err) = event_tx.try_send(event) {
                warn!(
                    "VM Service: event channel full or closed, dropping event: {}",
                    err
                );
            }
        }
        VmServiceMessage::Unknown(raw) => {
            debug!(
                "VM Service: ignoring unknown message: {}",
                &raw[..raw.len().min(120)]
            );
        }
    }
}

/// Register a pending request in the tracker, serialize it, and write it to
/// the WebSocket sink.  Delivers an error to `response_tx` if serialization
/// or send fails.
async fn handle_send_request(
    method: &str,
    params: Option<serde_json::Value>,
    response_tx: oneshot::Sender<Result<serde_json::Value>>,
    tracker: &mut VmRequestTracker,
    ws_sink: &mut SplitSink<WsStream, WsMessage>,
) {
    // Register a slot in the tracker before touching the wire so the slot
    // exists if the response races the send.
    let (id, response_rx) = tracker.register();
    let request = VmServiceRequest::new(id, method, params);

    let json = match serde_json::to_string(&request) {
        Ok(j) => j,
        Err(err) => {
            let e = Error::protocol(format!("Failed to serialize VM Service request: {err}"));
            let _ = response_tx.send(Err(e));
            return;
        }
    };

    if let Err(err) = ws_sink.send(WsMessage::Text(json.into())).await {
        let e = Error::daemon(format!("Failed to send VM Service request: {err}"));
        let _ = response_tx.send(Err(e));
        return;
    }

    // Spawn a task to wait for the response and forward it to `response_tx`.
    tokio::spawn(async move {
        match response_rx.await {
            Ok(response) => {
                let result = vm_response_to_result(response);
                let _ = response_tx.send(result);
            }
            Err(_) => {
                // oneshot sender in the tracker was dropped (e.g. during stale
                // cleanup or reconnection).
                let _ = response_tx.send(Err(Error::ChannelClosed));
            }
        }
    });
}

/// Convert a [`VmServiceResponse`] to a [`Result<serde_json::Value>`].
fn vm_response_to_result(
    response: super::protocol::VmServiceResponse,
) -> Result<serde_json::Value> {
    if let Some(error) = response.error {
        Err(vm_error_to_error(error))
    } else if let Some(result) = response.result {
        Ok(result)
    } else {
        Err(Error::daemon(
            "VM Service response contained neither result nor error",
        ))
    }
}

/// Convert a [`VmServiceError`] to our domain [`Error`].
fn vm_error_to_error(err: VmServiceError) -> Error {
    Error::protocol(format!("VM Service error {}: {}", err.code, err.message))
}

/// Send a WebSocket Close frame, ignoring any write errors.
async fn send_close(ws_sink: &mut SplitSink<WsStream, WsMessage>) {
    let _ = ws_sink.send(WsMessage::Close(None)).await;
    let _ = ws_sink.close().await;
}

/// Re-subscribe to Extension and Logging streams after a reconnection.
///
/// After a WebSocket reconnect, the VM Service connection is fresh with no
/// active subscriptions. This sends `streamListen` requests directly on the
/// raw stream to restore them before entering the I/O loop.
///
/// Responses are registered in the tracker but not awaited — they will be
/// routed by the subsequent `run_io_loop`.
async fn resubscribe_streams(
    ws_sink: &mut SplitSink<WsStream, WsMessage>,
    tracker: &mut VmRequestTracker,
) {
    for stream_id in RESUBSCRIBE_STREAMS {
        let (id, _response_rx) = tracker.register();
        let request = VmServiceRequest::new(
            id,
            "streamListen",
            Some(serde_json::json!({ "streamId": stream_id })),
        );
        match serde_json::to_string(&request) {
            Ok(json) => {
                if let Err(err) = ws_sink.send(WsMessage::Text(json.into())).await {
                    warn!(
                        "VM Service: failed to re-subscribe to '{}' stream: {}",
                        stream_id, err
                    );
                } else {
                    debug!("VM Service: re-subscribed to '{}' stream", stream_id);
                }
            }
            Err(err) => {
                warn!(
                    "VM Service: failed to serialize streamListen for '{}': {}",
                    stream_id, err
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- ConnectionState -----------------------------------------------------

    #[test]
    fn test_connection_state_eq() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_ne!(ConnectionState::Connected, ConnectionState::Disconnected);
        assert_eq!(
            ConnectionState::Reconnecting { attempt: 1 },
            ConnectionState::Reconnecting { attempt: 1 }
        );
        assert_ne!(
            ConnectionState::Reconnecting { attempt: 1 },
            ConnectionState::Reconnecting { attempt: 2 }
        );
    }

    #[test]
    fn test_connection_state_clone() {
        let state = ConnectionState::Reconnecting { attempt: 3 };
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    // -- compute_backoff -----------------------------------------------------

    #[test]
    fn test_reconnection_backoff_calculation_first_attempt() {
        // 1s * 2^0 = 1s
        assert_eq!(compute_backoff(1), Duration::from_secs(1));
    }

    #[test]
    fn test_reconnection_backoff_calculation_second_attempt() {
        // 1s * 2^1 = 2s
        assert_eq!(compute_backoff(2), Duration::from_secs(2));
    }

    #[test]
    fn test_reconnection_backoff_calculation_third_attempt() {
        // 1s * 2^2 = 4s
        assert_eq!(compute_backoff(3), Duration::from_secs(4));
    }

    #[test]
    fn test_reconnection_backoff_calculation_fourth_attempt() {
        // 1s * 2^3 = 8s
        assert_eq!(compute_backoff(4), Duration::from_secs(8));
    }

    #[test]
    fn test_reconnection_backoff_calculation_fifth_attempt() {
        // 1s * 2^4 = 16s
        assert_eq!(compute_backoff(5), Duration::from_secs(16));
    }

    #[test]
    fn test_reconnection_backoff_capped_at_max() {
        // 1s * 2^5 = 32s → capped at 30s
        assert_eq!(compute_backoff(6), MAX_BACKOFF);
        // Higher attempts should also return MAX_BACKOFF
        assert_eq!(compute_backoff(10), MAX_BACKOFF);
        assert_eq!(compute_backoff(MAX_RECONNECT_ATTEMPTS), MAX_BACKOFF);
    }

    #[test]
    fn test_reconnection_backoff_large_attempt_does_not_overflow() {
        // Very large attempt numbers should not panic or overflow.
        let dur = compute_backoff(u32::MAX);
        assert_eq!(dur, MAX_BACKOFF);
    }

    // -- vm_response_to_result -----------------------------------------------

    #[test]
    fn test_vm_response_to_result_success() {
        let response = super::super::protocol::VmServiceResponse {
            id: Some("1".to_string()),
            result: Some(serde_json::json!({ "type": "VM" })),
            error: None,
        };
        let result = vm_response_to_result(response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["type"], "VM");
    }

    #[test]
    fn test_vm_response_to_result_error() {
        let response = super::super::protocol::VmServiceResponse {
            id: Some("2".to_string()),
            result: None,
            error: Some(VmServiceError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        };
        let result = vm_response_to_result(response);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Method not found"));
    }

    #[test]
    fn test_vm_response_to_result_neither() {
        let response = super::super::protocol::VmServiceResponse {
            id: Some("3".to_string()),
            result: None,
            error: None,
        };
        let result = vm_response_to_result(response);
        assert!(result.is_err());
    }

    // -- vm_error_to_error ---------------------------------------------------

    #[test]
    fn test_vm_error_to_error_contains_code_and_message() {
        let err = VmServiceError {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        };
        let domain_err = vm_error_to_error(err);
        let msg = domain_err.to_string();
        assert!(msg.contains("-32700"));
        assert!(msg.contains("Parse error"));
    }

    // -- Introspection helpers (unit-testable logic) -------------------------

    /// Build a [`VmInfo`] fixture with a mix of system and non-system isolates.
    fn make_vm_info(
        isolates: Vec<super::super::protocol::IsolateRef>,
    ) -> super::super::protocol::VmInfo {
        super::super::protocol::VmInfo {
            name: "vm".to_string(),
            version: "3.0.0".to_string(),
            isolates,
            isolate_groups: None,
        }
    }

    fn make_isolate_ref(
        id: &str,
        name: &str,
        is_system: Option<bool>,
    ) -> super::super::protocol::IsolateRef {
        super::super::protocol::IsolateRef {
            id: id.to_string(),
            name: name.to_string(),
            number: None,
            is_system_isolate: is_system,
        }
    }

    #[test]
    fn test_discover_main_isolate_skips_system_isolates() {
        // Simulate the discovery logic used in discover_main_isolate().
        let isolates = vec![
            make_isolate_ref("isolates/vm-service", "vm-service", Some(true)),
            make_isolate_ref("isolates/1", "main", Some(false)),
        ];
        let vm = make_vm_info(isolates);

        // The same logic as discover_main_isolate but synchronous.
        let found = vm
            .isolates
            .iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false));

        assert!(found.is_some(), "should find a non-system isolate");
        let isolate = found.unwrap();
        assert_eq!(isolate.id, "isolates/1");
        assert_eq!(isolate.name, "main");
    }

    #[test]
    fn test_discover_main_isolate_returns_error_when_none() {
        // All isolates are system isolates — discovery should fail.
        let isolates = vec![
            make_isolate_ref("isolates/vm-service", "vm-service", Some(true)),
            make_isolate_ref("isolates/kernel", "kernel-service", Some(true)),
        ];
        let vm = make_vm_info(isolates);

        let found = vm
            .isolates
            .iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false));

        assert!(
            found.is_none(),
            "should return None when all isolates are system isolates"
        );

        // Verify that the error construction works correctly.
        let err = Error::vm_service("no non-system isolate found");
        assert!(err.to_string().contains("no non-system isolate found"));
    }

    #[test]
    fn test_discover_main_isolate_treats_missing_flag_as_non_system() {
        // An isolate with no is_system_isolate field should be treated as non-system.
        let isolates = vec![
            make_isolate_ref("isolates/vm-service", "vm-service", Some(true)),
            make_isolate_ref("isolates/1", "main", None), // no flag → non-system
        ];
        let vm = make_vm_info(isolates);

        let found = vm
            .isolates
            .iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false));

        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "isolates/1");
    }

    #[test]
    fn test_get_vm_request_format() {
        // Verify that a getVM request serializes to the expected JSON-RPC shape.
        let req = VmServiceRequest::new("10".to_string(), "getVM", None);
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

        assert_eq!(val["jsonrpc"], "2.0");
        assert_eq!(val["id"], "10");
        assert_eq!(val["method"], "getVM");
        // getVM takes no parameters — params key must be absent.
        assert!(
            !val.as_object().unwrap().contains_key("params"),
            "getVM should send no params"
        );
    }

    #[test]
    fn test_stream_listen_request_format() {
        // Verify streamListen sends the correct streamId param.
        let params = serde_json::json!({ "streamId": "Extension" });
        let req = VmServiceRequest::new("11".to_string(), "streamListen", Some(params));
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

        assert_eq!(val["method"], "streamListen");
        assert_eq!(val["params"]["streamId"], "Extension");
    }

    #[test]
    fn test_stream_cancel_request_format() {
        // Verify streamCancel sends the correct streamId param.
        let params = serde_json::json!({ "streamId": "Logging" });
        let req = VmServiceRequest::new("12".to_string(), "streamCancel", Some(params));
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

        assert_eq!(val["method"], "streamCancel");
        assert_eq!(val["params"]["streamId"], "Logging");
    }

    #[test]
    fn test_get_isolate_request_format() {
        // Verify getIsolate sends the correct isolateId param.
        let isolate_id = "isolates/42";
        let params = serde_json::json!({ "isolateId": isolate_id });
        let req = VmServiceRequest::new("13".to_string(), "getIsolate", Some(params));
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

        assert_eq!(val["method"], "getIsolate");
        assert_eq!(val["params"]["isolateId"], "isolates/42");
    }

    #[test]
    fn test_vm_service_error_variant() {
        // Confirm the Error::VmService variant and constructor are present.
        let err = Error::vm_service("test error message");
        assert!(err.to_string().contains("test error message"));
        // Should not be fatal (VM Service errors are recoverable connection issues).
        assert!(!err.is_fatal());
    }

    // -- Client command serialization (via VmServiceRequest) ----------------

    #[test]
    fn test_vm_service_request_produces_valid_json_rpc() {
        let req = VmServiceRequest::new("42".to_string(), "getVM", None);
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

        assert_eq!(val["jsonrpc"], "2.0");
        assert_eq!(val["id"], "42");
        assert_eq!(val["method"], "getVM");
        // params should be absent when None
        assert!(!val.as_object().unwrap().contains_key("params"));
    }

    #[test]
    fn test_vm_service_request_with_params_produces_valid_json_rpc() {
        let params = serde_json::json!({ "streamId": "Extension" });
        let req = VmServiceRequest::new("7".to_string(), "streamListen", Some(params));
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

        assert_eq!(val["method"], "streamListen");
        assert_eq!(val["params"]["streamId"], "Extension");
    }

    // -- VmRequestHandle tests -----------------------------------------------

    #[test]
    fn test_request_handle_is_clone() {
        // VmRequestHandle must be Clone for Message derive
        fn assert_clone<T: Clone>() {}
        assert_clone::<VmRequestHandle>();
    }

    #[test]
    fn test_request_handle_is_debug() {
        // VmRequestHandle must be Debug for Message derive
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<VmRequestHandle>();
    }

    #[tokio::test]
    async fn test_handle_channel_closed_after_drop() {
        // Create a mock channel and handle
        let (cmd_tx, cmd_rx) = mpsc::channel::<ClientCommand>(1);
        let handle = VmRequestHandle {
            cmd_tx,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::new(Mutex::new(None)),
        };
        // Drop the receiver to simulate disconnection
        drop(cmd_rx);
        let result = handle.request("getVM", None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_request_handle_debug_shows_state() {
        let handle = VmRequestHandle {
            cmd_tx: mpsc::channel::<ClientCommand>(1).0,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::new(Mutex::new(None)),
        };
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("VmRequestHandle"));
        assert!(debug_str.contains("Connected"));
    }

    #[test]
    fn test_request_handle_clone_shares_state() {
        let state = Arc::new(std::sync::RwLock::new(ConnectionState::Connected));
        let handle = VmRequestHandle {
            cmd_tx: mpsc::channel::<ClientCommand>(1).0,
            state: Arc::clone(&state),
            isolate_id_cache: Arc::new(Mutex::new(None)),
        };
        let cloned = handle.clone();

        // Both handles see the same state
        assert!(handle.is_connected());
        assert!(cloned.is_connected());

        // Mutate via the Arc and both see the update
        {
            let mut guard = state.write().unwrap();
            *guard = ConnectionState::Disconnected;
        }
        assert!(!handle.is_connected());
        assert!(!cloned.is_connected());
    }

    #[test]
    fn test_request_handle_is_send_sync() {
        // VmRequestHandle must be Send + Sync for use in background tasks
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VmRequestHandle>();
    }

    // -- invalidate_isolate_cache --------------------------------------------

    #[tokio::test]
    async fn test_invalidate_isolate_cache_clears_cached_value() {
        // Pre-populate the cache with a known isolate ID.
        let isolate_id_cache = Arc::new(Mutex::new(Some("isolates/12345".to_string())));

        let handle = VmRequestHandle {
            cmd_tx: mpsc::channel::<ClientCommand>(1).0,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::clone(&isolate_id_cache),
        };

        // Confirm the cache has a value.
        {
            let guard = isolate_id_cache.lock().await;
            assert_eq!(*guard, Some("isolates/12345".to_string()));
        }

        // Invalidate the cache.
        handle.invalidate_isolate_cache();

        // The cache should now be None.
        let guard = isolate_id_cache.lock().await;
        assert!(
            guard.is_none(),
            "cache should be cleared after invalidation"
        );
    }

    #[tokio::test]
    async fn test_invalidate_isolate_cache_is_idempotent_when_already_empty() {
        // Cache starts empty — invalidating should be a no-op without panic.
        let isolate_id_cache: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let handle = VmRequestHandle {
            cmd_tx: mpsc::channel::<ClientCommand>(1).0,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::clone(&isolate_id_cache),
        };

        handle.invalidate_isolate_cache();

        let guard = isolate_id_cache.lock().await;
        assert!(
            guard.is_none(),
            "cache should remain None after invalidating an already-empty cache"
        );
    }

    #[tokio::test]
    async fn test_invalidate_isolate_cache_shared_across_clones() {
        // All clones of a VmRequestHandle share the same Arc<Mutex<...>>,
        // so invalidation via one clone is visible from another.
        let isolate_id_cache = Arc::new(Mutex::new(Some("isolates/99".to_string())));

        let handle = VmRequestHandle {
            cmd_tx: mpsc::channel::<ClientCommand>(1).0,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::clone(&isolate_id_cache),
        };
        let cloned = handle.clone();

        // Invalidate via the original handle.
        handle.invalidate_isolate_cache();

        // The cloned handle should see the same cleared cache.
        let guard = isolate_id_cache.lock().await;
        assert!(
            guard.is_none(),
            "cloned handle should observe cache cleared by original"
        );
        // Drop the guard before the cloned handle goes out of scope.
        drop(guard);
        drop(cloned);
    }
}
