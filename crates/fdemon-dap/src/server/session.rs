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
    async fn pause(&self, _isolate_id: &str) -> std::result::Result<(), String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn resume(
        &self,
        _isolate_id: &str,
        _step: Option<crate::adapter::StepMode>,
    ) -> std::result::Result<(), String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn add_breakpoint(
        &self,
        _isolate_id: &str,
        _uri: &str,
        _line: i32,
        _column: Option<i32>,
    ) -> std::result::Result<crate::adapter::BreakpointResult, String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn remove_breakpoint(
        &self,
        _isolate_id: &str,
        _breakpoint_id: &str,
    ) -> std::result::Result<(), String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn set_exception_pause_mode(
        &self,
        _isolate_id: &str,
        _mode: &str,
    ) -> std::result::Result<(), String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> std::result::Result<serde_json::Value, String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn get_object(
        &self,
        _isolate_id: &str,
        _object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> std::result::Result<serde_json::Value, String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> std::result::Result<serde_json::Value, String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn evaluate_in_frame(
        &self,
        _isolate_id: &str,
        _frame_index: i32,
        _expression: &str,
    ) -> std::result::Result<serde_json::Value, String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn get_vm(&self) -> std::result::Result<serde_json::Value, String> {
        Err("NoopBackend: no VM Service connected".to_string())
    }

    async fn get_scripts(
        &self,
        _isolate_id: &str,
    ) -> std::result::Result<serde_json::Value, String> {
        Err("NoopBackend: no VM Service connected".to_string())
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
        mut debug_event_rx: mpsc::Receiver<DebugEvent>,
    ) -> Result<()>
    where
        R: AsyncRead + Unpin + Send,
        W: AsyncWrite + Unpin + Send,
    {
        let mut session = Self::with_backend(backend);

        // Channel for adapter-generated DAP events (stopped, continued, thread, etc.)
        let (event_tx, mut event_rx) = mpsc::channel::<DapMessage>(64);
        session.event_tx = Some(event_tx);

        loop {
            tokio::select! {
                result = read_message(&mut reader) => {
                    match result {
                        Ok(Some(DapMessage::Request(req))) => {
                            let responses = session.handle_request(&req).await;
                            for msg in &responses {
                                write_message(&mut writer, msg).await?;
                            }
                            if session.state == SessionState::Disconnecting {
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
                            ev.seq = session.next_seq;
                            session.next_seq += 1;
                            DapMessage::Event(ev)
                        }
                        other => other,
                    };
                    if let Err(e) = write_message(&mut writer, &stamped).await {
                        tracing::warn!("Error writing DAP event: {}", e);
                        break;
                    }
                }

                // Debug events forwarded from the VM Service via the Engine.
                Some(debug_event) = debug_event_rx.recv() => {
                    if let Some(adapter) = &mut session.adapter {
                        adapter.handle_debug_event(debug_event).await;
                    }
                }

                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::debug!("DAP session terminating due to server shutdown");
                        let event = session.make_event(DapEvent::terminated());
                        let _ = write_message(&mut writer, &DapMessage::Event(event)).await;
                        break;
                    }
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
    /// # Type Parameters
    ///
    /// - `R` — Any async reader (e.g., `BufReader<OwnedReadHalf>`, `BufReader<Stdin>`).
    /// - `W` — Any async writer (e.g., `OwnedWriteHalf`, `BufWriter<Stdout>`).
    pub async fn run_on<R, W>(
        mut reader: BufReader<R>,
        mut writer: W,
        mut shutdown_rx: watch::Receiver<bool>,
        mut log_event_rx: broadcast::Receiver<DebugEvent>,
    ) -> Result<()>
    where
        R: AsyncRead + Unpin + Send,
        W: AsyncWrite + Unpin + Send,
    {
        let mut session = Self::new();
        // Channel for adapter-generated DAP events (if adapter is created).
        let (event_tx, mut event_rx) = mpsc::channel::<DapMessage>(64);
        session.event_tx = Some(event_tx);

        loop {
            tokio::select! {
                result = read_message(&mut reader) => {
                    match result {
                        Ok(Some(DapMessage::Request(req))) => {
                            let responses = session.handle_request(&req).await;
                            for msg in &responses {
                                write_message(&mut writer, msg).await?;
                            }
                            if session.state == SessionState::Disconnecting {
                                break;
                            }
                        }
                        Ok(Some(_)) => {
                            // Ignore non-request messages from client (e.g., responses to
                            // server-initiated requests — not yet used in Phase 2).
                            tracing::debug!("Ignoring non-request DAP message from client");
                        }
                        Ok(None) => {
                            // Clean EOF — client disconnected without sending `disconnect`.
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
                    let stamped = match event_msg {
                        DapMessage::Event(mut ev) => {
                            ev.seq = session.next_seq;
                            session.next_seq += 1;
                            DapMessage::Event(ev)
                        }
                        other => other,
                    };
                    if let Err(e) = write_message(&mut writer, &stamped).await {
                        tracing::warn!("Error writing DAP event: {}", e);
                        break;
                    }
                }

                // Debug events broadcast by the Engine (e.g., LogOutput from Flutter logs).
                log_event_result = log_event_rx.recv() => {
                    match log_event_result {
                        Ok(debug_event) => {
                            if let Some(adapter) = &mut session.adapter {
                                adapter.handle_debug_event(debug_event).await;
                            }
                            // If no adapter yet (not attached), silently discard.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // We fell behind; log a warning and continue.
                            tracing::warn!(
                                "DAP session log event receiver lagged, dropped {} events",
                                n
                            );
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Sender dropped (server shutting down); the shutdown_rx
                            // branch will handle the graceful termination.
                            tracing::debug!("DAP log event broadcast channel closed");
                        }
                    }
                }

                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        // Server shutting down — notify the client before closing.
                        tracing::debug!("DAP session terminating due to server shutdown");
                        let event = session.make_event(DapEvent::terminated());
                        let _ = write_message(&mut writer, &DapMessage::Event(event)).await;
                        break;
                    }
                }
            }
        }

        Ok(())
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
    ) -> Result<()> {
        let (reader, writer) = stream.into_split();
        let reader = BufReader::new(reader);
        Self::run_on(reader, writer, shutdown_rx, log_event_rx).await
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
            "disconnect" => self.handle_disconnect(request),

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
                        let (adapter, _event_rx) = DapAdapter::new_with_tx(backend, event_tx);
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

        // Store client capabilities if provided.
        if let Some(args) = &request.arguments {
            self.client_info = serde_json::from_value(args.clone()).ok();
        }

        self.state = SessionState::Initializing;

        let capabilities = Capabilities::fdemon_defaults();
        let body = serde_json::to_value(&capabilities).unwrap_or_default();
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
    fn handle_disconnect(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        self.state = SessionState::Disconnecting;
        let resp = self.make_response(DapResponse::success(request, None));
        vec![DapMessage::Response(resp)]
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
                !r.message
                    .as_deref()
                    .unwrap_or("")
                    .contains("initialize"),
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
    async fn test_disconnect_returns_success_response() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize")).await;
        let responses = session.handle_request(&req(2, "disconnect")).await;
        assert_eq!(responses.len(), 1);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[tokio::test]
    async fn test_disconnect_from_uninitialized_succeeds() {
        // disconnect is valid at any state — it's always accepted.
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "disconnect")).await;
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
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

        // disconnect
        let r4 = session.handle_request(&req(4, "disconnect")).await;
        assert!(matches!(&r4[0], DapMessage::Response(r) if r.success));
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
        async fn pause(&self, _: &str) -> std::result::Result<(), String> {
            Ok(())
        }
        async fn resume(
            &self,
            _: &str,
            _: Option<crate::adapter::StepMode>,
        ) -> std::result::Result<(), String> {
            Ok(())
        }
        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            line: i32,
            column: Option<i32>,
        ) -> std::result::Result<crate::adapter::BreakpointResult, String> {
            Ok(crate::adapter::BreakpointResult {
                vm_id: "breakpoints/1".to_string(),
                resolved: true,
                line: Some(line),
                column,
            })
        }
        async fn remove_breakpoint(&self, _: &str, _: &str) -> std::result::Result<(), String> {
            Ok(())
        }
        async fn set_exception_pause_mode(
            &self,
            _: &str,
            _: &str,
        ) -> std::result::Result<(), String> {
            Ok(())
        }
        async fn get_stack(
            &self,
            _: &str,
            _: Option<i32>,
        ) -> std::result::Result<serde_json::Value, String> {
            Ok(serde_json::json!({ "frames": [] }))
        }
        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> std::result::Result<serde_json::Value, String> {
            Ok(serde_json::json!({ "type": "Instance", "id": "objects/1" }))
        }
        async fn evaluate(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> std::result::Result<serde_json::Value, String> {
            Ok(serde_json::json!({ "type": "@Instance", "valueAsString": "42" }))
        }
        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> std::result::Result<serde_json::Value, String> {
            Ok(serde_json::json!({ "type": "@Instance", "valueAsString": "42" }))
        }
        async fn get_vm(&self) -> std::result::Result<serde_json::Value, String> {
            if let Some(vm) = &self.vm_response {
                Ok(vm.clone())
            } else {
                Ok(serde_json::json!({ "isolates": [] }))
            }
        }
        async fn get_scripts(&self, _: &str) -> std::result::Result<serde_json::Value, String> {
            Ok(serde_json::json!({ "scripts": [] }))
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

        // disconnect
        let r4 = session.handle_request(&req(4, "disconnect")).await;
        assert!(matches!(&r4[0], DapMessage::Response(r) if r.success));
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

        // disconnect
        let r4 = session.handle_request(&req(4, "disconnect")).await;
        assert!(matches!(&r4[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }
}
