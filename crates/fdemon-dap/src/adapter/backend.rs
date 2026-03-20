//! # Debug Backend Traits and Type-Erased Wrapper
//!
//! This module defines the [`LocalDebugBackend`] / [`DebugBackend`] trait that
//! the DAP adapter uses to issue commands to the Dart VM Service, along with
//! [`DynDebugBackend`] — an object-safe wrapper that bridges the `async fn`
//! trait to a `Box<dyn ...>` vtable.

use std::future::Future;
use std::pin::Pin;

use crate::adapter::types::{BreakpointResult, DapExceptionPauseMode, StepMode};

// Re-export BackendError at this level for convenience.
pub use crate::adapter::types::BackendError;

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
    async fn pause(&self, isolate_id: &str) -> Result<(), BackendError>;

    /// Resume a paused isolate, optionally with a step mode and frame index.
    ///
    /// `frame_index` is only used when `step` is [`StepMode::Rewind`] (the
    /// `restartFrame` DAP request). All other step modes must pass `None` for
    /// `frame_index`.
    async fn resume(
        &self,
        isolate_id: &str,
        step: Option<StepMode>,
        frame_index: Option<i32>,
    ) -> Result<(), BackendError>;

    // ── Breakpoints ───────────────────────────────────────────────────────

    /// Add a breakpoint at the given source URI and line.
    async fn add_breakpoint(
        &self,
        isolate_id: &str,
        uri: &str,
        line: i32,
        column: Option<i32>,
    ) -> Result<BreakpointResult, BackendError>;

    /// Remove a previously added breakpoint by its VM ID.
    async fn remove_breakpoint(
        &self,
        isolate_id: &str,
        breakpoint_id: &str,
    ) -> Result<(), BackendError>;

    /// Set the exception pause mode for an isolate.
    async fn set_exception_pause_mode(
        &self,
        isolate_id: &str,
        mode: DapExceptionPauseMode,
    ) -> Result<(), BackendError>;

    // ── Stack inspection ──────────────────────────────────────────────────

    /// Get the current call stack for a paused isolate.
    async fn get_stack(
        &self,
        isolate_id: &str,
        limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError>;

    /// Get a VM Service object (for variable expansion).
    async fn get_object(
        &self,
        isolate_id: &str,
        object_id: &str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError>;

    // ── Evaluation ────────────────────────────────────────────────────────

    /// Evaluate an expression in the context of a target object.
    async fn evaluate(
        &self,
        isolate_id: &str,
        target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError>;

    /// Evaluate an expression in the context of a specific stack frame.
    async fn evaluate_in_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError>;

    // ── Thread / isolate info ─────────────────────────────────────────────

    /// Get the VM object (lists all isolates).
    async fn get_vm(&self) -> Result<serde_json::Value, BackendError>;

    /// Get the full isolate object for the given isolate ID.
    ///
    /// Returns the isolate object including `rootLib`, `libraries[]`,
    /// `pauseEvent`, etc. This is the reliable way to obtain `rootLib` and is
    /// needed for globals scope (library enumeration) and `updateDebugOptions`
    /// (setting library debuggability).
    async fn get_isolate(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError>;

    /// Get the list of scripts loaded in an isolate.
    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError>;

    // ── Generic VM Service RPC ────────────────────────────────────────────

    /// Forward an arbitrary VM Service RPC call.
    ///
    /// Used by the `callService` custom DAP request to expose VM Service
    /// extension methods (e.g., DevTools integration RPCs) without adding
    /// dedicated methods to this trait.
    async fn call_service(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError>;

    /// Set the debuggability flag for a library in an isolate.
    ///
    /// Calls `setLibraryDebuggable` VM Service RPC. Used by `updateDebugOptions`
    /// to toggle SDK/external library stepping behaviour.
    async fn set_library_debuggable(
        &self,
        isolate_id: &str,
        library_id: &str,
        is_debuggable: bool,
    ) -> Result<(), BackendError>;

    /// Get a source report for a script in an isolate.
    ///
    /// Calls `getSourceReport` VM Service RPC. `report_kinds` is a slice of
    /// report kind strings (e.g., `["PossibleBreakpoints"]`). `token_pos` and
    /// `end_token_pos` are optional token position bounds for partial reports.
    /// Used by `breakpointLocations` to find valid breakpoint positions.
    async fn get_source_report(
        &self,
        isolate_id: &str,
        script_id: &str,
        report_kinds: &[&str],
        token_pos: Option<i64>,
        end_token_pos: Option<i64>,
    ) -> Result<serde_json::Value, BackendError>;

    // ── Source retrieval ──────────────────────────────────────────────────

    /// Fetch the source text of a Dart script by its VM Service object ID.
    ///
    /// Called by the `source` DAP request handler to serve read-only source
    /// content for SDK (`dart:`) and unresolvable package URIs. The VM Service
    /// `getObject` RPC on a `Script` object returns a `source` field with the
    /// full source text.
    ///
    /// Returns the source text on success, or an error string on failure.
    async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, String>;

    // ── Hot reload / restart ──────────────────────────────────────────────

    /// Trigger a Flutter hot reload.
    ///
    /// This sends `Message::HotReload` through the TEA pipeline, which calls
    /// `FlutterController::reload()` on the active session. The operation is
    /// fire-and-forget from the adapter's perspective; the IDE will receive a
    /// `dart.hotReloadComplete` custom event when reload finishes (emitted by
    /// the Engine event loop).
    async fn hot_reload(&self) -> Result<(), BackendError>;

    /// Trigger a Flutter hot restart.
    ///
    /// Sends `Message::HotRestart` through the TEA pipeline. Hot restart
    /// creates a new Dart isolate, invalidating all breakpoints and variable
    /// references. Breakpoint re-application after restart is handled by
    /// Task 10 (breakpoint persistence).
    async fn hot_restart(&self) -> Result<(), BackendError>;

    /// Stop the running Flutter application.
    ///
    /// Sends `Message::StopApp` through the TEA pipeline, terminating the
    /// Flutter process. Called by `handle_disconnect` when
    /// `terminateDebuggee: true` is set — the IDE wants the app to stop when
    /// the debug session ends.
    async fn stop_app(&self) -> Result<(), BackendError>;

    // ── Session metadata ──────────────────────────────────────────────────

    /// Return the VM Service WebSocket URI for this debug session, if available.
    ///
    /// Used by [`DapAdapter::handle_attach`] to emit the `dart.debuggerUris`
    /// custom event after a successful attach. IDEs (notably VS Code's Dart
    /// extension) use this URI to connect supplementary tooling such as
    /// DevTools.
    ///
    /// Returns `None` when no VM Service connection is established (e.g., when
    /// using [`NoopBackend`] in tests or before attach completes).
    async fn ws_uri(&self) -> Option<String>;

    /// Return the device ID for this debug session, if available.
    ///
    /// Used by [`DapAdapter::handle_attach`] to emit the `flutter.appStart`
    /// custom event. Mirrors the `deviceId` field expected by the Flutter DAP
    /// convention.
    ///
    /// Returns `None` when device information is unavailable.
    async fn device_id(&self) -> Option<String>;

    /// Return the build mode for this debug session (e.g., `"debug"`, `"profile"`, `"release"`).
    ///
    /// Used by [`DapAdapter::handle_attach`] to populate the `mode` field in
    /// the `flutter.appStart` custom event.
    ///
    /// Returns `"debug"` by default when the mode is unknown.
    async fn build_mode(&self) -> String;
}

// ─────────────────────────────────────────────────────────────────────────────
// DynDebugBackend — object-safe type-erased wrapper
// ─────────────────────────────────────────────────────────────────────────────

/// Object-safe vtable for debug backend operations.
///
/// [`DebugBackend`] (generated by `trait_variant::make`) is **not** dyn-compatible
/// because its methods return `impl Future` (RPIT). This internal trait replaces
/// every method return type with `Pin<Box<dyn Future + Send>>`, making it
/// compatible with `Box<dyn DynDebugBackendInner>`.
///
/// External code (in `fdemon-app`) implements this trait for `VmServiceBackend`
/// and constructs a [`DynDebugBackend`] via [`DynDebugBackend::new`].
pub trait DynDebugBackendInner: Send + Sync + 'static {
    fn pause_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>>;

    fn resume_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        step: Option<StepMode>,
        frame_index: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>>;

    fn add_breakpoint_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        uri: &'a str,
        line: i32,
        column: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<BreakpointResult, BackendError>> + Send + 'a>>;

    fn remove_breakpoint_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        breakpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>>;

    fn set_exception_pause_mode_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        mode: DapExceptionPauseMode,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>>;

    fn get_stack_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        limit: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn get_object_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        object_id: &'a str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn evaluate_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        target_id: &'a str,
        expression: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn evaluate_in_frame_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        frame_index: i32,
        expression: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn get_vm_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + '_>>;

    fn get_isolate_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn get_scripts_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn call_service_boxed<'a>(
        &'a self,
        method: &'a str,
        params: Option<serde_json::Value>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn set_library_debuggable_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        library_id: &'a str,
        is_debuggable: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>>;

    fn get_source_report_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        script_id: &'a str,
        report_kinds: Vec<String>,
        token_pos: Option<i64>,
        end_token_pos: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>>;

    fn get_source_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        script_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

    fn hot_reload_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>>;

    fn hot_restart_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>>;

    fn stop_app_boxed(&self)
        -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>>;

    fn ws_uri_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>>;

    fn device_id_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>>;

    fn build_mode_boxed(&self) -> Pin<Box<dyn Future<Output = String> + Send + '_>>;
}

/// Type-erased debug backend that satisfies the [`DebugBackend`] bound.
///
/// Wraps a `Box<dyn DynDebugBackendInner>` and implements [`DebugBackend`] by
/// delegating each `async fn` through the boxed-future vtable.  This is the
/// concrete type used in [`crate::server::BackendHandle`].
///
/// ## Usage (in `fdemon-app`)
///
/// ```ignore
/// // Step 1: implement DynDebugBackendInner for your concrete backend
/// impl DynDebugBackendInner for VmServiceBackend {
///     fn pause_boxed<'a>(&'a self, isolate_id: &'a str)
///         -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>
///     {
///         Box::pin(self.pause(isolate_id))
///     }
///     // ... rest of methods
/// }
///
/// // Step 2: wrap it
/// let backend = DynDebugBackend::new(Box::new(VmServiceBackend::new(handle)));
/// ```
pub struct DynDebugBackend {
    inner: Box<dyn DynDebugBackendInner>,
}

impl DynDebugBackend {
    /// Wrap a [`DynDebugBackendInner`] in a type-erased backend.
    pub fn new(inner: Box<dyn DynDebugBackendInner>) -> Self {
        Self { inner }
    }
}

impl DebugBackend for DynDebugBackend {
    async fn pause(&self, isolate_id: &str) -> Result<(), BackendError> {
        self.inner.pause_boxed(isolate_id).await
    }

    async fn resume(
        &self,
        isolate_id: &str,
        step: Option<StepMode>,
        frame_index: Option<i32>,
    ) -> Result<(), BackendError> {
        self.inner.resume_boxed(isolate_id, step, frame_index).await
    }

    async fn add_breakpoint(
        &self,
        isolate_id: &str,
        uri: &str,
        line: i32,
        column: Option<i32>,
    ) -> Result<BreakpointResult, BackendError> {
        self.inner
            .add_breakpoint_boxed(isolate_id, uri, line, column)
            .await
    }

    async fn remove_breakpoint(
        &self,
        isolate_id: &str,
        breakpoint_id: &str,
    ) -> Result<(), BackendError> {
        self.inner
            .remove_breakpoint_boxed(isolate_id, breakpoint_id)
            .await
    }

    async fn set_exception_pause_mode(
        &self,
        isolate_id: &str,
        mode: DapExceptionPauseMode,
    ) -> Result<(), BackendError> {
        self.inner
            .set_exception_pause_mode_boxed(isolate_id, mode)
            .await
    }

    async fn get_stack(
        &self,
        isolate_id: &str,
        limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        self.inner.get_stack_boxed(isolate_id, limit).await
    }

    async fn get_object(
        &self,
        isolate_id: &str,
        object_id: &str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        self.inner
            .get_object_boxed(isolate_id, object_id, offset, count)
            .await
    }

    async fn evaluate(
        &self,
        isolate_id: &str,
        target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        self.inner
            .evaluate_boxed(isolate_id, target_id, expression)
            .await
    }

    async fn evaluate_in_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        self.inner
            .evaluate_in_frame_boxed(isolate_id, frame_index, expression)
            .await
    }

    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        self.inner.get_vm_boxed().await
    }

    async fn get_isolate(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        self.inner.get_isolate_boxed(isolate_id).await
    }

    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        self.inner.get_scripts_boxed(isolate_id).await
    }

    async fn call_service(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        self.inner.call_service_boxed(method, params).await
    }

    async fn set_library_debuggable(
        &self,
        isolate_id: &str,
        library_id: &str,
        is_debuggable: bool,
    ) -> Result<(), BackendError> {
        self.inner
            .set_library_debuggable_boxed(isolate_id, library_id, is_debuggable)
            .await
    }

    async fn get_source_report(
        &self,
        isolate_id: &str,
        script_id: &str,
        report_kinds: &[&str],
        token_pos: Option<i64>,
        end_token_pos: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        self.inner
            .get_source_report_boxed(
                isolate_id,
                script_id,
                report_kinds.iter().map(|s| s.to_string()).collect(),
                token_pos,
                end_token_pos,
            )
            .await
    }

    async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, String> {
        self.inner.get_source_boxed(isolate_id, script_id).await
    }

    async fn hot_reload(&self) -> Result<(), BackendError> {
        self.inner.hot_reload_boxed().await
    }

    async fn hot_restart(&self) -> Result<(), BackendError> {
        self.inner.hot_restart_boxed().await
    }

    async fn stop_app(&self) -> Result<(), BackendError> {
        self.inner.stop_app_boxed().await
    }

    async fn ws_uri(&self) -> Option<String> {
        self.inner.ws_uri_boxed().await
    }

    async fn device_id(&self) -> Option<String> {
        self.inner.device_id_boxed().await
    }

    async fn build_mode(&self) -> String {
        self.inner.build_mode_boxed().await
    }
}
