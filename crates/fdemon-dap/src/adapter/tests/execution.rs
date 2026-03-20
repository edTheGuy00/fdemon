//! Tests for continue/next/stepIn/stepOut/pause execution control commands.

use super::make_request;
use crate::adapter::events::pause_reason_to_dap_str;
use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;

fn make_continue_request(seq: i64, thread_id: i64) -> crate::DapRequest {
    crate::DapRequest {
        seq,
        command: "continue".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    }
}

fn make_step_request(seq: i64, command: &str, thread_id: i64) -> crate::DapRequest {
    crate::DapRequest {
        seq,
        command: command.into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    }
}

fn make_pause_request(seq: i64, thread_id: i64) -> crate::DapRequest {
    crate::DapRequest {
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
    let resp = adapter.handle_request(&make_pause_request(1, 1)).await;
    assert!(resp.success);
    assert!(resp.body.is_none(), "pause response should have no body");
}

#[tokio::test]
async fn test_pause_cmd_unknown_thread_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let resp = adapter.handle_request(&make_pause_request(1, 99)).await;
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
            breakpoint_id: None,
            exception: None,
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
            breakpoint_id: None,
            exception: None,
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
            breakpoint_id: None,
            exception: None,
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
            breakpoint_id: None,
            exception: None,
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
