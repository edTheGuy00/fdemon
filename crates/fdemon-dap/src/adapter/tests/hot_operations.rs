//! Tests for `hotReload` and `hotRestart` custom DAP requests.

use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapRequest;

/// Build a `hotReload` or `hotRestart` request with given arguments.
fn make_hot_request(seq: i64, command: &str, args: serde_json::Value) -> DapRequest {
    DapRequest {
        seq,
        command: command.into(),
        arguments: Some(args),
    }
}

// Test 1: hotReload dispatches to backend and returns success
#[tokio::test]
async fn test_hot_reload_request_returns_success() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(1, "hotReload", serde_json::json!({"reason": "manual"}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "hotReload should succeed when backend returns Ok(())"
    );
}

// Test 2: hotRestart dispatches to backend and returns success
#[tokio::test]
async fn test_hot_restart_request_returns_success() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(1, "hotRestart", serde_json::json!({"reason": "manual"}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "hotRestart should succeed when backend returns Ok(())"
    );
}

// Test 3: hotReload request with no arguments still succeeds (reason is optional)
#[tokio::test]
async fn test_hot_reload_request_no_arguments_succeeds() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(2, "hotReload", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "hotReload with empty arguments should succeed"
    );
}

// Test 4: hotRestart request with no arguments still succeeds
#[tokio::test]
async fn test_hot_restart_request_no_arguments_succeeds() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(2, "hotRestart", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "hotRestart with empty arguments should succeed"
    );
}

// Test 5: hotReload returns error when backend is not connected
#[tokio::test]
async fn test_hot_reload_returns_error_when_backend_fails() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::failing());
    let req = make_hot_request(1, "hotReload", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "hotReload should return error when backend fails"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Hot reload failed"),
        "Error message should indicate reload failure, got: {:?}",
        msg
    );
}

// Test 6: hotRestart returns error when backend is not connected
#[tokio::test]
async fn test_hot_restart_returns_error_when_backend_fails() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::failing());
    let req = make_hot_request(1, "hotRestart", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "hotRestart should return error when backend fails"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Hot restart failed"),
        "Error message should indicate restart failure, got: {:?}",
        msg
    );
}

// Test 7: hotReload success response has no body
#[tokio::test]
async fn test_hot_reload_success_response_has_no_body() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(3, "hotReload", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    assert!(
        resp.body.is_none(),
        "hotReload success response should have no body"
    );
}

// Test 8: hotRestart success response has no body
#[tokio::test]
async fn test_hot_restart_success_response_has_no_body() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(3, "hotRestart", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    assert!(
        resp.body.is_none(),
        "hotRestart success response should have no body"
    );
}

// Test 9: hotReload with reason=save still succeeds (reason is informational)
#[tokio::test]
async fn test_hot_reload_reason_save_succeeds() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(4, "hotReload", serde_json::json!({"reason": "save"}));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success, "hotReload with reason=save should succeed");
}

// Test 10: hotRestart with reason=save still succeeds
#[tokio::test]
async fn test_hot_restart_reason_save_succeeds() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());
    let req = make_hot_request(4, "hotRestart", serde_json::json!({"reason": "save"}));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success, "hotRestart with reason=save should succeed");
}

// Test 11: unknown custom command returns error with command name
#[tokio::test]
async fn test_unknown_custom_command_returns_error_with_name() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = DapRequest {
        seq: 5,
        command: "unknownCustomCommand".into(),
        arguments: Some(serde_json::json!({})),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success, "Unknown command should return error");
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("unknownCustomCommand"),
        "Error message should include the unknown command name, got: {:?}",
        msg
    );
}

// Test 12: hotReload and hotRestart response commands match the request commands
#[tokio::test]
async fn test_hot_reload_and_hot_restart_response_commands_match() {
    let (mut adapter, _rx) = DapAdapter::new(HotOpMockBackend::ok());

    let reload_req = make_hot_request(1, "hotReload", serde_json::json!({}));
    let restart_req = make_hot_request(2, "hotRestart", serde_json::json!({}));

    let reload_resp = adapter.handle_request(&reload_req).await;
    let restart_resp = adapter.handle_request(&restart_req).await;

    assert!(reload_resp.success);
    assert!(restart_resp.success);
    assert_eq!(
        reload_resp.command, "hotReload",
        "Response command should echo the request command"
    );
    assert_eq!(
        restart_resp.command, "hotRestart",
        "Response command should echo the request command"
    );
}

// Test 13: hotReload with NoopBackend (simulates no Flutter session running)
#[tokio::test]
async fn test_hot_reload_with_no_session_returns_error() {
    use crate::server::session::NoopBackend;
    let (mut adapter, _rx) = DapAdapter::new(NoopBackend);
    let req = make_hot_request(1, "hotReload", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "hotReload with NoopBackend should return error (no Flutter session)"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Hot reload failed"),
        "Error should mention reload failure, got: {:?}",
        msg
    );
}

// Test 14: hotRestart with NoopBackend (simulates no Flutter session running)
#[tokio::test]
async fn test_hot_restart_with_no_session_returns_error() {
    use crate::server::session::NoopBackend;
    let (mut adapter, _rx) = DapAdapter::new(NoopBackend);
    let req = make_hot_request(1, "hotRestart", serde_json::json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "hotRestart with NoopBackend should return error (no Flutter session)"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Hot restart failed"),
        "Error should mention restart failure, got: {:?}",
        msg
    );
}
