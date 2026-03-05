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

use fdemon_daemon::vm_service::{
    debugger,
    debugger_types::{ExceptionPauseMode, StepOption},
    VmRequestHandle,
};
use fdemon_dap::adapter::{BreakpointResult, DebugBackend, StepMode};

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
    async fn pause(&self, isolate_id: &str) -> Result<(), String> {
        debugger::pause(&self.handle, isolate_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), String> {
        let vm_step = step.map(|s| match s {
            StepMode::Over => StepOption::Over,
            StepMode::Into => StepOption::Into,
            StepMode::Out => StepOption::Out,
        });
        debugger::resume(&self.handle, isolate_id, vm_step)
            .await
            .map_err(|e| e.to_string())
    }

    async fn add_breakpoint(
        &self,
        isolate_id: &str,
        uri: &str,
        line: i32,
        column: Option<i32>,
    ) -> Result<BreakpointResult, String> {
        let bp =
            debugger::add_breakpoint_with_script_uri(&self.handle, isolate_id, uri, line, column)
                .await
                .map_err(|e| e.to_string())?;

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

    async fn remove_breakpoint(&self, isolate_id: &str, breakpoint_id: &str) -> Result<(), String> {
        debugger::remove_breakpoint(&self.handle, isolate_id, breakpoint_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn set_exception_pause_mode(&self, isolate_id: &str, mode: &str) -> Result<(), String> {
        let vm_mode = match mode {
            "All" => ExceptionPauseMode::All,
            "Unhandled" => ExceptionPauseMode::Unhandled,
            _ => ExceptionPauseMode::None,
        };
        debugger::set_isolate_pause_mode(&self.handle, isolate_id, vm_mode)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_stack(
        &self,
        isolate_id: &str,
        limit: Option<i32>,
    ) -> Result<serde_json::Value, String> {
        let stack = debugger::get_stack(&self.handle, isolate_id, limit)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&stack).map_err(|e| e.to_string())
    }

    async fn get_object(
        &self,
        isolate_id: &str,
        object_id: &str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Result<serde_json::Value, String> {
        debugger::get_object(&self.handle, isolate_id, object_id, offset, count)
            .await
            .map_err(|e| e.to_string())
    }

    async fn evaluate(
        &self,
        isolate_id: &str,
        target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, String> {
        let result = debugger::evaluate(&self.handle, isolate_id, target_id, expression)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&result).map_err(|e| e.to_string())
    }

    async fn evaluate_in_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
        expression: &str,
    ) -> Result<serde_json::Value, String> {
        let result = debugger::evaluate_in_frame(&self.handle, isolate_id, frame_index, expression)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&result).map_err(|e| e.to_string())
    }

    async fn get_vm(&self) -> Result<serde_json::Value, String> {
        self.handle
            .request("getVM", None)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, String> {
        let scripts = debugger::get_scripts(&self.handle, isolate_id)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&scripts).map_err(|e| e.to_string())
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
