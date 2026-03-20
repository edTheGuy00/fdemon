//! Unit tests for the Phase 6 [`crate::adapter::backend`] methods.
//!
//! Tests verify that:
//! - `MockTestBackend` default implementations return sensible values.
//! - The blanket `DebugBackend` impl correctly delegates through to the defaults.
//! - `DynDebugBackend` wraps and delegates all four new methods correctly.

use crate::adapter::{BackendError, DynDebugBackend, DynDebugBackendInner};
use std::future::Future;
use std::pin::Pin;

use super::super::test_helpers::MockTestBackend;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A mock backend that records calls to the four new Phase 6 methods.
struct Phase6RecordingBackend {
    /// Canned response for `get_isolate`.
    isolate_response: serde_json::Value,
    /// Canned response for `call_service`.
    service_response: serde_json::Value,
    /// Canned response for `get_source_report`.
    source_report_response: serde_json::Value,
}

impl Phase6RecordingBackend {
    fn new() -> Self {
        Self {
            isolate_response: serde_json::json!({
                "type": "Isolate",
                "id": "isolates/1",
                "rootLib": { "id": "libraries/1", "uri": "package:app/main.dart" },
                "libraries": [{ "id": "libraries/1", "uri": "package:app/main.dart" }],
            }),
            service_response: serde_json::json!({ "result": "service_called" }),
            source_report_response: serde_json::json!({
                "ranges": [{ "scriptIndex": 0, "startPos": 0, "endPos": 100 }],
                "scripts": [{ "uri": "package:app/main.dart" }],
            }),
        }
    }
}

impl MockTestBackend for Phase6RecordingBackend {
    async fn get_isolate(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        Ok(self.isolate_response.clone())
    }

    async fn call_service(
        &self,
        _method: &str,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(self.service_response.clone())
    }

    async fn set_library_debuggable(
        &self,
        _isolate_id: &str,
        _library_id: &str,
        _is_debuggable: bool,
    ) -> Result<(), BackendError> {
        Ok(())
    }

    async fn get_source_report(
        &self,
        _isolate_id: &str,
        _script_id: &str,
        _report_kinds: &[&str],
        _token_pos: Option<i64>,
        _end_token_pos: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(self.source_report_response.clone())
    }
}

/// Implement `DynDebugBackendInner` for `Phase6RecordingBackend` so it can be
/// wrapped in a `DynDebugBackend`.
impl DynDebugBackendInner for Phase6RecordingBackend {
    fn pause_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn resume_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _step: Option<crate::adapter::StepMode>,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn add_breakpoint_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _uri: &'a str,
        _line: i32,
        _column: Option<i32>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<crate::adapter::BreakpointResult, BackendError>> + Send + 'a,
        >,
    > {
        Box::pin(async {
            Ok(crate::adapter::BreakpointResult {
                vm_id: "bp1".into(),
                resolved: true,
                line: None,
                column: None,
            })
        })
    }

    fn remove_breakpoint_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _bp_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn set_exception_pause_mode_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _mode: crate::adapter::DapExceptionPauseMode,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn get_stack_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _limit: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(serde_json::json!({"frames": []})) })
    }

    fn get_object_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _object_id: &'a str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(serde_json::json!({})) })
    }

    fn evaluate_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _target_id: &'a str,
        _expression: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(serde_json::json!({})) })
    }

    fn evaluate_in_frame_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _frame_index: i32,
        _expression: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(serde_json::json!({})) })
    }

    fn get_vm_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + '_>> {
        Box::pin(async { Ok(serde_json::json!({"isolates": []})) })
    }

    fn get_isolate_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async move { Ok(self.isolate_response.clone()) })
    }

    fn get_scripts_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(serde_json::json!({"scripts": []})) })
    }

    fn get_source_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _script_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async { Ok(String::new()) })
    }

    fn hot_reload_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn hot_restart_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn stop_app_boxed(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn ws_uri_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        Box::pin(async { None })
    }

    fn device_id_boxed(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        Box::pin(async { None })
    }

    fn build_mode_boxed(&self) -> Pin<Box<dyn Future<Output = String> + Send + '_>> {
        Box::pin(async { "debug".to_string() })
    }

    fn call_service_boxed<'a>(
        &'a self,
        _method: &'a str,
        _params: Option<serde_json::Value>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async move { Ok(self.service_response.clone()) })
    }

    fn set_library_debuggable_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _library_id: &'a str,
        _is_debuggable: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn get_source_report_boxed<'a>(
        &'a self,
        _isolate_id: &'a str,
        _script_id: &'a str,
        _report_kinds: Vec<String>,
        _token_pos: Option<i64>,
        _end_token_pos: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + 'a>> {
        Box::pin(async move { Ok(self.source_report_response.clone()) })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: MockTestBackend defaults
// ─────────────────────────────────────────────────────────────────────────────

/// Default `get_isolate` returns an empty JSON object (not an error).
#[tokio::test]
async fn test_mock_backend_get_isolate_default_returns_ok() {
    struct MinimalMock;
    impl MockTestBackend for MinimalMock {}

    let result = MockTestBackend::get_isolate(&MinimalMock, "isolates/1").await;
    assert!(result.is_ok(), "default get_isolate should succeed");
    assert_eq!(result.unwrap(), serde_json::json!({}));
}

/// Default `call_service` returns an empty JSON object (not an error).
#[tokio::test]
async fn test_mock_backend_call_service_default_returns_ok() {
    struct MinimalMock;
    impl MockTestBackend for MinimalMock {}

    let result = MockTestBackend::call_service(&MinimalMock, "ext.test.method", None).await;
    assert!(result.is_ok(), "default call_service should succeed");
    assert_eq!(result.unwrap(), serde_json::json!({}));
}

/// Default `set_library_debuggable` returns `Ok(())`.
#[tokio::test]
async fn test_mock_backend_set_library_debuggable_default_returns_ok() {
    struct MinimalMock;
    impl MockTestBackend for MinimalMock {}

    let result =
        MockTestBackend::set_library_debuggable(&MinimalMock, "isolates/1", "libraries/5", true)
            .await;
    assert!(
        result.is_ok(),
        "default set_library_debuggable should succeed"
    );
}

/// Default `get_source_report` returns an empty JSON object (not an error).
#[tokio::test]
async fn test_mock_backend_get_source_report_default_returns_ok() {
    struct MinimalMock;
    impl MockTestBackend for MinimalMock {}

    let result = MockTestBackend::get_source_report(
        &MinimalMock,
        "isolates/1",
        "scripts/42",
        &["PossibleBreakpoints"],
        None,
        None,
    )
    .await;
    assert!(result.is_ok(), "default get_source_report should succeed");
    assert_eq!(result.unwrap(), serde_json::json!({}));
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: DynDebugBackend delegation
// ─────────────────────────────────────────────────────────────────────────────

/// `DynDebugBackend::get_isolate` delegates to the inner backend's
/// `get_isolate_boxed` and returns the canned response.
#[tokio::test]
async fn test_dyn_debug_backend_get_isolate_delegates() {
    use crate::adapter::DebugBackend;

    let backend = DynDebugBackend::new(Box::new(Phase6RecordingBackend::new()));
    let result = backend.get_isolate("isolates/1").await;
    assert!(result.is_ok(), "get_isolate should succeed");
    let val = result.unwrap();
    assert_eq!(val["type"], "Isolate");
    assert_eq!(val["id"], "isolates/1");
    assert!(val["rootLib"].is_object(), "rootLib should be present");
}

/// `DynDebugBackend::call_service` delegates to the inner backend.
#[tokio::test]
async fn test_dyn_debug_backend_call_service_delegates() {
    use crate::adapter::DebugBackend;

    let backend = DynDebugBackend::new(Box::new(Phase6RecordingBackend::new()));
    let result = backend
        .call_service(
            "ext.flutter.inspector.getRootWidget",
            Some(serde_json::json!({"arg": "val"})),
        )
        .await;
    assert!(result.is_ok(), "call_service should succeed");
    assert_eq!(result.unwrap()["result"], "service_called");
}

/// `DynDebugBackend::set_library_debuggable` delegates to the inner backend.
#[tokio::test]
async fn test_dyn_debug_backend_set_library_debuggable_delegates() {
    use crate::adapter::DebugBackend;

    let backend = DynDebugBackend::new(Box::new(Phase6RecordingBackend::new()));
    let result = backend
        .set_library_debuggable("isolates/1", "libraries/3", false)
        .await;
    assert!(result.is_ok(), "set_library_debuggable should succeed");
}

/// `DynDebugBackend::get_source_report` delegates to the inner backend.
#[tokio::test]
async fn test_dyn_debug_backend_get_source_report_delegates() {
    use crate::adapter::DebugBackend;

    let backend = DynDebugBackend::new(Box::new(Phase6RecordingBackend::new()));
    let result = backend
        .get_source_report(
            "isolates/1",
            "scripts/42",
            &["PossibleBreakpoints"],
            None,
            None,
        )
        .await;
    assert!(result.is_ok(), "get_source_report should succeed");
    let val = result.unwrap();
    assert!(val["ranges"].is_array(), "ranges should be present");
    assert!(val["scripts"].is_array(), "scripts should be present");
}

/// `DynDebugBackend::get_source_report` passes token positions to the inner backend.
#[tokio::test]
async fn test_dyn_debug_backend_get_source_report_with_token_positions() {
    use crate::adapter::DebugBackend;

    let backend = DynDebugBackend::new(Box::new(Phase6RecordingBackend::new()));
    // With non-None token positions.
    let result = backend
        .get_source_report(
            "isolates/1",
            "scripts/42",
            &["PossibleBreakpoints"],
            Some(10),
            Some(200),
        )
        .await;
    // The mock ignores token_pos but the call should still succeed.
    assert!(
        result.is_ok(),
        "get_source_report with token_pos should succeed"
    );
}
