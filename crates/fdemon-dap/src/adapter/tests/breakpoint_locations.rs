//! Unit tests for the `breakpointLocations` DAP request handler.
//!
//! Covers:
//! - Basic breakpoint location lookup with token position mapping
//! - Line range filtering
//! - Empty result for files/lines with no possible breakpoints
//! - Script-not-found case
//! - Missing isolate case
//! - `supportsBreakpointLocationsRequest` capability advertisement

use super::super::test_helpers::MockTestBackend;
use super::*;
use crate::adapter::BackendError;
use crate::DapRequest;

// ─────────────────────────────────────────────────────────────────────────────
// Helper: build a breakpointLocations request
// ─────────────────────────────────────────────────────────────────────────────

fn make_breakpoint_locations_request(
    seq: i64,
    path: &str,
    line: i64,
    end_line: Option<i64>,
) -> DapRequest {
    use crate::protocol::types::DapSource;
    let mut args = serde_json::json!({
        "source": DapSource {
            path: Some(path.to_string()),
            ..Default::default()
        },
        "line": line,
    });
    if let Some(el) = end_line {
        args["endLine"] = serde_json::json!(el);
    }
    DapRequest {
        seq,
        command: "breakpointLocations".into(),
        arguments: Some(args),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends
// ─────────────────────────────────────────────────────────────────────────────

/// A mock backend that returns a script list with one script and a source
/// report with known token positions mapped through a `tokenPosTable`.
struct BreakpointLocationsMock;

impl MockTestBackend for BreakpointLocationsMock {
    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "isolates": [{ "id": "isolates/1", "name": "main" }] }))
    }

    async fn get_scripts(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "scripts": [
                {
                    "id": "scripts/1",
                    "uri": "file:///app/lib/main.dart"
                }
            ]
        }))
    }

    async fn get_source_report(
        &self,
        _isolate_id: &str,
        _script_id: &str,
        _report_kinds: &[&str],
        _token_pos: Option<i64>,
        _end_token_pos: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        // tokenPosTable: line 10 → token 100 @ col 1, token 120 @ col 20
        //                line 11 → token 200 @ col 3
        //                line 20 → token 300 @ col 5
        Ok(serde_json::json!({
            "ranges": [
                {
                    "scriptIndex": 0,
                    "startPos": 100,
                    "endPos": 400,
                    "possibleBreakpoints": [100, 120, 200, 300]
                }
            ],
            "scripts": [
                {
                    "id": "scripts/1",
                    "uri": "file:///app/lib/main.dart",
                    "tokenPosTable": [
                        [10, 100, 1, 120, 20],
                        [11, 200, 3],
                        [20, 300, 5]
                    ]
                }
            ]
        }))
    }
}

/// A mock backend that returns a source report with no possible breakpoints
/// in any range — simulates a comment-only or generated section.
struct EmptyBreakpointsMock;

impl MockTestBackend for EmptyBreakpointsMock {
    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "isolates": [{ "id": "isolates/1", "name": "main" }] }))
    }

    async fn get_scripts(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "scripts": [
                { "id": "scripts/1", "uri": "file:///app/lib/main.dart" }
            ]
        }))
    }

    async fn get_source_report(
        &self,
        _isolate_id: &str,
        _script_id: &str,
        _report_kinds: &[&str],
        _token_pos: Option<i64>,
        _end_token_pos: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "ranges": [
                {
                    "scriptIndex": 0,
                    "startPos": 0,
                    "endPos": 50,
                    "possibleBreakpoints": []
                }
            ],
            "scripts": [
                {
                    "id": "scripts/1",
                    "uri": "file:///app/lib/main.dart",
                    "tokenPosTable": []
                }
            ]
        }))
    }
}

/// A mock backend that returns a script list where the requested file is
/// absent — simulates a file that is not loaded in the Dart VM.
struct ScriptNotFoundMock;

impl MockTestBackend for ScriptNotFoundMock {
    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "isolates": [{ "id": "isolates/1", "name": "main" }] }))
    }

    async fn get_scripts(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        // Return a different file — the requested file is not present.
        Ok(serde_json::json!({
            "scripts": [
                { "id": "scripts/99", "uri": "file:///app/lib/other.dart" }
            ]
        }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// `breakpointLocations` returns correct positions with column information
/// when the script has a `tokenPosTable`.
#[tokio::test]
async fn test_breakpoint_locations_returns_positions_with_columns() {
    let (mut adapter, mut rx) = DapAdapter::new(BreakpointLocationsMock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_breakpoint_locations_request(1, "/app/lib/main.dart", 10, Some(11));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "expected success, got: {:?}", resp.message);
    let body = resp.body.expect("body must be present");
    let breakpoints = body["breakpoints"].as_array().expect("breakpoints array");

    // Lines 10 and 11 should produce 3 positions (tokens 100, 120, 200).
    assert_eq!(breakpoints.len(), 3, "expected 3 breakpoint locations");

    // First two are on line 10: col 1 and col 20.
    assert_eq!(breakpoints[0]["line"], 10);
    assert_eq!(breakpoints[0]["column"], 1);
    assert_eq!(breakpoints[1]["line"], 10);
    assert_eq!(breakpoints[1]["column"], 20);

    // Third is on line 11: col 3.
    assert_eq!(breakpoints[2]["line"], 11);
    assert_eq!(breakpoints[2]["column"], 3);
}

/// `breakpointLocations` filters to the requested line range and excludes
/// positions outside it.
#[tokio::test]
async fn test_breakpoint_locations_filters_to_line_range() {
    let (mut adapter, mut rx) = DapAdapter::new(BreakpointLocationsMock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Request only line 20 — should return only token 300.
    let req = make_breakpoint_locations_request(1, "/app/lib/main.dart", 20, None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "expected success, got: {:?}", resp.message);
    let body = resp.body.expect("body must be present");
    let breakpoints = body["breakpoints"].as_array().expect("breakpoints array");

    assert_eq!(
        breakpoints.len(),
        1,
        "expected exactly 1 location on line 20"
    );
    assert_eq!(breakpoints[0]["line"], 20);
    assert_eq!(breakpoints[0]["column"], 5);
}

/// `breakpointLocations` returns an empty array when the source report has
/// no possible breakpoints in any range (e.g., comment-only section).
#[tokio::test]
async fn test_breakpoint_locations_empty_for_comment_line() {
    let (mut adapter, mut rx) = DapAdapter::new(EmptyBreakpointsMock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_breakpoint_locations_request(1, "/app/lib/main.dart", 1, Some(5));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "expected success, got: {:?}", resp.message);
    let body = resp.body.expect("body must be present");
    let breakpoints = body["breakpoints"].as_array().expect("breakpoints array");
    assert!(
        breakpoints.is_empty(),
        "expected empty breakpoints for comment-only range"
    );
}

/// `breakpointLocations` returns an empty array (not an error) when the
/// requested source file is not among the loaded scripts.
#[tokio::test]
async fn test_breakpoint_locations_script_not_found_returns_empty() {
    let (mut adapter, mut rx) = DapAdapter::new(ScriptNotFoundMock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_breakpoint_locations_request(1, "/app/lib/main.dart", 10, None);
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "script not found should return success with empty list, got: {:?}",
        resp.message
    );
    let body = resp.body.expect("body must be present");
    let breakpoints = body["breakpoints"].as_array().expect("breakpoints array");
    assert!(
        breakpoints.is_empty(),
        "expected empty breakpoints when script is not loaded"
    );
}

/// `breakpointLocations` returns an error when no isolate is attached.
#[tokio::test]
async fn test_breakpoint_locations_no_isolate_returns_error() {
    // Do not register any isolate.
    let (mut adapter, _rx) = DapAdapter::new(BreakpointLocationsMock);

    let req = make_breakpoint_locations_request(1, "/app/lib/main.dart", 10, None);
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "expected error when no isolate is active");
    let msg = resp.message.unwrap_or_default();
    assert!(
        msg.contains("no active isolate"),
        "error should mention missing isolate, got: {}",
        msg
    );
}

/// `breakpointLocations` returns an error when the source path is missing
/// from the request arguments.
#[tokio::test]
async fn test_breakpoint_locations_missing_source_path_returns_error() {
    let (mut adapter, mut rx) = DapAdapter::new(BreakpointLocationsMock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Request with an empty source path.
    let req = DapRequest {
        seq: 1,
        command: "breakpointLocations".into(),
        arguments: Some(serde_json::json!({
            "source": { "path": "" },
            "line": 10
        })),
    };
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "expected error for empty source path");
    let msg = resp.message.unwrap_or_default();
    assert!(
        msg.contains("source path is required"),
        "error should mention missing path, got: {}",
        msg
    );
}

/// `supportsBreakpointLocationsRequest` is advertised as `true` in the
/// default capabilities returned during `initialize`.
#[test]
fn test_capabilities_supports_breakpoint_locations() {
    use crate::protocol::types::Capabilities;
    let caps = Capabilities::fdemon_defaults();
    assert_eq!(
        caps.supports_breakpoint_locations_request,
        Some(true),
        "supportsBreakpointLocationsRequest should be true in fdemon capabilities"
    );
}
