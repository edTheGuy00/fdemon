//! # DAP Protocol Types
//!
//! Hand-rolled subset of the Debug Adapter Protocol (DAP) message types needed
//! for Flutter debugging. References the [DAP specification](https://microsoft.github.io/debug-adapter-protocol/specification).
//!
//! The wire format uses a `"type"` discriminator field to distinguish between
//! `"request"`, `"response"`, and `"event"` messages. This module models that
//! structure using a tagged serde enum.

use serde::{Deserialize, Serialize};

/// Top-level DAP protocol message — discriminated by the `"type"` field.
///
/// All three variants are transmitted in the same Content-Length framed format.
/// The `"type"` field selects the variant during deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DapMessage {
    /// A request from the debug adapter client (e.g., VS Code).
    #[serde(rename = "request")]
    Request(DapRequest),

    /// A response from the debug adapter server to a prior client request.
    #[serde(rename = "response")]
    Response(DapResponse),

    /// An unsolicited event sent by the debug adapter server.
    #[serde(rename = "event")]
    Event(DapEvent),
}

/// A DAP request from the client (e.g., VS Code).
///
/// The `seq` is a monotonically increasing sequence number used to correlate
/// requests with responses. The `command` names the operation (e.g.,
/// `"initialize"`, `"launch"`, `"setBreakpoints"`). The optional `arguments`
/// payload is command-specific.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapRequest {
    /// Sequence number (monotonically increasing, per-sender).
    pub seq: i64,

    /// The command name (e.g., `"initialize"`, `"launch"`).
    pub command: String,

    /// Command-specific arguments, or `None` if the command takes no arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// A DAP response sent from the server to the client in reply to a request.
///
/// The `request_seq` correlates this response with the originating request.
/// `success` indicates whether the command succeeded. On failure, `message`
/// carries a human-readable error description.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapResponse {
    /// Sequence number assigned by the server.
    pub seq: i64,

    /// The `seq` of the request this response answers.
    pub request_seq: i64,

    /// Whether the request was handled successfully.
    pub success: bool,

    /// Echoes the command name from the originating request.
    pub command: String,

    /// Human-readable error message when `success` is `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Command-specific response body, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// A DAP event sent unsolicited from the server to the client.
///
/// Events notify the client of state changes (e.g., `"initialized"`,
/// `"stopped"`, `"output"`). The `body` is event-specific.
///
/// Note: `seq` is set to 0 in convenience constructors. The DAP server session
/// is responsible for assigning monotonic sequence numbers before transmission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapEvent {
    /// Sequence number assigned by the server.
    pub seq: i64,

    /// The event name (e.g., `"initialized"`, `"stopped"`, `"output"`).
    pub event: String,

    /// Event-specific payload, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// Server capabilities advertised during the DAP initialization handshake.
///
/// Fields correspond to the `Capabilities` object in the DAP specification.
/// Only capabilities relevant to Flutter debugging are included here; additional
/// fields can be added as Phase 3+ features are implemented.
///
/// Unknown fields are ignored by compliant clients, so adding new fields is
/// backward-compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// The debug adapter supports the `configurationDone` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_configuration_done_request: Option<bool>,

    /// The debug adapter supports the `terminateDebuggee` attribute on the
    /// `disconnect` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_terminate_debuggee: Option<bool>,

    /// The debug adapter supports a `format` attribute on value-returning
    /// requests (`variables`, `evaluate`, `stackTrace`, `exceptionInfo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_evaluate_for_hovers: Option<bool>,

    /// The debug adapter supports the `exceptionInfo` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_exception_info_request: Option<bool>,

    /// The debug adapter supports the `setVariable` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_set_variable: Option<bool>,

    /// The debug adapter supports the `format` attribute on value-returning requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_value_formatting_options: Option<bool>,

    /// The debug adapter supports the `loadedSources` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_loaded_sources_request: Option<bool>,

    /// The debug adapter supports log points by interpreting the `logMessage`
    /// attribute of the `SourceBreakpoint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_log_points: Option<bool>,

    /// The debug adapter supports the `breakpointLocations` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_breakpoint_locations_request: Option<bool>,

    /// The debug adapter supports the `startFrame` and `levels` arguments on
    /// the `stackTrace` request. Used for lazy/paginated stack trace loading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_delayed_stack_trace_loading: Option<bool>,
    // Phase 3+ additions go here (new fields are backward-compatible with
    // existing clients because unknown capability fields are ignored).
}

/// Arguments sent by the client in the DAP `"initialize"` request.
///
/// These fields describe the client's identity and capability preferences.
/// Fields the client omits are deserialized as `None`.
///
/// Note: `clientID` and `adapterID` use uppercase "ID" per the DAP spec (not
/// `clientId`/`adapterId`). These are explicitly renamed using `#[serde(rename)]`
/// rather than relying on `camelCase` which would produce lowercase `d`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequestArguments {
    /// A unique identifier for the client implementation (e.g., `"vscode"`).
    /// DAP spec uses `clientID` (uppercase ID).
    #[serde(default, rename = "clientID")]
    pub client_id: Option<String>,

    /// A human-readable name for the client (e.g., `"Visual Studio Code"`).
    #[serde(default)]
    pub client_name: Option<String>,

    /// The ID of the debug adapter (e.g., `"dart"`).
    /// DAP spec uses `adapterID` (uppercase ID).
    #[serde(default, rename = "adapterID")]
    pub adapter_id: Option<String>,

    /// The locale of the client, e.g., `"en-US"`.
    #[serde(default)]
    pub locale: Option<String>,

    /// Whether line numbers are 1-based. Defaults to `true` if omitted.
    #[serde(default)]
    pub lines_start_at1: Option<bool>,

    /// Whether column numbers are 1-based. Defaults to `true` if omitted.
    #[serde(default)]
    pub columns_start_at1: Option<bool>,

    /// Determines how paths are reported: `"path"` or `"uri"`.
    #[serde(default)]
    pub path_format: Option<String>,

    /// Client supports the optional `type` field in `Variable`.
    #[serde(default)]
    pub supports_variable_type: Option<bool>,

    /// Client supports the paging of variables in a `VariablesResponse`.
    #[serde(default)]
    pub supports_variable_paging: Option<bool>,

    /// Client supports the `runInTerminal` request.
    #[serde(default)]
    pub supports_run_in_terminal_request: Option<bool>,

    /// Client supports memory references.
    #[serde(default)]
    pub supports_memory_references: Option<bool>,

    /// Client supports progress reporting via `ProgressStart`, `ProgressUpdate`,
    /// and `ProgressEnd` events.
    #[serde(default)]
    pub supports_progress_reporting: Option<bool>,

    /// Client supports the `invalidated` event.
    #[serde(default)]
    pub supports_invalidated_event: Option<bool>,

    /// Client supports the `memory` event.
    #[serde(default)]
    pub supports_memory_event: Option<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Constructors
// ─────────────────────────────────────────────────────────────────────────────

impl DapResponse {
    /// Create a success response for the given request.
    ///
    /// Sets `request_seq` to the request's `seq`, `success` to `true`, and
    /// echoes the `command`. Sequence number is initialized to 0; the DAP
    /// server session assigns the final value before transmission.
    pub fn success(request: &DapRequest, body: Option<serde_json::Value>) -> Self {
        Self {
            seq: 0,
            request_seq: request.seq,
            success: true,
            command: request.command.clone(),
            message: None,
            body,
        }
    }

    /// Create an error response for the given request.
    ///
    /// Sets `request_seq` to the request's `seq`, `success` to `false`, and
    /// stores the error description in `message`.
    pub fn error(request: &DapRequest, message: impl Into<String>) -> Self {
        Self {
            seq: 0,
            request_seq: request.seq,
            success: false,
            command: request.command.clone(),
            message: Some(message.into()),
            body: None,
        }
    }
}

impl DapEvent {
    /// Create a new event with the given name and optional body.
    ///
    /// Sequence number is initialized to 0; the DAP server session assigns the
    /// final value before transmission.
    pub fn new(event: impl Into<String>, body: Option<serde_json::Value>) -> Self {
        Self {
            seq: 0,
            event: event.into(),
            body,
        }
    }

    /// Create the `"initialized"` event.
    ///
    /// This event is sent by the server immediately after a successful
    /// `"initialize"` response to signal that the adapter is ready to accept
    /// configuration requests (`setBreakpoints`, etc.).
    pub fn initialized() -> Self {
        Self::new("initialized", None)
    }

    /// Create a `"terminated"` event.
    ///
    /// Signals to the client that debugging has ended (e.g., the debuggee
    /// exited or the session was disconnected).
    pub fn terminated() -> Self {
        Self::new("terminated", None)
    }

    /// Create an `"output"` event for debug console output.
    ///
    /// # Arguments
    /// * `category` - The output category: `"console"`, `"stdout"`, `"stderr"`, or `"telemetry"`.
    /// * `output` - The text to display. Should end with a newline where appropriate.
    pub fn output(category: &str, output: &str) -> Self {
        let body = serde_json::json!({
            "category": category,
            "output": output,
        });
        Self::new("output", Some(body))
    }
}

impl Capabilities {
    /// Default capabilities for fdemon's Flutter DAP adapter.
    ///
    /// Declares the subset of DAP capabilities that fdemon implements for
    /// Flutter debugging. Fields not listed here are left as `None`, which
    /// signals to the client that the adapter does not support them.
    pub fn fdemon_defaults() -> Self {
        Self {
            supports_configuration_done_request: Some(true),
            support_terminate_debuggee: Some(true),
            supports_evaluate_for_hovers: Some(true),
            supports_exception_info_request: Some(true),
            supports_loaded_sources_request: Some(true),
            supports_log_points: Some(true),
            supports_delayed_stack_trace_loading: Some(true),
            ..Default::default()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── DapMessage serialization ──────────────────────────────────────────────

    #[test]
    fn test_dap_request_serialization() {
        let req = DapRequest {
            seq: 1,
            command: "initialize".into(),
            arguments: None,
        };
        let json = serde_json::to_value(DapMessage::Request(req)).unwrap();
        assert_eq!(json["type"], "request");
        assert_eq!(json["command"], "initialize");
        assert_eq!(json["seq"], 1);
        // `arguments` should be absent (skip_serializing_if = "Option::is_none")
        assert!(json.get("arguments").is_none());
    }

    #[test]
    fn test_dap_request_with_arguments_serialization() {
        let args = serde_json::json!({"clientID": "vscode"});
        let req = DapRequest {
            seq: 2,
            command: "initialize".into(),
            arguments: Some(args.clone()),
        };
        let json = serde_json::to_value(DapMessage::Request(req)).unwrap();
        assert_eq!(json["type"], "request");
        assert_eq!(json["arguments"]["clientID"], "vscode");
    }

    #[test]
    fn test_dap_response_serialization() {
        let resp = DapResponse {
            seq: 1,
            request_seq: 5,
            success: true,
            command: "initialize".into(),
            message: None,
            body: None,
        };
        let json = serde_json::to_value(DapMessage::Response(resp)).unwrap();
        assert_eq!(json["type"], "response");
        assert_eq!(json["success"], true);
        assert_eq!(json["requestSeq"], 5);
        assert_eq!(json["command"], "initialize");
    }

    #[test]
    fn test_dap_event_serialization() {
        let event = DapEvent::initialized();
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["type"], "event");
        assert_eq!(json["event"], "initialized");
        assert_eq!(json["seq"], 0);
        // body absent when None
        assert!(json.get("body").is_none());
    }

    #[test]
    fn test_dap_message_roundtrip_request() {
        let original = DapMessage::Request(DapRequest {
            seq: 42,
            command: "launch".into(),
            arguments: Some(serde_json::json!({"program": "/my/app"})),
        });
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: DapMessage = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            DapMessage::Request(r) => {
                assert_eq!(r.seq, 42);
                assert_eq!(r.command, "launch");
                assert!(r.arguments.is_some());
            }
            other => panic!("Expected Request, got {:?}", other),
        }
    }

    #[test]
    fn test_dap_message_roundtrip_response() {
        let original = DapMessage::Response(DapResponse {
            seq: 1,
            request_seq: 42,
            success: false,
            command: "launch".into(),
            message: Some("Not supported".into()),
            body: None,
        });
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: DapMessage = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            DapMessage::Response(r) => {
                assert_eq!(r.request_seq, 42);
                assert!(!r.success);
                assert_eq!(r.message.as_deref(), Some("Not supported"));
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[test]
    fn test_dap_message_roundtrip_event() {
        let original = DapMessage::Event(DapEvent::output("stdout", "Hello\n"));
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: DapMessage = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            DapMessage::Event(e) => {
                assert_eq!(e.event, "output");
                let body = e.body.as_ref().unwrap();
                assert_eq!(body["category"], "stdout");
                assert_eq!(body["output"], "Hello\n");
            }
            other => panic!("Expected Event, got {:?}", other),
        }
    }

    // ── DapResponse helpers ──────────────────────────────────────────────────

    #[test]
    fn test_dap_response_success_helper() {
        let req = DapRequest {
            seq: 5,
            command: "initialize".into(),
            arguments: None,
        };
        let resp = DapResponse::success(&req, None);
        assert_eq!(resp.request_seq, 5);
        assert!(resp.success);
        assert_eq!(resp.command, "initialize");
        assert!(resp.message.is_none());
        assert!(resp.body.is_none());
    }

    #[test]
    fn test_dap_response_success_with_body() {
        let req = DapRequest {
            seq: 7,
            command: "initialize".into(),
            arguments: None,
        };
        let body = serde_json::to_value(Capabilities::fdemon_defaults()).unwrap();
        let resp = DapResponse::success(&req, Some(body));
        assert_eq!(resp.request_seq, 7);
        assert!(resp.success);
        assert!(resp.body.is_some());
    }

    #[test]
    fn test_dap_response_error_helper() {
        let req = DapRequest {
            seq: 3,
            command: "launch".into(),
            arguments: None,
        };
        let resp = DapResponse::error(&req, "Project not found");
        assert_eq!(resp.request_seq, 3);
        assert!(!resp.success);
        assert_eq!(resp.command, "launch");
        assert_eq!(resp.message.as_deref(), Some("Project not found"));
        assert!(resp.body.is_none());
    }

    // ── DapEvent helpers ─────────────────────────────────────────────────────

    #[test]
    fn test_dap_event_initialized() {
        let event = DapEvent::initialized();
        assert_eq!(event.seq, 0);
        assert_eq!(event.event, "initialized");
        assert!(event.body.is_none());

        // Verify wire format matches DAP spec exactly
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["type"], "event");
        assert_eq!(json["event"], "initialized");
        assert_eq!(json["seq"], 0);
    }

    #[test]
    fn test_dap_event_terminated() {
        let event = DapEvent::terminated();
        assert_eq!(event.event, "terminated");
        assert!(event.body.is_none());
    }

    #[test]
    fn test_dap_event_output_category_and_text() {
        let event = DapEvent::output("stderr", "error: null check\n");
        assert_eq!(event.event, "output");
        let body = event.body.as_ref().unwrap();
        assert_eq!(body["category"], "stderr");
        assert_eq!(body["output"], "error: null check\n");
    }

    #[test]
    fn test_dap_event_new() {
        let body = serde_json::json!({"reason": "breakpoint"});
        let event = DapEvent::new("stopped", Some(body.clone()));
        assert_eq!(event.seq, 0);
        assert_eq!(event.event, "stopped");
        assert_eq!(event.body.as_ref().unwrap()["reason"], "breakpoint");
    }

    // ── Capabilities ─────────────────────────────────────────────────────────

    #[test]
    fn test_capabilities_fdemon_defaults() {
        let caps = Capabilities::fdemon_defaults();
        assert_eq!(caps.supports_configuration_done_request, Some(true));
        assert_eq!(caps.support_terminate_debuggee, Some(true));
        assert_eq!(caps.supports_evaluate_for_hovers, Some(true));
        assert_eq!(caps.supports_exception_info_request, Some(true));
        assert_eq!(caps.supports_loaded_sources_request, Some(true));
        assert_eq!(caps.supports_log_points, Some(true));
        assert_eq!(caps.supports_delayed_stack_trace_loading, Some(true));
        // Not set by defaults
        assert!(caps.supports_set_variable.is_none());
        assert!(caps.supports_value_formatting_options.is_none());
        assert!(caps.supports_breakpoint_locations_request.is_none());
    }

    #[test]
    fn test_capabilities_default_all_none() {
        let caps = Capabilities::default();
        assert!(caps.supports_configuration_done_request.is_none());
        assert!(caps.support_terminate_debuggee.is_none());
        assert!(caps.supports_evaluate_for_hovers.is_none());
    }

    #[test]
    fn test_capabilities_serialization_camel_case() {
        let caps = Capabilities {
            supports_configuration_done_request: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_value(&caps).unwrap();
        // DAP spec uses camelCase field names
        assert_eq!(json["supportsConfigurationDoneRequest"], true);
        // None fields are omitted
        assert!(json.get("supportTerminateDebuggee").is_none());
    }

    // ── InitializeRequestArguments ───────────────────────────────────────────

    #[test]
    fn test_initialize_request_from_vscode_json() {
        // Real VS Code initialize request payload
        let json = serde_json::json!({
            "clientID": "vscode",
            "clientName": "Visual Studio Code",
            "adapterID": "dart",
            "pathFormat": "path",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "supportsVariableType": true,
            "supportsVariablePaging": true,
            "supportsRunInTerminalRequest": true,
            "supportsMemoryReferences": true,
            "supportsProgressReporting": true,
            "supportsInvalidatedEvent": true,
            "supportsMemoryEvent": true
        });
        let args: InitializeRequestArguments = serde_json::from_value(json).unwrap();
        assert_eq!(args.client_id.as_deref(), Some("vscode"));
        assert_eq!(args.client_name.as_deref(), Some("Visual Studio Code"));
        assert_eq!(args.adapter_id.as_deref(), Some("dart"));
        assert_eq!(args.path_format.as_deref(), Some("path"));
        assert_eq!(args.lines_start_at1, Some(true));
        assert_eq!(args.columns_start_at1, Some(true));
        assert_eq!(args.supports_variable_type, Some(true));
        assert_eq!(args.supports_variable_paging, Some(true));
        assert_eq!(args.supports_run_in_terminal_request, Some(true));
        assert_eq!(args.supports_memory_references, Some(true));
        assert_eq!(args.supports_progress_reporting, Some(true));
        assert_eq!(args.supports_invalidated_event, Some(true));
        assert_eq!(args.supports_memory_event, Some(true));
    }

    #[test]
    fn test_initialize_request_partial_fields() {
        // Minimal client that only sends required fields
        let json = serde_json::json!({
            "clientID": "neovim",
            "adapterID": "dart"
        });
        let args: InitializeRequestArguments = serde_json::from_value(json).unwrap();
        assert_eq!(args.client_id.as_deref(), Some("neovim"));
        assert!(args.client_name.is_none());
        assert!(args.lines_start_at1.is_none());
        assert!(args.supports_memory_event.is_none());
    }

    #[test]
    fn test_initialize_request_empty_object() {
        // Edge case: empty object must deserialize without error
        let json = serde_json::json!({});
        let args: InitializeRequestArguments = serde_json::from_value(json).unwrap();
        assert!(args.client_id.is_none());
        assert!(args.adapter_id.is_none());
    }
}
