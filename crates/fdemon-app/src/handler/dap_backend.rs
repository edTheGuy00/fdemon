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

use fdemon_daemon::vm_service::{
    debugger,
    debugger_types::{ExceptionPauseMode, StepOption},
    VmRequestHandle,
};
use fdemon_dap::adapter::{
    BackendError, BreakpointResult, DapExceptionPauseMode, DebugBackend, DynDebugBackendInner,
    StepMode,
};

// ─────────────────────────────────────────────────────────────────────────────
// VmServiceBackend
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete [`DebugBackend`] that delegates all debug operations to the Dart
/// VM Service via a [`VmRequestHandle`].
///
/// Constructed by the Engine when a DAP client attaches to an active Flutter
/// session. The handle is cloned so it can be shared safely across the session
/// task and any concurrent requests.
#[derive(Clone)]
pub struct VmServiceBackend {
    handle: VmRequestHandle,
}

impl VmServiceBackend {
    /// Create a new backend wrapping the given VM Service request handle.
    pub fn new(handle: VmRequestHandle) -> Self {
        Self { handle }
    }
}

impl DebugBackend for VmServiceBackend {
    async fn pause(&self, isolate_id: &str) -> Result<(), BackendError> {
        debugger::pause(&self.handle, isolate_id)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))
    }

    async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), BackendError> {
        let vm_step = step.map(|s| match s {
            StepMode::Over => StepOption::Over,
            StepMode::Into => StepOption::Into,
            StepMode::Out => StepOption::Out,
        });
        debugger::resume(&self.handle, isolate_id, vm_step)
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

    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        let scripts = debugger::get_scripts(&self.handle, isolate_id)
            .await
            .map_err(|e| BackendError::VmServiceError(e.to_string()))?;
        serde_json::to_value(&scripts).map_err(|e| BackendError::VmServiceError(e.to_string()))
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
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(self.resume(isolate_id, step))
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

    fn get_scripts_boxed<'a>(
        &'a self,
        isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(self.get_scripts(isolate_id))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// VmBackendFactory — per-connection backend factory for the TCP accept loop
// ─────────────────────────────────────────────────────────────────────────────

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
///    active session's `VmRequestHandle` slot.
/// 2. The factory is passed to [`fdemon_dap::DapService::start_tcp_with_factory`].
/// 3. Each time a DAP client connects, the accept loop calls `factory.create()`.
///    - If the slot is `Some`, a [`VmServiceBackend`] is constructed and the
///      session uses real VM Service debugging.
///    - If the slot is `None` (VM not yet connected or disconnected), the
///      session falls back to [`fdemon_dap::server::NoopBackend`].
/// 4. The per-session `mpsc::Sender<DebugEvent>` is registered somewhere so
///    the Engine can forward VM pause/stopped events. (Task 06 wires this up.)
pub struct VmBackendFactory {
    /// Shared slot for the active session's VM request handle.
    ///
    /// The Engine holds a `Mutex<Option<VmRequestHandle>>` and updates it
    /// as sessions start and stop. The factory clones the handle out of the
    /// slot so the session gets its own clone for exclusive use.
    vm_handle_slot: std::sync::Arc<std::sync::Mutex<Option<VmRequestHandle>>>,
}

impl VmBackendFactory {
    /// Create a new factory from a shared VM handle slot.
    pub fn new(vm_handle_slot: std::sync::Arc<std::sync::Mutex<Option<VmRequestHandle>>>) -> Self {
        Self { vm_handle_slot }
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

        let backend = VmServiceBackend::new(vm_handle);

        // Create a per-session debug event channel. The receiver goes to the
        // session loop; the sender is used by the Engine to forward VM events.
        // Task 06 will wire the sender back to the Engine; for now it is
        // dropped (sessions still work, but won't receive stopped/resumed events
        // until the full event routing is in place).
        let (_, debug_event_rx) = tokio::sync::mpsc::channel::<fdemon_dap::adapter::DebugEvent>(64);

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
}
