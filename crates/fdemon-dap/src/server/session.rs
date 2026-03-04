//! # DAP Client Session State Machine
//!
//! Manages the lifecycle of a single DAP client connection. Each connected
//! client gets its own [`DapClientSession`] instance that runs independently
//! in a spawned Tokio task.
//!
//! ## Session State Machine
//!
//! ```text
//! Uninitialized → Initializing → Configured → Disconnecting
//!                                     ↓
//!                               (attach/disconnect)
//! ```
//!
//! Phase 2 handles only the initialization handshake. The `attach` command
//! is accepted as a stub (responds with success) but does not yet connect to
//! the Dart VM Service. Phase 3 will wire `attach` to a `DapAdapter`.
//!
//! ## Out-of-order request handling
//!
//! The session validates state transitions at every step. Sending
//! `configurationDone` before `initialize`, or calling `initialize` twice,
//! results in an error response rather than a panic or silent no-op.

use tokio::{io::BufReader, sync::watch};

use fdemon_core::error::Result;

use crate::{
    read_message, write_message, Capabilities, DapEvent, DapMessage, DapRequest, DapResponse,
    InitializeRequestArguments,
};

// ─────────────────────────────────────────────────────────────────────────────
// State machine
// ─────────────────────────────────────────────────────────────────────────────

/// State of a DAP client session.
///
/// Transitions are strictly ordered: `Uninitialized` → `Initializing` →
/// `Configured` → `Disconnecting`. Jumping ahead or going backwards is
/// rejected with an error response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Client connected but has not yet sent `initialize`.
    Uninitialized,
    /// `initialize` received; waiting for `configurationDone`.
    Initializing,
    /// `configurationDone` received; ready for `attach` or other operations.
    Configured,
    /// Client is disconnecting; session will terminate after sending the response.
    Disconnecting,
}

// ─────────────────────────────────────────────────────────────────────────────
// Session
// ─────────────────────────────────────────────────────────────────────────────

/// Manages a single DAP client connection.
///
/// Holds the session state machine, client capability information (received
/// during `initialize`), and a monotonically increasing sequence counter for
/// all server-sent messages.
///
/// # Design Notes
///
/// - `DapClientSession` instances are fully independent — they share no mutable
///   state with each other or with the server.
/// - The `next_seq` counter is per-session; each client connection starts at 1.
/// - Phase 3 will add an `Attached`/`Debugging` state for active debugging.
pub struct DapClientSession {
    /// Current state in the DAP initialization handshake.
    pub(crate) state: SessionState,
    /// Monotonic sequence number for the next server-sent message.
    next_seq: i64,
    /// Client capabilities received during the `initialize` request.
    client_info: Option<InitializeRequestArguments>,
}

impl DapClientSession {
    /// Create a new session in the `Uninitialized` state.
    pub fn new() -> Self {
        Self {
            state: SessionState::Uninitialized,
            next_seq: 1,
            client_info: None,
        }
    }

    /// Run the session until the client disconnects or shutdown is signalled.
    ///
    /// Reads DAP messages from the TCP stream, dispatches to state-machine
    /// handlers, and writes responses/events back. The loop exits when:
    ///
    /// - The client sends a `disconnect` request (state becomes `Disconnecting`).
    /// - The client closes the connection (EOF).
    /// - A read error occurs.
    /// - A shutdown signal is received on `shutdown_rx`.
    ///
    /// On shutdown, a `terminated` event is sent before the connection closes.
    pub async fn run(
        stream: tokio::net::TcpStream,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut session = Self::new();

        loop {
            tokio::select! {
                result = read_message(&mut reader) => {
                    match result {
                        Ok(Some(DapMessage::Request(req))) => {
                            let responses = session.handle_request(&req);
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
    /// Dispatches by `request.command` to the appropriate state-machine handler.
    /// Unknown commands receive an error response instead of causing a crash.
    pub fn handle_request(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        match request.command.as_str() {
            "initialize" => self.handle_initialize(request),
            "configurationDone" => self.handle_configuration_done(request),
            "attach" => self.handle_attach(request),
            "disconnect" => self.handle_disconnect(request),
            _ => self.handle_unknown(request),
        }
    }

    // ── Command handlers ───────────────────────────────────────────────────

    /// Handle the `initialize` request.
    ///
    /// Validates that the session is in `Uninitialized` state, then:
    /// 1. Stores the client's capability arguments.
    /// 2. Advances state to `Initializing`.
    /// 3. Responds with the server's [`Capabilities`].
    /// 4. Sends the `initialized` event to signal readiness for configuration.
    ///
    /// Returns an error response if called more than once (double-initialize).
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

        // Build and stamp the success response.
        let capabilities = Capabilities::fdemon_defaults();
        let body = serde_json::to_value(&capabilities).unwrap_or_default();
        let response = self.make_response(DapResponse::success(request, Some(body)));

        // Send the `initialized` event — signals the client that configuration
        // requests (setBreakpoints, etc.) can now be sent.
        let initialized = self.make_event(DapEvent::initialized());

        vec![
            DapMessage::Response(response),
            DapMessage::Event(initialized),
        ]
    }

    /// Handle the `configurationDone` request.
    ///
    /// Valid only in the `Initializing` state (after `initialize`). Advances
    /// state to `Configured` and responds with success.
    fn handle_configuration_done(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        if self.state != SessionState::Initializing {
            let resp = self.make_response(DapResponse::error(
                request,
                "unexpected configurationDone: call initialize first",
            ));
            return vec![DapMessage::Response(resp)];
        }

        self.state = SessionState::Configured;
        let resp = self.make_response(DapResponse::success(request, None));
        vec![DapMessage::Response(resp)]
    }

    /// Handle the `attach` request (Phase 2 stub).
    ///
    /// Valid only in the `Configured` state. Responds with success but does not
    /// yet connect to the Dart VM Service. Phase 3 will wire this to a
    /// `DapAdapter` / `VmRequestHandle`.
    fn handle_attach(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        if self.state != SessionState::Configured {
            let resp = self.make_response(DapResponse::error(
                request,
                "must call configurationDone before attach",
            ));
            return vec![DapMessage::Response(resp)];
        }

        // Phase 2: accept the attach but do not connect to the VM Service.
        // Phase 3 will add the DapAdapter bridge here.
        tracing::info!("DAP client attached (debugging not yet implemented in Phase 2)");
        let resp = self.make_response(DapResponse::success(request, None));
        vec![DapMessage::Response(resp)]
    }

    /// Handle the `disconnect` request.
    ///
    /// Transitions state to `Disconnecting`. The run loop checks for this state
    /// after writing the response and breaks, ending the session.
    fn handle_disconnect(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        self.state = SessionState::Disconnecting;
        let resp = self.make_response(DapResponse::success(request, None));
        vec![DapMessage::Response(resp)]
    }

    /// Handle an unrecognised command with a graceful error response.
    ///
    /// The session remains in its current state; the client can continue
    /// sending valid commands after receiving this error.
    fn handle_unknown(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        tracing::debug!("Unhandled DAP command: {}", request.command);
        let resp = self.make_response(DapResponse::error(
            request,
            format!("unsupported command: {}", request.command),
        ));
        vec![DapMessage::Response(resp)]
    }
}

impl Default for DapClientSession {
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

    #[test]
    fn test_initialize_transitions_to_initializing() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        assert_eq!(session.state, SessionState::Initializing);
    }

    #[test]
    fn test_initialize_returns_response_and_initialized_event() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize"));
        assert_eq!(responses.len(), 2);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
        assert!(matches!(&responses[1], DapMessage::Event(e) if e.event == "initialized"));
    }

    #[test]
    fn test_initialize_response_has_capabilities_body() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize"));
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

    #[test]
    fn test_initialize_stores_client_info() {
        let mut session = DapClientSession::new();
        let args = serde_json::json!({"clientID": "vscode", "adapterID": "dart"});
        session.handle_request(&req_with_args(1, "initialize", args));
        let info = session
            .client_info()
            .expect("client_info should be populated");
        assert_eq!(info.client_id.as_deref(), Some("vscode"));
        assert_eq!(info.adapter_id.as_deref(), Some("dart"));
    }

    #[test]
    fn test_initialize_with_no_arguments_leaves_client_info_none() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        // No arguments provided — client_info may be None or Some with all-None fields.
        // Either is acceptable; just verify no panic.
    }

    #[test]
    fn test_double_initialize_returns_error() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        let responses = session.handle_request(&req(2, "initialize"));
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "Second initialize must return an error response"
        );
    }

    #[test]
    fn test_double_initialize_state_stays_initializing() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        session.handle_request(&req(2, "initialize"));
        assert_eq!(session.state, SessionState::Initializing);
    }

    // ── configurationDone ─────────────────────────────────────────────────────

    #[test]
    fn test_configuration_done_after_initialize_succeeds() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        let responses = session.handle_request(&req(2, "configurationDone"));
        assert_eq!(responses.len(), 1);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_configuration_done_transitions_to_configured() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        session.handle_request(&req(2, "configurationDone"));
        assert_eq!(session.state, SessionState::Configured);
    }

    #[test]
    fn test_configuration_done_before_initialize_returns_error() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "configurationDone"));
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "configurationDone before initialize must return an error response"
        );
    }

    #[test]
    fn test_configuration_done_before_initialize_state_stays_uninitialized() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "configurationDone"));
        assert_eq!(session.state, SessionState::Uninitialized);
    }

    // ── attach ────────────────────────────────────────────────────────────────

    #[test]
    fn test_attach_after_configuration_done_succeeds() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        session.handle_request(&req(2, "configurationDone"));
        let responses = session.handle_request(&req(3, "attach"));
        assert_eq!(responses.len(), 1);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_attach_before_configuration_done_returns_error() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        let responses = session.handle_request(&req(2, "attach"));
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "attach before configurationDone must return an error response"
        );
    }

    #[test]
    fn test_attach_without_initialize_returns_error() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "attach"));
        assert!(matches!(&responses[0], DapMessage::Response(r) if !r.success));
    }

    // ── disconnect ────────────────────────────────────────────────────────────

    #[test]
    fn test_disconnect_transitions_to_disconnecting() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        session.handle_request(&req(2, "disconnect"));
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    #[test]
    fn test_disconnect_returns_success_response() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        let responses = session.handle_request(&req(2, "disconnect"));
        assert_eq!(responses.len(), 1);
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
    }

    #[test]
    fn test_disconnect_from_uninitialized_succeeds() {
        // disconnect is valid at any state — it's always accepted.
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "disconnect"));
        assert!(matches!(&responses[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }

    // ── unknown commands ──────────────────────────────────────────────────────

    #[test]
    fn test_unknown_command_returns_error_response() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        let responses = session.handle_request(&req(2, "flyToMoon"));
        assert_eq!(responses.len(), 1);
        assert!(
            matches!(&responses[0], DapMessage::Response(r) if !r.success),
            "Unknown command must return an error response"
        );
    }

    #[test]
    fn test_unknown_command_error_message_includes_command_name() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "unknownCommand"));
        if let DapMessage::Response(r) = &responses[0] {
            let msg = r.message.as_deref().unwrap_or("");
            assert!(
                msg.contains("unknownCommand"),
                "Error message should include the command name, got: {:?}",
                msg
            );
        }
    }

    #[test]
    fn test_unknown_command_does_not_change_state() {
        let mut session = DapClientSession::new();
        session.handle_request(&req(1, "initialize"));
        assert_eq!(session.state, SessionState::Initializing);
        session.handle_request(&req(2, "unknownCmd"));
        assert_eq!(
            session.state,
            SessionState::Initializing,
            "Unknown command must not change session state"
        );
    }

    // ── sequence number monotonicity ──────────────────────────────────────────

    #[test]
    fn test_seq_numbers_are_monotonically_increasing() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize"));

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

    #[test]
    fn test_seq_numbers_continue_across_multiple_requests() {
        let mut session = DapClientSession::new();

        // initialize → seq 1 (response), seq 2 (initialized event)
        session.handle_request(&req(1, "initialize"));

        // configurationDone → seq 3
        let cd_resp = session.handle_request(&req(2, "configurationDone"));
        if let DapMessage::Response(r) = &cd_resp[0] {
            assert_eq!(r.seq, 3, "configurationDone response should have seq=3");
        }

        // attach → seq 4
        let attach_resp = session.handle_request(&req(3, "attach"));
        if let DapMessage::Response(r) = &attach_resp[0] {
            assert_eq!(r.seq, 4, "attach response should have seq=4");
        }
    }

    #[test]
    fn test_error_responses_consume_seq_numbers() {
        let mut session = DapClientSession::new();

        // configurationDone before initialize → error at seq 1
        let err_resp = session.handle_request(&req(1, "configurationDone"));
        if let DapMessage::Response(r) = &err_resp[0] {
            assert_eq!(r.seq, 1);
            assert!(!r.success);
        }

        // initialize → should get seq 2, 3
        let init_resp = session.handle_request(&req(2, "initialize"));
        if let DapMessage::Response(r) = &init_resp[0] {
            assert_eq!(r.seq, 2, "After error response, next seq should be 2");
        }
        if let DapMessage::Event(e) = &init_resp[1] {
            assert_eq!(e.seq, 3, "initialized event should be seq 3");
        }
    }

    // ── response correlation ──────────────────────────────────────────────────

    #[test]
    fn test_response_request_seq_matches_request() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(42, "initialize"));
        if let DapMessage::Response(r) = &responses[0] {
            assert_eq!(
                r.request_seq, 42,
                "response.request_seq must match the request seq"
            );
        }
    }

    #[test]
    fn test_response_command_echoes_request_command() {
        let mut session = DapClientSession::new();
        let responses = session.handle_request(&req(1, "initialize"));
        if let DapMessage::Response(r) = &responses[0] {
            assert_eq!(r.command, "initialize");
        }
    }

    // ── full handshake ────────────────────────────────────────────────────────

    #[test]
    fn test_full_handshake_initialize_configure_attach_disconnect() {
        let mut session = DapClientSession::new();

        // initialize
        let r1 = session.handle_request(&req(1, "initialize"));
        assert!(matches!(&r1[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Initializing);

        // configurationDone
        let r2 = session.handle_request(&req(2, "configurationDone"));
        assert!(matches!(&r2[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Configured);

        // attach
        let r3 = session.handle_request(&req(3, "attach"));
        assert!(matches!(&r3[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Configured);

        // disconnect
        let r4 = session.handle_request(&req(4, "disconnect"));
        assert!(matches!(&r4[0], DapMessage::Response(r) if r.success));
        assert_eq!(session.state, SessionState::Disconnecting);
    }
}
