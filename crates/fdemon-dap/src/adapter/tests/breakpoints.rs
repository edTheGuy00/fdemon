//! Tests for `handle_set_breakpoints`, `handle_set_exception_breakpoints`,
//! and `BreakpointResolved` event handling.

use super::{make_request, make_set_breakpoints_request, make_set_exception_breakpoints_request};
use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;

// ── handle_set_breakpoints ────────────────────────────────────────────

#[tokio::test]
async fn test_set_breakpoints_without_isolate_returns_unverified() {
    // No isolate registered → breakpoints come back unverified.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "setBreakpoints should succeed even without an isolate"
    );
    let body = resp.body.unwrap();
    let bps = body["breakpoints"].as_array().unwrap();
    assert_eq!(bps.len(), 2);
    for bp in bps {
        assert_eq!(
            bp["verified"], false,
            "Breakpoints without isolate must be unverified"
        );
    }
    // Breakpoints should NOT be stored when there's no isolate.
    assert!(adapter.breakpoint_state.is_empty());
}

#[tokio::test]
async fn test_set_breakpoints_with_isolate_adds_and_returns_verified() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    // Register an isolate so breakpoints can be sent to the VM.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok(); // Drain the thread event.

    let req = make_set_breakpoints_request(2, "/lib/main.dart", &[10]);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let bps = body["breakpoints"].as_array().unwrap();
    assert_eq!(bps.len(), 1);
    // MockBackend returns resolved=true.
    assert_eq!(bps[0]["verified"], true);
    assert!(bps[0]["id"].as_i64().is_some());
    // State should have one tracked breakpoint.
    assert_eq!(adapter.breakpoint_state.len(), 1);
}

#[tokio::test]
async fn test_set_breakpoints_diff_removes_old_adds_new() {
    // Acceptance criteria: setBreakpoints replaces all breakpoints for a file.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // First call: lines 10 and 20.
    let req1 = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
    adapter.handle_request(&req1).await;
    assert_eq!(adapter.breakpoint_state.len(), 2);

    // Second call: lines 10 and 30 only.  Line 20 should be removed.
    let req2 = make_set_breakpoints_request(2, "/lib/main.dart", &[10, 30]);
    let resp = adapter.handle_request(&req2).await;

    assert!(resp.success);
    assert_eq!(
        adapter.breakpoint_state.len(),
        2,
        "Should have 2 breakpoints after diff (10 kept, 20 removed, 30 added)"
    );
}

#[tokio::test]
async fn test_set_breakpoints_empty_list_removes_all() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // Add some breakpoints.
    let req1 = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
    adapter.handle_request(&req1).await;
    assert_eq!(adapter.breakpoint_state.len(), 2);

    // Clear all breakpoints by sending empty list.
    let req2 = make_set_breakpoints_request(2, "/lib/main.dart", &[]);
    let resp = adapter.handle_request(&req2).await;

    assert!(resp.success);
    let bps = resp.body.unwrap()["breakpoints"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        bps.is_empty(),
        "Empty desired list should return empty array"
    );
    assert!(adapter.breakpoint_state.is_empty());
}

#[tokio::test]
async fn test_set_breakpoints_existing_line_reused() {
    // If the same line is requested twice the second request reuses the entry.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req1 = make_set_breakpoints_request(1, "/lib/main.dart", &[10]);
    let resp1 = adapter.handle_request(&req1).await;
    let id1 = resp1.body.unwrap()["breakpoints"][0]["id"]
        .as_i64()
        .unwrap();

    let req2 = make_set_breakpoints_request(2, "/lib/main.dart", &[10]);
    let resp2 = adapter.handle_request(&req2).await;
    let id2 = resp2.body.unwrap()["breakpoints"][0]["id"]
        .as_i64()
        .unwrap();

    assert_eq!(
        id1, id2,
        "Same line should reuse the existing DAP breakpoint ID"
    );
    assert_eq!(adapter.breakpoint_state.len(), 1);
}

#[tokio::test]
async fn test_set_breakpoints_no_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_request(1, "setBreakpoints");
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "setBreakpoints without arguments must return error"
    );
}

// ── handle_set_exception_breakpoints ─────────────────────────────────

#[tokio::test]
async fn test_set_exception_breakpoints_empty_filters_returns_success() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_set_exception_breakpoints_request(1, &[]);
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let body = resp.body.unwrap();
    assert!(body["breakpoints"].as_array().unwrap().is_empty());
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::None);
}

#[tokio::test]
async fn test_set_exception_breakpoints_unhandled_mode() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_set_exception_breakpoints_request(1, &["Unhandled"]);
    adapter.handle_request(&req).await;
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::Unhandled);
}

#[tokio::test]
async fn test_set_exception_breakpoints_all_mode() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_set_exception_breakpoints_request(1, &["All"]);
    adapter.handle_request(&req).await;
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::All);
}

#[tokio::test]
async fn test_set_exception_breakpoints_all_takes_precedence() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_set_exception_breakpoints_request(1, &["Unhandled", "All"]);
    adapter.handle_request(&req).await;
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::All);
}

#[tokio::test]
async fn test_set_exception_breakpoints_updates_mode_for_isolates() {
    // Verify the adapter applies the mode to all known isolates without
    // crashing. The MockBackend silently succeeds, so we just check the
    // stored mode and a successful response.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_exception_breakpoints_request(1, &["All"]);
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::All);
}

#[tokio::test]
async fn test_set_exception_breakpoints_unknown_filter_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_set_exception_breakpoints_request(1, &["UserUnhandled"]);
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "Unknown exception filter should return DAP error"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Unknown exception filter"),
        "Error should mention the unknown filter, got: {:?}",
        msg
    );
    // Mode should remain the default (not changed on error).
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::Unhandled);
}

#[tokio::test]
async fn test_set_exception_breakpoints_no_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = make_request(1, "setExceptionBreakpoints");
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "setExceptionBreakpoints without arguments must return error"
    );
}

// ── BreakpointResolved event → IDE notification ───────────────────────

#[tokio::test]
async fn test_breakpoint_resolved_event_sends_breakpoint_changed_event() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // Add a breakpoint so there is a VM ID to resolve.
    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[10]);
    adapter.handle_request(&req).await;

    // Get the VM ID that was assigned (MockBackend returns "bp/line:<N>").
    let vm_id = "bp/line:10".to_string();

    // Drain any remaining events.
    while rx.try_recv().is_ok() {}

    // Fire a BreakpointResolved event.
    adapter
        .handle_debug_event(DebugEvent::BreakpointResolved {
            vm_breakpoint_id: vm_id,
            line: Some(11),
            column: None,
        })
        .await;

    // The adapter should emit a breakpoint event with reason "changed".
    let msg = rx.try_recv().expect("Expected a breakpoint event");
    if let DapMessage::Event(e) = msg {
        assert_eq!(e.event, "breakpoint");
        let body = e.body.unwrap();
        assert_eq!(body["reason"], "changed");
        assert_eq!(body["breakpoint"]["verified"], true);
    } else {
        panic!("Expected Event, got: {:?}", msg);
    }
}

#[tokio::test]
async fn test_breakpoint_resolved_unknown_vm_id_sends_no_event() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::BreakpointResolved {
            vm_breakpoint_id: "bp/unknown".to_string(),
            line: Some(5),
            column: None,
        })
        .await;
    // Unknown VM ID: no event should be emitted.
    assert!(rx.try_recv().is_err());
}
