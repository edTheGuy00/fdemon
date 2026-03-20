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

pub mod backend;
pub mod breakpoints;
pub mod evaluate;
mod events;
mod handlers;
pub mod stack;
pub mod threads;
pub mod types;
mod variables;

/// Shared test infrastructure: [`test_helpers::MockTestBackend`] trait with
/// default no-op implementations for all [`DebugBackend`] methods, plus a
/// blanket [`DebugBackend`] impl for types that implement it.
#[cfg(test)]
pub(crate) mod test_helpers;

use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::DapMessage;

pub use backend::{
    BackendError, DebugBackend, DynDebugBackend, DynDebugBackendInner, LocalDebugBackend,
};
pub use breakpoints::{
    parse_log_message, BreakpointCondition, BreakpointManager, BreakpointState, DesiredBreakpoint,
    LogSegment,
};
pub use stack::{
    dart_uri_to_path, extract_line_column, extract_source, extract_source_with_store,
    resolve_package_uri, FrameRef, FrameStore, ScopeKind, SourceRefInfo, SourceReferenceStore,
    VariableRef, VariableStore,
};
pub use threads::{
    session_index_from_thread_id, session_thread_base, DapSessionId, MultiSessionThreadMap,
    ThreadMap, MAX_SESSIONS, THREADS_PER_SESSION,
};
pub use types::{
    log_level_to_category, BreakpointResult, DapExceptionPauseMode, DebugEvent, PauseReason,
    StepMode,
};

use types::EVENT_CHANNEL_CAPACITY;

// ─────────────────────────────────────────────────────────────────────────────
// DapAdapter
// ─────────────────────────────────────────────────────────────────────────────

/// Stores the exception `InstanceRef` captured when an isolate pauses at an
/// exception (`PauseException` event).
///
/// Keyed by DAP thread ID. Cleared when the isolate resumes.
#[derive(Debug, Clone)]
pub struct ExceptionRef {
    /// The Dart VM isolate ID that owns the exception.
    pub isolate_id: String,
    /// The raw `InstanceRef` JSON from the VM Service `PauseException` event.
    pub instance_ref: serde_json::Value,
}

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
    /// Used by command handlers (threads, breakpoints, stack traces, evaluate).
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

    /// Breakpoint tracking state (active VM-tracked breakpoints).
    ///
    /// This is cleared on hot restart and rebuilt from `desired_breakpoints`.
    breakpoint_state: BreakpointState,

    /// Desired breakpoints as requested by the IDE, keyed by source URI.
    ///
    /// This is the "intended" state that **survives hot restart**. On
    /// `IsolateRunnable` after restart, all entries are re-applied to the
    /// new isolate via `addBreakpointWithScriptUri`. DAP IDs here are stable
    /// and match those in `breakpoint_state`.
    desired_breakpoints: HashMap<String, Vec<DesiredBreakpoint>>,

    /// Current exception pause mode.
    ///
    /// Defaults to [`DapExceptionPauseMode::Unhandled`].
    /// Set by `setExceptionBreakpoints` and applied to all known isolates.
    exception_mode: DapExceptionPauseMode,

    /// Ordered list of paused isolate IDs.
    ///
    /// The most recently paused isolate is at the back of the list. When an
    /// isolate resumes, it is removed from this list. Used by `handle_evaluate`
    /// to pick the isolate context when no `threadId` is provided.
    paused_isolates: Vec<String>,

    /// Source reference allocator and lookup for SDK and unresolvable package URIs.
    ///
    /// Unlike frame IDs and variable references, source references persist
    /// across stop/resume transitions — the IDE may request source text at any
    /// time. They are invalidated on hot restart via
    /// [`DapAdapter::on_hot_restart`], which calls
    /// [`SourceReferenceStore::clear`].
    source_reference_store: SourceReferenceStore,

    /// Whether the VM Service has disconnected mid-session (e.g., `AppExited`).
    ///
    /// When `true`, all subsequent DAP requests return a structured error
    /// response with code [`ERR_VM_DISCONNECTED`] rather than attempting any
    /// backend calls. This prevents spurious errors when the IDE continues
    /// sending requests after the app exits.
    vm_disconnected: bool,

    /// Exception references keyed by DAP thread ID.
    ///
    /// Populated when an isolate pauses at a `PauseException` event and the
    /// event carries an `exception` `InstanceRef`. Cleared when the isolate
    /// resumes via [`DapAdapter::on_resume`]. Used by [`handle_scopes`] to
    /// conditionally include an "Exceptions" scope, and by [`handle_evaluate`]
    /// to support the `$_threadException` magic expression.
    pub exception_refs: HashMap<i64, ExceptionRef>,

    /// Maps variable references (i64) to their `evaluateName` expressions.
    ///
    /// Populated when a variable reference is allocated by
    /// `instance_ref_to_variable_with_eval_name` and an `evaluate_name` is
    /// provided. Cleared on every resume alongside `var_store`.
    ///
    /// Used by `expand_object` to look up the parent's evaluate expression and
    /// construct child evaluate expressions (e.g., `obj.field`, `list[0]`).
    pub(crate) evaluate_name_map: HashMap<i64, String>,

    /// Whether to eagerly evaluate getter methods when expanding objects.
    ///
    /// When `true` (the default), getters on `PlainInstance` objects are
    /// evaluated immediately with a 1-second timeout when the user expands the
    /// object in the variables panel. Getter results appear alongside regular
    /// fields with `presentationHint.attributes: ["hasSideEffects"]`.
    ///
    /// When `false`, getters appear as lazy items with `presentationHint.lazy:
    /// true`. The user must explicitly expand the getter to evaluate it; the
    /// expansion triggers a `GetterEval` variable reference lookup.
    ///
    /// Settable from the `attach` request args (`evaluateGettersInDebugViews`).
    pub(crate) evaluate_getters_in_debug_views: bool,

    /// Whether to call `toString()` on `PlainInstance` objects and append the
    /// result to the variable display value.
    ///
    /// When `true` (the default), a `toString()` call with a 1-second timeout
    /// is issued for each `PlainInstance`, `RegExp`, and `StackTrace` variable
    /// in a scope. If the result is not the default Dart `"Instance of
    /// 'ClassName'"` pattern, it is appended to the display value:
    /// `"MyClass (custom string repr)"`.
    ///
    /// When `false`, no `toString()` calls are made and the display value is
    /// just the class name.
    ///
    /// Settable from the `attach` request args (`evaluateToStringInDebugViews`).
    pub(crate) evaluate_to_string_in_debug_views: bool,

    /// The 0-based frame index of the first `AsyncSuspensionMarker` frame in
    /// the current stopped state, or `None` if no async marker was seen.
    ///
    /// Populated during `handle_stack_trace` by scanning frames for
    /// `kind: "AsyncSuspensionMarker"`. Cleared on every resume via
    /// [`DapAdapter::on_resume`].
    ///
    /// Used by `handle_restart_frame` to reject rewind requests that target
    /// frames at or above the first async suspension boundary. The VM does not
    /// allow rewinding past an async suspension marker.
    pub(crate) first_async_marker_index: Option<i32>,
}

impl<B: DebugBackend> DapAdapter<B> {
    /// Create a new [`DapAdapter`] with the given backend.
    ///
    /// Returns the adapter and the receiver end of the event channel. The
    /// caller (session task) should poll the receiver and forward events
    /// to the DAP client.
    pub fn new(backend: B) -> (Self, mpsc::Receiver<DapMessage>) {
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (adapter, ()) = Self::new_with_tx(backend, event_tx);
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
            desired_breakpoints: HashMap::new(),
            exception_mode: DapExceptionPauseMode::Unhandled,
            paused_isolates: Vec::new(),
            source_reference_store: SourceReferenceStore::new(),
            vm_disconnected: false,
            exception_refs: HashMap::new(),
            evaluate_name_map: HashMap::new(),
            evaluate_getters_in_debug_views: true,
            evaluate_to_string_in_debug_views: true,
            first_async_marker_index: None,
        };
        (adapter, ())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NOTE: handle_stack_trace, handle_scopes, handle_variables,
// get_scope_variables, instance_ref_to_variable, expand_object
// were extracted to adapter/variables.rs (Task 04).
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
