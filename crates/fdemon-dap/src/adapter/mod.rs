//! # DAP Adapter Core
//!
//! The central bridge between the Debug Adapter Protocol (DAP) and the Dart VM
//! Service. Each [`DapAdapter`] is bound to a single DAP client session and
//! a single Flutter debug session.
//!
//! ## Architecture
//!
//! `fdemon-dap` cannot depend on `fdemon-daemon` or `fdemon-app` — that would
//! create circular dependencies. Instead this module defines the [`DebugBackend`]
//! trait that describes the debug operations the adapter needs. The concrete
//! implementation lives in `fdemon-app` (the Engine integration layer).
//!
//! ## Sub-modules
//!
//! - [`threads`] — Thread/isolate ID mapping, `handle_threads`, `handle_attach`
//! - [`breakpoints`] — Breakpoint state, `handle_set_breakpoints`
//! - [`stack`] — Frame/variable stores, `handle_stack_trace`, `handle_scopes`, `handle_variables`
//! - [`evaluate`] — Expression evaluation, `handle_evaluate`

pub mod breakpoints;
pub mod evaluate;
pub mod stack;
pub mod threads;

use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::protocol::types::{
    AttachRequestArguments, ContinueArguments, DapBreakpoint, DapScope, DapSource, DapStackFrame,
    DapThread, DapVariable, PauseArguments, ScopesArguments, SetBreakpointsArguments,
    SetExceptionBreakpointsArguments, StackTraceArguments, StepArguments, VariablesArguments,
};
use crate::{DapMessage, DapRequest, DapResponse};

pub use breakpoints::BreakpointState;
pub use stack::{
    dart_uri_to_path, extract_line_column, extract_source, FrameRef, FrameStore, ScopeKind,
    VariableRef, VariableStore,
};
pub use threads::ThreadMap;

// ─────────────────────────────────────────────────────────────────────────────
// DebugBackend trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait abstracting the debug operations the DAP adapter needs.
///
/// Implemented by the Engine integration layer to bridge to the actual
/// VM Service client. This avoids `fdemon-dap` depending on `fdemon-daemon`.
///
/// The trait uses `trait-variant` to automatically generate a `Send`-compatible
/// version, matching the pattern used in `fdemon-app/src/services/`.
#[trait_variant::make(DebugBackend: Send)]
pub trait LocalDebugBackend: Sync + 'static {
    // ── Execution control ─────────────────────────────────────────────────

    /// Pause a running isolate.
    async fn pause(&self, isolate_id: &str) -> Result<(), String>;

    /// Resume a paused isolate, optionally with a step mode.
    async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), String>;

    // ── Breakpoints ───────────────────────────────────────────────────────

    /// Add a breakpoint at the given source URI and line.
    async fn add_breakpoint(
        &self,
        isolate_id: &str,
        uri: &str,
        line: i32,
        column: Option<i32>,
    ) -> Result<BreakpointResult, String>;

    /// Remove a previously added breakpoint by its VM ID.
    async fn remove_breakpoint(&self, isolate_id: &str, breakpoint_id: &str) -> Result<(), String>;

    /// Set the exception pause mode (`"None"`, `"Unhandled"`, or `"All"`).
    async fn set_exception_pause_mode(&self, isolate_id: &str, mode: &str) -> Result<(), String>;

    // ── Stack inspection ──────────────────────────────────────────────────

    /// Get the current call stack for a paused isolate.
    async fn get_stack(
        &self,
        isolate_id: &str,
        limit: Option<i32>,
    ) -> Result<serde_json::Value, String>;

    /// Get a VM Service object (for variable expansion).
    async fn get_object(
        &self,
        isolate_id: &str,
        object_id: &str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Result<serde_json::Value, String>;

    // ── Evaluation ────────────────────────────────────────────────────────

    /// Evaluate an expression in the context of a target object.
    async fn evaluate(
        &self,
        isolate_id: &str,
        target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, String>;

    /// Evaluate an expression in the context of a specific stack frame.
    async fn evaluate_in_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
        expression: &str,
    ) -> Result<serde_json::Value, String>;

    // ── Thread / isolate info ─────────────────────────────────────────────

    /// Get the VM object (lists all isolates).
    async fn get_vm(&self) -> Result<serde_json::Value, String>;

    /// Get the list of scripts loaded in an isolate.
    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, String>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Supporting types
// ─────────────────────────────────────────────────────────────────────────────

/// Step mode for resume operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// Step over the current statement (stay in the same function).
    Over,
    /// Step into a function call.
    Into,
    /// Step out of the current function.
    Out,
}

/// Result from adding a breakpoint via the VM Service.
#[derive(Debug, Clone)]
pub struct BreakpointResult {
    /// The VM Service breakpoint ID.
    pub vm_id: String,
    /// Whether the breakpoint has been resolved to source.
    pub resolved: bool,
    /// The actual line the breakpoint was placed on (may differ from requested).
    pub line: Option<i32>,
    /// The actual column (if supported).
    pub column: Option<i32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Debug events
// ─────────────────────────────────────────────────────────────────────────────

/// Debug events forwarded from the Engine to the adapter.
///
/// These are translated from Dart VM Service stream events and sent to the
/// adapter via the event channel. The adapter converts them to DAP events and
/// forwards them to the client.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// An isolate paused (e.g., at a breakpoint, step, or exception).
    Paused {
        /// Dart VM isolate ID (e.g., `"isolates/12345"`).
        isolate_id: String,
        /// Why the isolate paused.
        reason: PauseReason,
    },
    /// An isolate resumed execution.
    Resumed {
        /// Dart VM isolate ID.
        isolate_id: String,
    },
    /// A new isolate started (e.g., an isolate spawned by the Flutter app).
    IsolateStart {
        /// Dart VM isolate ID.
        isolate_id: String,
        /// Human-readable name of the isolate.
        name: String,
    },
    /// An isolate exited.
    IsolateExit {
        /// Dart VM isolate ID.
        isolate_id: String,
    },
    /// A breakpoint was resolved to a specific source location by the VM.
    BreakpointResolved {
        /// The VM Service breakpoint ID.
        vm_breakpoint_id: String,
        /// The resolved source line (1-based).
        line: Option<i32>,
        /// The resolved source column (1-based), if applicable.
        column: Option<i32>,
    },
    /// The Flutter app process exited.
    AppExited {
        /// The process exit code, if available.
        exit_code: Option<i64>,
    },
    /// A Flutter application log message to forward to the debug console.
    ///
    /// The `level` field uses a lowercase string representation of the log
    /// level (e.g., `"error"`, `"warning"`, `"info"`, `"debug"`) to keep
    /// the `DebugEvent` enum independent of fdemon-core's `LogLevel` type.
    /// Use [`log_level_to_category`] to map it to a DAP output category.
    LogOutput {
        /// The log message text.
        message: String,
        /// Log level as a lowercase string (`"error"`, `"warning"`, `"info"`, `"debug"`).
        level: String,
        /// Optional source file URI (`"file:///path/to/file.dart"`).
        source_uri: Option<String>,
        /// Optional source line number (1-based).
        line: Option<i32>,
    },
}

/// Reason for a pause event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseReason {
    /// Paused at a breakpoint.
    Breakpoint,
    /// Paused due to an exception.
    Exception,
    /// Paused after a step operation.
    Step,
    /// Paused by a `pause` request (user-initiated).
    Interrupted,
    /// Paused at isolate entry (before any user code).
    Entry,
    /// Paused at isolate exit.
    Exit,
}

// ─────────────────────────────────────────────────────────────────────────────
// Log level → DAP output category mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a log level string to a DAP output event `category` field.
///
/// | Log level      | DAP category | Notes                                    |
/// |----------------|--------------|------------------------------------------|
/// | `"error"`      | `"stderr"`   | Red/highlighted in most IDE consoles     |
/// | `"info"`       | `"stdout"`   | Standard application output              |
/// | `"warning"`    | `"console"`  | Informational messages                   |
/// | `"debug"`      | `"console"`  | Debug-level messages                     |
/// | anything else  | `"console"`  | Fallback (verbose, unknown, etc.)        |
///
/// The `"telemetry"` category is intentionally not used — it is hidden by
/// most IDEs and is reserved for machine-readable telemetry data.
pub fn log_level_to_category(level: &str) -> &'static str {
    match level {
        "error" => "stderr",
        "info" => "stdout",
        _ => "console",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Capacity constant
// ─────────────────────────────────────────────────────────────────────────────

/// Capacity of the event channel from the Engine to the adapter.
///
/// Bounded to prevent unbounded memory growth if the session writer falls
/// behind. 64 events is sufficient for typical debugging workloads; the
/// Engine will block if the channel fills (which indicates a slow writer).
const EVENT_CHANNEL_CAPACITY: usize = 64;

// ─────────────────────────────────────────────────────────────────────────────
// DapAdapter
// ─────────────────────────────────────────────────────────────────────────────

/// The core DAP adapter that translates between DAP protocol and VM Service.
///
/// Each `DapAdapter` instance is bound to a single DAP client session and
/// a single Flutter debug session. It holds per-session state for ID
/// allocation, variable references, and thread mapping.
///
/// # Lifecycle
///
/// 1. Construct with [`DapAdapter::new`] (providing the backend and event sender).
/// 2. Call [`DapAdapter::handle_request`] for each incoming DAP request.
/// 3. Call [`DapAdapter::handle_debug_event`] for each VM Service debug event.
/// 4. On resume, call [`DapAdapter::on_resume`] to invalidate per-stop state.
pub struct DapAdapter<B: DebugBackend> {
    /// The backend providing VM Service operations.
    ///
    /// Used by command handlers in Tasks 04–09. Suppressed until those tasks
    /// implement the handlers.
    #[allow(dead_code)]
    backend: B,

    /// Channel to send DAP events back to the session writer.
    event_tx: mpsc::Sender<DapMessage>,

    /// Isolate ID → DAP thread ID mapping.
    thread_map: ThreadMap,

    /// Human-readable name for each thread ID.
    ///
    /// Populated from the isolate name supplied in `IsolateStart` events and
    /// via `getVM()` during `attach`. When the isolate name is absent the
    /// handler falls back to `"Thread N"`.
    thread_names: HashMap<i64, String>,

    /// Per-stopped-state variable reference allocator and lookup.
    ///
    /// Reset on every resume; rebuilt from scratch on the next stop.
    var_store: VariableStore,

    /// Per-stopped-state frame ID allocator and lookup.
    ///
    /// Reset on every resume; rebuilt from scratch on the next stop.
    frame_store: FrameStore,

    /// Breakpoint tracking state.
    breakpoint_state: BreakpointState,

    /// Current exception pause mode.
    ///
    /// One of `"None"`, `"Unhandled"`, or `"All"`. Defaults to `"Unhandled"`.
    /// Set by `setExceptionBreakpoints` and applied to all known isolates.
    exception_mode: String,

    /// Ordered list of paused isolate IDs.
    ///
    /// The most recently paused isolate is at the back of the list. When an
    /// isolate resumes, it is removed from this list. Used by `handle_evaluate`
    /// to pick the isolate context when no `threadId` is provided.
    paused_isolates: Vec<String>,
}

impl<B: DebugBackend> DapAdapter<B> {
    /// Create a new [`DapAdapter`] with the given backend.
    ///
    /// Returns the adapter and the receiver end of the event channel. The
    /// caller (session task) should poll the receiver and forward events
    /// to the DAP client.
    pub fn new(backend: B) -> (Self, mpsc::Receiver<DapMessage>) {
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let adapter = Self {
            backend,
            event_tx,
            thread_map: ThreadMap::new(),
            thread_names: HashMap::new(),
            var_store: VariableStore::new(),
            frame_store: FrameStore::new(),
            breakpoint_state: BreakpointState::new(),
            exception_mode: "Unhandled".to_string(),
            paused_isolates: Vec::new(),
        };
        (adapter, event_rx)
    }

    /// Create a new [`DapAdapter`] with the given backend and a pre-existing
    /// event sender.
    ///
    /// Unlike [`DapAdapter::new`], this constructor takes an existing
    /// `mpsc::Sender<DapMessage>` rather than creating a new channel. The
    /// caller is responsible for polling the corresponding receiver.
    ///
    /// Used by [`DapClientSession`] when it creates the event channel itself
    /// and needs to share the sender with the adapter while retaining the
    /// receiver for the main select loop.
    pub fn new_with_tx(backend: B, event_tx: mpsc::Sender<DapMessage>) -> (Self, ()) {
        let adapter = Self {
            backend,
            event_tx,
            thread_map: ThreadMap::new(),
            thread_names: HashMap::new(),
            var_store: VariableStore::new(),
            frame_store: FrameStore::new(),
            breakpoint_state: BreakpointState::new(),
            exception_mode: "Unhandled".to_string(),
            paused_isolates: Vec::new(),
        };
        (adapter, ())
    }

    /// Handle a DAP request and return the response.
    ///
    /// This is the main dispatch point for all debugging commands. The session
    /// calls this for every request that requires adapter involvement.
    /// Lifecycle requests (`initialize`, `configurationDone`) are handled by
    /// the session layer before this is called.
    pub async fn handle_request(&mut self, request: &DapRequest) -> DapResponse {
        match request.command.as_str() {
            "attach" => self.handle_attach(request).await,
            "threads" => self.handle_threads(request).await,
            "setBreakpoints" => self.handle_set_breakpoints(request).await,
            "setExceptionBreakpoints" => self.handle_set_exception_breakpoints(request).await,
            "continue" => self.handle_continue(request).await,
            "next" => self.handle_next(request).await,
            "stepIn" => self.handle_step_in(request).await,
            "stepOut" => self.handle_step_out(request).await,
            "pause" => self.handle_pause(request).await,
            "stackTrace" => self.handle_stack_trace(request).await,
            "scopes" => self.handle_scopes(request).await,
            "variables" => self.handle_variables(request).await,
            "evaluate" => self.handle_evaluate(request).await,
            "disconnect" => self.handle_disconnect(request).await,
            _ => DapResponse::error(request, format!("unsupported command: {}", request.command)),
        }
    }

    /// Notify the adapter of a VM Service debug event.
    ///
    /// Called by the Engine integration layer when a debug stream event arrives.
    /// The adapter translates it to the appropriate DAP events and sends them
    /// via [`event_tx`](DapAdapter::event_tx).
    pub async fn handle_debug_event(&mut self, event: DebugEvent) {
        match event {
            DebugEvent::IsolateStart { isolate_id, name } => {
                let thread_id = self.thread_map.get_or_create(&isolate_id);
                self.thread_names.insert(thread_id, name.clone());
                tracing::debug!(
                    "Isolate started: {} (thread {}), name: {}",
                    isolate_id,
                    thread_id,
                    name
                );
                let body = serde_json::json!({
                    "reason": "started",
                    "threadId": thread_id,
                });
                self.send_event("thread", Some(body)).await;
            }

            DebugEvent::IsolateExit { isolate_id } => {
                if let Some(thread_id) = self.thread_map.remove(&isolate_id) {
                    self.thread_names.remove(&thread_id);
                    tracing::debug!("Isolate exited: {} (thread {})", isolate_id, thread_id);
                    let body = serde_json::json!({
                        "reason": "exited",
                        "threadId": thread_id,
                    });
                    self.send_event("thread", Some(body)).await;
                }
            }

            DebugEvent::Paused { isolate_id, reason } => {
                let thread_id = self.thread_map.get_or_create(&isolate_id);
                let reason_str = pause_reason_to_dap_str(&reason);
                tracing::debug!(
                    "Isolate paused: {} (thread {}), reason: {}",
                    isolate_id,
                    thread_id,
                    reason_str
                );
                // Track the paused isolate for evaluate context resolution.
                // Remove any prior entry for this isolate, then push to back
                // so that the most recently paused isolate is last.
                self.paused_isolates.retain(|id| id != &isolate_id);
                self.paused_isolates.push(isolate_id);
                let body = serde_json::json!({
                    "reason": reason_str,
                    "threadId": thread_id,
                    "allThreadsStopped": true,
                });
                self.send_event("stopped", Some(body)).await;
            }

            DebugEvent::Resumed { isolate_id } => {
                if let Some(thread_id) = self.thread_map.thread_id_for(&isolate_id) {
                    tracing::debug!("Isolate resumed: {} (thread {})", isolate_id, thread_id);
                    // Remove the isolate from the paused set.
                    self.paused_isolates.retain(|id| id != &isolate_id);
                    self.on_resume();
                    let body = serde_json::json!({
                        "threadId": thread_id,
                        "allThreadsContinued": true,
                    });
                    self.send_event("continued", Some(body)).await;
                }
            }

            DebugEvent::BreakpointResolved {
                vm_breakpoint_id,
                line,
                column,
            } => {
                tracing::debug!("Breakpoint resolved: {}", vm_breakpoint_id);
                if let Some(bp) =
                    self.breakpoint_state
                        .resolve_breakpoint(&vm_breakpoint_id, line, column)
                {
                    let body = serde_json::json!({
                        "reason": "changed",
                        "breakpoint": {
                            "id": bp.dap_id,
                            "verified": true,
                            "line": bp.line,
                            "column": bp.column,
                        },
                    });
                    self.send_event("breakpoint", Some(body)).await;
                }
            }

            DebugEvent::AppExited { exit_code } => {
                tracing::debug!("App exited with code: {:?}", exit_code);
                let body = serde_json::json!({
                    "exitCode": exit_code.unwrap_or(0),
                });
                self.send_event("exited", Some(body)).await;
                self.send_event("terminated", None).await;
            }

            DebugEvent::LogOutput {
                message,
                level,
                source_uri,
                line,
            } => {
                let category = log_level_to_category(&level);

                // Ensure message ends with newline (DAP convention for output events).
                let output = if message.ends_with('\n') {
                    message
                } else {
                    format!("{}\n", message)
                };

                let mut body = serde_json::json!({
                    "category": category,
                    "output": output,
                });

                // Resolve source location for clickable links in IDE consoles.
                if let Some(uri) = source_uri {
                    let path = dart_uri_to_path(&uri);
                    let name = uri.rsplit('/').next().unwrap_or(&uri).to_string();
                    let source = DapSource {
                        name: Some(name),
                        path,
                        source_reference: None,
                        presentation_hint: None,
                    };
                    body["source"] = serde_json::to_value(&source).unwrap_or_default();
                    if let Some(line_number) = line {
                        body["line"] = serde_json::json!(line_number);
                    }
                }

                self.send_event("output", Some(body)).await;
            }
        }
    }

    /// Emit a plain text `output` event to the IDE debug console.
    ///
    /// This is a convenience wrapper for lifecycle messages (e.g., "Attached
    /// to VM Service", "Hot reload completed"). The `category` must be one of
    /// `"console"`, `"stdout"`, or `"stderr"`.
    ///
    /// The output text is sent as-is; append `'\n'` to the message if a
    /// newline separator is desired.
    pub async fn emit_output(&self, category: &str, output: &str) {
        let body = serde_json::json!({
            "category": category,
            "output": output,
        });
        self.send_event("output", Some(body)).await;
    }

    /// Invalidate per-stop state (variable references and frame IDs).
    ///
    /// Must be called whenever the debuggee resumes. Variable references and
    /// frame IDs are only valid while the debuggee is stopped; they must be
    /// rebuilt from scratch on the next stop.
    pub fn on_resume(&mut self) {
        self.var_store.reset();
        self.frame_store.reset();
    }

    /// Send a DAP event to the client via the event channel.
    async fn send_event(&self, event: &str, body: Option<serde_json::Value>) {
        use crate::DapEvent;

        let dap_event = DapEvent {
            seq: 0, // Sequence number is assigned by the session writer.
            event: event.to_string(),
            body,
        };
        // Ignore send errors — the channel closing means the session ended.
        let _ = self.event_tx.send(DapMessage::Event(dap_event)).await;
    }

    // ── Stub command handlers ──────────────────────────────────────────────
    //
    // These stubs return "not yet implemented" errors. Subsequent tasks
    // (04 through 09) replace each stub with a real implementation.

    /// Handle the `attach` request.
    ///
    /// Parses the attach arguments, calls `get_vm()` on the backend to
    /// discover existing isolates, populates the thread map, and emits a
    /// `thread` started event for each isolate found.
    async fn handle_attach(&mut self, request: &DapRequest) -> DapResponse {
        let _args: AttachRequestArguments = match request.arguments.as_ref() {
            Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
            None => AttachRequestArguments::default(),
        };

        match self.backend.get_vm().await {
            Ok(vm_info) => {
                // Discover pre-existing isolates from the VM object.
                if let Some(isolates) = vm_info.get("isolates").and_then(|v| v.as_array()) {
                    for isolate in isolates {
                        let id = isolate.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let name = isolate.get("name").and_then(|v| v.as_str()).unwrap_or("");

                        if id.is_empty() {
                            continue;
                        }

                        let thread_id = self.thread_map.get_or_create(id);
                        let display_name = if name.is_empty() {
                            format!("Thread {thread_id}")
                        } else {
                            name.to_string()
                        };
                        self.thread_names.insert(thread_id, display_name);

                        let body = serde_json::json!({
                            "reason": "started",
                            "threadId": thread_id,
                        });
                        self.send_event("thread", Some(body)).await;
                    }
                }
                DapResponse::success(request, None)
            }
            Err(e) => DapResponse::error(request, format!("Failed to attach: {e}")),
        }
    }

    /// Handle the `threads` request.
    ///
    /// Returns all known threads with their human-readable names. When a
    /// thread name is unavailable the fallback `"Thread N"` is used.
    async fn handle_threads(&mut self, request: &DapRequest) -> DapResponse {
        let mut threads: Vec<DapThread> = self
            .thread_map
            .all_threads()
            .map(|(id, _isolate_id)| {
                let name = self
                    .thread_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| format!("Thread {id}"));
                DapThread { id, name }
            })
            .collect();

        // Sort by ID for deterministic ordering.
        threads.sort_by_key(|t| t.id);

        let body = serde_json::json!({ "threads": threads });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `setBreakpoints` request.
    ///
    /// The `setBreakpoints` request is **per-file**: the client sends the
    /// complete desired set of breakpoints for one source file. This handler
    /// diffs the incoming list against the current state, removes breakpoints
    /// that are no longer wanted, and adds new ones via the VM Service backend.
    ///
    /// Breakpoints that cannot be immediately verified (e.g., no isolate is
    /// attached yet) are returned with `verified: false` and a descriptive
    /// message. The IDE will receive a `breakpoint` event (via
    /// [`handle_debug_event`] `BreakpointResolved`) when they resolve.
    ///
    /// # Phase 3 Limitations
    ///
    /// - Source paths are converted to `file://` URIs. Full `package:` URI
    ///   resolution (via `.dart_tool/package_config.json`) is deferred to
    ///   Phase 4.
    /// - Conditional breakpoints (`condition`) and log points (`logMessage`)
    ///   are stored in the request but are **not evaluated**. They behave as
    ///   unconditional breakpoints in Phase 3. Evaluation requires
    ///   `evaluateInFrame` at the pause point, which is implemented in Phase 4.
    async fn handle_set_breakpoints(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: setBreakpoints");

        let args = match parse_args::<SetBreakpointsArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Convert the source path to a file:// URI for the VM Service.
        let source_path = args.source.path.as_deref().unwrap_or("");
        let uri = path_to_dart_uri(source_path);

        // Desired breakpoints from the request (empty = clear all for this source).
        let desired = args.breakpoints.unwrap_or_default();

        // ── Step 1: Remove breakpoints no longer wanted ───────────────────────

        // Snapshot existing entries for this source before mutating.
        let existing: Vec<(i64, i32, String)> = self
            .breakpoint_state
            .iter_for_uri(&uri)
            .map(|e| (e.dap_id, e.line.unwrap_or(0), e.vm_id.clone()))
            .collect();

        for (dap_id, existing_line, vm_id) in &existing {
            let still_wanted = desired.iter().any(|d| d.line as i32 == *existing_line);

            if !still_wanted {
                if let Some(isolate_id) = self.primary_isolate_id() {
                    let _ = self.backend.remove_breakpoint(&isolate_id, vm_id).await;
                }
                self.breakpoint_state.remove_by_dap_id(*dap_id);
                tracing::debug!("Removed breakpoint {} (dap_id={})", vm_id, dap_id);
            }
        }

        // ── Step 2: Add / preserve breakpoints from the desired set ──────────

        let mut response_breakpoints: Vec<DapBreakpoint> = Vec::with_capacity(desired.len());

        for sbp in &desired {
            // Reuse an existing breakpoint at this exact line.
            if let Some(dap_id) = self.breakpoint_state.find_by_source_line(&uri, sbp.line) {
                if let Some(entry) = self.breakpoint_state.lookup_by_dap_id(dap_id) {
                    response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
                }
                continue;
            }

            // New breakpoint: attempt to add via the VM Service backend.
            match self.primary_isolate_id() {
                Some(isolate_id) => {
                    match self
                        .backend
                        .add_breakpoint(
                            &isolate_id,
                            &uri,
                            sbp.line as i32,
                            sbp.column.map(|c| c as i32),
                        )
                        .await
                    {
                        Ok(result) => {
                            let actual_line = result.line.or(Some(sbp.line as i32));
                            let actual_col = result.column.or(sbp.column.map(|c| c as i32));
                            let dap_id = self.breakpoint_state.add(
                                result.vm_id.clone(),
                                uri.clone(),
                                actual_line,
                                actual_col,
                                result.resolved,
                            );
                            tracing::debug!(
                                "Added breakpoint {}:{} → vm_id={} dap_id={}",
                                uri,
                                sbp.line,
                                result.vm_id,
                                dap_id,
                            );
                            let entry = self
                                .breakpoint_state
                                .lookup_by_dap_id(dap_id)
                                .expect("entry was just inserted");
                            response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
                        }
                        Err(err) => {
                            tracing::warn!(
                                "Failed to add breakpoint at {}:{}: {}",
                                uri,
                                sbp.line,
                                err
                            );
                            response_breakpoints.push(DapBreakpoint {
                                id: None,
                                verified: false,
                                message: Some(format!("Could not set breakpoint: {}", err)),
                                source: Some(args.source.clone()),
                                line: Some(sbp.line),
                                column: sbp.column,
                                ..Default::default()
                            });
                        }
                    }
                }
                None => {
                    // No isolate attached yet: return unverified pending breakpoint.
                    tracing::debug!(
                        "No active isolate; breakpoint at {}:{} is pending",
                        uri,
                        sbp.line
                    );
                    response_breakpoints.push(DapBreakpoint {
                        id: None,
                        verified: false,
                        message: Some(
                            "Breakpoint pending: no active debug session attached yet".to_string(),
                        ),
                        source: Some(args.source.clone()),
                        line: Some(sbp.line),
                        column: sbp.column,
                        ..Default::default()
                    });
                }
            }
        }

        let body = serde_json::json!({ "breakpoints": response_breakpoints });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `setExceptionBreakpoints` request.
    ///
    /// Maps DAP exception filter names to VM Service exception pause modes and
    /// applies the mode to all known isolates.
    ///
    /// # Supported Filters
    ///
    /// | DAP Filter   | VM Service Mode |
    /// |--------------|-----------------|
    /// | `"All"`      | `"All"`         |
    /// | `"Unhandled"`| `"Unhandled"`   |
    /// | (none)       | `"None"`        |
    ///
    /// `"All"` takes precedence when both `"All"` and `"Unhandled"` are present.
    async fn handle_set_exception_breakpoints(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: setExceptionBreakpoints");

        let args = match parse_args::<SetExceptionBreakpointsArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let mode = exception_filter_to_mode(&args.filters);
        self.exception_mode = mode.to_string();

        // Apply the mode to all known isolates.
        let isolate_ids: Vec<String> = self
            .thread_map
            .all_threads()
            .map(|(_, iso)| iso.to_string())
            .collect();

        for isolate_id in &isolate_ids {
            let _ = self
                .backend
                .set_exception_pause_mode(isolate_id, mode)
                .await;
        }

        tracing::debug!(
            "Exception pause mode set to '{}' across {} isolate(s)",
            mode,
            isolate_ids.len()
        );

        // DAP spec: exception breakpoints response has empty breakpoints array.
        let body = serde_json::json!({ "breakpoints": [] });
        DapResponse::success(request, Some(body))
    }

    /// Return the isolate ID of the primary (first registered) isolate, if any.
    ///
    /// Used as the target for breakpoint operations when no specific isolate is
    /// requested. In a typical Flutter app there is exactly one main isolate.
    fn primary_isolate_id(&self) -> Option<String> {
        self.thread_map
            .all_threads()
            .next()
            .map(|(_, iso)| iso.to_string())
    }

    /// Return the isolate ID of the most recently paused isolate, if any.
    ///
    /// Used by `handle_evaluate` to pick the evaluation context when no
    /// `frameId` is given. Returns `None` if no isolate is currently paused.
    fn most_recent_paused_isolate(&self) -> Option<&str> {
        self.paused_isolates.last().map(String::as_str)
    }

    /// Handle the `continue` request.
    ///
    /// Resumes the isolate associated with the given thread ID. Invalidates all
    /// per-stop state (variable references and frame IDs) before resuming, since
    /// those references are only valid while the debuggee is stopped.
    ///
    /// Returns `allThreadsContinued: true` because Dart resumes all isolates
    /// together when a continue is issued.
    async fn handle_continue(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: continue");

        let args = match parse_args::<ContinueArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        // Invalidate stopped-state references before resuming.
        self.on_resume();

        match self.backend.resume(&isolate_id, None).await {
            Ok(()) => {
                let body = serde_json::json!({ "allThreadsContinued": true });
                DapResponse::success(request, Some(body))
            }
            Err(e) => DapResponse::error(request, format!("Continue failed: {e}")),
        }
    }

    /// Handle the `next` (step over) request.
    ///
    /// Steps over the current statement, remaining in the same function.
    async fn handle_next(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: next");
        self.step(request, StepMode::Over).await
    }

    /// Handle the `stepIn` request.
    ///
    /// Steps into a function call on the current line.
    async fn handle_step_in(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: stepIn");
        self.step(request, StepMode::Into).await
    }

    /// Handle the `stepOut` request.
    ///
    /// Steps out of the current function, resuming execution after the call site.
    async fn handle_step_out(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: stepOut");
        self.step(request, StepMode::Out).await
    }

    /// Common implementation for step operations (`next`, `stepIn`, `stepOut`).
    ///
    /// Parses `StepArguments`, resolves the thread ID to an isolate ID,
    /// invalidates per-stop state, and calls `resume` with the given step mode.
    ///
    /// The `granularity` field (if present) is ignored in Phase 3 — Dart VM
    /// only supports line-level stepping.
    async fn step(&mut self, request: &DapRequest, mode: StepMode) -> DapResponse {
        let args = match parse_args::<StepArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        // Invalidate stopped-state references before resuming.
        self.on_resume();

        match self.backend.resume(&isolate_id, Some(mode)).await {
            Ok(()) => DapResponse::success(request, None),
            Err(e) => DapResponse::error(request, format!("Step failed: {e}")),
        }
    }

    /// Handle the `pause` request.
    ///
    /// Requests the Dart VM to pause the specified isolate. The isolate will
    /// pause at the next safe point and emit a `PauseInterrupted` event, which
    /// is translated to a `stopped` DAP event with reason `"pause"`.
    async fn handle_pause(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: pause");

        let args = match parse_args::<PauseArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        match self.backend.pause(&isolate_id).await {
            Ok(()) => DapResponse::success(request, None),
            Err(e) => DapResponse::error(request, format!("Pause failed: {e}")),
        }
    }

    /// Handle the `stackTrace` request.
    ///
    /// Returns the current call stack for a paused isolate, mapped from VM
    /// Service frame objects to [`DapStackFrame`] objects. Each frame is
    /// allocated a unique frame ID via [`FrameStore`] for later `scopes`
    /// and `variables` requests.
    ///
    /// # Pagination
    ///
    /// The `startFrame` and `levels` arguments are respected so that Zed (which
    /// sends `supportsDelayedStackTraceLoading: true`) can fetch frames lazily.
    ///
    /// # Async frames
    ///
    /// Dart's VM reports async suspension markers as frames with
    /// `kind: "AsyncSuspensionMarker"`. These are rendered with name
    /// `"<asynchronous gap>"` and `presentation_hint: "label"` to serve as
    /// visual separators, matching the behavior of the official Dart debugger.
    async fn handle_stack_trace(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: stackTrace");

        let args = match parse_args::<StackTraceArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        // Clamp the `levels` argument for the VM Service call.
        let limit = args.levels.map(|l| l as i32);

        let stack_json = match self.backend.get_stack(&isolate_id, limit).await {
            Ok(v) => v,
            Err(e) => return DapResponse::error(request, format!("Failed to get stack: {e}")),
        };

        let frames: &[serde_json::Value] = stack_json
            .get("frames")
            .and_then(|f| f.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        let total_frames = frames.len();
        let start_frame = args.start_frame.unwrap_or(0) as usize;

        let mut dap_frames: Vec<DapStackFrame> = Vec::new();

        for (i, frame) in frames.iter().enumerate().skip(start_frame) {
            let frame_index = i as i32;

            // Allocate a stable DAP frame ID for this frame.
            let frame_id = self.frame_store.allocate(FrameRef {
                isolate_id: isolate_id.clone(),
                frame_index,
            });

            let kind = frame.get("kind").and_then(|k| k.as_str()).unwrap_or("");

            // Async suspension markers are visual separators, not real frames.
            let (name, presentation_hint) = if kind == "AsyncSuspensionMarker" {
                ("<asynchronous gap>".to_string(), Some("label".to_string()))
            } else {
                let code_name = frame
                    .get("code")
                    .and_then(|c| c.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("<unknown>")
                    .to_string();
                (code_name, None)
            };

            let source = extract_source(frame);
            let (line, column) = extract_line_column(frame);

            dap_frames.push(DapStackFrame {
                id: frame_id,
                name,
                source,
                line: line.unwrap_or(0) as i64,
                column: column.unwrap_or(0) as i64,
                end_line: None,
                end_column: None,
                presentation_hint,
            });
        }

        let body = serde_json::json!({
            "stackFrames": dap_frames,
            "totalFrames": total_frames,
        });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `scopes` request.
    ///
    /// Returns the scopes (variable groupings) for a given stack frame. This
    /// handler is **synchronous** — it only allocates variable references for
    /// the scopes without making VM Service calls. The expensive work happens
    /// when the client later calls `variables` with each reference.
    ///
    /// # Scopes Returned
    ///
    /// - **Locals** (`expensive: false`) — local variables visible in this frame
    /// - **Globals** (`expensive: true`) — module-level variables (can be large)
    ///
    /// # Helix Compatibility
    ///
    /// Helix sets `supportsVariablePaging: false`, so the adapter must return
    /// the complete variable list when `variables` is called. The paging
    /// parameters (`start`, `count`) from `VariablesArguments` are ignored.
    async fn handle_scopes(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: scopes");

        let args = match parse_args::<ScopesArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let frame_ref = match self.frame_store.lookup(args.frame_id) {
            Some(fr) => fr.clone(),
            None => {
                return DapResponse::error(
                    request,
                    format!(
                        "Invalid frame ID {} (stale or unknown — did the program resume?)",
                        args.frame_id
                    ),
                )
            }
        };

        // Allocate a variable reference for the Locals scope.
        let locals_ref = self.var_store.allocate(VariableRef::Scope {
            frame_index: frame_ref.frame_index,
            scope_kind: ScopeKind::Locals,
        });

        // Allocate a variable reference for the Globals scope.
        let globals_ref = self.var_store.allocate(VariableRef::Scope {
            frame_index: frame_ref.frame_index,
            scope_kind: ScopeKind::Globals,
        });

        let scopes = vec![
            DapScope {
                name: "Locals".to_string(),
                presentation_hint: Some("locals".to_string()),
                variables_reference: locals_ref,
                named_variables: None,
                indexed_variables: None,
                expensive: false,
            },
            DapScope {
                name: "Globals".to_string(),
                // "globals" is not a standard DAP hint, but it is informative for
                // clients that support custom hints.
                presentation_hint: Some("globals".to_string()),
                variables_reference: globals_ref,
                named_variables: None,
                indexed_variables: None,
                expensive: true, // Globals can be large — flag for lazy loading.
            },
        ];

        let body = serde_json::json!({ "scopes": scopes });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `variables` request.
    ///
    /// Resolves a variable reference (from a prior `scopes` or `variables`
    /// response) to a list of DAP variables. Two kinds of reference are
    /// supported:
    ///
    /// - [`VariableRef::Scope`] — fetch the frame's locals from the VM Service
    ///   and map each `InstanceRef` to a [`DapVariable`].
    /// - [`VariableRef::Object`] — call `getObject` on the VM Service and
    ///   expand the object's children (list elements, map entries, or fields).
    ///
    /// Stale or unknown references (i.e., those from a previous stop that were
    /// invalidated by [`DapAdapter::on_resume`]) return a clear error.
    async fn handle_variables(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: variables");

        let args = match parse_args::<VariablesArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Look up what this reference points to.
        let var_ref = match self.var_store.lookup(args.variables_reference) {
            Some(vr) => vr.clone(),
            None => {
                return DapResponse::error(
                    request,
                    format!(
                    "Invalid variables reference {} (stale or unknown — did the program resume?)",
                    args.variables_reference
                ),
                )
            }
        };

        let variables = match var_ref {
            VariableRef::Scope {
                frame_index,
                scope_kind,
            } => self.get_scope_variables(frame_index, scope_kind).await,
            VariableRef::Object {
                isolate_id,
                object_id,
            } => {
                self.expand_object(
                    &isolate_id.clone(),
                    &object_id.clone(),
                    args.start,
                    args.count,
                )
                .await
            }
        };

        match variables {
            Ok(vars) => {
                let body = serde_json::json!({ "variables": vars });
                DapResponse::success(request, Some(body))
            }
            Err(e) => DapResponse::error(request, format!("Failed to get variables: {e}")),
        }
    }

    /// Fetch the variables for a scope (locals or globals) from the VM Service.
    ///
    /// For `Locals`: calls `get_stack` on the backend and maps each frame
    /// variable's `InstanceRef` to a [`DapVariable`].
    ///
    /// For `Globals`: returns an empty list in Phase 3 (globals are expensive
    /// and deferred to Phase 4).
    async fn get_scope_variables(
        &mut self,
        frame_index: i32,
        scope_kind: ScopeKind,
    ) -> Result<Vec<DapVariable>, String> {
        match scope_kind {
            ScopeKind::Locals => {
                // Look up the isolate ID for this frame.
                let isolate_id = self
                    .frame_store
                    .lookup_by_index(frame_index)
                    .map(|fr| fr.isolate_id.clone())
                    .ok_or_else(|| {
                        format!("Frame index {} not found in frame store", frame_index)
                    })?;

                // Fetch the stack up to frame_index + 1 to include our frame.
                let stack = self
                    .backend
                    .get_stack(&isolate_id, Some(frame_index + 1))
                    .await?;

                let frames = stack
                    .get("frames")
                    .and_then(|f| f.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[]);

                let frame = frames
                    .get(frame_index as usize)
                    .ok_or_else(|| format!("Frame index {} out of bounds", frame_index))?;

                let vars: Vec<serde_json::Value> = frame
                    .get("vars")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let isolate_id_clone = isolate_id.clone();
                let mut result = Vec::with_capacity(vars.len());
                for var in &vars {
                    let name = var
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let value = var.get("value").cloned().unwrap_or(serde_json::Value::Null);
                    result.push(self.instance_ref_to_variable(&name, &value, &isolate_id_clone));
                }
                Ok(result)
            }
            ScopeKind::Globals => {
                // Globals are expensive — return empty for now.
                // Phase 4 will add full support via the isolate's libraries.
                Ok(Vec::new())
            }
        }
    }

    /// Convert a VM Service `InstanceRef` JSON value to a DAP [`DapVariable`].
    ///
    /// Primitives (`Null`, `Bool`, `Int`, `Double`, `String`) are rendered
    /// inline with `variables_reference: 0` (no expansion). Complex types
    /// (collections and plain instances) are allocated a variable reference
    /// that the IDE can use to drill in further.
    fn instance_ref_to_variable(
        &mut self,
        name: &str,
        instance_ref: &serde_json::Value,
        isolate_id: &str,
    ) -> DapVariable {
        let kind = instance_ref
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("");
        let class_name = instance_ref
            .get("class")
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str());
        let value_as_string = instance_ref.get("valueAsString").and_then(|v| v.as_str());
        let obj_id = instance_ref.get("id").and_then(|i| i.as_str());

        match kind {
            // ── Primitives: inline value, no expansion ───────────────────────
            "Null" => DapVariable {
                name: name.to_string(),
                value: "null".to_string(),
                type_field: Some("Null".to_string()),
                variables_reference: 0,
                ..Default::default()
            },

            "Bool" => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("false").to_string(),
                type_field: Some("bool".to_string()),
                variables_reference: 0,
                ..Default::default()
            },

            "Int" | "Double" => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("0").to_string(),
                type_field: Some(kind.to_lowercase()),
                variables_reference: 0,
                ..Default::default()
            },

            "String" => {
                let value = value_as_string
                    .map(|s| format!("\"{}\"", s))
                    .unwrap_or_else(|| "\"\"".to_string());
                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some("String".to_string()),
                    variables_reference: 0,
                    ..Default::default()
                }
            }

            // ── Collections: expandable ──────────────────────────────────────
            "List" | "Map" | "Set" | "Uint8ClampedList" | "Uint8List" | "Int32List"
            | "Float64List" => {
                let length = instance_ref
                    .get("length")
                    .and_then(|l| l.as_i64())
                    .unwrap_or(0);
                let type_name = class_name.unwrap_or(kind);
                let value = format!("{} (length: {})", type_name, length);

                let var_ref = if let Some(id) = obj_id {
                    self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    })
                } else {
                    0
                };

                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some(type_name.to_string()),
                    variables_reference: var_ref,
                    indexed_variables: Some(length),
                    ..Default::default()
                }
            }

            // ── Plain instances: expandable via fields ───────────────────────
            "PlainInstance" | "Closure" | "RegExp" | "Type" | "StackTrace" => {
                let type_name = class_name.unwrap_or(kind);
                let value = value_as_string
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{} instance", type_name));

                let var_ref = if let Some(id) = obj_id {
                    self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    })
                } else {
                    0
                };

                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some(type_name.to_string()),
                    variables_reference: var_ref,
                    ..Default::default()
                }
            }

            // ── Fallback ─────────────────────────────────────────────────────
            _ => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("<unknown>").to_string(),
                type_field: class_name.map(|s| s.to_string()),
                variables_reference: 0,
                ..Default::default()
            },
        }
    }

    /// Expand a VM Service object into a list of [`DapVariable`] children.
    ///
    /// Fetches the full object via `get_object` and dispatches based on the
    /// object's `kind`:
    ///
    /// - `List` / typed arrays — indexed elements `[0]`, `[1]`, …
    /// - `Map` — keyed entries `[key]`, …
    /// - `PlainInstance` and others — named fields
    ///
    /// The `start` and `count` paging parameters are forwarded to the VM
    /// Service so that large collections can be fetched in chunks.
    async fn expand_object(
        &mut self,
        isolate_id: &str,
        object_id: &str,
        start: Option<i64>,
        count: Option<i64>,
    ) -> Result<Vec<DapVariable>, String> {
        let obj = self
            .backend
            .get_object(isolate_id, object_id, start, count)
            .await?;

        let obj_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match obj_type {
            "Instance" => {
                let kind = obj.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                match kind {
                    "List" | "Uint8List" | "Uint8ClampedList" | "Int32List" | "Float64List" => {
                        // Expand list elements.
                        let elements: Vec<serde_json::Value> = obj
                            .get("elements")
                            .and_then(|e| e.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let offset = start.unwrap_or(0);
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(elements.len());
                        for (i, elem) in elements.iter().enumerate() {
                            let index = offset + i as i64;
                            let elem_name = format!("[{}]", index);
                            result.push(self.instance_ref_to_variable(
                                &elem_name,
                                elem,
                                &isolate_id,
                            ));
                        }
                        Ok(result)
                    }

                    "Map" => {
                        // Expand map associations.
                        let associations: Vec<serde_json::Value> = obj
                            .get("associations")
                            .and_then(|a| a.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(associations.len());
                        for assoc in &associations {
                            let key = assoc
                                .get("key")
                                .and_then(|k| k.get("valueAsString"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let value = assoc
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let entry_name = format!("[{}]", key);
                            result.push(self.instance_ref_to_variable(
                                &entry_name,
                                &value,
                                &isolate_id,
                            ));
                        }
                        Ok(result)
                    }

                    _ => {
                        // Expand instance fields.
                        let fields: Vec<serde_json::Value> = obj
                            .get("fields")
                            .and_then(|f| f.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(fields.len());
                        for field in &fields {
                            let name = field
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let value = field
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            result.push(self.instance_ref_to_variable(&name, &value, &isolate_id));
                        }
                        Ok(result)
                    }
                }
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Handle the `evaluate` request.
    ///
    /// Evaluates an expression in the context of the current debug session.
    /// Dispatches to [`evaluate::handle_evaluate`] which calls either
    /// `evaluateInFrame` (when a `frameId` is provided) or `evaluate` on the
    /// root library (when no `frameId` is given).
    ///
    /// # Error Handling
    ///
    /// - No paused isolate → DAP error response
    /// - Invalid frame ID → DAP error response
    /// - VM Service error → DAP error response with the error message
    async fn handle_evaluate(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: evaluate");
        let paused = self.most_recent_paused_isolate().map(|s| s.to_string());
        evaluate::handle_evaluate(
            &self.backend,
            &self.frame_store,
            &mut self.var_store,
            paused.as_deref(),
            request,
        )
        .await
    }

    /// Handle the `disconnect` request.
    ///
    /// Sends a `terminated` event and returns success. The session layer will
    /// close the connection after receiving the response.
    async fn handle_disconnect(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: disconnect");
        self.send_event("terminated", None).await;
        DapResponse::success(request, None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Parse the `arguments` field of a [`DapRequest`] as `T`.
///
/// Returns `Err` with a human-readable message if the field is absent or
/// cannot be deserialized.
fn parse_args<T: serde::de::DeserializeOwned>(request: &DapRequest) -> Result<T, String> {
    match &request.arguments {
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|e| format!("invalid arguments: {}", e))
        }
        None => Err(format!("'{}' request requires arguments", request.command)),
    }
}

/// Convert an absolute filesystem path to a `file://` URI suitable for the
/// Dart VM Service.
///
/// # Phase 3 Note
///
/// This returns a plain `file://` URI. Full `package:` URI resolution (which
/// requires reading `.dart_tool/package_config.json`) is deferred to Phase 4.
/// The Dart VM Service accepts both `file://` and `package:` URIs for
/// `addBreakpointWithScriptUri`.
fn path_to_dart_uri(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    // Pass through paths that already have a URI scheme (file://, package:, etc.).
    if path.starts_with("file://") || path.starts_with("package:") || path.starts_with("dart:") {
        return path.to_string();
    }
    format!("file://{}", path)
}

/// Build a [`DapBreakpoint`] from a tracked [`BreakpointEntry`].
///
/// The `source` from the original `setBreakpoints` request is echoed back so
/// the IDE can correlate the response breakpoint with the source file.
fn entry_to_dap_breakpoint(
    entry: &breakpoints::BreakpointEntry,
    source: &DapSource,
) -> DapBreakpoint {
    DapBreakpoint {
        id: Some(entry.dap_id),
        verified: entry.verified,
        message: if entry.verified {
            None
        } else {
            Some("Breakpoint not yet resolved".to_string())
        },
        source: Some(source.clone()),
        line: entry.line.map(|l| l as i64),
        column: entry.column.map(|c| c as i64),
        ..Default::default()
    }
}

/// Map a set of DAP exception filter IDs to a VM Service exception pause mode.
///
/// The VM Service accepts `"None"`, `"Unhandled"`, or `"All"`.  `"All"` takes
/// precedence when both filters are requested.
fn exception_filter_to_mode(filters: &[String]) -> &'static str {
    if filters.iter().any(|f| f == "All") {
        "All"
    } else if filters.iter().any(|f| f == "Unhandled") {
        "Unhandled"
    } else {
        "None"
    }
}

/// Convert a [`PauseReason`] to the DAP `stopped` event reason string.
fn pause_reason_to_dap_str(reason: &PauseReason) -> &'static str {
    match reason {
        PauseReason::Breakpoint => "breakpoint",
        PauseReason::Exception => "exception",
        PauseReason::Step => "step",
        PauseReason::Interrupted => "pause",
        PauseReason::Entry => "entry",
        PauseReason::Exit => "exit",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock backend ──────────────────────────────────────────────────────

    /// A no-op backend for testing the adapter dispatch and state logic.
    struct MockBackend;

    impl DebugBackend for MockBackend {
        async fn pause(&self, _isolate_id: &str) -> Result<(), String> {
            Ok(())
        }

        async fn resume(&self, _isolate_id: &str, _step: Option<StepMode>) -> Result<(), String> {
            Ok(())
        }

        async fn add_breakpoint(
            &self,
            _isolate_id: &str,
            _uri: &str,
            line: i32,
            column: Option<i32>,
        ) -> Result<BreakpointResult, String> {
            // Echo the requested line and produce a unique-ish VM ID so that
            // tests can distinguish breakpoints on different lines.
            Ok(BreakpointResult {
                vm_id: format!("bp/line:{}", line),
                resolved: true,
                line: Some(line),
                column,
            })
        }

        async fn remove_breakpoint(
            &self,
            _isolate_id: &str,
            _breakpoint_id: &str,
        ) -> Result<(), String> {
            Ok(())
        }

        async fn set_exception_pause_mode(
            &self,
            _isolate_id: &str,
            _mode: &str,
        ) -> Result<(), String> {
            Ok(())
        }

        async fn get_stack(
            &self,
            _isolate_id: &str,
            _limit: Option<i32>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_object(
            &self,
            _isolate_id: &str,
            _object_id: &str,
            _offset: Option<i64>,
            _count: Option<i64>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate(
            &self,
            _isolate_id: &str,
            _target_id: &str,
            _expression: &str,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate_in_frame(
            &self,
            _isolate_id: &str,
            _frame_index: i32,
            _expression: &str,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_vm(&self) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_scripts(&self, _isolate_id: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }
    }

    fn make_request(seq: i64, command: &str) -> DapRequest {
        DapRequest {
            seq,
            command: command.into(),
            arguments: None,
        }
    }

    // ── DapAdapter construction ────────────────────────────────────────────

    #[test]
    fn test_adapter_new_returns_receiver() {
        let (_adapter, rx) = DapAdapter::new(MockBackend);
        // The receiver must be valid (not closed) as long as the adapter is alive.
        assert!(!rx.is_closed());
    }

    // ── handle_request dispatch ────────────────────────────────────────────

    #[tokio::test]
    async fn test_handle_request_unknown_command_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_request(1, "flyToMoon");
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("flyToMoon"),
            "Error message should include the command name, got: {:?}",
            msg
        );
    }

    // All previously-stub commands are now implemented:
    // - "attach" and "threads"            — Task 04
    // - "setBreakpoints", "setExceptionBreakpoints" — Task 05
    // - "continue", "next", "stepIn", "stepOut", "pause" — Task 06
    // - "stackTrace" and "scopes"         — Task 07
    // - "variables"                       — Task 08
    // - "evaluate"                        — Task 09
    //
    // This test verifies that `evaluate` without a paused isolate returns a
    // meaningful error rather than "not yet implemented".
    #[tokio::test]
    async fn test_handle_evaluate_no_paused_isolate_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);

        // Send an evaluate request with no isolate paused.
        let req = DapRequest {
            seq: 1,
            command: "evaluate".into(),
            arguments: Some(serde_json::json!({"expression": "1 + 1"})),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(
            !resp.success,
            "evaluate without paused isolate should return an error"
        );
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("No paused isolate"),
            "Expected 'No paused isolate' error, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_handle_evaluate_after_paused_event_succeeds() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);

        // Trigger a Paused event to register an isolate as paused.
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Breakpoint,
            })
            .await;

        // Allocate a frame so we can evaluate with a frameId (avoids the
        // get_root_library_id path which requires a real VM response).
        let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

        let req = DapRequest {
            seq: 2,
            command: "evaluate".into(),
            arguments: Some(serde_json::json!({"expression": "x", "frameId": frame_id})),
        };
        let resp = adapter.handle_request(&req).await;
        // MockBackend evaluate_in_frame returns Ok({}) — formats as "Object instance"
        assert!(
            resp.success,
            "evaluate with a paused isolate should succeed, got: {:?}",
            resp.message
        );
    }

    #[tokio::test]
    async fn test_handle_evaluate_clears_paused_on_resume() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);

        // Pause then resume.
        adapter.thread_map.get_or_create("isolates/1");
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Step,
            })
            .await;
        adapter
            .handle_debug_event(DebugEvent::Resumed {
                isolate_id: "isolates/1".into(),
            })
            .await;

        // After resume, no paused isolate.
        assert!(
            adapter.most_recent_paused_isolate().is_none(),
            "No isolate should be paused after resume"
        );
    }

    #[tokio::test]
    async fn test_handle_disconnect_returns_success() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_request(1, "disconnect");
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
    }

    #[tokio::test]
    async fn test_handle_disconnect_sends_terminated_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        let req = make_request(1, "disconnect");
        adapter.handle_request(&req).await;

        let msg = rx.try_recv().expect("Expected a terminated event");
        assert!(
            matches!(msg, DapMessage::Event(ref e) if e.event == "terminated"),
            "Expected terminated event, got: {:?}",
            msg
        );
    }

    // ── handle_debug_event ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_isolate_start_sends_thread_started_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;

        let msg = rx.try_recv().expect("Expected a thread event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.event, "thread");
            let body = e.body.unwrap();
            assert_eq!(body["reason"], "started");
            assert_eq!(body["threadId"], 1);
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_isolate_exit_sends_thread_exited_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        // Register the isolate first.
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        // Drain the start event.
        rx.try_recv().ok();

        adapter
            .handle_debug_event(DebugEvent::IsolateExit {
                isolate_id: "isolates/1".into(),
            })
            .await;

        let msg = rx.try_recv().expect("Expected a thread event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.event, "thread");
            let body = e.body.unwrap();
            assert_eq!(body["reason"], "exited");
            assert_eq!(body["threadId"], 1);
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_isolate_exit_unknown_isolate_sends_no_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateExit {
                isolate_id: "isolates/999".into(),
            })
            .await;
        // No event should be sent for an unknown isolate.
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_paused_sends_stopped_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Breakpoint,
            })
            .await;

        let msg = rx.try_recv().expect("Expected a stopped event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.event, "stopped");
            let body = e.body.unwrap();
            assert_eq!(body["reason"], "breakpoint");
            assert_eq!(body["allThreadsStopped"], true);
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_resumed_sends_continued_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        // Register the isolate first (to assign a thread ID).
        adapter.thread_map.get_or_create("isolates/1");

        adapter
            .handle_debug_event(DebugEvent::Resumed {
                isolate_id: "isolates/1".into(),
            })
            .await;

        let msg = rx.try_recv().expect("Expected a continued event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.event, "continued");
            let body = e.body.unwrap();
            assert_eq!(body["allThreadsContinued"], true);
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_app_exited_sends_exited_and_terminated_events() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::AppExited { exit_code: Some(0) })
            .await;

        let ev1 = rx.try_recv().expect("Expected exited event");
        let ev2 = rx.try_recv().expect("Expected terminated event");

        assert!(matches!(ev1, DapMessage::Event(ref e) if e.event == "exited"));
        assert!(matches!(ev2, DapMessage::Event(ref e) if e.event == "terminated"));
    }

    // ── on_resume ─────────────────────────────────────────────────────────

    #[test]
    fn test_on_resume_resets_var_and_frame_stores() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);

        // Allocate in var_store and frame_store.
        let var_ref = adapter.var_store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        let frame_ref = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

        assert!(adapter.var_store.lookup(var_ref).is_some());
        assert!(adapter.frame_store.lookup(frame_ref).is_some());

        adapter.on_resume();

        assert!(
            adapter.var_store.lookup(var_ref).is_none(),
            "VariableStore should be reset on resume"
        );
        assert!(
            adapter.frame_store.lookup(frame_ref).is_none(),
            "FrameStore should be reset on resume"
        );
    }

    // ── pause_reason_to_dap_str ───────────────────────────────────────────

    #[test]
    fn test_pause_reason_to_dap_str_all_variants() {
        assert_eq!(
            pause_reason_to_dap_str(&PauseReason::Breakpoint),
            "breakpoint"
        );
        assert_eq!(
            pause_reason_to_dap_str(&PauseReason::Exception),
            "exception"
        );
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Step), "step");
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Interrupted), "pause");
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Entry), "entry");
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Exit), "exit");
    }

    // ── path_to_dart_uri ──────────────────────────────────────────────────

    #[test]
    fn test_path_to_dart_uri_empty_returns_empty() {
        assert_eq!(path_to_dart_uri(""), "");
    }

    #[test]
    fn test_path_to_dart_uri_converts_absolute_path() {
        assert_eq!(
            path_to_dart_uri("/home/user/myapp/lib/main.dart"),
            "file:///home/user/myapp/lib/main.dart"
        );
    }

    #[test]
    fn test_path_to_dart_uri_passthrough_existing_uri() {
        let uri = "file:///home/user/myapp/lib/main.dart";
        assert_eq!(path_to_dart_uri(uri), uri);
    }

    #[test]
    fn test_path_to_dart_uri_passthrough_package_uri() {
        let uri = "package:myapp/main.dart";
        assert_eq!(path_to_dart_uri(uri), uri);
    }

    // ── exception_filter_to_mode ──────────────────────────────────────────

    #[test]
    fn test_exception_filter_empty_gives_none() {
        assert_eq!(exception_filter_to_mode(&[]), "None");
    }

    #[test]
    fn test_exception_filter_unhandled() {
        assert_eq!(
            exception_filter_to_mode(&["Unhandled".to_string()]),
            "Unhandled"
        );
    }

    #[test]
    fn test_exception_filter_all() {
        assert_eq!(exception_filter_to_mode(&["All".to_string()]), "All");
    }

    #[test]
    fn test_exception_filter_all_takes_precedence_over_unhandled() {
        assert_eq!(
            exception_filter_to_mode(&["All".to_string(), "Unhandled".to_string()]),
            "All"
        );
        assert_eq!(
            exception_filter_to_mode(&["Unhandled".to_string(), "All".to_string()]),
            "All"
        );
    }

    #[test]
    fn test_exception_filter_unknown_gives_none() {
        assert_eq!(exception_filter_to_mode(&["SomeOther".to_string()]), "None");
    }

    // ── handle_set_breakpoints ────────────────────────────────────────────

    /// Build a `setBreakpoints` request with a list of lines.
    fn make_set_breakpoints_request(seq: i64, source_path: &str, lines: &[i64]) -> DapRequest {
        use crate::protocol::types::{DapSource, SourceBreakpoint};
        let breakpoints: Vec<SourceBreakpoint> = lines
            .iter()
            .map(|&l| SourceBreakpoint {
                line: l,
                ..Default::default()
            })
            .collect();
        DapRequest {
            seq,
            command: "setBreakpoints".into(),
            arguments: Some(serde_json::json!({
                "source": DapSource {
                    path: Some(source_path.to_string()),
                    ..Default::default()
                },
                "breakpoints": breakpoints,
            })),
        }
    }

    #[tokio::test]
    async fn test_set_breakpoints_without_isolate_returns_unverified() {
        // No isolate registered → breakpoints come back unverified.
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
        let resp = adapter.handle_request(&req).await;

        assert!(
            resp.success,
            "setBreakpoints should succeed even without an isolate"
        );
        let body = resp.body.unwrap();
        let bps = body["breakpoints"].as_array().unwrap();
        assert_eq!(bps.len(), 2);
        for bp in bps {
            assert_eq!(
                bp["verified"], false,
                "Breakpoints without isolate must be unverified"
            );
        }
        // Breakpoints should NOT be stored when there's no isolate.
        assert!(adapter.breakpoint_state.is_empty());
    }

    #[tokio::test]
    async fn test_set_breakpoints_with_isolate_adds_and_returns_verified() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        // Register an isolate so breakpoints can be sent to the VM.
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok(); // Drain the thread event.

        let req = make_set_breakpoints_request(2, "/lib/main.dart", &[10]);
        let resp = adapter.handle_request(&req).await;

        assert!(resp.success);
        let body = resp.body.unwrap();
        let bps = body["breakpoints"].as_array().unwrap();
        assert_eq!(bps.len(), 1);
        // MockBackend returns resolved=true.
        assert_eq!(bps[0]["verified"], true);
        assert!(bps[0]["id"].as_i64().is_some());
        // State should have one tracked breakpoint.
        assert_eq!(adapter.breakpoint_state.len(), 1);
    }

    #[tokio::test]
    async fn test_set_breakpoints_diff_removes_old_adds_new() {
        // Acceptance criteria: setBreakpoints replaces all breakpoints for a file.
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        // First call: lines 10 and 20.
        let req1 = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
        adapter.handle_request(&req1).await;
        assert_eq!(adapter.breakpoint_state.len(), 2);

        // Second call: lines 10 and 30 only.  Line 20 should be removed.
        let req2 = make_set_breakpoints_request(2, "/lib/main.dart", &[10, 30]);
        let resp = adapter.handle_request(&req2).await;

        assert!(resp.success);
        assert_eq!(
            adapter.breakpoint_state.len(),
            2,
            "Should have 2 breakpoints after diff (10 kept, 20 removed, 30 added)"
        );
    }

    #[tokio::test]
    async fn test_set_breakpoints_empty_list_removes_all() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        // Add some breakpoints.
        let req1 = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
        adapter.handle_request(&req1).await;
        assert_eq!(adapter.breakpoint_state.len(), 2);

        // Clear all breakpoints by sending empty list.
        let req2 = make_set_breakpoints_request(2, "/lib/main.dart", &[]);
        let resp = adapter.handle_request(&req2).await;

        assert!(resp.success);
        let bps = resp.body.unwrap()["breakpoints"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            bps.is_empty(),
            "Empty desired list should return empty array"
        );
        assert!(adapter.breakpoint_state.is_empty());
    }

    #[tokio::test]
    async fn test_set_breakpoints_existing_line_reused() {
        // If the same line is requested twice the second request reuses the entry.
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        let req1 = make_set_breakpoints_request(1, "/lib/main.dart", &[10]);
        let resp1 = adapter.handle_request(&req1).await;
        let id1 = resp1.body.unwrap()["breakpoints"][0]["id"]
            .as_i64()
            .unwrap();

        let req2 = make_set_breakpoints_request(2, "/lib/main.dart", &[10]);
        let resp2 = adapter.handle_request(&req2).await;
        let id2 = resp2.body.unwrap()["breakpoints"][0]["id"]
            .as_i64()
            .unwrap();

        assert_eq!(
            id1, id2,
            "Same line should reuse the existing DAP breakpoint ID"
        );
        assert_eq!(adapter.breakpoint_state.len(), 1);
    }

    #[tokio::test]
    async fn test_set_breakpoints_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_request(1, "setBreakpoints");
        let resp = adapter.handle_request(&req).await;
        assert!(
            !resp.success,
            "setBreakpoints without arguments must return error"
        );
    }

    // ── handle_set_exception_breakpoints ─────────────────────────────────

    fn make_set_exception_breakpoints_request(seq: i64, filters: &[&str]) -> DapRequest {
        DapRequest {
            seq,
            command: "setExceptionBreakpoints".into(),
            arguments: Some(serde_json::json!({
                "filters": filters,
            })),
        }
    }

    #[tokio::test]
    async fn test_set_exception_breakpoints_empty_filters_returns_success() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_set_exception_breakpoints_request(1, &[]);
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let body = resp.body.unwrap();
        assert!(body["breakpoints"].as_array().unwrap().is_empty());
        assert_eq!(adapter.exception_mode, "None");
    }

    #[tokio::test]
    async fn test_set_exception_breakpoints_unhandled_mode() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_set_exception_breakpoints_request(1, &["Unhandled"]);
        adapter.handle_request(&req).await;
        assert_eq!(adapter.exception_mode, "Unhandled");
    }

    #[tokio::test]
    async fn test_set_exception_breakpoints_all_mode() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_set_exception_breakpoints_request(1, &["All"]);
        adapter.handle_request(&req).await;
        assert_eq!(adapter.exception_mode, "All");
    }

    #[tokio::test]
    async fn test_set_exception_breakpoints_all_takes_precedence() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_set_exception_breakpoints_request(1, &["Unhandled", "All"]);
        adapter.handle_request(&req).await;
        assert_eq!(adapter.exception_mode, "All");
    }

    #[tokio::test]
    async fn test_set_exception_breakpoints_updates_mode_for_isolates() {
        // Verify the adapter applies the mode to all known isolates without
        // crashing. The MockBackend silently succeeds, so we just check the
        // stored mode and a successful response.
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        let req = make_set_exception_breakpoints_request(1, &["All"]);
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        assert_eq!(adapter.exception_mode, "All");
    }

    #[tokio::test]
    async fn test_set_exception_breakpoints_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_request(1, "setExceptionBreakpoints");
        let resp = adapter.handle_request(&req).await;
        assert!(
            !resp.success,
            "setExceptionBreakpoints without arguments must return error"
        );
    }

    // ── BreakpointResolved event → IDE notification ───────────────────────

    #[tokio::test]
    async fn test_breakpoint_resolved_event_sends_breakpoint_changed_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        // Add a breakpoint so there is a VM ID to resolve.
        let req = make_set_breakpoints_request(1, "/lib/main.dart", &[10]);
        adapter.handle_request(&req).await;

        // Get the VM ID that was assigned (MockBackend returns "bp/line:<N>").
        let vm_id = "bp/line:10".to_string();

        // Drain any remaining events.
        while rx.try_recv().is_ok() {}

        // Fire a BreakpointResolved event.
        adapter
            .handle_debug_event(DebugEvent::BreakpointResolved {
                vm_breakpoint_id: vm_id,
                line: Some(11),
                column: None,
            })
            .await;

        // The adapter should emit a breakpoint event with reason "changed".
        let msg = rx.try_recv().expect("Expected a breakpoint event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.event, "breakpoint");
            let body = e.body.unwrap();
            assert_eq!(body["reason"], "changed");
            assert_eq!(body["breakpoint"]["verified"], true);
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_breakpoint_resolved_unknown_vm_id_sends_no_event() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::BreakpointResolved {
                vm_breakpoint_id: "bp/unknown".to_string(),
                line: Some(5),
                column: None,
            })
            .await;
        // Unknown VM ID: no event should be emitted.
        assert!(rx.try_recv().is_err());
    }

    // ── handle_attach / handle_threads (Task 04) ──────────────────────────

    /// Backend that returns two named isolates from get_vm().
    struct AttachMockBackend;

    impl DebugBackend for AttachMockBackend {
        async fn pause(&self, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), String> {
            Ok(())
        }

        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            _: i32,
            _: Option<i32>,
        ) -> Result<BreakpointResult, String> {
            Ok(BreakpointResult {
                vm_id: "bp/1".into(),
                resolved: true,
                line: Some(10),
                column: None,
            })
        }

        async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn set_exception_pause_mode(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn get_stack(&self, _: &str, _: Option<i32>) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate(&self, _: &str, _: &str, _: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_vm(&self) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "isolates": [
                    { "id": "isolates/1", "name": "main" },
                    { "id": "isolates/2", "name": "background" }
                ]
            }))
        }

        async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }
    }

    /// Backend whose get_vm() always fails.
    struct FailingVmBackend;

    impl DebugBackend for FailingVmBackend {
        async fn pause(&self, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), String> {
            Ok(())
        }

        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            _: i32,
            _: Option<i32>,
        ) -> Result<BreakpointResult, String> {
            Err("not connected".into())
        }

        async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn set_exception_pause_mode(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn get_stack(&self, _: &str, _: Option<i32>) -> Result<serde_json::Value, String> {
            Err("not connected".into())
        }

        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> Result<serde_json::Value, String> {
            Err("not connected".into())
        }

        async fn evaluate(&self, _: &str, _: &str, _: &str) -> Result<serde_json::Value, String> {
            Err("not connected".into())
        }

        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> Result<serde_json::Value, String> {
            Err("not connected".into())
        }

        async fn get_vm(&self) -> Result<serde_json::Value, String> {
            Err("VM not reachable".into())
        }

        async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, String> {
            Err("not connected".into())
        }
    }

    // ── handle_attach tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_handle_attach_success_populates_thread_map() {
        let (mut adapter, _rx) = DapAdapter::new(AttachMockBackend);
        let resp = adapter.handle_request(&make_request(1, "attach")).await;
        assert!(resp.success, "attach should succeed when VM is reachable");
        assert_eq!(
            adapter.thread_map.len(),
            2,
            "Both isolates should be registered"
        );
    }

    #[tokio::test]
    async fn test_handle_attach_emits_thread_started_events() {
        let (mut adapter, mut rx) = DapAdapter::new(AttachMockBackend);
        adapter.handle_request(&make_request(1, "attach")).await;

        let mut started_count = 0;
        while let Ok(msg) = rx.try_recv() {
            if let DapMessage::Event(e) = msg {
                assert_eq!(e.event, "thread");
                let body = e.body.unwrap();
                assert_eq!(body["reason"], "started");
                started_count += 1;
            }
        }
        assert_eq!(
            started_count, 2,
            "Should emit one started event per isolate"
        );
    }

    #[tokio::test]
    async fn test_handle_attach_stores_thread_names() {
        let (mut adapter, _rx) = DapAdapter::new(AttachMockBackend);
        adapter.handle_request(&make_request(1, "attach")).await;

        let name1 = adapter.thread_names.get(&1).map(String::as_str);
        let name2 = adapter.thread_names.get(&2).map(String::as_str);
        assert_eq!(name1, Some("main"));
        assert_eq!(name2, Some("background"));
    }

    #[tokio::test]
    async fn test_handle_attach_vm_failure_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(FailingVmBackend);
        let resp = adapter.handle_request(&make_request(1, "attach")).await;
        assert!(!resp.success, "attach should fail when VM is unreachable");
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("Failed to attach"),
            "Error should mention attach failure, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_handle_attach_empty_vm_response_succeeds() {
        // MockBackend.get_vm() returns {} with no "isolates" key.
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter.handle_request(&make_request(1, "attach")).await;
        assert!(
            resp.success,
            "attach should succeed even with empty VM response"
        );
        assert_eq!(
            adapter.thread_map.len(),
            0,
            "No threads should be registered when VM has no isolates"
        );
    }

    // ── handle_threads tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_handle_threads_returns_success_with_empty_list() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter.handle_request(&make_request(1, "threads")).await;
        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        let threads = body["threads"].as_array().unwrap();
        assert!(
            threads.is_empty(),
            "Should return empty list when no threads registered"
        );
    }

    #[tokio::test]
    async fn test_handle_threads_returns_all_registered_threads() {
        let (mut adapter, _rx) = DapAdapter::new(AttachMockBackend);
        adapter.handle_request(&make_request(1, "attach")).await;

        let resp = adapter.handle_request(&make_request(2, "threads")).await;
        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        let threads = body["threads"].as_array().unwrap();
        assert_eq!(threads.len(), 2);
        // Threads are sorted by ID.
        assert_eq!(threads[0]["id"], 1);
        assert_eq!(threads[0]["name"], "main");
        assert_eq!(threads[1]["id"], 2);
        assert_eq!(threads[1]["name"], "background");
    }

    #[tokio::test]
    async fn test_handle_threads_uses_default_name_when_missing() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        // Manually register a thread without inserting a name — fallback to "Thread N".
        let thread_id = adapter.thread_map.get_or_create("isolates/7");

        let resp = adapter.handle_request(&make_request(1, "threads")).await;
        assert!(resp.success);
        let body = resp.body.as_ref().unwrap();
        let threads = body["threads"].as_array().unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0]["name"], format!("Thread {thread_id}"));
    }

    // ── thread name lifecycle ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_isolate_start_stores_thread_name() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/42".into(),
                name: "worker".into(),
            })
            .await;
        rx.try_recv().ok();

        let thread_id = adapter.thread_map.thread_id_for("isolates/42").unwrap();
        assert_eq!(
            adapter.thread_names.get(&thread_id).map(String::as_str),
            Some("worker"),
            "IsolateStart should store the thread name"
        );
    }

    #[tokio::test]
    async fn test_isolate_exit_removes_thread_name() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/42".into(),
                name: "worker".into(),
            })
            .await;
        rx.try_recv().ok();

        let thread_id = adapter.thread_map.thread_id_for("isolates/42").unwrap();
        assert!(adapter.thread_names.contains_key(&thread_id));

        adapter
            .handle_debug_event(DebugEvent::IsolateExit {
                isolate_id: "isolates/42".into(),
            })
            .await;
        rx.try_recv().ok();

        assert!(
            !adapter.thread_names.contains_key(&thread_id),
            "IsolateExit should remove the thread name"
        );
    }

    #[tokio::test]
    async fn test_isolate_exit_removes_thread_from_map() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: "isolates/1".into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        assert_eq!(adapter.thread_map.len(), 1);

        adapter
            .handle_debug_event(DebugEvent::IsolateExit {
                isolate_id: "isolates/1".into(),
            })
            .await;
        rx.try_recv().ok();

        assert_eq!(
            adapter.thread_map.len(),
            0,
            "IsolateExit should remove the thread from the thread map"
        );
    }

    // ── Execution control (Task 06) ───────────────────────────────────────

    fn make_continue_request(seq: i64, thread_id: i64) -> DapRequest {
        DapRequest {
            seq,
            command: "continue".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        }
    }

    fn make_step_request(seq: i64, command: &str, thread_id: i64) -> DapRequest {
        DapRequest {
            seq,
            command: command.into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        }
    }

    fn make_pause_request_t06(seq: i64, thread_id: i64) -> DapRequest {
        DapRequest {
            seq,
            command: "pause".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        }
    }

    #[tokio::test]
    async fn test_continue_returns_success_with_all_threads_continued() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        adapter.thread_map.get_or_create("isolates/1");
        let req = make_continue_request(1, 1);
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success, "continue should succeed for a known thread");
        let body = resp.body.unwrap();
        assert_eq!(body["allThreadsContinued"], true);
    }

    #[tokio::test]
    async fn test_continue_unknown_thread_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = make_continue_request(1, 99);
        let resp = adapter.handle_request(&req).await;
        assert!(
            !resp.success,
            "continue with unknown thread must return error"
        );
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("99"),
            "Error should mention thread ID, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_continue_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter.handle_request(&make_request(1, "continue")).await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_continue_invalidates_var_and_frame_stores() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let thread_id = adapter.thread_map.get_or_create("isolates/1");
        let var_ref = adapter.var_store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        let frame_ref = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));
        assert!(adapter.var_store.lookup(var_ref).is_some());
        assert!(adapter.frame_store.lookup(frame_ref).is_some());
        adapter
            .handle_request(&make_continue_request(1, thread_id))
            .await;
        assert!(
            adapter.var_store.lookup(var_ref).is_none(),
            "var_store must reset"
        );
        assert!(
            adapter.frame_store.lookup(frame_ref).is_none(),
            "frame_store must reset"
        );
    }

    #[tokio::test]
    async fn test_next_returns_success_for_known_thread() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        adapter.thread_map.get_or_create("isolates/1");
        let resp = adapter
            .handle_request(&make_step_request(1, "next", 1))
            .await;
        assert!(resp.success);
        assert!(resp.body.is_none(), "next response should have no body");
    }

    #[tokio::test]
    async fn test_next_unknown_thread_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter
            .handle_request(&make_step_request(1, "next", 99))
            .await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_next_invalidates_stores() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let thread_id = adapter.thread_map.get_or_create("isolates/1");
        let var_ref = adapter.var_store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        assert!(adapter.var_store.lookup(var_ref).is_some());
        adapter
            .handle_request(&make_step_request(1, "next", thread_id))
            .await;
        assert!(adapter.var_store.lookup(var_ref).is_none());
    }

    #[tokio::test]
    async fn test_step_in_returns_success_for_known_thread() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        adapter.thread_map.get_or_create("isolates/1");
        let resp = adapter
            .handle_request(&make_step_request(1, "stepIn", 1))
            .await;
        assert!(resp.success);
    }

    #[tokio::test]
    async fn test_step_in_unknown_thread_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter
            .handle_request(&make_step_request(1, "stepIn", 99))
            .await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_step_in_invalidates_stores() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let thread_id = adapter.thread_map.get_or_create("isolates/1");
        let var_ref = adapter.var_store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        assert!(adapter.var_store.lookup(var_ref).is_some());
        adapter
            .handle_request(&make_step_request(1, "stepIn", thread_id))
            .await;
        assert!(adapter.var_store.lookup(var_ref).is_none());
    }

    #[tokio::test]
    async fn test_step_out_returns_success_for_known_thread() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        adapter.thread_map.get_or_create("isolates/1");
        let resp = adapter
            .handle_request(&make_step_request(1, "stepOut", 1))
            .await;
        assert!(resp.success);
    }

    #[tokio::test]
    async fn test_step_out_unknown_thread_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter
            .handle_request(&make_step_request(1, "stepOut", 99))
            .await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_step_out_invalidates_stores() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let thread_id = adapter.thread_map.get_or_create("isolates/1");
        let frame_ref = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));
        assert!(adapter.frame_store.lookup(frame_ref).is_some());
        adapter
            .handle_request(&make_step_request(1, "stepOut", thread_id))
            .await;
        assert!(adapter.frame_store.lookup(frame_ref).is_none());
    }

    #[tokio::test]
    async fn test_pause_cmd_returns_success_for_known_thread() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        adapter.thread_map.get_or_create("isolates/1");
        let resp = adapter.handle_request(&make_pause_request_t06(1, 1)).await;
        assert!(resp.success);
        assert!(resp.body.is_none(), "pause response should have no body");
    }

    #[tokio::test]
    async fn test_pause_cmd_unknown_thread_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter.handle_request(&make_pause_request_t06(1, 99)).await;
        assert!(!resp.success);
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("99"),
            "Error should mention thread ID, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_pause_cmd_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let resp = adapter.handle_request(&make_request(1, "pause")).await;
        assert!(!resp.success);
    }

    #[test]
    fn test_pause_reason_variants_map_to_correct_dap_strings() {
        assert_eq!(
            pause_reason_to_dap_str(&PauseReason::Breakpoint),
            "breakpoint"
        );
        assert_eq!(
            pause_reason_to_dap_str(&PauseReason::Exception),
            "exception"
        );
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Step), "step");
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Interrupted), "pause");
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Entry), "entry");
        assert_eq!(pause_reason_to_dap_str(&PauseReason::Exit), "exit");
    }

    #[tokio::test]
    async fn test_paused_exception_emits_exception_reason() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Exception,
            })
            .await;
        let msg = rx.try_recv().expect("Expected stopped event");
        if let DapMessage::Event(e) = msg {
            let body = e.body.unwrap();
            assert_eq!(body["reason"], "exception");
            assert_eq!(body["allThreadsStopped"], true);
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_paused_step_emits_step_reason() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Step,
            })
            .await;
        let msg = rx.try_recv().expect("Expected stopped event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.body.unwrap()["reason"], "step");
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_paused_interrupted_emits_pause_reason() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Interrupted,
            })
            .await;
        let msg = rx.try_recv().expect("Expected stopped event");
        if let DapMessage::Event(e) = msg {
            assert_eq!(e.body.unwrap()["reason"], "pause");
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_resumed_event_includes_all_threads_continued() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter.thread_map.get_or_create("isolates/1");
        adapter
            .handle_debug_event(DebugEvent::Resumed {
                isolate_id: "isolates/1".into(),
            })
            .await;
        let msg = rx.try_recv().expect("Expected continued event");
        if let DapMessage::Event(e) = msg {
            let body = e.body.unwrap();
            assert_eq!(body["allThreadsContinued"], true);
            assert!(body["threadId"].as_i64().is_some());
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_stopped_event_includes_all_threads_stopped() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Breakpoint,
            })
            .await;
        let msg = rx.try_recv().expect("Expected stopped event");
        if let DapMessage::Event(e) = msg {
            let body = e.body.unwrap();
            assert_eq!(body["allThreadsStopped"], true);
            assert!(body["threadId"].as_i64().is_some());
        } else {
            panic!("Expected Event, got: {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_step_commands_no_arguments_return_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        for cmd in ["next", "stepIn", "stepOut"] {
            let resp = adapter.handle_request(&make_request(1, cmd)).await;
            assert!(!resp.success, "{} without arguments must return error", cmd);
        }
    }

    // ── handle_stack_trace / handle_scopes (Task 07) ──────────────────────

    /// A backend that returns a realistic two-frame stack from `get_stack()`.
    struct StackMockBackend;

    impl DebugBackend for StackMockBackend {
        async fn pause(&self, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), String> {
            Ok(())
        }

        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            _: i32,
            _: Option<i32>,
        ) -> Result<BreakpointResult, String> {
            Ok(BreakpointResult {
                vm_id: "bp/1".into(),
                resolved: true,
                line: Some(10),
                column: None,
            })
        }

        async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn set_exception_pause_mode(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn get_stack(
            &self,
            _isolate_id: &str,
            _limit: Option<i32>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "frames": [
                    {
                        "kind": "Regular",
                        "code": { "name": "main" },
                        "location": {
                            "script": { "uri": "file:///app/lib/main.dart" },
                            "line": 42,
                            "column": 5
                        }
                    },
                    {
                        "kind": "Regular",
                        "code": { "name": "runApp" },
                        "location": {
                            "script": { "uri": "package:flutter/src/widgets/binding.dart" },
                            "line": 100
                        }
                    },
                    {
                        "kind": "AsyncSuspensionMarker"
                    }
                ]
            }))
        }

        async fn get_object(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
            _: Option<i64>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate(&self, _: &str, _: &str, _: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_vm(&self) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }
    }

    /// Helper: register an isolate and return its thread ID.
    async fn register_isolate(
        adapter: &mut DapAdapter<impl DebugBackend>,
        rx: &mut tokio::sync::mpsc::Receiver<DapMessage>,
        isolate_id: &str,
    ) -> i64 {
        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: isolate_id.into(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();
        adapter
            .thread_map
            .thread_id_for(isolate_id)
            .expect("isolate should be registered")
    }

    // ── stackTrace tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_stack_trace_unknown_thread_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(StackMockBackend);
        let req = DapRequest {
            seq: 1,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": 999 })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("Unknown thread ID"),
            "Expected unknown thread error, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_stack_trace_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(StackMockBackend);
        let req = DapRequest {
            seq: 1,
            command: "stackTrace".into(),
            arguments: None,
        };
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_stack_trace_returns_all_frames() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;

        assert!(
            resp.success,
            "stackTrace should succeed: {:?}",
            resp.message
        );
        let body = resp.body.unwrap();
        let frames = body["stackFrames"].as_array().unwrap();
        // StackMockBackend returns 3 frames.
        assert_eq!(frames.len(), 3);
        assert_eq!(body["totalFrames"], 3);
    }

    #[tokio::test]
    async fn test_stack_trace_frame_ids_are_unique_and_monotonic() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let frames = resp.body.unwrap()["stackFrames"]
            .as_array()
            .unwrap()
            .clone();

        let ids: Vec<i64> = frames.iter().map(|f| f["id"].as_i64().unwrap()).collect();
        // IDs are monotonically increasing starting at 1.
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(id, (i as i64) + 1, "Frame IDs must be monotonic from 1");
        }
        // All IDs are unique.
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(deduped.len(), ids.len(), "Frame IDs must be unique");
    }

    #[tokio::test]
    async fn test_stack_trace_user_code_has_path_and_no_hint() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let frames = resp.body.unwrap()["stackFrames"]
            .as_array()
            .unwrap()
            .clone();

        // Frame 0 is "main" — user code at file:///app/lib/main.dart.
        let frame0 = &frames[0];
        assert_eq!(frame0["name"], "main");
        assert_eq!(frame0["line"], 42);
        assert_eq!(frame0["column"], 5);
        assert_eq!(frame0["source"]["path"], "/app/lib/main.dart");
        // User code: no presentation hint.
        assert!(
            frame0["source"].get("presentationHint").is_none()
                || frame0["source"]["presentationHint"].is_null(),
            "User code should have no presentation hint"
        );
    }

    #[tokio::test]
    async fn test_stack_trace_flutter_framework_frame_deemphasized() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let frames = resp.body.unwrap()["stackFrames"]
            .as_array()
            .unwrap()
            .clone();

        // Frame 1 is "runApp" — Flutter framework source.
        let frame1 = &frames[1];
        assert_eq!(frame1["name"], "runApp");
        assert_eq!(
            frame1["source"]["presentationHint"], "deemphasize",
            "Flutter framework frames should be de-emphasized"
        );
    }

    #[tokio::test]
    async fn test_stack_trace_async_suspension_marker_frame() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let frames = resp.body.unwrap()["stackFrames"]
            .as_array()
            .unwrap()
            .clone();

        // Frame 2 is an async gap marker.
        let frame2 = &frames[2];
        assert_eq!(frame2["name"], "<asynchronous gap>");
        assert_eq!(
            frame2["presentationHint"], "label",
            "AsyncSuspensionMarker must have presentation_hint: label"
        );
    }

    #[tokio::test]
    async fn test_stack_trace_start_frame_offsets_results() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        // Request frames starting at index 1 (skip frame 0).
        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({
                "threadId": thread_id,
                "startFrame": 1,
            })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let body = resp.body.unwrap();
        let frames = body["stackFrames"].as_array().unwrap();
        // 3 total frames, skip 1 → 2 returned.
        assert_eq!(frames.len(), 2, "startFrame=1 should skip the first frame");
        // Total is still the full count.
        assert_eq!(body["totalFrames"], 3);
        // First returned frame should now be the flutter framework frame.
        assert_eq!(frames[0]["name"], "runApp");
    }

    #[tokio::test]
    async fn test_stack_trace_frame_ids_stored_in_frame_store() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let frames = resp.body.unwrap()["stackFrames"]
            .as_array()
            .unwrap()
            .clone();

        // Every frame ID returned should be lookupable in the frame_store.
        for frame in &frames {
            let id = frame["id"].as_i64().unwrap();
            assert!(
                adapter.frame_store.lookup(id).is_some(),
                "Frame ID {} should be in frame_store",
                id
            );
        }
    }

    #[tokio::test]
    async fn test_stack_trace_empty_frames_returns_success() {
        // MockBackend returns {} with no "frames" key — should succeed with 0 frames.
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(resp.success);
        let body = resp.body.unwrap();
        let frames = body["stackFrames"].as_array().unwrap();
        assert!(frames.is_empty());
        assert_eq!(body["totalFrames"], 0);
    }

    // ── scopes tests ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_scopes_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = DapRequest {
            seq: 1,
            command: "scopes".into(),
            arguments: None,
        };
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_scopes_invalid_frame_id_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = DapRequest {
            seq: 1,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": 999 })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("Invalid frame ID"),
            "Expected invalid frame error, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_scopes_returns_locals_and_globals() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        // First get a frame ID via stackTrace.
        let stack_req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let stack_resp = adapter.handle_request(&stack_req).await;
        assert!(stack_resp.success);
        let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
            .as_i64()
            .unwrap();

        // Now request scopes for that frame.
        let scopes_req = DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        };
        let resp = adapter.handle_request(&scopes_req).await;
        assert!(resp.success, "scopes should succeed: {:?}", resp.message);

        let body = resp.body.unwrap();
        let scopes = body["scopes"].as_array().unwrap();
        assert_eq!(scopes.len(), 2, "Should return exactly 2 scopes");

        // First scope: Locals.
        assert_eq!(scopes[0]["name"], "Locals");
        assert_eq!(scopes[0]["presentationHint"], "locals");
        assert_eq!(scopes[0]["expensive"], false);
        let locals_ref = scopes[0]["variablesReference"].as_i64().unwrap();
        assert!(locals_ref > 0, "Locals variablesReference must be positive");

        // Second scope: Globals.
        assert_eq!(scopes[1]["name"], "Globals");
        assert_eq!(scopes[1]["presentationHint"], "globals");
        assert_eq!(scopes[1]["expensive"], true);
        let globals_ref = scopes[1]["variablesReference"].as_i64().unwrap();
        assert!(
            globals_ref > 0,
            "Globals variablesReference must be positive"
        );

        // References must be distinct.
        assert_ne!(
            locals_ref, globals_ref,
            "Locals and Globals must have different variablesReference values"
        );
    }

    #[tokio::test]
    async fn test_scopes_variable_references_stored_in_var_store() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        // Get a frame ID.
        let stack_req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let stack_resp = adapter.handle_request(&stack_req).await;
        let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
            .as_i64()
            .unwrap();

        // Get scopes.
        let scopes_req = DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        };
        let scopes_resp = adapter.handle_request(&scopes_req).await;
        assert!(scopes_resp.success);
        let scopes = scopes_resp.body.unwrap()["scopes"]
            .as_array()
            .unwrap()
            .clone();

        for scope in &scopes {
            let var_ref = scope["variablesReference"].as_i64().unwrap();
            assert!(
                adapter.var_store.lookup(var_ref).is_some(),
                "variablesReference {} should be in var_store",
                var_ref
            );
        }
    }

    #[tokio::test]
    async fn test_scopes_locals_scope_has_correct_var_ref_kind() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        let stack_req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let stack_resp = adapter.handle_request(&stack_req).await;
        let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
            .as_i64()
            .unwrap();

        let scopes_req = DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        };
        let scopes_resp = adapter.handle_request(&scopes_req).await;
        let scopes = scopes_resp.body.unwrap()["scopes"]
            .as_array()
            .unwrap()
            .clone();

        let locals_ref = scopes[0]["variablesReference"].as_i64().unwrap();
        let var_ref = adapter.var_store.lookup(locals_ref).unwrap();
        assert!(
            matches!(
                var_ref,
                VariableRef::Scope {
                    scope_kind: ScopeKind::Locals,
                    ..
                }
            ),
            "Locals scope should store a VariableRef::Scope(Locals)"
        );

        let globals_ref = scopes[1]["variablesReference"].as_i64().unwrap();
        let var_ref = adapter.var_store.lookup(globals_ref).unwrap();
        assert!(
            matches!(
                var_ref,
                VariableRef::Scope {
                    scope_kind: ScopeKind::Globals,
                    ..
                }
            ),
            "Globals scope should store a VariableRef::Scope(Globals)"
        );
    }

    #[tokio::test]
    async fn test_scopes_stale_frame_id_after_resume_returns_error() {
        let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        // Get a frame ID while stopped.
        let stack_req = DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        };
        let stack_resp = adapter.handle_request(&stack_req).await;
        let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
            .as_i64()
            .unwrap();

        // Simulate a resume — invalidates all frame IDs.
        adapter.on_resume();

        // The previously valid frame ID should now be stale.
        let scopes_req = DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        };
        let resp = adapter.handle_request(&scopes_req).await;
        assert!(
            !resp.success,
            "Stale frame ID after resume should return error"
        );
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("Invalid frame ID"),
            "Error should mention invalid frame ID, got: {:?}",
            msg
        );
    }

    // ── handle_variables (Task 08) ────────────────────────────────────────

    /// Backend that returns a two-variable stack frame for variables tests.
    struct VarMockBackend;

    impl DebugBackend for VarMockBackend {
        async fn pause(&self, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), String> {
            Ok(())
        }

        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            _: i32,
            _: Option<i32>,
        ) -> Result<BreakpointResult, String> {
            Ok(BreakpointResult {
                vm_id: "bp/1".into(),
                resolved: true,
                line: Some(10),
                column: None,
            })
        }

        async fn remove_breakpoint(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn set_exception_pause_mode(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn get_stack(
            &self,
            _isolate_id: &str,
            _limit: Option<i32>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "frames": [
                    {
                        "kind": "Regular",
                        "code": { "name": "main" },
                        "location": {
                            "script": { "uri": "file:///app/lib/main.dart" },
                            "line": 42,
                            "column": 5
                        },
                        "vars": [
                            {
                                "name": "count",
                                "value": {
                                    "type": "InstanceRef",
                                    "kind": "Int",
                                    "valueAsString": "42",
                                    "id": "objects/int1"
                                }
                            },
                            {
                                "name": "label",
                                "value": {
                                    "type": "InstanceRef",
                                    "kind": "String",
                                    "valueAsString": "hello",
                                    "id": "objects/str1"
                                }
                            }
                        ]
                    }
                ]
            }))
        }

        async fn get_object(
            &self,
            _isolate_id: &str,
            object_id: &str,
            _offset: Option<i64>,
            _count: Option<i64>,
        ) -> Result<serde_json::Value, String> {
            if object_id == "objects/list1" {
                Ok(serde_json::json!({
                    "type": "Instance",
                    "kind": "List",
                    "elements": [
                        { "kind": "Int", "valueAsString": "10", "id": "objects/e0" },
                        { "kind": "Int", "valueAsString": "20", "id": "objects/e1" }
                    ]
                }))
            } else if object_id == "objects/map1" {
                Ok(serde_json::json!({
                    "type": "Instance",
                    "kind": "Map",
                    "associations": [
                        {
                            "key": { "kind": "String", "valueAsString": "a" },
                            "value": { "kind": "Int", "valueAsString": "1", "id": "objects/mv1" }
                        }
                    ]
                }))
            } else if object_id == "objects/inst1" {
                Ok(serde_json::json!({
                    "type": "Instance",
                    "kind": "PlainInstance",
                    "fields": [
                        {
                            "name": "width",
                            "value": { "kind": "Double", "valueAsString": "3.14", "id": "objects/f1" }
                        }
                    ]
                }))
            } else {
                Ok(serde_json::json!({ "type": "Instance", "kind": "Null" }))
            }
        }

        async fn evaluate(&self, _: &str, _: &str, _: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_vm(&self) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn get_scripts(&self, _: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }
    }

    // ── instance_ref_to_variable (unit tests) ─────────────────────────────

    #[test]
    fn test_primitive_null_no_expansion() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var =
            adapter.instance_ref_to_variable("x", &serde_json::json!({"kind": "Null"}), "i/1");
        assert_eq!(var.value, "null");
        assert_eq!(var.variables_reference, 0);
        assert_eq!(var.type_field.as_deref(), Some("Null"));
    }

    #[test]
    fn test_primitive_bool_no_expansion() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "flag",
            &serde_json::json!({"kind": "Bool", "valueAsString": "true"}),
            "i/1",
        );
        assert_eq!(var.value, "true");
        assert_eq!(var.variables_reference, 0);
        assert_eq!(var.type_field.as_deref(), Some("bool"));
    }

    #[test]
    fn test_primitive_int_no_expansion() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "n",
            &serde_json::json!({"kind": "Int", "valueAsString": "42"}),
            "i/1",
        );
        assert_eq!(var.value, "42");
        assert_eq!(var.variables_reference, 0);
        assert_eq!(var.type_field.as_deref(), Some("int"));
    }

    #[test]
    fn test_primitive_double_no_expansion() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "x",
            &serde_json::json!({"kind": "Double", "valueAsString": "3.14"}),
            "i/1",
        );
        assert_eq!(var.value, "3.14");
        assert_eq!(var.variables_reference, 0);
        assert_eq!(var.type_field.as_deref(), Some("double"));
    }

    #[test]
    fn test_string_quoted() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "name",
            &serde_json::json!({"kind": "String", "valueAsString": "hello"}),
            "i/1",
        );
        assert_eq!(var.value, "\"hello\"");
        assert_eq!(var.variables_reference, 0);
        assert_eq!(var.type_field.as_deref(), Some("String"));
    }

    #[test]
    fn test_string_empty_value_as_string_produces_empty_quotes() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var =
            adapter.instance_ref_to_variable("s", &serde_json::json!({"kind": "String"}), "i/1");
        assert_eq!(var.value, "\"\"");
        assert_eq!(var.variables_reference, 0);
    }

    #[test]
    fn test_list_shows_length_and_is_expandable() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "items",
            &serde_json::json!({
                "kind": "List", "length": 3, "id": "objects/1",
                "class": {"name": "List"}
            }),
            "i/1",
        );
        assert!(
            var.value.contains("length: 3"),
            "Expected 'length: 3' in value, got: {:?}",
            var.value
        );
        assert!(
            var.variables_reference > 0,
            "List must have a positive variablesReference"
        );
        assert_eq!(var.indexed_variables, Some(3));
        assert_eq!(var.type_field.as_deref(), Some("List"));
    }

    #[test]
    fn test_list_without_id_has_zero_ref() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "items",
            &serde_json::json!({"kind": "List", "length": 2}),
            "i/1",
        );
        assert_eq!(var.variables_reference, 0);
    }

    #[test]
    fn test_plain_instance_expandable() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "widget",
            &serde_json::json!({
                "kind": "PlainInstance", "id": "objects/2",
                "class": {"name": "Container"}
            }),
            "i/1",
        );
        assert!(
            var.value.contains("Container"),
            "Expected 'Container' in value, got: {:?}",
            var.value
        );
        assert!(
            var.variables_reference > 0,
            "PlainInstance must have a positive variablesReference"
        );
        assert_eq!(var.type_field.as_deref(), Some("Container"));
    }

    #[test]
    fn test_plain_instance_without_class_uses_kind() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "closure",
            &serde_json::json!({"kind": "Closure", "id": "objects/3"}),
            "i/1",
        );
        assert_eq!(var.type_field.as_deref(), Some("Closure"));
        assert!(var.variables_reference > 0);
    }

    #[test]
    fn test_fallback_unknown_kind_no_expansion() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var = adapter.instance_ref_to_variable(
            "mystery",
            &serde_json::json!({"kind": "FutureSomething", "valueAsString": "future"}),
            "i/1",
        );
        assert_eq!(var.value, "future");
        assert_eq!(var.variables_reference, 0);
    }

    #[test]
    fn test_each_collection_type_is_expandable() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        for kind in &[
            "Map",
            "Set",
            "Uint8List",
            "Uint8ClampedList",
            "Int32List",
            "Float64List",
        ] {
            let var = adapter.instance_ref_to_variable(
                "col",
                &serde_json::json!({"kind": kind, "id": "objects/col", "length": 0}),
                "i/1",
            );
            assert!(
                var.variables_reference > 0,
                "Collection kind '{}' should be expandable",
                kind
            );
        }
    }

    #[test]
    fn test_var_store_grows_for_each_expandable_instance() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        assert!(adapter.var_store.is_empty());

        adapter.instance_ref_to_variable(
            "a",
            &serde_json::json!({"kind": "PlainInstance", "id": "objects/1"}),
            "i/1",
        );
        adapter.instance_ref_to_variable(
            "b",
            &serde_json::json!({"kind": "List", "id": "objects/2", "length": 0}),
            "i/1",
        );
        assert_eq!(adapter.var_store.len(), 2);
    }

    // ── handle_variables dispatch tests ───────────────────────────────────

    #[tokio::test]
    async fn test_variables_stale_reference_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": 9999 })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
        let msg = resp.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("9999"),
            "Error should mention the invalid reference, got: {:?}",
            msg
        );
    }

    #[tokio::test]
    async fn test_variables_no_arguments_returns_error() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let req = DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: None,
        };
        let resp = adapter.handle_request(&req).await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_variables_globals_scope_returns_empty_list() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        let var_ref = adapter.var_store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Globals,
        });

        let req = DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(
            resp.success,
            "Globals scope should succeed with empty list: {:?}",
            resp.message
        );
        let body = resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        assert!(
            vars.is_empty(),
            "Globals should return empty list in Phase 3"
        );
    }

    #[tokio::test]
    async fn test_variables_locals_scope_returns_frame_vars() {
        let (mut adapter, mut rx) = DapAdapter::new(VarMockBackend);
        let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

        // 1. Get the stack to populate the frame store.
        let stack_resp = adapter
            .handle_request(&DapRequest {
                seq: 2,
                command: "stackTrace".into(),
                arguments: Some(serde_json::json!({ "threadId": thread_id })),
            })
            .await;
        assert!(stack_resp.success);
        let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
            .as_i64()
            .unwrap();

        // 2. Get scopes to get the locals variable reference.
        let scopes_resp = adapter
            .handle_request(&DapRequest {
                seq: 3,
                command: "scopes".into(),
                arguments: Some(serde_json::json!({ "frameId": frame_id })),
            })
            .await;
        assert!(scopes_resp.success);
        let locals_ref = scopes_resp.body.unwrap()["scopes"][0]["variablesReference"]
            .as_i64()
            .unwrap();

        // 3. Request variables for the locals scope.
        let vars_resp = adapter
            .handle_request(&DapRequest {
                seq: 4,
                command: "variables".into(),
                arguments: Some(serde_json::json!({ "variablesReference": locals_ref })),
            })
            .await;
        assert!(
            vars_resp.success,
            "Variables for locals should succeed: {:?}",
            vars_resp.message
        );

        let body = vars_resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        // VarMockBackend returns 2 variables: "count" (Int) and "label" (String).
        assert_eq!(vars.len(), 2, "Expected 2 local variables");

        let count_var = &vars[0];
        assert_eq!(count_var["name"], "count");
        assert_eq!(count_var["value"], "42");
        assert_eq!(count_var["variablesReference"], 0);

        let label_var = &vars[1];
        assert_eq!(label_var["name"], "label");
        assert_eq!(label_var["value"], "\"hello\"");
        assert_eq!(label_var["variablesReference"], 0);
    }

    #[tokio::test]
    async fn test_variables_expand_list_object() {
        let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

        let var_ref = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/list1".into(),
        });

        let vars_resp = adapter
            .handle_request(&DapRequest {
                seq: 1,
                command: "variables".into(),
                arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
            })
            .await;
        assert!(
            vars_resp.success,
            "Expanding list should succeed: {:?}",
            vars_resp.message
        );

        let body = vars_resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        assert_eq!(vars.len(), 2, "Expected 2 list elements");
        assert_eq!(vars[0]["name"], "[0]");
        assert_eq!(vars[0]["value"], "10");
        assert_eq!(vars[1]["name"], "[1]");
        assert_eq!(vars[1]["value"], "20");
    }

    #[tokio::test]
    async fn test_variables_expand_map_object() {
        let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

        let var_ref = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/map1".into(),
        });

        let vars_resp = adapter
            .handle_request(&DapRequest {
                seq: 1,
                command: "variables".into(),
                arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
            })
            .await;
        assert!(
            vars_resp.success,
            "Expanding map should succeed: {:?}",
            vars_resp.message
        );

        let body = vars_resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        assert_eq!(vars.len(), 1, "Expected 1 map entry");
        assert_eq!(vars[0]["name"], "[a]");
        assert_eq!(vars[0]["value"], "1");
    }

    #[tokio::test]
    async fn test_variables_expand_instance_fields() {
        let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

        let var_ref = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/inst1".into(),
        });

        let vars_resp = adapter
            .handle_request(&DapRequest {
                seq: 1,
                command: "variables".into(),
                arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
            })
            .await;
        assert!(
            vars_resp.success,
            "Expanding instance should succeed: {:?}",
            vars_resp.message
        );

        let body = vars_resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        assert_eq!(vars.len(), 1, "Expected 1 field");
        assert_eq!(vars[0]["name"], "width");
        assert_eq!(vars[0]["value"], "3.14");
    }

    #[tokio::test]
    async fn test_variables_stale_after_resume() {
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);

        let var_ref = adapter.var_store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });

        // Simulate resume (invalidates all references).
        adapter.on_resume();

        let req = DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
        };
        let resp = adapter.handle_request(&req).await;
        assert!(
            !resp.success,
            "Stale reference should return error after resume"
        );
    }

    #[tokio::test]
    async fn test_variables_nested_expansion_allocates_unique_refs() {
        // Each expandable object gets its own unique variablesReference.
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);

        let ref_a = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/a".into(),
        });
        let ref_b = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/b".into(),
        });
        assert_ne!(ref_a, ref_b, "Each expansion should get a unique reference");
    }

    #[tokio::test]
    async fn test_variables_list_with_start_offset() {
        let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

        let var_ref = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/list1".into(),
        });

        // start=1 → index label should be [1] for the first returned element.
        let vars_resp = adapter
            .handle_request(&DapRequest {
                seq: 1,
                command: "variables".into(),
                arguments: Some(serde_json::json!({ "variablesReference": var_ref, "start": 1 })),
            })
            .await;
        assert!(vars_resp.success);
        let body = vars_resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        assert_eq!(
            vars[0]["name"], "[1]",
            "First element with start=1 should be labeled [1]"
        );
    }

    #[tokio::test]
    async fn test_variables_unknown_object_type_returns_empty() {
        // If getObject returns an unrecognized type, return empty variables list.
        let (mut adapter, _rx) = DapAdapter::new(MockBackend);
        // MockBackend returns {} (no "type" field) for any object.
        let var_ref = adapter.var_store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/unknown".into(),
        });

        let vars_resp = adapter
            .handle_request(&DapRequest {
                seq: 1,
                command: "variables".into(),
                arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
            })
            .await;
        assert!(
            vars_resp.success,
            "Unknown object type should succeed with empty list"
        );
        let body = vars_resp.body.unwrap();
        let vars = body["variables"].as_array().unwrap();
        assert!(
            vars.is_empty(),
            "Unknown object type should return empty list"
        );
    }

    // ── log_level_to_category ──────────────────────────────────────────────

    #[test]
    fn test_log_level_to_category_error_is_stderr() {
        assert_eq!(log_level_to_category("error"), "stderr");
    }

    #[test]
    fn test_log_level_to_category_info_is_stdout() {
        assert_eq!(log_level_to_category("info"), "stdout");
    }

    #[test]
    fn test_log_level_to_category_debug_is_console() {
        assert_eq!(log_level_to_category("debug"), "console");
    }

    #[test]
    fn test_log_level_to_category_warning_is_console() {
        assert_eq!(log_level_to_category("warning"), "console");
    }

    #[test]
    fn test_log_level_to_category_unknown_is_console() {
        assert_eq!(log_level_to_category("verbose"), "console");
        assert_eq!(log_level_to_category("trace"), "console");
        assert_eq!(log_level_to_category(""), "console");
    }

    // ── LogOutput event handling ───────────────────────────────────────────

    #[tokio::test]
    async fn test_log_output_error_emits_stderr_category() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "Something went wrong".to_string(),
                level: "error".to_string(),
                source_uri: None,
                line: None,
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            assert_eq!(ev.event, "output");
            let body = ev.body.unwrap();
            assert_eq!(body["category"], "stderr");
            assert_eq!(body["output"], "Something went wrong\n");
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_log_output_info_emits_stdout_category() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "App started".to_string(),
                level: "info".to_string(),
                source_uri: None,
                line: None,
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            assert_eq!(ev.event, "output");
            let body = ev.body.unwrap();
            assert_eq!(body["category"], "stdout");
            assert_eq!(body["output"], "App started\n");
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_log_output_debug_emits_console_category() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "debug message".to_string(),
                level: "debug".to_string(),
                source_uri: None,
                line: None,
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            assert_eq!(ev.event, "output");
            let body = ev.body.unwrap();
            assert_eq!(body["category"], "console");
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_log_output_message_ends_with_newline() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        // Message without trailing newline — adapter must add one.
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "no newline".to_string(),
                level: "info".to_string(),
                source_uri: None,
                line: None,
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            let body = ev.body.unwrap();
            let output = body["output"].as_str().unwrap();
            assert!(output.ends_with('\n'), "output must end with newline");
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_log_output_message_already_has_newline_not_doubled() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "has newline\n".to_string(),
                level: "info".to_string(),
                source_uri: None,
                line: None,
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            let body = ev.body.unwrap();
            let output = body["output"].as_str().unwrap();
            assert_eq!(output, "has newline\n", "newline should not be doubled");
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_log_output_with_source_location() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "Error: null check".to_string(),
                level: "error".to_string(),
                source_uri: Some("file:///home/user/app/lib/main.dart".to_string()),
                line: Some(42),
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            let body = ev.body.unwrap();
            assert_eq!(body["category"], "stderr");
            assert_eq!(body["output"], "Error: null check\n");
            assert_eq!(body["source"]["path"], "/home/user/app/lib/main.dart");
            assert_eq!(body["source"]["name"], "main.dart");
            assert_eq!(body["line"], 42);
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    #[tokio::test]
    async fn test_log_output_without_source_has_no_source_field() {
        let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .handle_debug_event(DebugEvent::LogOutput {
                message: "plain log".to_string(),
                level: "info".to_string(),
                source_uri: None,
                line: None,
            })
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            let body = ev.body.unwrap();
            assert!(
                body.get("source").is_none() || body["source"].is_null(),
                "source should not be present when source_uri is None"
            );
            assert!(
                body.get("line").is_none() || body["line"].is_null(),
                "line should not be present when source_uri is None"
            );
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }

    // ── DapEvent::output constructor ───────────────────────────────────────

    #[test]
    fn test_output_event_structure() {
        use crate::{DapEvent, DapMessage};
        let event = DapEvent::output("stderr", "Error: null check\n");
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(json["type"], "event");
        assert_eq!(json["event"], "output");
        assert_eq!(json["body"]["category"], "stderr");
        assert_eq!(json["body"]["output"], "Error: null check\n");
    }

    #[test]
    fn test_output_event_with_source_in_body() {
        use crate::{DapEvent, DapMessage};
        let body = serde_json::json!({
            "category": "stderr",
            "output": "Error: null check\n",
            "source": {
                "name": "main.dart",
                "path": "/home/user/app/lib/main.dart"
            },
            "line": 42
        });
        let event = DapEvent::new("output", Some(body));
        let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
        assert_eq!(
            json["body"]["source"]["path"],
            "/home/user/app/lib/main.dart"
        );
        assert_eq!(json["body"]["line"], 42);
    }

    // ── emit_output helper ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_emit_output_sends_output_event() {
        let (adapter, mut rx) = DapAdapter::new(MockBackend);
        adapter
            .emit_output("console", "Flutter Demon: Attached to VM Service\n")
            .await;

        let msg = rx.try_recv().expect("Should have received an event");
        if let DapMessage::Event(ev) = msg {
            assert_eq!(ev.event, "output");
            let body = ev.body.unwrap();
            assert_eq!(body["category"], "console");
            assert_eq!(body["output"], "Flutter Demon: Attached to VM Service\n");
        } else {
            panic!("Expected Event, got {:?}", msg);
        }
    }
}
