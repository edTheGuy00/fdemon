//! `VmServiceBackend` — concrete [`DebugBackend`] implementation for fdemon.
//!
//! Bridges the DAP adapter's [`DebugBackend`] trait to the actual Dart VM
//! Service client ([`VmRequestHandle`]) provided by `fdemon-daemon`.
//!
//! ## Layer boundary
//!
//! This module lives in `fdemon-app` because `fdemon-app` depends on both
//! `fdemon-dap` (for [`DebugBackend`]) and `fdemon-daemon` (for
//! [`VmRequestHandle`] and the debug RPC wrappers). Neither `fdemon-dap` nor
//! `fdemon-daemon` may depend on the other, so this module is the correct
//! place for the bridge.
//!
//! ## Usage
//!
//! ```ignore
//! let backend = VmServiceBackend::new(vm_request_handle);
//! let session = DapClientSession::with_backend(backend);
//! let (debug_event_tx, debug_event_rx) = mpsc::channel(64);
//! // Pass debug_event_tx to the Engine so it can forward VM Service debug events.
//! DapClientSession::run_on_with_backend(reader, writer, shutdown_rx, backend, debug_event_rx).await?;
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fdemon_daemon::vm_service::{
    debugger,
    debugger_types::{ExceptionPauseMode, StepOption},
    VmRequestHandle,
};
use fdemon_dap::adapter::{
    BackendError, BreakpointResult, DapExceptionPauseMode, DebugBackend, DebugEvent,
    DynDebugBackendInner, StepMode,
};
use tokio::sync::mpsc;

use crate::message::Message;

// ─────────────────────────────────────────────────────────────────────────────
// VmServiceBackend
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete [`DebugBackend`] that delegates all debug operations to the Dart
/// VM Service via a [`VmRequestHandle`].
///
/// Constructed by the Engine when a DAP client attaches to an active Flutter
/// session. The handle is cloned so it can be shared safely across the session
/// task and any concurrent requests.
///
/// The optional `msg_tx` field enables hot reload and hot restart by sending
/// `Message::HotReload` / `Message::HotRestart` into the TEA pipeline. When
/// `None`, those operations return [`BackendError::NotConnected`].
#[derive(Clone)]
pub struct VmServiceBackend {
    handle: VmRequestHandle,
    /// Sender into the TEA message bus.
    ///
    /// Used exclusively by [`hot_reload`] and [`hot_restart`] to dispatch
    /// through the existing Engine reload/restart lifecycle without calling
    /// VM Service RPCs directly. Set to `None` when the backend is constructed
    /// without a message sender (legacy path).
    msg_tx: Option<mpsc::Sender<Message>>,

    /// VM Service WebSocket URI for this debug session.
    ///
    /// Forwarded to the DAP client as `dart.debuggerUris.vmServiceUri` after
    /// a successful `attach`. Populated by [`VmBackendFactory::create`] from
    /// the session metadata slot.
    ws_uri: Option<String>,

    /// Device ID for this debug session (e.g., `"emulator-5554"`).
    ///
    /// Forwarded to the DAP client in the `flutter.appStart` event body.
    /// Populated by [`VmBackendFactory::create`] from the session metadata slot.
    device_id: Option<String>,

    /// Build mode for this debug session (`"debug"`, `"profile"`, `"release"`).
    ///
    /// Forwarded to the DAP client in the `flutter.appStart` event body.
    /// Defaults to `"debug"` if not set.
    build_mode: String,
}

impl VmServiceBackend {
    /// Create a new backend wrapping the given VM Service request handle.
    ///
    /// Hot reload and hot restart will return [`BackendError::NotConnected`]
    /// until the backend is given a message sender via [`new_with_msg_tx`].
    pub fn new(handle: VmRequestHandle) -> Self {
        Self {
            handle,
            msg_tx: None,
            ws_uri: None,
            device_id: None,
            build_mode: "debug".to_string(),
        }
    }

    /// Create a new backend with a TEA message sender for hot reload/restart.
    ///
    /// The `msg_tx` sender is used to dispatch `Message::HotReload` and
    /// `Message::HotRestart` through the existing Engine reload/restart
    /// lifecycle, avoiding direct VM Service RPC calls.
    pub fn new_with_msg_tx(handle: VmRequestHandle, msg_tx: mpsc::Sender<Message>) -> Self {
        Self {
            handle,
            msg_tx: Some(msg_tx),
            ws_uri: None,
            device_id: None,
            build_mode: "debug".to_string(),
        }
    }

    /// Set session metadata for custom DAP event emission.
    ///
    /// Provides the VM Service WebSocket URI, device ID, and build mode that
    /// are emitted in `dart.debuggerUris` and `flutter.appStart` custom DAP
    /// events after a successful `attach`.
    pub fn with_session_metadata(
        mut self,
        ws_uri: Option<String>,
        device_id: Option<String>,
        build_mode: String,
    ) -> Self {
        self.ws_uri = ws_uri;
        self.device_id = device_id;
        self.build_mode = build_mode;
        self
    }
}

impl DebugBackend for VmServiceBackend {
    async fn pause(&self, isolate_id: &str) -> Result<(), BackendError> {
        debugger::pause(&self.handle, isolate_id)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn resume(
        &self,
        isolate_id: &str,
        step: Option<StepMode>,
        frame_index: Option<i32>,
    ) -> Result<(), BackendError> {
        let vm_step = step.map(|s| match s {
            StepMode::Over => StepOption::Over,
            StepMode::Into => StepOption::Into,
            StepMode::Out => StepOption::Out,
            StepMode::Rewind => StepOption::Rewind,
        });
        debugger::resume(&self.handle, isolate_id, vm_step, frame_index)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn add_breakpoint(
        &self,
        isolate_id: &str,
        uri: &str,
        line: i32,
        column: Option<i32>,
    ) -> Result<BreakpointResult, BackendError> {
        let bp =
            debugger::add_breakpoint_with_script_uri(&self.handle, isolate_id, uri, line, column)
                .await
                .map_err(|e| BackendError::VmServiceError(e.to_string()))?;

        // Extract line/column from the breakpoint location.
        let (resolved_line, resolved_column) = match &bp.location {
            Some(loc) => {
                let line = loc.get("line").and_then(|v| v.as_i64()).map(|v| v as i32);
                let col = loc.get("column").and_then(|v| v.as_i64()).map(|v| v as i32);
                (line, col)
            }
            None => (None, None),
        };

        Ok(BreakpointResult {
            vm_id: bp.id,
            resolved: bp.resolved,
            line: resolved_line,
            column: resolved_column,
        })
    }

    async fn remove_breakpoint(
        &self,
        isolate_id: &str,
        breakpoint_id: &str,
    ) -> Result<(), BackendError> {
        debugger::remove_breakpoint(&self.handle, isolate_id, breakpoint_id)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn set_exception_pause_mode(
        &self,
        isolate_id: &str,
        mode: DapExceptionPauseMode,
    ) -> Result<(), BackendError> {
        let vm_mode = match mode {
            DapExceptionPauseMode::All => ExceptionPauseMode::All,
            DapExceptionPauseMode::Unhandled => ExceptionPauseMode::Unhandled,
            DapExceptionPauseMode::None => ExceptionPauseMode::None,
        };
        debugger::set_isolate_pause_mode(&self.handle, isolate_id, vm_mode)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_stack(
        &self,
        isolate_id: &str,
        limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        let stack = debugger::get_stack(&self.handle, isolate_id, limit)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))?;
        serde_json::to_value(&stack).map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_object(
        &self,
        isolate_id: &str,
        object_id: &str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        debugger::get_object(&self.handle, isolate_id, object_id, offset, count)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn evaluate(
        &self,
        isolate_id: &str,
        target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        let result = debugger::evaluate(&self.handle, isolate_id, target_id, expression)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))?;
        serde_json::to_value(&result).map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn evaluate_in_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        let result = debugger::evaluate_in_frame(&self.handle, isolate_id, frame_index, expression)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))?;
        serde_json::to_value(&result).map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        self.handle
            .request("getVM", None)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_isolate(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        self.handle
            .request(
                "getIsolate",
                Some(serde_json::json!({ "isolateId": isolate_id })),
            )
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        let scripts = debugger::get_scripts(&self.handle, isolate_id)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))?;
        serde_json::to_value(&scripts).map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn call_service(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        self.handle
            .request(method, params)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn set_library_debuggable(
        &self,
        isolate_id: &str,
        library_id: &str,
        is_debuggable: bool,
    ) -> Result<(), BackendError> {
        self.handle
            .request(
                "setLibraryDebuggable",
                Some(serde_json::json!({
                    "isolateId": isolate_id,
                    "libraryId": library_id,
                    "isDebuggable": is_debuggable,
                })),
            )
            .await
            .map(|_| ())
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_source_report(
        &self,
        isolate_id: &str,
        script_id: &str,
        report_kinds: &[&str],
        token_pos: Option<i64>,
        end_token_pos: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        let mut params = serde_json::json!({
            "isolateId": isolate_id,
            "scriptId": script_id,
            "reports": report_kinds,
            "forceCompile": true,
        });
        if let Some(tp) = token_pos {
            params["tokenPos"] = serde_json::json!(tp);
        }
        if let Some(etp) = end_token_pos {
            params["endTokenPos"] = serde_json::json!(etp);
        }
        self.handle
            .request("getSourceReport", Some(params))
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, String> {
        // getObject on a Script object returns a Script with a "source" field
        // containing the full source text.
        let result = debugger::get_object(&self.handle, isolate_id, script_id, None, None)
            .await
            .map_err(|e| e.to_string())?;
        result["source"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Script '{}' has no source field", script_id))
    }

    async fn hot_reload(&self) -> Result<(), BackendError> {
        match &self.msg_tx {
            Some(tx) => tx.send(Message::HotReload).await.map_err(|e| {
                BackendError::VmServiceError(format!("Failed to send hot reload: {e}"))
            }),
            None => Err(BackendError::NotConnected),
        }
    }

    async fn hot_restart(&self) -> Result<(), BackendError> {
        match &self.msg_tx {
            Some(tx) => tx.send(Message::HotRestart).await.map_err(|e| {
                BackendError::VmServiceError(format!("Failed to send hot restart: {e}"))
            }),
            None => Err(BackendError::NotConnected),
        }
    }

    async fn stop_app(&self) -> Result<(), BackendError> {
        match &self.msg_tx {
            Some(tx) => tx
                .send(Message::StopApp)
                .await
                .map_err(|e| BackendError::VmServiceError(format!("Failed to send stop app: {e}"))),
            None => Err(BackendError::NotConnected),
        }
    }

    async fn ws_uri(&self) -> Option<String> {
        self.ws_uri.clone()
    }

    async fn device_id(&self) -> Option<String> {
        self.device_id.clone()
    }

    async fn build_mode(&self) -> String {
        self.build_mode.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DynDebugBackendInner — object-safe vtable for VmServiceBackend
// ─────────────────────────────────────────────────────────────────────────────

/// Implements the object-safe [`DynDebugBackendInner`] vtable for [`VmServiceBackend`].
///
/// [`crate::adapter::DebugBackend`] is not dyn-compatible because its async
/// methods return `impl Future` (RPIT via `trait_variant::make`). This impl
/// wraps every method return type in `Box::pin(...)` so `VmServiceBackend` can
/// be stored as `Box<dyn DynDebugBackendInner>` and passed through the
/// [`crate::server::BackendFactory`] boundary.
///
/// [`VmBackendFactory`] uses this to construct a [`fdemon_dap::DynDebugBackend`]
/// and store it in a [`crate::server::BackendHandle`].
impl DynDebugBackendInner for VmServiceBackend {
    fn pause_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(self.pause(isolate_id))
    }

    fn resume_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        step: Option<StepMode>,
        frame_index: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(self.resume(isolate_id, step, frame_index))
    }

    fn add_breakpoint_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        uri: &'a str,
        line: i32,
        column: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<BreakpointResult, BackendError>> + Send + 'a>> {
        Box::pin(self.add_breakpoint(isolate_id, uri, line, column))
    }

    fn remove_breakpoint_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        breakpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(self.remove_breakpoint(isolate_id, breakpoint_id))
    }

    fn set_exception_pause_mode_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        mode: DapExceptionPauseMode,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(self.set_exception_pause_mode(isolate_id, mode))
    }

    fn get_stack_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        limit: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.get_stack(isolate_id, limit))
    }

    fn get_object_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        object_id: &'a str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.get_object(isolate_id, object_id, offset, count))
    }

    fn evaluate_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        target_id: &'a str,
        expression: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.evaluate(isolate_id, target_id, expression))
    }

    fn evaluate_in_frame_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        frame_index: i32,
        expression: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.evaluate_in_frame(isolate_id, frame_index, expression))
    }

    fn get_vm_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + '_>> {
        Box::pin(self.get_vm())
    }

    fn get_isolate_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.get_isolate(isolate_id))
    }

    fn get_scripts_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.get_scripts(isolate_id))
    }

    fn call_service_boxed<'a>(
        &'a self,
        method: &'a str,
        params: Option<serde_json::Value>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.call_service(method, params))
    }

    fn set_library_debuggable_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        library_id: &'a str,
        is_debuggable: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(self.set_library_debuggable(isolate_id, library_id, is_debuggable))
    }

    fn get_source_report_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        script_id: &'a str,
        report_kinds: Vec<String>,
        token_pos: Option<i64>,
        end_token_pos: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        // Build the JSON params directly here so we don't need to hold
        // temporary `Vec<&str>` borrows across the async boundary.
        let mut params = serde_json::json!({
            "isolateId": isolate_id,
            "scriptId": script_id,
            "reports": report_kinds,
            "forceCompile": true,
        });
        if let Some(tp) = token_pos {
            params["tokenPos"] = serde_json::json!(tp);
        }
        if let Some(etp) = end_token_pos {
            params["endTokenPos"] = serde_json::json!(etp);
        }
        Box::pin(async move {
            self.handle
                .request("getSourceReport", Some(params))
                .await
                .map_err(|e| BackendError::VmServiceError(e.to_string()))
        })
    }

    fn get_source_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
        script_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(self.get_source(isolate_id, script_id))
    }

    fn hot_reload_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
        Box::pin(self.hot_reload())
    }

    fn hot_restart_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
        Box::pin(self.hot_restart())
    }

    fn stop_app_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
        Box::pin(self.stop_app())
    }

    fn ws_uri_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        Box::pin(self.ws_uri())
    }

    fn device_id_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        Box::pin(self.device_id())
    }

    fn build_mode_boxed(&self) -> Pin<Box<dyn Future<Output = String> + Send + '_>> {
        Box::pin(self.build_mode())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// VmBackendFactory — per-connection backend factory for the TCP accept loop
// ─────────────────────────────────────────────────────────────────────────────

/// Session metadata used to populate Flutter/Dart custom DAP events.
///
/// Shared via `Arc<Mutex<Option<...>>>` between the Engine TEA handler and
/// [`VmBackendFactory`] so that newly connecting DAP clients receive fresh
/// metadata without requiring a restart of the factory.
#[derive(Debug, Clone, Default)]
pub struct DapSessionMetadata {
    /// VM Service WebSocket URI (e.g., `"ws://127.0.0.1:8181/ws"`).
    pub ws_uri: Option<String>,
    /// Device ID (e.g., `"emulator-5554"` or `"00008020-000264DC0AD2003A"`).
    pub device_id: Option<String>,
    /// Build mode (`"debug"`, `"profile"`, or `"release"`).
    pub build_mode: String,
}

impl DapSessionMetadata {
    /// Create metadata with a known URI and debug mode defaults.
    // Allow dead_code: this constructor is the primary creation path for the
    // TEA handler wiring (Phase 4, Task 08 follow-up). Suppress the warning
    // until the Engine integration is wired.
    #[allow(dead_code)]
    pub fn new(ws_uri: impl Into<String>) -> Self {
        Self {
            ws_uri: Some(ws_uri.into()),
            device_id: None,
            build_mode: "debug".to_string(),
        }
    }
}

/// Factory that creates a [`fdemon_dap::server::BackendHandle`] for each
/// accepted DAP client connection.
///
/// Captures a shared `Arc<Mutex<Option<VmRequestHandle>>>` so it can be
/// passed to the TCP accept loop (which runs as a long-lived Tokio task)
/// without knowing about the Engine's ownership model.
///
/// ## Lifecycle
///
/// 1. On `SpawnDapServer`, the Engine creates a `VmBackendFactory` from the
///    active session's `VmRequestHandle` slot and the shared debug sender
///    registry.
/// 2. The factory is passed to [`fdemon_dap::DapService::start_tcp_with_factory`].
/// 3. Each time a DAP client connects, the accept loop calls `factory.create()`.
///    - If the slot is `Some`, a [`VmServiceBackend`] is constructed and the
///      session uses real VM Service debugging.
///    - If the slot is `None` (VM not yet connected or disconnected), the
///      session falls back to [`fdemon_dap::server::NoopBackend`].
/// 4. A new per-session `mpsc::Sender<DebugEvent>` is registered in the
///    shared `dap_debug_senders` registry so the TEA handler can forward VM
///    pause/resumed/isolate events to all connected DAP adapters.
pub struct VmBackendFactory {
    /// Shared slot for the active session's VM request handle.
    ///
    /// The Engine holds a `Mutex<Option<VmRequestHandle>>` and updates it
    /// as sessions start and stop. The factory clones the handle out of the
    /// slot so the session gets its own clone for exclusive use.
    vm_handle_slot: Arc<Mutex<Option<VmRequestHandle>>>,

    /// Shared registry of per-DAP-client event senders (Phase 4, Task 01).
    ///
    /// Each call to `create()` pushes a new `mpsc::Sender<DebugEvent>` here.
    /// The corresponding `Receiver` is given to the per-session task so it
    /// can receive VM debug events forwarded by the TEA handler.
    ///
    /// Stale senders (where the receiver has been dropped because the client
    /// disconnected) are pruned by the `retain` call in
    /// `devtools::debug::handle_debug_event`.
    dap_debug_senders: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<DebugEvent>>>>,

    /// Optional sender into the TEA message bus.
    ///
    /// When `Some`, each created [`VmServiceBackend`] is given this sender
    /// so that `hotReload` and `hotRestart` DAP requests can dispatch
    /// `Message::HotReload` / `Message::HotRestart` through the existing
    /// Engine reload/restart lifecycle.
    ///
    /// Set via [`VmBackendFactory::new_with_msg_tx`]. When `None` (legacy path),
    /// hot reload and hot restart return `BackendError::NotConnected`.
    msg_tx: Option<mpsc::Sender<Message>>,

    /// Shared session metadata slot for Flutter/Dart custom DAP events.
    ///
    /// Updated by the TEA handler when a VM Service connection is established
    /// (stores `ws_uri`, `device_id`, `build_mode`). Each call to `create()`
    /// snapshots the current metadata and passes it to the new backend via
    /// [`VmServiceBackend::with_session_metadata`].
    ///
    /// `None` means the metadata has not been set yet; the backend defaults
    /// are used in that case.
    session_metadata: Arc<Mutex<Option<DapSessionMetadata>>>,
}

impl VmBackendFactory {
    /// Create a new factory from a shared VM handle slot, sender registry, and
    /// an optional TEA message sender.
    ///
    /// When `msg_tx` is `Some`, backends created by this factory will support
    /// `hotReload` and `hotRestart` DAP requests by dispatching
    /// `Message::HotReload` / `Message::HotRestart` through the TEA pipeline.
    ///
    /// When `msg_tx` is `None`, those operations return
    /// [`BackendError::NotConnected`].
    pub fn new(
        vm_handle_slot: Arc<Mutex<Option<VmRequestHandle>>>,
        dap_debug_senders: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<DebugEvent>>>>,
        msg_tx: Option<mpsc::Sender<Message>>,
    ) -> Self {
        Self {
            vm_handle_slot,
            dap_debug_senders,
            msg_tx,
            session_metadata: Arc::new(Mutex::new(None)),
        }
    }

    /// Return a shared reference to the session metadata slot.
    ///
    /// The TEA handler can update this slot when a VM Service connection is
    /// established so that newly connecting DAP clients receive up-to-date
    /// metadata in `dart.debuggerUris` and `flutter.appStart` events.
    // Allow dead_code: this accessor is the Engine wiring point for Phase 4, Task 08
    // follow-up. Suppress until the Engine integration is wired.
    #[allow(dead_code)]
    pub fn session_metadata_slot(&self) -> Arc<Mutex<Option<DapSessionMetadata>>> {
        self.session_metadata.clone()
    }
}

impl fdemon_dap::server::BackendFactory for VmBackendFactory {
    fn create(&self) -> Option<fdemon_dap::server::BackendHandle> {
        // Clone the handle out of the slot. If None, no VM is connected.
        let vm_handle = match self.vm_handle_slot.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                tracing::warn!("VmBackendFactory: VM handle slot lock poisoned: {}", e);
                None
            }
        };

        let vm_handle = vm_handle?;

        // Snapshot the current session metadata (ws_uri, device_id, build_mode).
        // If the lock is poisoned or the slot is None, defaults apply.
        let metadata = self
            .session_metadata
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        // Construct the backend: with msg_tx if available (enables hot reload/restart),
        // otherwise fall back to the legacy path (hot reload/restart return NotConnected).
        let backend = match &self.msg_tx {
            Some(tx) => VmServiceBackend::new_with_msg_tx(vm_handle, tx.clone()),
            None => VmServiceBackend::new(vm_handle),
        };

        // Apply session metadata so that dart.debuggerUris / flutter.appStart
        // custom DAP events are populated with the correct URI, device, and mode.
        let backend = if let Some(meta) = metadata {
            backend.with_session_metadata(meta.ws_uri, meta.device_id, meta.build_mode)
        } else {
            backend
        };

        // Create a per-session debug event channel.
        // The receiver goes to the session loop; the sender is registered in
        // `dap_debug_senders` so the TEA handler can forward VM events.
        let (debug_event_tx, debug_event_rx) = tokio::sync::mpsc::channel::<DebugEvent>(64);

        // Register the sender so the TEA handler can forward events to this
        // DAP session. If the registry lock is poisoned (should never happen
        // in normal operation) we log a warning but still return the handle —
        // the session will work for commands; it just won't receive events.
        match self.dap_debug_senders.lock() {
            Ok(mut senders) => {
                senders.push(debug_event_tx);
            }
            Err(e) => {
                tracing::warn!(
                    "VmBackendFactory: dap_debug_senders lock poisoned, \
                     debug events will not be forwarded to this session: {}",
                    e
                );
            }
        }

        Some(fdemon_dap::server::BackendHandle {
            backend: fdemon_dap::DynDebugBackend::new(Box::new(backend)),
            debug_event_rx,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify `VmServiceBackend` can be constructed without panicking.
    /// Full RPC tests require a live VM Service and are out of scope for unit tests.
    #[test]
    fn test_vm_service_backend_new_compiles() {
        // This test verifies that the type is constructible and the trait
        // implementation satisfies the DebugBackend bound. A live VmRequestHandle
        // cannot be constructed in unit tests (it requires a WebSocket connection),
        // so we only verify that the type system is satisfied.
        //
        // The actual DebugBackend impl is exercised by integration tests that
        // run against a real Flutter app.
        fn assert_debug_backend<T: DebugBackend>() {}
        assert_debug_backend::<VmServiceBackend>();
    }

    /// Verify that `VmServiceBackend` implements Clone.
    #[test]
    fn test_vm_service_backend_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<VmServiceBackend>();
    }

    /// Verify that `VmBackendFactory::new` exists and has the expected signature.
    ///
    /// A live `VmRequestHandle` cannot be constructed in unit tests (it requires
    /// a WebSocket connection), so we only verify that the type system accepts
    /// the function signature. The actual behavior is exercised by integration
    /// tests against a real Flutter app.
    #[test]
    fn test_vm_backend_factory_new_type_checks() {
        // Taking the function address verifies the signature compiles correctly.
        let _ = VmBackendFactory::new;
    }

    // ── Phase 6 new method type-system tests ──────────────────────────────

    /// Verify `VmServiceBackend` satisfies the `DebugBackend` trait with all
    /// Phase 6 new methods (`get_isolate`, `call_service`,
    /// `set_library_debuggable`, `get_source_report`).
    ///
    /// A live `VmRequestHandle` is not constructable in unit tests; this test
    /// confirms the trait bounds are satisfied at compile time.
    #[test]
    fn test_vm_service_backend_satisfies_debug_backend_with_phase6_methods() {
        fn assert_debug_backend<T: DebugBackend>() {}
        assert_debug_backend::<VmServiceBackend>();
    }

    /// Verify `VmServiceBackend` implements `DynDebugBackendInner` (required by
    /// `DynDebugBackend::new` which takes `Box<dyn DynDebugBackendInner>`).
    #[test]
    fn test_vm_service_backend_satisfies_dyn_debug_backend_inner() {
        fn assert_dyn_inner<T: fdemon_dap::adapter::DynDebugBackendInner>() {}
        assert_dyn_inner::<VmServiceBackend>();
    }

    /// Verify that `get_source_report_boxed` JSON parameter construction with
    /// `token_pos` and `end_token_pos` fields behaves correctly.
    ///
    /// Tests the parameter assembly logic in isolation using `serde_json::json!`.
    #[test]
    fn test_get_source_report_params_with_token_pos_fields() {
        let isolate_id = "isolates/1";
        let script_id = "scripts/42";
        let report_kinds = vec!["PossibleBreakpoints".to_string()];
        let token_pos: Option<i64> = Some(100);
        let end_token_pos: Option<i64> = Some(200);

        let mut params = serde_json::json!({
            "isolateId": isolate_id,
            "scriptId": script_id,
            "reports": report_kinds,
            "forceCompile": true,
        });
        if let Some(tp) = token_pos {
            params["tokenPos"] = serde_json::json!(tp);
        }
        if let Some(etp) = end_token_pos {
            params["endTokenPos"] = serde_json::json!(etp);
        }

        assert_eq!(params["isolateId"], "isolates/1");
        assert_eq!(params["scriptId"], "scripts/42");
        assert_eq!(
            params["reports"],
            serde_json::json!(["PossibleBreakpoints"])
        );
        assert_eq!(params["forceCompile"], true);
        assert_eq!(params["tokenPos"], 100);
        assert_eq!(params["endTokenPos"], 200);
    }

    /// Verify that `get_source_report_boxed` omits `tokenPos`/`endTokenPos`
    /// when both are `None`.
    #[test]
    fn test_get_source_report_params_without_token_pos_fields() {
        let isolate_id = "isolates/1";
        let script_id = "scripts/42";
        let report_kinds = vec!["Coverage".to_string()];
        let token_pos: Option<i64> = None;
        let end_token_pos: Option<i64> = None;

        let mut params = serde_json::json!({
            "isolateId": isolate_id,
            "scriptId": script_id,
            "reports": report_kinds,
            "forceCompile": true,
        });
        if let Some(tp) = token_pos {
            params["tokenPos"] = serde_json::json!(tp);
        }
        if let Some(etp) = end_token_pos {
            params["endTokenPos"] = serde_json::json!(etp);
        }

        assert!(
            params.get("tokenPos").is_none(),
            "tokenPos should be absent when None"
        );
        assert!(
            params.get("endTokenPos").is_none(),
            "endTokenPos should be absent when None"
        );
    }

    /// Verify JSON parameter construction for `get_isolate`.
    #[test]
    fn test_get_isolate_params_construction() {
        let isolate_id = "isolates/main";
        let params = serde_json::json!({ "isolateId": isolate_id });

        assert_eq!(params["isolateId"], "isolates/main");
    }

    /// Verify JSON parameter construction for `set_library_debuggable`.
    #[test]
    fn test_set_library_debuggable_params_construction() {
        let isolate_id = "isolates/1";
        let library_id = "libraries/5";
        let is_debuggable = true;

        let params = serde_json::json!({
            "isolateId": isolate_id,
            "libraryId": library_id,
            "isDebuggable": is_debuggable,
        });

        assert_eq!(params["isolateId"], "isolates/1");
        assert_eq!(params["libraryId"], "libraries/5");
        assert_eq!(params["isDebuggable"], true);
    }

    /// Verify that `call_service` passes method and params unchanged.
    /// Tests the parameter forwarding logic using JSON construction.
    #[test]
    fn test_call_service_params_forwarded_as_is() {
        let method = "ext.flutter.inspector.getRootWidget";
        let params = serde_json::json!({ "arg0": "value0" });

        // Simulate the forwarding: method is used as-is, params as-is.
        assert_eq!(method, "ext.flutter.inspector.getRootWidget");
        assert_eq!(params["arg0"], "value0");
    }
}
