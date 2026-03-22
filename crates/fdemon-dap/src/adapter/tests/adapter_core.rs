//! Tests for DapAdapter construction, request dispatch, `on_resume`,
//! `pause_reason_to_dap_str`, `path_to_dart_uri`, and `exception_filter_to_mode`.

use super::make_request;
use crate::adapter::events::pause_reason_to_dap_str;
use crate::adapter::handlers::{exception_filter_to_mode, path_to_dart_uri};
use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;

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
    let req = crate::DapRequest {
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
            breakpoint_id: None,
            exception: None,
        })
        .await;

    // Allocate a frame so we can evaluate with a frameId (avoids the
    // get_root_library_id path which requires a real VM response).
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    let req = crate::DapRequest {
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
            breakpoint_id: None,
            exception: None,
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
            breakpoint_id: None,
            exception: None,
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
    assert_eq!(exception_filter_to_mode(&[]), DapExceptionPauseMode::None);
}

#[test]
fn test_exception_filter_unhandled() {
    assert_eq!(
        exception_filter_to_mode(&["Unhandled".to_string()]),
        DapExceptionPauseMode::Unhandled
    );
}

#[test]
fn test_exception_filter_all() {
    assert_eq!(
        exception_filter_to_mode(&["All".to_string()]),
        DapExceptionPauseMode::All
    );
}

#[test]
fn test_exception_filter_all_takes_precedence_over_unhandled() {
    assert_eq!(
        exception_filter_to_mode(&["All".to_string(), "Unhandled".to_string()]),
        DapExceptionPauseMode::All
    );
    assert_eq!(
        exception_filter_to_mode(&["Unhandled".to_string(), "All".to_string()]),
        DapExceptionPauseMode::All
    );
}

#[test]
fn test_exception_filter_unknown_gives_none() {
    // Unknown filters fall through to None in the low-level helper;
    // the adapter layer rejects them with an error before reaching here.
    assert_eq!(
        exception_filter_to_mode(&["SomeOther".to_string()]),
        DapExceptionPauseMode::None
    );
}
