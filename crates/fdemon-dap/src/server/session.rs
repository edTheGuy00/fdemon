//! # DAP Client Session State Machine
//!
//! Manages the lifecycle of a single DAP client connection. Each connected
//! client gets its own [`DapClientSession`] instance that runs independently
//! in a spawned Tokio task.
//!
//! ## Session State Machine
//!
//! ```text
//! Uninitialized → Initializing → Configured → Attached → Disconnecting
//!                                     ↓
//!                               (attach/disconnect)
//! ```
//!
//! Phase 2 handles the initialization handshake. Phase 3 wires the `attach`
//! command to a [`DapAdapter`] via the [`DebugBackend`] trait so that
//! debugging commands are dispatched to the actual VM Service.
//!
//! ## Out-of-order request handling
//!
//! The session validates state transitions at every step. Sending
//! `configurationDone` before `initialize`, or calling `initialize` twice,
//! results in an error response rather than a panic or silent no-op.
//!
//! ## Generics
//!
//! `DapClientSession<B>` is generic over the debug backend. When no real VM
//! Service is available (tests, TCP server without attached Flutter session),
//! use [`NoopBackend`] as the type argument. Backends are provided at session
//! construction time via [`DapClientSession::with_backend`].

use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader},
    sync::{broadcast, mpsc, watch},
};

use fdemon_core::error::Result;

use crate::{
    adapter::{DapAdapter, DebugBackend, DebugEvent},
    read_message, write_message, Capabilities, DapEvent, DapMessage, DapRequest, DapResponse,
    InitializeRequestArguments,
};

// ─────────────────────────────────────────────────────────────────────────────
// Timeout constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum time to wait for the `initialize` request after a client connects.
///
/// VS Code typically sends `initialize` within 100 ms of connecting. This 30 s
/// timeout primarily catches broken or abandoned connections (e.g., a port
/// scanner that opens a TCP connection but never sends any data).
const INIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Idle timeout for sessions that are not in the `Attached` state.
///
/// Sessions in `Initializing` or `Configured` states (i.e., clients that
/// performed the DAP handshake but never attached a debuggee, or are stuck
/// in the configuration phase) are closed after this duration of inactivity.
///
/// Active `Attached` sessions are **not** affected — a debuggee may be running
/// silently for extended periods, which is not a sign of abandonment.
const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

// ─────────────────────────────────────────────────────────────────────────────
// DebugEventSource
// ─────────────────────────────────────────────────────────────────────────────

/// Abstracts over the two channel types used to deliver [`DebugEvent`]s to
/// the session select loop.
///
/// - [`Dedicated`][Self::Dedicated] wraps an `mpsc::Receiver` provided by a
///   real backend factory (task 01). Each client gets its own channel, so
///   there is no backpressure sharing and no `Lagged` error.
/// - [`Broadcast`][Self::Broadcast] wraps an optional `broadcast::Receiver`
///   used when the Engine fans out log events to all connected clients. The
///   `Option` wrapper lets the branch be permanently disabled once the sender
///   is dropped, preventing a CPU busy-spin on `RecvError::Closed`.
/// - [`None`][Self::None] has no event source; the select arm parks on
///   `std::future::pending()` so it never wakes.
enum DebugEventSource {
    /// Per-client channel from a backend factory (no `Lagged` errors possible).
    Dedicated(mpsc::Receiver<DebugEvent>),
    /// Shared broadcast channel (Engine fan-out). `None` after sender drops.
    Broadcast(Option<broadcast::Receiver<DebugEvent>>),
    /// No event source — select arm is permanently parked.
    None,
}

impl DebugEventSource {
    /// Await the next [`DebugEvent`], or return `Option::None` on lag/close.
    ///
    /// - `Dedicated`: yields `Some(event)` or `None` when the sender drops.
    /// - `Broadcast(Some)`: yields `Some(event)`, logs a warning on lag, and
    ///   transitions to `None` on close (preventing busy-spin).
    /// - `Broadcast(None)` / `None`: parks on [`std::future::pending`] forever.
    async fn recv(&mut self) -> Option<DebugEvent> {
        match self {
            Self::Dedicated(rx) => rx.recv().await,
            Self::Broadcast(Some(rx)) => match rx.recv().await {
                Ok(event) => Some(event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("DAP log event receiver lagged, dropped {} events", n);
                    // Continue the loop; try the next event.
                    Option::None
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::debug!("DAP log event broadcast channel closed, disabling");
                    // Disable permanently so the select arm parks instead of spinning.
                    *self = Self::None;
                    Option::None
                }
            },
            Self::Broadcast(Option::None) | Self::None => {
                std::future::pending::<Option<DebugEvent>>().await
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// State machine
// ─────────────────────────────────────────────────────────────────────────────

/// State of a DAP client session.
///
/// The DAP specification defines this initialization sequence:
///
/// 1. Client sends `initialize` → Server responds with capabilities + `initialized` event
/// 2. Client sends `attach` (or `launch`) → Server responds
/// 3. Client sends configuration requests (`setBreakpoints`, etc.)
/// 4. Client sends `configurationDone` → Server responds
///
/// Note: `attach`/`launch` comes **before** `configurationDone` in the spec.
/// Some adapters accept them in either order; we follow the spec ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Client connected but has not yet sent `initialize`.
    Uninitialized,
    /// `initialize` received; ready for `attach`/`launch` and configuration requests.
    Initializing,
    /// `configurationDone` received (configuration phase complete).
    Configured,
    /// `attach` succeeded; actively debugging via the `DapAdapter`.
    Attached,
    /// Client is disconnecting; session will terminate after sending the response.
    Disconnecting,
}

// ─────────────────────────────────────────────────────────────────────────────
// NoopBackend
// ─────────────────────────────────────────────────────────────────────────────

/// A no-operation [`DebugBackend`] that returns errors for all operations.
///
/// Used as the default backend when no real VM Service connection is
/// available (e.g., the TCP server before a Flutter session is attached,
/// or in tests that only exercise the initialization handshake).
#[derive(Clone, Debug, Default)]
pub struct NoopBackend;

impl crate::adapter::DebugBackend for NoopBackend {
    async fn pause(
        &self,
        _isolate_id: &str,
    ) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn resume(
        &self,
        _isolate_id: &str,
        _step: Option<crate::adapter::StepMode>,
        _frame_index: Option<i32>,
    ) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn add_breakpoint(
        &self,
        _isolate_id: &str,
        _uri: &str,
        _line: i32,
        _column: Option<i32>,
    ) -> std::result::Result<crate::adapter::BreakpointResult, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn remove_breakpoint(
        &self,
        _isolate_id: &str,
        _breakpoint_id: &str,
    ) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn set_exception_pause_mode(
        &self,
        _isolate_id: &str,
        _mode: crate::adapter::DapExceptionPauseMode,
    ) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_object(
        &self,
        _isolate_id: &str,
        _object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn evaluate_in_frame(
        &self,
        _isolate_id: &str,
        _frame_index: i32,
        _expression: &str,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_vm(&self) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_isolate(
        &self,
        _isolate_id: &str,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_scripts(
        &self,
        _isolate_id: &str,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn call_service(
        &self,
        _method: &str,
        _params: Option<serde_json::Value>,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn set_library_debuggable(
        &self,
        _isolate_id: &str,
        _library_id: &str,
        _is_debuggable: bool,
    ) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_source_report(
        &self,
        _isolate_id: &str,
        _script_id: &str,
        _report_kinds: &[&str],
        _token_pos: Option<i64>,
        _end_token_pos: Option<i64>,
    ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn get_source(
        &self,
        _isolate_id: &str,
        _script_id: &str,
    ) -> std::result::Result<String, crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn hot_reload(&self) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn hot_restart(&self) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn stop_app(&self) -> std::result::Result<(), crate::adapter::BackendError> {
        Err(crate::adapter::BackendError::NotConnected)
    }

    async fn ws_uri(&self) -> Option<String> {
        None
    }

    async fn device_id(&self) -> Option<String> {
        None
    }

    async fn build_mode(&self) -> String {
        "debug".to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Session
// ─────────────────────────────────────────────────────────────────────────────

/// Manages a single DAP client connection.
///
/// Generic over the debug backend `B` so that test code can use
/// [`NoopBackend`] while production code provides a [`VmServiceBackend`]
/// (defined in `fdemon-app`).
///
/// # Lifecycle
///
/// 1. Construct with [`DapClientSession::new`] (uses [`NoopBackend`]) or
///    [`DapClientSession::with_backend`] (real backend).
/// 2. Call [`DapClientSession::run_on`] or [`DapClientSession::run`] to
///    start the read/write loop.
pub struct DapClientSession<B: DebugBackend = NoopBackend> {
    /// Current state in the DAP initialization handshake.
    pub(crate) state: SessionState,
    /// Monotonic sequence number for the next server-sent message.
    next_seq: i64,
    /// Client capabilities received during the `initialize` request.
    client_info: Option<InitializeRequestArguments>,
    /// The adapter instance (created on `attach`).
    adapter: Option<DapAdapter<B>>,
    /// Channel for outbound DAP events produced by the adapter.
    event_tx: Option<mpsc::Sender<DapMessage>>,
    /// The backend to give to the adapter on `attach`.
    backend: Option<B>,
    /// When `Some`, the client must provide this exact token in the
    /// `authToken` field of its `initialize` request.  `None` means auth
    /// is disabled and any client is accepted.
    auth_token: Option<String>,
}

impl DapClientSession<NoopBackend> {
    /// Create a new session in the `Uninitialized` state with a [`NoopBackend`].
    ///
    /// Prefer [`DapClientSession::with_backend`] when a real VM Service
    /// connection is available.
    pub fn new() -> Self {
        Self {
            state: SessionState::Uninitialized,
            next_seq: 1,
            client_info: None,
            adapter: None,
            event_tx: None,
            backend: Some(NoopBackend),
            auth_token: None,
        }
    }
}

impl<B: DebugBackend> DapClientSession<B> {
    /// Create a new session with a real debug backend.
    ///
    /// The backend is consumed on the first `attach` request to construct the
    /// [`DapAdapter`]. After `attach`, the `backend` field is `None` (the
    /// adapter owns it). Sending a second `attach` while already `Attached`
    /// returns an error response.
    pub fn with_backend(backend: B) -> Self {
        Self {
            state: SessionState::Uninitialized,
            next_seq: 1,
            client_info: None,
            adapter: None,
            event_tx: None,
            backend: Some(backend),
            auth_token: None,
        }
    }

    /// Run the session on any async reader/writer pair, with a real backend.
    ///
    /// This is the entry point used when a debug backend is available (e.g.,
    /// when the VM Service is connected). Reads DAP messages from `reader`,
    /// dispatches to state-machine handlers, and writes responses/events back to
    /// `writer`. Also polls `debug_event_rx` for VM Service debug events and
    /// forwards them to the adapter.
    ///
    /// The loop exits when:
    ///
    /// - The client sends a `disconnect` request (state becomes `Disconnecting`).
    /// - The client closes the connection (EOF from `reader`).
    /// - A read error occurs.
    /// - A shutdown signal is received on `shutdown_rx`.
    ///
    /// On shutdown, a `terminated` event is sent before the connection closes.
    ///
    /// # Type Parameters
    ///
    /// - `R` — Any async reader (e.g., `BufReader<OwnedReadHalf>`, `BufReader<Stdin>`).
    /// - `W` — Any async writer (e.g., `OwnedWriteHalf`, `BufWriter<Stdout>`).
    pub async fn run_on_with_backend<R, W>(
        mut reader: BufReader<R>,
        mut writer: W,
        mut shutdown_rx: watch::Receiver<bool>,
        backend: B,
        debug_event_rx: mpsc::Receiver<DebugEvent>,
        auth_token: Option<String>,
    ) -> Result<()>
    where
        R: AsyncRead + Unpin + Send,
        W: AsyncWrite + Unpin + Send,
    {
        let mut session = Self::with_backend(backend);
        session.auth_token = auth_token;

        // Channel for adapter-generated DAP events (stopped, continued, thread, etc.)
        let (event_tx, mut event_rx) = mpsc::channel::<DapMessage>(64);
        session.event_tx = Some(event_tx);

        let mut debug_events = DebugEventSource::Dedicated(debug_event_rx);
        session
            .run_inner(
                &mut reader,
                &mut writer,
                &mut shutdown_rx,
                &mut event_rx,
                &mut debug_events,
            )
            .await
    }

    /// Unified select loop shared by all entry points.
    ///
    /// Drives four concurrent arms:
    ///
    /// 1. Inbound DAP requests from the client.
    /// 2. Outbound DAP events produced by the [`DapAdapter`] (stamped with a
    ///    monotonic sequence number and forwarded to the writer).
    /// 3. Debug events from the engine/VM Service, routed through a
    ///    [`DebugEventSource`] that abstracts over `mpsc` and `broadcast`
    ///    channel types.
    /// 4. Server shutdown signal from a [`watch::Receiver<bool>`].
    ///
    /// ## Connection timeout
    ///
    /// If the client does not send an `initialize` request within [`INIT_TIMEOUT`]
    /// (30 s), the connection is closed cleanly. This catches broken or abandoned
    /// connections (e.g., port scanners) that never send any DAP data.
    ///
    /// The loop exits when the client disconnects, an unrecoverable I/O error
    /// occurs, or a shutdown signal is received.
    async fn run_inner<R, W>(
        &mut self,
        reader: &mut BufReader<R>,
        writer: &mut W,
        shutdown_rx: &mut watch::Receiver<bool>,
        event_rx: &mut mpsc::Receiver<DapMessage>,
        debug_events: &mut DebugEventSource,
    ) -> Result<()>
    where
        R: AsyncRead + Unpin + Send,
        W: AsyncWrite + Unpin + Send,
    {
        // Connection timeout: track when we last received *any* message so we
        // can close connections that never send `initialize`. The deadline is
        // only active while in the `Uninitialized` state.
        let init_deadline = tokio::time::Instant::now() + INIT_TIMEOUT;

        // Idle timeout: track the last time any message was received from the
        // client.  Only sessions in non-`Attached` states are disconnected on
        // idle — an attached session's debuggee may legitimately be quiet.
        let mut last_activity = tokio::time::Instant::now();

        loop {
            // Compute the remaining time until the init deadline.
            // Once the session transitions out of `Uninitialized`, the timeout
            // is effectively disabled (the deadline has passed or we skip it).
            let init_timed_out = self.state == SessionState::Uninitialized
                && tokio::time::Instant::now() >= init_deadline;

            if init_timed_out {
                tracing::warn!(
                    "DAP client did not send initialize within {}s, closing connection",
                    INIT_TIMEOUT.as_secs()
                );
                return Ok(());
            }

            // Build a sleep future that fires at the init deadline (only
            // meaningful while still Uninitialized).
            let init_sleep = tokio::time::sleep_until(init_deadline);

            // Build the idle sleep deadline.  Fires when `last_activity + IDLE_TIMEOUT`
            // is reached; only effective when the session is not `Attached`.
            let idle_deadline = last_activity + IDLE_TIMEOUT;
            let idle_sleep = tokio::time::sleep_until(idle_deadline);

            tokio::select! {
                result = read_message(reader) => {
                    // Update activity timestamp on every received message.
                    last_activity = tokio::time::Instant::now();
                    match result {
                        Ok(Some(DapMessage::Request(req))) => {
                            tracing::debug!(
                                "DAP ← {} (seq={})",
                                req.command,
                                req.seq,
                            );
                            let responses = self.handle_request(&req).await;
                            for msg in &responses {
                                match msg {
                                    DapMessage::Response(resp) => {
                                        tracing::debug!(
                                            "DAP → response {} (req_seq={}, success={})",
                                            resp.command,
                                            resp.request_seq,
                                            resp.success,
                                        );
                                    }
                                    DapMessage::Event(evt) => {
                                        tracing::debug!(
                                            "DAP → event {}",
                                            evt.event,
                                        );
                                    }
                                    _ => {}
                                }
                                write_message(writer, msg).await?;
                            }
                            if self.state == SessionState::Disconnecting {
                                break;
                            }
                        }
                        Ok(Some(_)) => {
                            // Ignore non-request messages from the client.
                            tracing::debug!("Ignoring non-request DAP message from client");
                        }
                        Ok(None) => {
                            tracing::debug!("DAP client disconnected (EOF)");
                            break;
                        }
                        Err(e) => {
                            tracing::warn!("Error reading DAP message: {}", e);
                            break;
                        }
                    }
                }

                // DAP events produced by the adapter (stopped, continued, thread, etc.)
                Some(event_msg) = event_rx.recv() => {
                    // Stamp the event with the next sequence number.
                    let stamped = match event_msg {
                        DapMessage::Event(mut ev) => {
                            ev.seq = self.next_seq;
                            self.next_seq += 1;
                            DapMessage::Event(ev)
                        }
                        other => other,
                    };
                    if let DapMessage::Event(ref ev) = stamped {
                        tracing::debug!("DAP → async event {} (seq={})", ev.event, ev.seq);
                    }
                    if let Err(e) = write_message(writer, &stamped).await {
                        tracing::warn!("Error writing DAP event: {}", e);
                        break;
                    }
                }

                // Debug events from the engine/VM Service, via DebugEventSource.
                // When the source is exhausted (None/Broadcast(None)), this arm
                // parks on std::future::pending() — no busy-spin.
                maybe_event = debug_events.recv() => {
                    if let Some(debug_event) = maybe_event {
                        if let Some(adapter) = &mut self.adapter {
                            adapter.handle_debug_event(debug_event).await;
                        }
                        // If no adapter yet (not attached), silently discard.
                    }
                }

                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::debug!("DAP session terminating due to server shutdown");
                        let event = self.make_event(DapEvent::terminated());
                        let _ = write_message(writer, &DapMessage::Event(event)).await;
                        break;
                    }
                }

                // Connection initialization timeout: only fires when still Uninitialized.
                _ = init_sleep, if self.state == SessionState::Uninitialized => {
                    tracing::warn!(
                        "DAP client did not send initialize within {}s, closing connection",
                        INIT_TIMEOUT.as_secs()
                    );
                    return Ok(());
                }

                // Idle timeout: disconnect sessions that are not actively debugging.
                // The guard ensures `Attached` sessions (which may be waiting for the
                // debuggee to hit a breakpoint) are never affected.
                _ = idle_sleep, if self.state != SessionState::Attached
                                  && self.state != SessionState::Uninitialized
                                  && self.state != SessionState::Disconnecting => {
                    tracing::warn!(
                        "DAP session idle for {:?}, closing (state: {:?})",
                        IDLE_TIMEOUT,
                        self.state
                    );
                    break;
                }
            }
        }

        Ok(())
    }
}

impl DapClientSession<NoopBackend> {
    /// Run the session on any async reader/writer pair (no real backend).
    ///
    /// This is the generalized entry point for the basic TCP server and stdio
    /// transport where no VM Service connection is available. Reads DAP
    /// messages from `reader`, dispatches to state-machine handlers, and
    /// writes responses/events back to `writer`. The loop exits when:
    ///
    /// - The client sends a `disconnect` request (state becomes `Disconnecting`).
    /// - The client closes the connection (EOF from `reader`).
    /// - A read error occurs.
    /// - A shutdown signal is received on `shutdown_rx`.
    ///
    /// On shutdown, a `terminated` event is sent before the connection closes.
    ///
    /// The `log_event_rx` broadcast receiver is polled for [`DebugEvent`]s
    /// forwarded by the Engine (e.g., [`DebugEvent::LogOutput`]). Events are
    /// forwarded to the adapter if it has been created (i.e., after a successful
    /// `attach`). If no adapter is active, events are silently discarded. Once
    /// the sender is dropped, the branch parks on [`std::future::pending`] to
    /// prevent a CPU busy-spin on the permanently-ready `RecvError::Closed`.
    ///
    /// # Type Parameters
    ///
    /// - `R` — Any async reader (e.g., `BufReader<OwnedReadHalf>`, `BufReader<Stdin>`).
    /// - `W` — Any async writer (e.g., `OwnedWriteHalf`, `BufWriter<Stdout>`).
    pub async fn run_on<R, W>(
        mut reader: BufReader<R>,
        mut writer: W,
        mut shutdown_rx: watch::Receiver<bool>,
        log_event_rx: broadcast::Receiver<DebugEvent>,
        auth_token: Option<String>,
    ) -> Result<()>
    where
        R: AsyncRead + Unpin + Send,
        W: AsyncWrite + Unpin + Send,
    {
        let mut session = Self::new();
        session.auth_token = auth_token;

        // Channel for adapter-generated DAP events (if adapter is created).
        let (event_tx, mut event_rx) = mpsc::channel::<DapMessage>(64);
        session.event_tx = Some(event_tx);

        let mut debug_events = DebugEventSource::Broadcast(Some(log_event_rx));
        session
            .run_inner(
                &mut reader,
                &mut writer,
                &mut shutdown_rx,
                &mut event_rx,
                &mut debug_events,
            )
            .await
    }

    /// Run the session until the client disconnects or shutdown is signalled.
    ///
    /// This is a convenience wrapper around [`DapClientSession::run_on`] for TCP
    /// connections. Splits the stream into owned read/write halves, wraps the
    /// reader in a [`BufReader`], then delegates to `run_on`.
    ///
    /// The `log_event_rx` broadcast receiver is polled for [`DebugEvent`]s
    /// forwarded by the Engine (e.g., [`DebugEvent::LogOutput`]). Events are
    /// forwarded to the adapter if it has been created (i.e., after a successful
    /// `attach`). If no adapter is active, events are silently discarded.
    pub async fn run(
        stream: tokio::net::TcpStream,
        shutdown_rx: watch::Receiver<bool>,
        log_event_rx: broadcast::Receiver<DebugEvent>,
        auth_token: Option<String>,
    ) -> Result<()> {
        let (reader, writer) = stream.into_split();
        let reader = BufReader::new(reader);
        Self::run_on(reader, writer, shutdown_rx, log_event_rx, auth_token).await
    }
}

impl<B: DebugBackend> DapClientSession<B> {
    /// Return the client's reported identity (from `initialize`), if available.
    pub fn client_info(&self) -> Option<&InitializeRequestArguments> {
        self.client_info.as_ref()
    }

    // ── Sequence numbering ─────────────────────────────────────────────────

    /// Stamp a [`DapEvent`] with the next monotonic sequence number.
    fn make_event(&mut self, mut event: DapEvent) -> DapEvent {
        event.seq = self.next_seq;
        self.next_seq += 1;
        event
    }

    /// Stamp a [`DapResponse`] with the next monotonic sequence number.
    fn make_response(&mut self, mut response: DapResponse) -> DapResponse {
        response.seq = self.next_seq;
        self.next_seq += 1;
        response
    }

    // ── Request dispatch ───────────────────────────────────────────────────

    /// Handle an incoming DAP request and return the messages to send back.
    ///
    /// Dispatches lifecycle commands (`initialize`, `configurationDone`,
    /// `disconnect`) to the session state machine. All other commands are
    /// delegated to the [`DapAdapter`] once in `Configured` or `Attached`
    /// state.
    pub async fn handle_request(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        match request.command.as_str() {
            "initialize" => self.handle_initialize(request),
            "configurationDone" => self.handle_configuration_done(request),
            "disconnect" => self.handle_disconnect(request).await,

            // All other commands are delegated to the adapter once the
            // session has been initialized (Initializing, Configured, or Attached).
            // Per the DAP spec, `attach`/`launch` comes BEFORE `configurationDone`.
            _ => {
                if self.state == SessionState::Uninitialized {
                    let resp = self.make_response(DapResponse::error(
                        request,
                        "Not ready — send initialize first",
                    ));
                    return vec![DapMessage::Response(resp)];
                }

                // For `attach`, the session also needs to transition state.
                let is_attach = request.command == "attach";

                // Ensure the adapter exists (create on first real command).
                // The adapter is created lazily so that sessions with a
                // NoopBackend still work for the initialization handshake.
                if self.adapter.is_none() {
                    if let Some(backend) = self.backend.take() {
                        let event_tx = self.event_tx.clone().unwrap_or_else(|| {
                            // Create a throwaway channel if no event_tx was set
                            // (e.g., in unit tests using handle_request directly).
                            let (tx, _) = mpsc::channel(1);
                            tx
                        });
                        let (mut adapter, _event_rx) = DapAdapter::new_with_tx(backend, event_tx);

                        // Propagate the client's progress-reporting capability so
                        // that hot reload/restart handlers know whether to emit
                        // progressStart/progressEnd events.
                        let supports_progress = self
                            .client_info
                            .as_ref()
                            .and_then(|ci| ci.supports_progress_reporting)
                            .unwrap_or(false);
                        adapter.set_client_supports_progress(supports_progress);

                        self.adapter = Some(adapter);
                    }
                }

                if let Some(adapter) = &mut self.adapter {
                    let response = adapter.handle_request(request).await;
                    let response = self.make_response(response);

                    // Transition to Attached on successful attach.
                    if is_attach && response.success {
                        self.state = SessionState::Attached;
                    }

                    vec![DapMessage::Response(response)]
                } else {
                    let resp = self
                        .make_response(DapResponse::error(request, "No debug backend available"));
                    vec![DapMessage::Response(resp)]
                }
            }
        }
    }

    // ── Command handlers ───────────────────────────────────────────────────

    /// Handle the `initialize` request.
    fn handle_initialize(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        if self.state != SessionState::Uninitialized {
            let resp = self.make_response(DapResponse::error(request, "initialize already called"));
            return vec![DapMessage::Response(resp)];
        }

        // Parse the client arguments (if any) so we can check the auth token.
        let client_args: Option<InitializeRequestArguments> = request
            .arguments
            .as_ref()
            .and_then(|a| serde_json::from_value(a.clone()).ok());

        // Token validation: when `auth_token` is `Some`, the client must provide
        // the exact same token in `args.authToken`.  A missing or wrong token is
        // immediately rejected to prevent unauthorized code execution.
        if let Some(expected) = &self.auth_token {
            let provided = client_args
                .as_ref()
                .and_then(|a| a.auth_token.as_deref())
                .unwrap_or("");
            if provided != expected.as_str() {
                tracing::warn!("DAP initialize rejected: invalid or missing auth token");
                let resp = self.make_response(DapResponse::error(
                    request,
                    "Authentication failed: invalid or missing auth token",
                ));
                return vec![DapMessage::Response(resp)];
            }
        }

        // Store client capabilities.
        self.client_info = client_args;

        self.state = SessionState::Initializing;

        let capabilities = Capabilities::fdemon_defaults();
        let body = match serde_json::to_value(&capabilities) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to serialize DAP capabilities: {}", e);
                serde_json::Value::Object(Default::default())
            }
        };
        let response = self.make_response(DapResponse::success(request, Some(body)));

        let initialized = self.make_event(DapEvent::initialized());

        vec![
            DapMessage::Response(response),
            DapMessage::Event(initialized),
        ]
    }

    /// Handle the `configurationDone` request.
    ///
    /// Per the DAP spec, `configurationDone` can arrive after `attach`/`launch`
    /// (the common case in VS Code / Zed) or before it. We accept it in
    /// `Initializing` or `Attached` state.
    fn handle_configuration_done(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        if self.state == SessionState::Uninitialized {
            let resp = self.make_response(DapResponse::error(
                request,
                "unexpected configurationDone: call initialize first",
            ));
            return vec![DapMessage::Response(resp)];
        }

        // Only transition to Configured if we haven't attached yet.
        // If already Attached, stay Attached (configurationDone is just an ack).
        if self.state == SessionState::Initializing {
            self.state = SessionState::Configured;
        }
        let resp = self.make_response(DapResponse::success(request, None));
        vec![DapMessage::Response(resp)]
    }

    /// Handle the `disconnect` request.
    ///
    /// Emits a `terminated` event before the disconnect response, as required
    /// by the DAP specification. The `terminated` event signals to the client
    /// that the debug session has ended so it can clean up its debug UI.
    ///
    /// When an adapter is attached, also delegates to [`DapAdapter::handle_request`]
    /// for the `disconnect` command so that `terminateDebuggee` is respected:
    /// the adapter either resumes paused isolates (when `terminateDebuggee: false`)
    /// or stops the Flutter app (when `terminateDebuggee: true`). The `terminated`
    /// event is always emitted by this session handler, not the adapter.
    async fn handle_disconnect(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        self.state = SessionState::Disconnecting;

        // DAP spec: send `terminated` before the disconnect response so the
        // client can transition its UI out of debug mode.
        let terminated = self.make_event(DapEvent::terminated());

        if let Some(adapter) = &mut self.adapter {
            // The adapter handles terminateDebuggee logic (resume isolates or
            // stop the Flutter app). The `terminated` event is always our responsibility.
            let adapter_resp = adapter.handle_request(request).await;
            let resp = self.make_response(adapter_resp);
            vec![DapMessage::Event(terminated), DapMessage::Response(resp)]
        } else {
            // No adapter attached — just return terminated + success.
            let resp = self.make_response(DapResponse::success(request, None));
            vec![DapMessage::Event(terminated), DapMessage::Response(resp)]
        }
    }
}

impl Default for DapClientSession<NoopBackend> {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a request with the given command (no arguments).
    fn req(seq: i64, command: &str) -> DapRequest {
        DapRequest {
            seq,
            command: command.into(),
            arguments: None,
        }
    }

    // Helper: build a request with JSON arguments.
    fn req_with_args(seq: i64, command: &str, args: serde_json::Value) -> DapRequest {
        DapRequest {
            seq,
            command: command.into(),
            arguments: Some(args),
        }
    }

    // ── Initial state ─────────────────────────────────────────────────────────

    #[test]
    fn test_session_starts_uninitialized() {
        let session = DapClientSession::new();
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    #[test]
    fn test_session_default_starts_uninitialized() {
        let session = DapClientSession::default();
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    #[test]
    fn test_session_initial_seq_is_one() {
        let session = DapClientSession::new();
        assert_eq!(session.next_seq, 1);
    }

    #[test]
    fn test_session_initial_client_info_is_none() {
        let session = DapClientSession::new();
        assert!(session.client_info().is_none());
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_initialize_transitions_to_initializing() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        assert_eq!(session.state, SessionState::Initializing);
    }

    #[tokio::test]
    async fn test_initialize_returns_response_and_initialized_event() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize")).await;
        assert_eq!(responses.len(), 2);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
        assert!(matches!(&responses[1], DapMessage::Event(e) if e.event == "initialized"));
    }

    #[tokio::test]
    async fn test_initialize_response_has_capabilities_body() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize")).await;
        if let DapMessage::Response(r) = &responses[0] {
            assert!(
                r.body.is_some(),
                "initialize response must include capabilities body"
            );
            let body = r.body.as_ref().unwrap();
            assert_eq!(
                body["supportsConfigurationDoneRequest"], true,
                "capabilities body must include supportsConfigurationDoneRequest"
            );
        } else {
            panic!("First response should be a DapMessage::Response");
        }
    }

    #[tokio::test]
    async fn test_initialize_stores_client_info() {
        let mut session = DapClientSession::new();
        let args = serde_json::json!({"clientID": "vscode", "adapterID": "dart"});
        session
            .handle_request(&req_with_args(1, "initialize", args))
            .await;
        let info = session
            .client_info()
            .expect("client_info should be populated");
        assert_eq!(info.client_id.as_deref(), Some("vscode"));
        assert_eq!(info.adapter_id.as_deref(), Some("dart"));
    }

    #[tokio::test]
    async fn test_initialize_with_no_arguments_leaves_client_info_none() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        // No arguments provided — client_info may be None or Some with all-None fields.
        // Either is acceptable; just verify no panic.
    }

    #[tokio::test]
    async fn test_double_initialize_returns_error() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        let responses = session.handle_request(&req(2, "initialize")).await;
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "Second initialize must return an error response"
        );
    }

    #[tokio::test]
    async fn test_double_initialize_state_stays_initializing() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "initialize")).await;
        assert_eq!(session.state, SessionState::Initializing);
    }

    // ── configurationDone ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_configuration_done_after_initialize_succeeds() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        let responses = session.handle_request(&req(2, "configurationDone")).await;
        assert_eq!(responses.len(), 1);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[tokio::test]
    async fn test_configuration_done_transitions_to_configured() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "configurationDone")).await;
        assert_eq!(session.state, SessionState::Configured);
    }

    #[tokio::test]
    async fn test_configuration_done_before_initialize_returns_error() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "configurationDone")).await;
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "configurationDone before initialize must return an error response"
        );
    }

    #[tokio::test]
    async fn test_configuration_done_before_initialize_state_stays_uninitialized() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "configurationDone")).await;
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    // ── attach ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_attach_after_configuration_done_succeeds_with_noop() {
        // With NoopBackend, attach goes to the adapter which calls get_vm()
        // and gets an error, but the outer session handler still succeeds
        // or fails depending on the adapter response.
        // The NoopBackend's get_vm() returns Err, so attach will fail in the adapter.
        // This is expected behaviour — no real VM means no real attach.
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "configurationDone")).await;
        let responses = session.handle_request(&req(3, "attach")).await;
        assert_eq!(responses.len(), 1);
        // With NoopBackend, the adapter returns an error response
        assert!(
            matches!(&responses[0], DapMessage::Response(_)),
            "attach must return a response"
        );
    }

    #[tokio::test]
    async fn test_attach_before_configuration_done_is_allowed() {
        // Per the DAP spec, attach comes BEFORE configurationDone.
        // With NoopBackend, the adapter returns an error (no VM), but
        // the state machine should not reject it.
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        let responses = session.handle_request(&req(2, "attach")).await;
        assert_eq!(responses.len(), 1);
        // The response is an error because NoopBackend has no VM, but
        // it's NOT the "send initialize first" state machine error.
        assert!(matches!(&responses[0], DapMessage::Response(r) if !r.success));
        if let DapMessage::Response(r) = &responses[0] {
            assert!(
                !r.message.as_deref().unwrap_or("").contains("initialize"),
                "Error should be from the adapter (no VM), not from the state machine"
            );
        }
    }

    #[tokio::test]
    async fn test_attach_without_initialize_returns_error() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "attach")).await;
        assert!(matches!(&responses[0], DapMessage::Response(r) if !r.success));
    }

    // ── disconnect ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_disconnect_transitions_to_disconnecting() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "disconnect")).await;
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    #[tokio::test]
    async fn test_disconnect_returns_terminated_event_then_success_response() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        let responses = session.handle_request(&req(2, "disconnect")).await;
        // DAP spec: terminated event comes first, then the disconnect response.
        assert_eq!(responses.len(), 2);
        assert!(
            matches!(&responses[0], DapMessage::Event(e) if e.event == "terminated"),
            "First message must be the terminated event"
        );
        assert!(
            matches!(&responses[1], DapMessage::Response(r) if r.success),
            "Second message must be a success response"
        );
    }

    #[tokio::test]
    async fn test_disconnect_from_uninitialized_succeeds() {
        // disconnect is valid at any state — it's always accepted.
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "disconnect")).await;
        // DAP spec: terminated event first, then success response.
        assert_eq!(responses.len(), 2);
        assert!(matches!(&responses[0], DapMessage::Event(e) if e.event == "terminated"));
        assert!(matches!(&responses[1], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    /// Verify that client-initiated disconnect produces a `terminated` event
    /// before the `disconnect` response, and that the terminated event has a
    /// strictly lower sequence number than the response.
    ///
    /// DAP spec: the `terminated` event must arrive before the disconnect
    /// response so clients can transition their debug UI out of debug mode.
    #[tokio::test]
    async fn test_disconnect_sends_terminated_event_before_response() {
        let mut session = DapClientSession::new();

        // Complete the initialization handshake first.
        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "configurationDone")).await;

        // Send disconnect.
        let responses = session.handle_request(&req(3, "disconnect")).await;

        // Must produce exactly 2 messages.
        assert_eq!(
            responses.len(),
            2,
            "disconnect must produce terminated event + response, got {:?}",
            responses
        );

        // First message: terminated event.
        let terminated_seq = match &responses[0] {
            DapMessage::Event(e) => {
                assert_eq!(
                    e.event, "terminated",
                    "First message must be the terminated event"
                );
                e.seq
            }
            other => panic!(
                "Expected terminated event as first message, got {:?}",
                other
            ),
        };

        // Second message: success response to the disconnect request.
        let response_seq = match &responses[1] {
            DapMessage::Response(r) => {
                assert!(r.success, "Disconnect response must be success");
                assert_eq!(r.command, "disconnect", "Response must echo 'disconnect'");
                assert_eq!(r.request_seq, 3, "Response must correlate to request seq 3");
                r.seq
            }
            other => panic!(
                "Expected disconnect response as second message, got {:?}",
                other
            ),
        };

        // Terminated event must carry a strictly lower seq than the response.
        assert!(
            terminated_seq < response_seq,
            "terminated event seq ({}) must precede disconnect response seq ({})",
            terminated_seq,
            response_seq
        );

        // Session is now in Disconnecting state.
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    // ── unknown commands ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_unknown_command_after_initialize_returns_error_response() {
        // After initialize, commands are delegated to the adapter.
        // NoopBackend has no adapter, so unknown commands get "No debug backend available".
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        let responses = session.handle_request(&req(2, "flyToMoon")).await;
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "Unknown command must return an error response"
        );
    }

    #[tokio::test]
    async fn test_unknown_command_returns_error_response_when_configured() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "configurationDone")).await;
        let responses = session.handle_request(&req(3, "flyToMoon")).await;
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "Unknown command must return an error response"
        );
    }

    #[tokio::test]
    async fn test_unknown_command_does_not_change_state() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        assert_eq!(session.state, SessionState::Initializing);
        session.handle_request(&req(2, "unknownCmd")).await;
        assert_eq!(
            session.state,
            SessionState::Initializing,
            "Unknown command must not change session state"
        );
    }

    // ── sequence number monotonicity ──────────────────────────────────────────

    #[tokio::test]
    async fn test_seq_numbers_are_monotonically_increasing() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize")).await;

        // initialize produces: [Response(seq=1), Event(seq=2)]
        if let DapMessage::Response(r) = &responses[0] {
            assert_eq!(r.seq, 1, "First response should have seq=1");
        } else {
            panic!("Expected Response");
        }
        if let DapMessage::Event(e) = &responses[1] {
            assert_eq!(e.seq, 2, "initialized event should have seq=2");
        } else {
            panic!("Expected Event");
        }
    }

    #[tokio::test]
    async fn test_seq_numbers_continue_across_multiple_requests() {
        let mut session = DapClientSession::new();

        // initialize → seq 1 (response), seq 2 (initialized event)
        session.handle_request(&req(1, "initialize")).await;

        // configurationDone → seq 3
        let cd_resp = session.handle_request(&req(2, "configurationDone")).await;
        if let DapMessage::Response(r) = &cd_resp[0] {
            assert_eq!(r.seq, 3, "configurationDone response should have seq=3");
        }
    }

    #[tokio::test]
    async fn test_error_responses_consume_seq_numbers() {
        let mut session = DapClientSession::new();

        // configurationDone before initialize → error at seq 1
        let err_resp = session.handle_request(&req(1, "configurationDone")).await;
        if let DapMessage::Response(r) = &err_resp[0] {
            assert_eq!(r.seq, 1);
            assert!(!r.success);
        }

        // initialize → should get seq 2, 3
        let init_resp = session.handle_request(&req(2, "initialize")).await;
        if let DapMessage::Response(r) = &init_resp[0] {
            assert_eq!(r.seq, 2, "After error response, next seq should be 2");
        }
        if let DapMessage::Event(e) = &init_resp[1] {
            assert_eq!(e.seq, 3, "initialized event should be seq 3");
        }
    }

    // ── response correlation ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_response_request_seq_matches_request() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(42, "initialize")).await;
        if let DapMessage::Response(r) = &responses[0] {
            assert_eq!(
                r.request_seq, 42,
                "response.request_seq must match the request seq"
            );
        }
    }

    #[tokio::test]
    async fn test_response_command_echoes_request_command() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize")).await;
        if let DapMessage::Response(r) = &responses[0] {
            assert_eq!(r.command, "initialize");
        }
    }

    // ── full handshake ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_full_handshake_initialize_configure_disconnect() {
        let mut session = DapClientSession::new();

        // initialize
        let r1 = session.handle_request(&req(1, "initialize")).await;
        assert!(matches!(&r1[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Initializing);

        // configurationDone
        let r2 = session.handle_request(&req(2, "configurationDone")).await;
        assert!(matches!(&r2[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Configured);

        // disconnect: terminated event first, then success response.
        let r4 = session.handle_request(&req(4, "disconnect")).await;
        assert!(matches!(&r4[0], DapMessage::Event(e) if e.event == "terminated"));
        assert!(matches!(&r4[1], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    // ── Attached state ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_attached_state_variant_exists() {
        // Verify the Attached state can be constructed and compared.
        let state = SessionState::Attached;
        assert_eq!(state, SessionState::Attached);
        assert_ne!(state, SessionState::Configured);
    }

    // ── MockBackend integration tests ─────────────────────────────────────────

    /// A mock backend that always returns success with pre-configured responses.
    #[derive(Clone, Default)]
    struct MockBackend {
        vm_response: Option<serde_json::Value>,
    }

    impl MockBackend {
        fn with_vm(vm_json: serde_json::Value) -> Self {
            Self {
                vm_response: Some(vm_json),
            }
        }
    }

    impl crate::adapter::DebugBackend for MockBackend {
        async fn pause(&self, _: &str) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }
        async fn resume(
            &self,
            _: &str,
            _: Option<crate::adapter::StepMode>,
            _: Option<i32>,
        ) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }
        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            line: i32,
            column: Option<i32>,
        ) -> std::result::Result<crate::adapter::BreakpointResult, crate::adapter::BackendError>
        {
            Ok(crate::adapter::BreakpointResult {
                vm_id: "breakpoints/1".to_string(),
                resolved: true,
                line: Some(line),
                column,
            })
        }
        async fn remove_breakpoint(
            &self,
            _: &str,
            _: &str,
        ) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }
        async fn set_exception_pause_mode(
            &self,
            _: &str,
            _: crate::adapter::DapExceptionPauseMode,
        ) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }
        async fn get_stack(
            &self,
            _: &str,
            _: Option<i32>,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({ "frames": [] }))
        }
        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({ "type": "Instance", "id": "objects/1" }))
        }
        async fn evaluate(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({ "type": "@Instance", "valueAsString": "42" }))
        }
        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({ "type": "@Instance", "valueAsString": "42" }))
        }
        async fn get_vm(
            &self,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            if let Some(vm) = &self.vm_response {
                Ok(vm.clone())
            } else {
                Ok(serde_json::json!({ "isolates": [] }))
            }
        }
        async fn get_isolate(
            &self,
            _: &str,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({}))
        }

        async fn get_scripts(
            &self,
            _: &str,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({ "scripts": [] }))
        }

        async fn call_service(
            &self,
            _: &str,
            _: Option<serde_json::Value>,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({}))
        }

        async fn set_library_debuggable(
            &self,
            _: &str,
            _: &str,
            _: bool,
        ) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }

        async fn get_source_report(
            &self,
            _: &str,
            _: &str,
            _: &[&str],
            _: Option<i64>,
            _: Option<i64>,
        ) -> std::result::Result<serde_json::Value, crate::adapter::BackendError> {
            Ok(serde_json::json!({}))
        }

        async fn get_source(
            &self,
            _: &str,
            _: &str,
        ) -> std::result::Result<String, crate::adapter::BackendError> {
            Ok(String::new())
        }

        async fn hot_reload(&self) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }

        async fn hot_restart(&self) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }

        async fn stop_app(&self) -> std::result::Result<(), crate::adapter::BackendError> {
            Ok(())
        }

        async fn ws_uri(&self) -> Option<String> {
            None
        }

        async fn device_id(&self) -> Option<String> {
            None
        }

        async fn build_mode(&self) -> String {
            "debug".to_string()
        }
    }

    #[tokio::test]
    async fn test_attach_with_mock_backend_transitions_to_attached() {
        let backend = MockBackend::with_vm(serde_json::json!({
            "isolates": [{ "id": "isolates/1", "name": "main" }]
        }));
        let mut session = DapClientSession::with_backend(backend);

        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "configurationDone")).await;
        let responses = session.handle_request(&req(3, "attach")).await;

        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if r.success),
            "attach with mock backend must succeed, got {:?}",
            responses[0]
        );
        assert_eq!(
            session.state,
            SessionState::Attached,
            "state should be Attached after successful attach"
        );
    }

    #[tokio::test]
    async fn test_threads_command_after_attach_dispatches_to_adapter() {
        let backend = MockBackend::with_vm(serde_json::json!({
            "isolates": [{ "id": "isolates/1", "name": "main" }]
        }));
        let mut session = DapClientSession::with_backend(backend);

        session.handle_request(&req(1, "initialize")).await;
        session.handle_request(&req(2, "configurationDone")).await;
        session.handle_request(&req(3, "attach")).await;

        // threads command should be dispatched to the adapter
        let responses = session.handle_request(&req(4, "threads")).await;
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if r.success),
            "threads command must succeed after attach"
        );
    }

    #[tokio::test]
    async fn test_full_debug_session_initialize_configure_attach_disconnect() {
        let backend = MockBackend::with_vm(serde_json::json!({
            "isolates": [{ "id": "isolates/1", "name": "main" }]
        }));
        let mut session = DapClientSession::with_backend(backend);

        // initialize
        let r1 = session.handle_request(&req(1, "initialize")).await;
        assert!(matches!(&r1[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Initializing);

        // configurationDone
        let r2 = session.handle_request(&req(2, "configurationDone")).await;
        assert!(matches!(&r2[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Configured);

        // attach
        let r3 = session.handle_request(&req(3, "attach")).await;
        assert!(matches!(&r3[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Attached);

        // disconnect: terminated event first, then success response.
        let r4 = session.handle_request(&req(4, "disconnect")).await;
        assert!(matches!(&r4[0], DapMessage::Event(e) if e.event == "terminated"));
        assert!(matches!(&r4[1], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    /// Tests the DAP-spec ordering: initialize → attach → configurationDone → disconnect
    #[tokio::test]
    async fn test_full_debug_session_dap_spec_ordering() {
        let backend = MockBackend::with_vm(serde_json::json!({
            "isolates": [{ "id": "isolates/1", "name": "main" }]
        }));
        let mut session = DapClientSession::with_backend(backend);

        // initialize
        let r1 = session.handle_request(&req(1, "initialize")).await;
        assert!(matches!(&r1[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Initializing);

        // attach (before configurationDone — per DAP spec)
        let r2 = session.handle_request(&req(2, "attach")).await;
        assert!(
            matches!(&r2[0], DapMessage::Response(r) if r.success),
            "attach before configurationDone must succeed, got {:?}",
            r2[0]
        );
        assert_eq!(session.state, SessionState::Attached);

        // configurationDone (after attach — per DAP spec)
        let r3 = session.handle_request(&req(3, "configurationDone")).await;
        assert!(matches!(&r3[0], DapMessage::Response(r) if r.success));
        // State stays Attached (configurationDone is just an ack)
        assert_eq!(session.state, SessionState::Attached);

        // disconnect: terminated event first, then success response.
        let r4 = session.handle_request(&req(4, "disconnect")).await;
        assert!(matches!(&r4[0], DapMessage::Event(e) if e.event == "terminated"));
        assert!(matches!(&r4[1], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    // ── Broadcast channel closed — no busy-poll ───────────────────────────────

    /// Verify that a dead broadcast channel (sender dropped immediately) does
    /// not cause the `run_on` select loop to busy-spin.
    ///
    /// The fix wraps `log_event_rx` in an `Option` and sets it to `None` on
    /// `RecvError::Closed`. When `None`, the select arm parks on
    /// `std::future::pending()`, so the loop only wakes on real I/O or the
    /// shutdown signal — not on a permanently-ready `Err(Closed)`.
    ///
    /// This test sends an initialize request and a disconnect request over a
    /// duplex stream. Both must be processed correctly even though the broadcast
    /// sender was dropped before the session started, proving that the other
    /// select arms continue to function after the broadcast branch is disabled.
    #[tokio::test]
    async fn test_run_on_handles_closed_broadcast_channel_gracefully() {
        use tokio::io::{BufReader, BufWriter};

        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Create a dead broadcast channel — sender is dropped immediately.
        // This is the scenario that previously caused 100% CPU spin.
        let (_, log_event_rx) = broadcast::channel::<crate::adapter::DebugEvent>(1);

        let server = tokio::spawn(async move {
            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx, None).await
        });

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // Send initialize — the session must process this despite the dead channel.
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // Expect response + initialized event.
        let r1 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("response timeout")
        .expect("read ok")
        .expect("not EOF");

        let r2 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("event timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r1, crate::DapMessage::Response(r) if r.success),
            "Expected success response to initialize, got {:?}",
            r1
        );
        assert!(
            matches!(&r2, crate::DapMessage::Event(e) if e.event == "initialized"),
            "Expected initialized event, got {:?}",
            r2
        );

        // Send disconnect — session must exit cleanly.
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 2,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // DAP spec: terminated event is sent before the disconnect response.
        let r3 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("terminated event timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r3, crate::DapMessage::Event(e) if e.event == "terminated"),
            "Expected terminated event before disconnect response, got {:?}",
            r3
        );

        let r4 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("disconnect response timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r4, crate::DapMessage::Response(r) if r.success),
            "Expected success response to disconnect, got {:?}",
            r4
        );

        // Session should exit on its own after disconnect.
        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit after disconnect")
            .expect("task ok")
            .expect("session ok");

        // Shut down the watch channel (not strictly needed, but keeps the test clean).
        let _ = shutdown_tx.send(true);

        // Drop the client writer to release any remaining resources.
        drop(writer);
    }

    // ── DebugEventSource::None — requests processed normally ──────────────────

    /// Verify that a session using `DebugEventSource::None` (no event source)
    /// processes inbound DAP requests normally. The select arm for debug events
    /// parks on `std::future::pending()` and never fires, but the request and
    /// shutdown arms continue to operate.
    #[tokio::test]
    async fn test_run_inner_with_debug_event_source_none_processes_requests() {
        use tokio::io::{BufReader, BufWriter};

        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Use run_on_with_backend with an mpsc channel that is immediately
        // closed (sender dropped) so that DebugEventSource::Dedicated returns
        // None on the first recv — equivalent to DebugEventSource::None
        // for practical purposes.
        let (_, debug_rx) = mpsc::channel::<crate::adapter::DebugEvent>(1);
        // Drop the sender immediately so the mpsc receiver returns None.
        // (sender is already dropped by binding to `_` above)

        let server = tokio::spawn(async move {
            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            DapClientSession::<NoopBackend>::run_on_with_backend(
                reader,
                writer,
                shutdown_rx,
                NoopBackend,
                debug_rx,
                None,
            )
            .await
        });

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // Send initialize — session must process this despite no debug events.
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // Expect response + initialized event.
        let r1 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("response timeout")
        .expect("read ok")
        .expect("not EOF");

        let r2 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("event timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r1, crate::DapMessage::Response(r) if r.success),
            "Expected success response to initialize, got {:?}",
            r1
        );
        assert!(
            matches!(&r2, crate::DapMessage::Event(e) if e.event == "initialized"),
            "Expected initialized event, got {:?}",
            r2
        );

        // Send disconnect to let the session exit cleanly.
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 2,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // DAP spec: terminated event is sent before the disconnect response.
        let r3 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("terminated event timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r3, crate::DapMessage::Event(e) if e.event == "terminated"),
            "Expected terminated event before disconnect response, got {:?}",
            r3
        );

        let r4 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("disconnect response timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r4, crate::DapMessage::Response(r) if r.success),
            "Expected success response to disconnect, got {:?}",
            r4
        );

        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit after disconnect")
            .expect("task ok")
            .expect("session ok");

        let _ = shutdown_tx.send(true);
        drop(writer);
    }

    // ── DebugEventSource::Broadcast Closed — branch disabled ──────────────────

    /// Verify that `DebugEventSource::Broadcast` transitions to `None` (and
    /// thus parks the select arm) when the broadcast sender is dropped mid-session.
    ///
    /// This is tested indirectly through `run_on`: a live broadcast sender is
    /// dropped after `initialize` is processed, and then a `disconnect` request
    /// is still processed correctly — proving the session did not stall or
    /// busy-spin after the channel closed.
    #[tokio::test]
    async fn test_run_on_broadcast_closed_mid_session_disables_branch() {
        use tokio::io::{BufReader, BufWriter};

        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Create a live broadcast channel so the session starts with a real
        // Broadcast(Some(rx)) source.
        let (log_tx, log_event_rx) = broadcast::channel::<crate::adapter::DebugEvent>(8);

        let server = tokio::spawn(async move {
            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx, None).await
        });

        let mut writer = BufWriter::new(client_writer);
        let mut reader = BufReader::new(client_reader);

        // Send initialize.
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // Consume the two messages (response + initialized event).
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("timeout")
        .expect("read ok")
        .expect("not EOF");

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("timeout")
        .expect("read ok")
        .expect("not EOF");

        // Drop the broadcast sender mid-session. The session's Broadcast(Some(rx))
        // will receive RecvError::Closed on the next recv and transition to None,
        // preventing any busy-spin.
        drop(log_tx);

        // The session must still process the disconnect request after the
        // broadcast channel closes.
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 2,
                command: "disconnect".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();

        // DAP spec: terminated event is sent before the disconnect response.
        let r3 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("terminated event timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r3, crate::DapMessage::Event(e) if e.event == "terminated"),
            "Expected terminated event before disconnect response, got {:?}",
            r3
        );

        let r4 = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("disconnect response timeout")
        .expect("read ok")
        .expect("not EOF");

        assert!(
            matches!(&r4, crate::DapMessage::Response(r) if r.success),
            "Expected success response to disconnect after broadcast closed, got {:?}",
            r4
        );

        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit after disconnect")
            .expect("task ok")
            .expect("session ok");

        let _ = shutdown_tx.send(true);
        drop(writer);
    }

    // ── Auth token validation ──────────────────────────────────────────────────

    /// Helper: create a session with auth required and return it.
    fn session_with_auth(token: &str) -> DapClientSession<NoopBackend> {
        let mut s = DapClientSession::new();
        s.auth_token = Some(token.to_string());
        s
    }

    #[tokio::test]
    async fn test_initialize_with_valid_token_succeeds() {
        let mut session = session_with_auth("secret123");
        let args = serde_json::json!({ "authToken": "secret123", "clientID": "test" });
        let responses = session
            .handle_request(&req_with_args(1, "initialize", args))
            .await;
        assert_eq!(
            responses.len(),
            2,
            "initialize with valid token must return response + event"
        );
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if r.success),
            "initialize with valid token must succeed, got {:?}",
            responses[0]
        );
        assert_eq!(session.state, SessionState::Initializing);
    }

    #[tokio::test]
    async fn test_initialize_with_invalid_token_rejected() {
        let mut session = session_with_auth("secret123");
        let args = serde_json::json!({ "authToken": "wrong_token", "clientID": "test" });
        let responses = session
            .handle_request(&req_with_args(1, "initialize", args))
            .await;
        assert_eq!(
            responses.len(),
            1,
            "rejected initialize must return exactly one error response"
        );
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "initialize with wrong token must fail, got {:?}",
            responses[0]
        );
        // State must not change.
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    #[tokio::test]
    async fn test_initialize_without_token_rejected_when_required() {
        let mut session = session_with_auth("secret123");
        // No authToken field in arguments.
        let args = serde_json::json!({ "clientID": "test" });
        let responses = session
            .handle_request(&req_with_args(1, "initialize", args))
            .await;
        assert_eq!(
            responses.len(),
            1,
            "missing token must return one error response"
        );
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "initialize without token must fail when auth is required, got {:?}",
            responses[0]
        );
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    #[tokio::test]
    async fn test_initialize_without_auth_configured_always_succeeds() {
        // When auth_token is None (default), any client can connect.
        let mut session = DapClientSession::new();
        // No authToken field — should still succeed.
        let responses = session.handle_request(&req(1, "initialize")).await;
        assert_eq!(responses.len(), 2);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if r.success),
            "initialize without auth configured must always succeed"
        );
    }

    // ── Idle timeout ───────────────────────────────────────────────────────────

    /// Test that a session in Initializing state is closed after the idle timeout.
    ///
    /// Uses `tokio::time::pause()` / `advance()` to speed-run the 300 s timeout
    /// without actually waiting.
    #[tokio::test(start_paused = true)]
    async fn test_idle_timeout_disconnects_non_attached_session() {
        use tokio::io::{BufReader, BufWriter};

        let (server_reader, _client_writer) = tokio::io::duplex(8192);
        let (_client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Keep `_log_event_tx` alive so the broadcast channel is not immediately closed.
        let (_log_event_tx, log_event_rx) = broadcast::channel::<crate::adapter::DebugEvent>(1);

        let server = tokio::spawn(async move {
            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx, None).await
        });

        // Give the server task a moment to start.
        tokio::task::yield_now().await;

        // Advance time past INIT_TIMEOUT (30 s) to move past Uninitialized state.
        // Without sending initialize, the session should close due to INIT_TIMEOUT.
        // That's fine — we just need to verify it closes (for any reason) after timeout.
        tokio::time::advance(INIT_TIMEOUT + std::time::Duration::from_secs(1)).await;
        tokio::task::yield_now().await;

        // Server should exit after the init timeout fires.
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit after init timeout")
            .expect("task ok");

        assert!(result.is_ok(), "session should exit cleanly on timeout");

        let _ = shutdown_tx.send(true);
    }

    /// Test that a session that has completed initialize (Initializing state)
    /// is closed after the idle timeout if no further activity occurs.
    #[tokio::test(start_paused = true)]
    async fn test_idle_timeout_closes_initializing_session() {
        use tokio::io::{AsyncWriteExt, BufReader, BufWriter};

        let (server_reader, client_writer) = tokio::io::duplex(8192);
        let (client_reader, server_writer) = tokio::io::duplex(8192);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_log_event_tx, log_event_rx) = broadcast::channel::<crate::adapter::DebugEvent>(1);

        let server = tokio::spawn(async move {
            let reader = BufReader::new(server_reader);
            let writer = BufWriter::new(server_writer);
            DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx, None).await
        });

        tokio::task::yield_now().await;

        // Send initialize so the session moves to Initializing state.
        let mut writer = BufWriter::new(client_writer);
        crate::write_message(
            &mut writer,
            &crate::DapMessage::Request(crate::DapRequest {
                seq: 1,
                command: "initialize".into(),
                arguments: None,
            }),
        )
        .await
        .unwrap();
        writer.flush().await.unwrap();

        // Read the response + initialized event.
        let mut reader = BufReader::new(client_reader);
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("response timeout")
        .expect("read ok")
        .expect("not EOF");

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::read_message(&mut reader),
        )
        .await
        .expect("event timeout")
        .expect("read ok")
        .expect("not EOF");

        // Now advance time past IDLE_TIMEOUT — the session should close.
        tokio::time::advance(IDLE_TIMEOUT + std::time::Duration::from_secs(1)).await;
        tokio::task::yield_now().await;

        let result = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server should exit after idle timeout")
            .expect("task ok");

        assert!(
            result.is_ok(),
            "session should exit cleanly after idle timeout"
        );

        let _ = shutdown_tx.send(true);
    }
}
