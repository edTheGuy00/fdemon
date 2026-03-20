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

    /// The debug adapter supports conditional breakpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_conditional_breakpoints: Option<bool>,

    /// The debug adapter supports breakpoints that break execution after a
    /// specified number of hits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_hit_conditional_breakpoints: Option<bool>,

    /// The debug adapter supports the `terminate` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_terminate_request: Option<bool>,

    /// The debug adapter supports the `restart` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_restart_request: Option<bool>,

    /// The debug adapter supports the `clipboard` context in the `evaluate` request.
    ///
    /// When `true`, IDEs may send `evaluate` requests with `context: "clipboard"`
    /// to retrieve a full, untruncated representation of a value for pasting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_clipboard_context: Option<bool>,

    /// The debug adapter supports the `restartFrame` request.
    ///
    /// When `true`, IDEs show a "Restart Frame" action in the call stack view
    /// that allows rewinding execution to the start of the selected frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_restart_frame: Option<bool>,

    /// Available exception filter options for the `setExceptionBreakpoints` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception_breakpoint_filters: Option<Vec<ExceptionBreakpointsFilter>>,
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
// Thread and Execution Types
// ─────────────────────────────────────────────────────────────────────────────

/// DAP Thread object (maps to a Dart isolate).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapThread {
    /// Unique thread identifier.
    pub id: i64,
    /// A human-readable name for the thread.
    pub name: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Stack Frame Types
// ─────────────────────────────────────────────────────────────────────────────

/// DAP StackFrame returned in `stackTrace` responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapStackFrame {
    /// Unique identifier for the stack frame.
    pub id: i64,
    /// The name of the stack frame (e.g., function name).
    pub name: String,
    /// The source file associated with this frame, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DapSource>,
    /// The line within the source file (1-based).
    pub line: i64,
    /// The column within the source line (1-based).
    pub column: i64,
    /// The last line of the range covered by this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    /// The last column of the range covered by this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
    /// A hint for how to present this frame in the UI.
    /// Values: `"normal"`, `"label"`, `"subtle"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
}

/// DAP Source object identifying a source file.
///
/// `path` must be a filesystem path, not a `file://` URI.
/// Helix and other editors send `pathFormat: "path"` during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapSource {
    /// A human-readable name for the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The absolute filesystem path to the source file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// A reference handle for sources that cannot be identified by a path.
    /// For Phase 4 (SDK/package sources); always `None` or `0` in Phase 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<i64>,
    /// A hint for how to present this source in the UI.
    /// Values: `"normal"`, `"emphasize"`, `"deemphasize"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope and Variable Types
// ─────────────────────────────────────────────────────────────────────────────

/// DAP Scope returned in `scopes` responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapScope {
    /// The display name of this scope.
    pub name: String,
    /// A hint for how to present this scope in the UI.
    /// Values: `"arguments"`, `"locals"`, `"registers"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    /// The `variablesReference` for fetching the variables in this scope.
    pub variables_reference: i64,
    /// The number of named variables in this scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<i64>,
    /// The number of indexed variables in this scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<i64>,
    /// Whether fetching variables for this scope is expensive.
    pub expensive: bool,
}

/// DAP Variable returned in `variables` responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapVariable {
    /// The variable's name.
    pub name: String,
    /// The variable's value as a string.
    pub value: String,
    /// The variable's type (e.g., `"String"`, `"int"`).
    /// Renamed from `type_field` to `"type"` on the wire per DAP spec.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_field: Option<String>,
    /// If non-zero, the variable has children accessible via `variablesRequest`.
    pub variables_reference: i64,
    /// The number of named child variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<i64>,
    /// The number of indexed child variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<i64>,
    /// An optional expression that can be evaluated in the current scope to
    /// obtain the variable's value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
    /// Optional UI presentation hints for this variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<DapVariablePresentationHint>,
}

/// Presentation hints that control how a variable is displayed in the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapVariablePresentationHint {
    /// The kind of variable. Common values: `"property"`, `"method"`, `"class"`,
    /// `"data"`, `"event"`, `"baseClass"`, `"innerClass"`, `"interface"`,
    /// `"mostDerivedClass"`, `"virtual"`, `"dataBreakpoint"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// A list of attributes. Common values: `"static"`, `"constant"`,
    /// `"readOnly"`, `"rawString"`, `"hasObjectId"`, `"canHaveObjectId"`,
    /// `"hasSideEffects"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<String>>,
    /// The visibility of the variable. Common values: `"public"`, `"private"`,
    /// `"protected"`, `"internal"`, `"final"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// If `true`, the client should show the variable as lazy — its value is
    /// not yet computed and the user must explicitly expand or click to evaluate
    /// it. Used for getter evaluation when `evaluateGettersInDebugViews` is
    /// `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lazy: Option<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Breakpoint Types
// ─────────────────────────────────────────────────────────────────────────────

/// DAP Breakpoint returned in `setBreakpoints` responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DapBreakpoint {
    /// The server-assigned breakpoint ID. Used to correlate with breakpoint events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Whether the breakpoint was set successfully.
    pub verified: bool,
    /// A human-readable message describing why the breakpoint could not be verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The source where this breakpoint was placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DapSource>,
    /// The actual line number of the breakpoint (may differ from requested).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    /// The actual column of the breakpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    /// The last line of the actual range covered by the breakpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    /// The last column of the actual range covered by the breakpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
}

/// A source breakpoint specified by the client in a `setBreakpoints` request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    /// The source line of the breakpoint (1-based).
    pub line: i64,
    /// An optional source column of the breakpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    /// An optional expression for conditional breakpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// An optional expression that controls how many hits of the breakpoint
    /// are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    /// If this attribute exists and is non-empty, the backend must not suspend
    /// the debuggee on this breakpoint but must instead log a message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

/// Arguments for the `setBreakpoints` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsArguments {
    /// The source file to set breakpoints in.
    pub source: DapSource,
    /// The code locations of the breakpoints. An empty list clears all
    /// breakpoints in the given source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoints: Option<Vec<SourceBreakpoint>>,
    /// A hint to the backend that the client will send a `setBreakpoints`
    /// request again if the source has been modified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_modified: Option<bool>,
}

/// Arguments for the `setExceptionBreakpoints` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetExceptionBreakpointsArguments {
    /// IDs of the exception filters to enable.
    pub filters: Vec<String>,
    /// Optional per-filter configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_options: Option<Vec<ExceptionFilterOptions>>,
}

/// Per-filter options for exception breakpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionFilterOptions {
    /// ID of the exception filter to configure.
    pub filter_id: String,
    /// An optional expression for conditional exception breakpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// An exception breakpoint filter advertised in `Capabilities`.
///
/// Clients show these filters as checkboxes in the exception breakpoints UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionBreakpointsFilter {
    /// The internal ID of this filter.
    pub filter: String,
    /// A human-readable label for the filter.
    pub label: String,
    /// A human-readable description of the filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the filter is initially enabled by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    /// Whether the filter supports a condition expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_condition: Option<bool>,
    /// A description for the condition expression field shown in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_description: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Evaluate Types
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments for the `evaluate` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateArguments {
    /// The expression to evaluate.
    pub expression: String,
    /// The stack frame in which to evaluate the expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<i64>,
    /// The evaluation context.
    /// Values: `"watch"`, `"repl"`, `"hover"`, `"clipboard"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Response body for the `evaluate` request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponseBody {
    /// The result of the evaluation.
    pub result: String,
    /// The type of the result.
    /// Renamed from `type_field` to `"type"` on the wire per DAP spec.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_field: Option<String>,
    /// If non-zero, the result has children accessible via `variablesRequest`.
    pub variables_reference: i64,
    /// The number of named child variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<i64>,
    /// The number of indexed child variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<i64>,
    /// Optional UI presentation hints for this result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<DapVariablePresentationHint>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Request Argument Types
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments for the `stackTrace` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceArguments {
    /// The thread for which to retrieve the stack trace.
    pub thread_id: i64,
    /// The index of the first frame to return. `0` or omitted means the innermost
    /// frame. Requires `supportsDelayedStackTraceLoading`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_frame: Option<i64>,
    /// The maximum number of frames to return. A value of `0` or omitted means
    /// all frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<i64>,
}

/// Arguments for the `scopes` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesArguments {
    /// The stack frame for which to retrieve scopes.
    pub frame_id: i64,
}

/// Arguments for the `variables` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesArguments {
    /// The `variablesReference` handle from a scope or variable.
    pub variables_reference: i64,
    /// Optional filter to retrieve only named or indexed children.
    /// Values: `"indexed"`, `"named"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// The index of the first variable to return (for paging).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    /// The number of variables to return (for paging).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

/// Arguments for the `continue` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueArguments {
    /// The thread to continue.
    pub thread_id: i64,
    /// If `true`, continue only the specified thread; others remain suspended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_thread: Option<bool>,
}

/// Arguments for `next`, `stepIn`, and `stepOut` requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepArguments {
    /// The thread to step.
    pub thread_id: i64,
    /// If `true`, step only the specified thread; others remain suspended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_thread: Option<bool>,
    /// The granularity of one step.
    /// Values: `"statement"`, `"line"`, `"instruction"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granularity: Option<String>,
}

/// Arguments for the `pause` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseArguments {
    /// The thread to pause.
    pub thread_id: i64,
}

/// Arguments for the `attach` request.
///
/// These are fdemon-specific fields sent by the IDE when attaching to a
/// running Flutter process.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachRequestArguments {
    /// The Dart VM Service URI to attach to (e.g., `"ws://127.0.0.1:8181/ws"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_service_uri: Option<String>,
    /// The fdemon session ID to attach to. When present, fdemon can reuse an
    /// existing session rather than spawning a new process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Whether to eagerly evaluate getter methods when expanding objects in
    /// the variables panel. When `true` (the default), getters are evaluated
    /// immediately with a 1-second timeout. When `false`, getters appear as
    /// lazy items the user can expand on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_getters_in_debug_views: Option<bool>,
    /// Whether to call `toString()` on `PlainInstance` objects and append the
    /// result to the display value in the variables panel.
    ///
    /// When `true` (the default), `toString()` is called on `PlainInstance`,
    /// `RegExp`, and `StackTrace` objects. If the result is useful (not the
    /// default `"Instance of 'ClassName'"` pattern), it is appended:
    /// `"MyClass (custom string repr)"`.
    ///
    /// When `false`, no `toString()` calls are made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_to_string_in_debug_views: Option<bool>,
    /// Whether to allow stepping into Dart SDK libraries (`dart:` URIs).
    ///
    /// When `true`, the debugger will step into SDK framework code. When
    /// `false` (the default), SDK libraries are marked as non-debuggable and
    /// the debugger skips over them during stepping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_sdk_libraries: Option<bool>,
    /// Whether to allow stepping into external package libraries.
    ///
    /// When `true`, the debugger will step into code from external packages
    /// (i.e., packages other than the app's own package). When `false` (the
    /// default), external packages are marked as non-debuggable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_external_package_libraries: Option<bool>,
    /// The name of the app's own package (from `pubspec.yaml`).
    ///
    /// When set, this is used to distinguish the app's own `package:` URIs
    /// (which are always debuggable) from external package URIs (controlled by
    /// `debug_external_package_libraries`). Defaults to using the session
    /// directory name when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
}

/// Arguments for the `restartFrame` request.
///
/// Sent by the IDE when the user chooses "Restart Frame" in the call stack.
/// The `frameId` identifies which frame to rewind to using the VM Service
/// `Rewind` step mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartFrameArguments {
    /// The frame to restart (rewind to).
    pub frame_id: i64,
}

/// Arguments for the `disconnect` request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectArguments {
    /// Indicates whether the debuggee should be restarted after disconnecting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<bool>,
    /// Indicates whether the debuggee should be terminated when the client
    /// disconnects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminate_debuggee: Option<bool>,
    /// Indicates whether the debuggee should be left in a suspended state after
    /// disconnecting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend_debuggee: Option<bool>,
}

/// Arguments for the `exceptionInfo` request.
///
/// Sent by the IDE when it needs structured exception details after the
/// debugger pauses at an exception. The adapter looks up the stored
/// exception reference for the given thread and returns rich exception
/// data including the exception class name, `toString()` output, and
/// optional stack trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionInfoArguments {
    /// The thread that is paused at the exception.
    pub thread_id: i64,
}

/// Arguments for the `breakpointLocations` request.
///
/// Sent by the IDE to discover valid breakpoint positions within a source
/// file, typically when the user hovers over the gutter. The adapter
/// queries the Dart VM Service `getSourceReport` RPC with the
/// `PossibleBreakpoints` report kind and returns the matching positions
/// filtered to the requested line range.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointLocationsArguments {
    /// The source file to query for valid breakpoint positions.
    pub source: DapSource,
    /// Start line of the range to query (1-based, inclusive).
    pub line: i64,
    /// Optional end line of the range to query (1-based, inclusive).
    ///
    /// When absent, only `line` is queried (a single-line range).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    /// Optional start column of the range (1-based, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    /// Optional end column of the range (1-based, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
}

/// A valid breakpoint position within a source file.
///
/// Returned in the `breakpoints` array of a `breakpointLocations` response.
/// Column information is included when available (column breakpoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointLocation {
    /// The 1-based line number where a breakpoint can be placed.
    pub line: i64,
    /// The 1-based column number where a breakpoint can be placed.
    ///
    /// When `None`, the entire line is a valid breakpoint location and the
    /// IDE may place the breakpoint at any column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    /// The last line of the range covered by this breakpoint location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    /// The last column of the range covered by this breakpoint location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
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

    /// Create an error response with a numeric error code.
    ///
    /// Produces a well-formed DAP error response with:
    /// - `success: false`
    /// - `message`: short human-readable description
    /// - `body.error.id`: numeric error code for programmatic handling
    /// - `body.error.format`: detailed error message (same as `message`)
    ///
    /// ## Error Code Conventions
    ///
    /// | Code | Meaning                          |
    /// |------|----------------------------------|
    /// | 1000 | VM Service not connected         |
    /// | 1001 | No active debug session          |
    /// | 1002 | Thread / isolate not found       |
    /// | 1003 | Evaluation failed                |
    /// | 1004 | Request timed out                |
    /// | 1005 | VM Service disconnected          |
    pub fn error_with_code(request: &DapRequest, code: i64, message: impl Into<String>) -> Self {
        let msg: String = message.into();
        Self {
            seq: 0,
            request_seq: request.seq,
            success: false,
            command: request.command.clone(),
            message: Some(msg.clone()),
            body: Some(serde_json::json!({
                "error": {
                    "id": code,
                    "format": msg,
                }
            })),
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

    /// Create a `"stopped"` event indicating the debuggee has halted.
    ///
    /// # Arguments
    /// * `reason` - The reason for stopping: `"step"`, `"breakpoint"`, `"exception"`,
    ///   `"pause"`, `"entry"`, `"goto"`, `"function breakpoint"`, `"data breakpoint"`.
    /// * `thread_id` - The thread that caused the stop.
    /// * `description` - Optional additional human-readable description.
    pub fn stopped(reason: &str, thread_id: i64, description: Option<&str>) -> Self {
        let mut body = serde_json::json!({
            "reason": reason,
            "threadId": thread_id,
            "allThreadsStopped": true,
        });
        if let Some(desc) = description {
            body["description"] = serde_json::Value::String(desc.to_owned());
        }
        Self::new("stopped", Some(body))
    }

    /// Create a `"continued"` event indicating the debuggee has resumed.
    ///
    /// # Arguments
    /// * `thread_id` - The thread that has continued.
    /// * `all_threads_continued` - Whether all threads continued or just the specified one.
    pub fn continued(thread_id: i64, all_threads_continued: bool) -> Self {
        let body = serde_json::json!({
            "threadId": thread_id,
            "allThreadsContinued": all_threads_continued,
        });
        Self::new("continued", Some(body))
    }

    /// Create a `"thread"` event indicating a thread started or exited.
    ///
    /// # Arguments
    /// * `reason` - The reason: `"started"` or `"exited"`.
    /// * `thread_id` - The thread that started or exited.
    pub fn thread(reason: &str, thread_id: i64) -> Self {
        let body = serde_json::json!({
            "reason": reason,
            "threadId": thread_id,
        });
        Self::new("thread", Some(body))
    }

    /// Create a `"breakpoint"` event indicating a breakpoint was added, changed, or removed.
    ///
    /// # Arguments
    /// * `reason` - The reason: `"changed"`, `"new"`, or `"removed"`.
    /// * `breakpoint` - The breakpoint that changed.
    pub fn breakpoint(reason: &str, breakpoint: &DapBreakpoint) -> Self {
        let body = serde_json::json!({
            "reason": reason,
            "breakpoint": serde_json::to_value(breakpoint).unwrap_or(serde_json::Value::Null),
        });
        Self::new("breakpoint", Some(body))
    }

    /// Create an `"exited"` event indicating the debuggee has exited.
    ///
    /// # Arguments
    /// * `exit_code` - The exit code returned by the debuggee.
    pub fn exited(exit_code: i64) -> Self {
        let body = serde_json::json!({
            "exitCode": exit_code,
        });
        Self::new("exited", Some(body))
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
            supports_conditional_breakpoints: Some(true),
            supports_hit_conditional_breakpoints: Some(true),
            supports_evaluate_for_hovers: Some(true),
            supports_clipboard_context: Some(true),
            supports_log_points: Some(true),
            supports_terminate_request: Some(true),
            supports_restart_frame: Some(true),
            // supports_restart_request is NOT advertised until Task 13 implements the handler.
            // Advertising it without a handler causes IDE errors when the restart button is used.
            supports_delayed_stack_trace_loading: Some(true),
            supports_loaded_sources_request: Some(true),
            supports_exception_info_request: Some(true),
            supports_breakpoint_locations_request: Some(true),
            exception_breakpoint_filters: Some(vec![
                ExceptionBreakpointsFilter {
                    filter: "All".into(),
                    label: "All Exceptions".into(),
                    description: Some("Break on all thrown exceptions".into()),
                    default: Some(false),
                    supports_condition: Some(false),
                    condition_description: None,
                },
                ExceptionBreakpointsFilter {
                    filter: "Unhandled".into(),
                    label: "Uncaught Exceptions".into(),
                    description: Some("Break on exceptions not caught by application code".into()),
                    default: Some(true),
                    supports_condition: Some(false),
                    condition_description: None,
                },
            ]),
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
        assert_eq!(json["request_seq"], 5);
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
        // Phase 3 capabilities are enabled.
        assert_eq!(caps.supports_configuration_done_request, Some(true));
        assert_eq!(caps.supports_conditional_breakpoints, Some(true));
        assert_eq!(caps.supports_hit_conditional_breakpoints, Some(true));
        assert_eq!(caps.supports_evaluate_for_hovers, Some(true));
        assert_eq!(caps.supports_log_points, Some(true));
        assert_eq!(caps.supports_terminate_request, Some(true));
        assert_eq!(caps.supports_delayed_stack_trace_loading, Some(true));
        // Exception filters are set.
        let filters = caps.exception_breakpoint_filters.as_ref().unwrap();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].filter, "All");
        assert_eq!(filters[1].filter, "Unhandled");
        // loadedSources is now enabled (Task 11).
        assert_eq!(caps.supports_loaded_sources_request, Some(true));
        // Unimplemented capabilities remain None.
        assert!(caps.support_terminate_debuggee.is_none());
        // exceptionInfo is implemented in Task 09 — capability is advertised.
        assert_eq!(caps.supports_exception_info_request, Some(true));
        assert!(caps.supports_set_variable.is_none());
        assert!(caps.supports_value_formatting_options.is_none());
        // breakpointLocations is implemented in Task 15 — capability is advertised.
        assert_eq!(caps.supports_breakpoint_locations_request, Some(true));
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

    // ── Phase 3 type tests ───────────────────────────────────────────────────

    #[test]
    fn test_stopped_event_serialization() {
        let event = DapEvent::stopped("breakpoint", 1, None);
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["body"]["reason"], "breakpoint");
        assert_eq!(json["body"]["threadId"], 1);
        assert_eq!(json["body"]["allThreadsStopped"], true);
        // No description when None
        assert!(json["body"].get("description").is_none());
    }

    #[test]
    fn test_stopped_event_with_description() {
        let event = DapEvent::stopped("exception", 2, Some("NullPointerException"));
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["body"]["reason"], "exception");
        assert_eq!(json["body"]["threadId"], 2);
        assert_eq!(json["body"]["description"], "NullPointerException");
    }

    #[test]
    fn test_continued_event_serialization() {
        let event = DapEvent::continued(3, true);
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["event"], "continued");
        assert_eq!(json["body"]["threadId"], 3);
        assert_eq!(json["body"]["allThreadsContinued"], true);
    }

    #[test]
    fn test_thread_event_serialization() {
        let event = DapEvent::thread("started", 4);
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["event"], "thread");
        assert_eq!(json["body"]["reason"], "started");
        assert_eq!(json["body"]["threadId"], 4);
    }

    #[test]
    fn test_exited_event_serialization() {
        let event = DapEvent::exited(0);
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["event"], "exited");
        assert_eq!(json["body"]["exitCode"], 0);
    }

    #[test]
    fn test_breakpoint_event_serialization() {
        let bp = DapBreakpoint {
            id: Some(42),
            verified: true,
            message: None,
            source: Some(DapSource {
                path: Some("/app/lib/main.dart".into()),
                ..Default::default()
            }),
            line: Some(10),
            ..Default::default()
        };
        let event = DapEvent::breakpoint("new", &bp);
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["event"], "breakpoint");
        assert_eq!(json["body"]["reason"], "new");
        assert_eq!(json["body"]["breakpoint"]["id"], 42);
        assert_eq!(json["body"]["breakpoint"]["verified"], true);
        assert_eq!(
            json["body"]["breakpoint"]["source"]["path"],
            "/app/lib/main.dart"
        );
    }

    #[test]
    fn test_dap_variable_type_field_rename() {
        let var = DapVariable {
            type_field: Some("String".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&var).unwrap();
        // Must serialize as "type", not "typeField"
        assert!(json.get("type").is_some());
        assert!(json.get("typeField").is_none());
        assert_eq!(json["type"], "String");
    }

    #[test]
    fn test_dap_variable_roundtrip() {
        let original = DapVariable {
            name: "myVar".into(),
            value: "42".into(),
            type_field: Some("int".into()),
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
            evaluate_name: Some("myVar".into()),
            presentation_hint: None,
        };
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: DapVariable = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, "myVar");
        assert_eq!(deserialized.value, "42");
        assert_eq!(deserialized.type_field.as_deref(), Some("int"));
        assert_eq!(deserialized.evaluate_name.as_deref(), Some("myVar"));
    }

    #[test]
    fn test_capabilities_exception_filters() {
        let caps = Capabilities::fdemon_defaults();
        let json = serde_json::to_value(&caps).unwrap();
        let filters = json["exceptionBreakpointFilters"].as_array().unwrap();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0]["filter"], "All");
        assert_eq!(filters[1]["filter"], "Unhandled");
        assert_eq!(filters[0]["label"], "All Exceptions");
        assert_eq!(filters[1]["label"], "Uncaught Exceptions");
        assert_eq!(filters[0]["default"], false);
        assert_eq!(filters[1]["default"], true);
    }

    #[test]
    fn test_source_breakpoint_with_condition() {
        let bp = SourceBreakpoint {
            line: 42,
            condition: Some("x > 5".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&bp).unwrap();
        assert_eq!(json["line"], 42);
        assert_eq!(json["condition"], "x > 5");
        // column omitted when None
        assert!(json.get("column").is_none());
    }

    #[test]
    fn test_source_breakpoint_roundtrip() {
        let original = SourceBreakpoint {
            line: 15,
            column: Some(3),
            condition: None,
            hit_condition: Some("3".into()),
            log_message: Some("hit: {x}".into()),
        };
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: SourceBreakpoint = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.line, 15);
        assert_eq!(deserialized.column, Some(3));
        assert_eq!(deserialized.hit_condition.as_deref(), Some("3"));
        assert_eq!(deserialized.log_message.as_deref(), Some("hit: {x}"));
    }

    #[test]
    fn test_set_breakpoints_arguments_roundtrip() {
        let args = SetBreakpointsArguments {
            source: DapSource {
                path: Some("/app/lib/main.dart".into()),
                name: Some("main.dart".into()),
                ..Default::default()
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 10,
                ..Default::default()
            }]),
            source_modified: Some(false),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["source"]["path"], "/app/lib/main.dart");
        assert_eq!(json["breakpoints"][0]["line"], 10);
        assert_eq!(json["sourceModified"], false);
    }

    #[test]
    fn test_dap_thread_serialization() {
        let thread = DapThread {
            id: 1,
            name: "main".into(),
        };
        let json = serde_json::to_value(&thread).unwrap();
        assert_eq!(json["id"], 1);
        assert_eq!(json["name"], "main");
    }

    #[test]
    fn test_dap_stack_frame_serialization() {
        let frame = DapStackFrame {
            id: 1,
            name: "main".into(),
            source: Some(DapSource {
                path: Some("/app/lib/main.dart".into()),
                ..Default::default()
            }),
            line: 42,
            column: 5,
            end_line: None,
            end_column: None,
            presentation_hint: Some("normal".into()),
        };
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["id"], 1);
        assert_eq!(json["name"], "main");
        assert_eq!(json["line"], 42);
        assert_eq!(json["column"], 5);
        assert_eq!(json["source"]["path"], "/app/lib/main.dart");
        assert_eq!(json["presentationHint"], "normal");
        assert!(json.get("endLine").is_none());
    }

    #[test]
    fn test_dap_scope_serialization() {
        let scope = DapScope {
            name: "Locals".into(),
            presentation_hint: Some("locals".into()),
            variables_reference: 100,
            named_variables: Some(3),
            indexed_variables: None,
            expensive: false,
        };
        let json = serde_json::to_value(&scope).unwrap();
        assert_eq!(json["name"], "Locals");
        assert_eq!(json["presentationHint"], "locals");
        assert_eq!(json["variablesReference"], 100);
        assert_eq!(json["namedVariables"], 3);
        assert_eq!(json["expensive"], false);
        assert!(json.get("indexedVariables").is_none());
    }

    #[test]
    fn test_dap_breakpoint_serialization() {
        let bp = DapBreakpoint {
            id: Some(1),
            verified: true,
            message: None,
            source: Some(DapSource {
                path: Some("/app/lib/main.dart".into()),
                ..Default::default()
            }),
            line: Some(42),
            column: None,
            end_line: None,
            end_column: None,
        };
        let json = serde_json::to_value(&bp).unwrap();
        assert_eq!(json["id"], 1);
        assert_eq!(json["verified"], true);
        assert_eq!(json["line"], 42);
        assert!(json.get("column").is_none());
        assert!(json.get("message").is_none());
    }

    #[test]
    fn test_evaluate_response_body_type_field_rename() {
        let body = EvaluateResponseBody {
            result: "hello".into(),
            type_field: Some("String".into()),
            variables_reference: 0,
            ..Default::default()
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["result"], "hello");
        assert_eq!(json["type"], "String");
        assert!(json.get("typeField").is_none());
    }

    #[test]
    fn test_stack_trace_arguments_roundtrip() {
        let args = StackTraceArguments {
            thread_id: 1,
            start_frame: Some(0),
            levels: Some(20),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["threadId"], 1);
        assert_eq!(json["startFrame"], 0);
        assert_eq!(json["levels"], 20);
    }

    #[test]
    fn test_continue_arguments_roundtrip() {
        let args = ContinueArguments {
            thread_id: 2,
            single_thread: Some(true),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["threadId"], 2);
        assert_eq!(json["singleThread"], true);
    }

    #[test]
    fn test_step_arguments_roundtrip() {
        let args = StepArguments {
            thread_id: 3,
            single_thread: None,
            granularity: Some("line".into()),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["threadId"], 3);
        assert_eq!(json["granularity"], "line");
        assert!(json.get("singleThread").is_none());
    }

    #[test]
    fn test_attach_request_arguments_roundtrip() {
        let args = AttachRequestArguments {
            vm_service_uri: Some("ws://127.0.0.1:8181/ws".into()),
            session_id: Some("abc-123".into()),
            evaluate_getters_in_debug_views: None,
            evaluate_to_string_in_debug_views: None,
            debug_sdk_libraries: None,
            debug_external_package_libraries: None,
            package_name: None,
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["vmServiceUri"], "ws://127.0.0.1:8181/ws");
        assert_eq!(json["sessionId"], "abc-123");
    }

    #[test]
    fn test_disconnect_arguments_optional_fields() {
        // All fields optional — empty object must serialize/deserialize cleanly
        let args = DisconnectArguments::default();
        let json = serde_json::to_value(&args).unwrap();
        assert!(json.get("restart").is_none());
        assert!(json.get("terminateDebuggee").is_none());
        assert!(json.get("suspendDebuggee").is_none());
    }

    #[test]
    fn test_set_exception_breakpoints_arguments() {
        let args = SetExceptionBreakpointsArguments {
            filters: vec!["Unhandled".into()],
            filter_options: None,
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["filters"][0], "Unhandled");
        assert!(json.get("filterOptions").is_none());
    }

    #[test]
    fn test_exception_filter_options_roundtrip() {
        let opt = ExceptionFilterOptions {
            filter_id: "All".into(),
            condition: Some("true".into()),
        };
        let json = serde_json::to_value(&opt).unwrap();
        assert_eq!(json["filterId"], "All");
        assert_eq!(json["condition"], "true");
    }

    #[test]
    fn test_variables_arguments_with_paging() {
        let args = VariablesArguments {
            variables_reference: 50,
            filter: Some("named".into()),
            start: Some(0),
            count: Some(10),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["variablesReference"], 50);
        assert_eq!(json["filter"], "named");
        assert_eq!(json["start"], 0);
        assert_eq!(json["count"], 10);
    }

    #[test]
    fn test_dap_source_path_not_uri() {
        // DapSource.path must be a filesystem path, not a file:// URI
        let src = DapSource {
            path: Some("/Users/dev/app/lib/main.dart".into()),
            name: Some("main.dart".into()),
            source_reference: None,
            presentation_hint: None,
        };
        let json = serde_json::to_value(&src).unwrap();
        // path must not start with "file://"
        let path_val = json["path"].as_str().unwrap();
        assert!(!path_val.starts_with("file://"), "path must not be a URI");
        assert_eq!(path_val, "/Users/dev/app/lib/main.dart");
    }

    #[test]
    fn test_variable_presentation_hint_roundtrip() {
        let hint = DapVariablePresentationHint {
            kind: Some("property".into()),
            attributes: Some(vec!["readOnly".into()]),
            visibility: Some("public".into()),
            lazy: None,
        };
        let serialized = serde_json::to_string(&hint).unwrap();
        let deserialized: DapVariablePresentationHint = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.kind.as_deref(), Some("property"));
        assert_eq!(
            deserialized.attributes.as_deref(),
            Some(["readOnly".to_owned()].as_slice())
        );
        assert_eq!(deserialized.visibility.as_deref(), Some("public"));
    }

    #[test]
    fn test_pause_arguments_serialization() {
        let args = PauseArguments { thread_id: 7 };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["threadId"], 7);
    }

    #[test]
    fn test_scopes_arguments_serialization() {
        let args = ScopesArguments { frame_id: 3 };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["frameId"], 3);
    }

    #[test]
    fn test_evaluate_arguments_roundtrip() {
        let args = EvaluateArguments {
            expression: "myVar.toString()".into(),
            frame_id: Some(1),
            context: Some("hover".into()),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["expression"], "myVar.toString()");
        assert_eq!(json["frameId"], 1);
        assert_eq!(json["context"], "hover");
    }

    #[test]
    fn test_capabilities_phase3_fields_in_json() {
        let caps = Capabilities::fdemon_defaults();
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["supportsConditionalBreakpoints"], true);
        assert_eq!(json["supportsHitConditionalBreakpoints"], true);
        assert_eq!(json["supportsEvaluateForHovers"], true);
        assert_eq!(json["supportsLogPoints"], true);
        assert_eq!(json["supportsTerminateRequest"], true);
        assert_eq!(json["supportsDelayedStackTraceLoading"], true);
        // supportsRestartRequest is NOT advertised until Task 10 implements the handler.
        // Advertising it without a handler causes IDE errors when the restart button is clicked.
        assert!(json.get("supportsRestartRequest").is_none());
        // exceptionInfo is implemented in Task 09 — capability is advertised.
        assert_eq!(json["supportsExceptionInfoRequest"], true);
    }
}
