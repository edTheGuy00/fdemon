//! Tests for conditional breakpoint evaluation and hit-condition logic.

use std::sync::{Arc, Mutex};

use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;
use crate::DapRequest;

/// Helper: set up an adapter with `CondMockBackend`, register an isolate,
/// add a breakpoint with the given condition/hit_condition, and return.
async fn make_conditional_adapter(
    eval_result: serde_json::Value,
    condition: Option<&str>,
    hit_condition: Option<&str>,
) -> (
    DapAdapter<CondMockBackend>,
    tokio::sync::mpsc::Receiver<DapMessage>,
    Arc<Mutex<u32>>,
    String, // vm_id of the breakpoint
) {
    let (backend, resume_calls) = CondMockBackend::returning(eval_result);
    let (mut adapter, rx) = DapAdapter::new(backend);

    // Register an isolate.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;

    // Add a conditional breakpoint directly into the state (bypasses RPC).
    let _dap_id = adapter.breakpoint_state.add_with_condition(
        "bp/vm/1",
        "file:///lib/main.dart",
        Some(10),
        None,
        true,
        breakpoints::BreakpointCondition {
            condition: condition.map(|s| s.to_string()),
            hit_condition: hit_condition.map(|s| s.to_string()),
            log_message: None,
        },
    );

    (adapter, rx, resume_calls, "bp/vm/1".to_string())
}

#[tokio::test]
async fn test_conditional_breakpoint_truthy_emits_stopped() {
    // condition "x > 5" evaluates to true → adapter emits stopped
    let bool_true = serde_json::json!({"kind": "Bool", "valueAsString": "true"});
    let (mut adapter, mut rx, resume_calls, vm_id) =
        make_conditional_adapter(bool_true, Some("x > 5"), None).await;

    // Drain the IsolateStart thread event.
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
            exception: None,
        })
        .await;

    // Should emit stopped (condition was truthy).
    let msg = rx.try_recv().expect("Expected a stopped event");
    assert!(matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"));
    assert_eq!(
        *resume_calls.lock().unwrap(),
        0,
        "Should NOT have called resume"
    );
}

#[tokio::test]
async fn test_conditional_breakpoint_falsy_resumes_silently() {
    // condition "x > 5" evaluates to false → adapter resumes silently
    let bool_false = serde_json::json!({"kind": "Bool", "valueAsString": "false"});
    let (mut adapter, mut rx, resume_calls, vm_id) =
        make_conditional_adapter(bool_false, Some("x > 5"), None).await;

    // Drain the IsolateStart thread event.
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
            exception: None,
        })
        .await;

    // Should NOT emit stopped — should call resume instead.
    assert!(
        rx.try_recv().is_err(),
        "No stopped event should be emitted when condition is falsy"
    );
    assert_eq!(
        *resume_calls.lock().unwrap(),
        1,
        "resume() should have been called once"
    );
}

#[tokio::test]
async fn test_hit_condition_resumes_before_threshold() {
    // hit_condition ">= 3" — first two hits should resume silently
    let (mut adapter, mut rx, resume_calls, vm_id) =
        make_conditional_adapter(serde_json::json!({}), None, Some(">= 3")).await;
    rx.try_recv().ok(); // Drain IsolateStart event.

    // Hit 1 — should resume silently.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id.clone()),
            exception: None,
        })
        .await;
    assert!(rx.try_recv().is_err(), "Hit 1: should not emit stopped");

    // Hit 2 — should resume silently.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
            exception: None,
        })
        .await;
    assert!(rx.try_recv().is_err(), "Hit 2: should not emit stopped");

    assert_eq!(
        *resume_calls.lock().unwrap(),
        2,
        "Should have resumed twice"
    );
}

#[tokio::test]
async fn test_hit_condition_stops_at_threshold() {
    // hit_condition ">= 3" — third hit should emit stopped
    let (mut adapter, mut rx, resume_calls, vm_id) =
        make_conditional_adapter(serde_json::json!({}), None, Some(">= 3")).await;
    rx.try_recv().ok();

    for _ in 0..2 {
        adapter
            .handle_debug_event(DebugEvent::Paused {
                isolate_id: "isolates/1".into(),
                reason: PauseReason::Breakpoint,
                breakpoint_id: Some(vm_id.clone()),
                exception: None,
            })
            .await;
        rx.try_recv().ok(); // Discard (should be None for silent resumes).
    }
    assert_eq!(*resume_calls.lock().unwrap(), 2);

    // Hit 3 — should emit stopped.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
            exception: None,
        })
        .await;

    let msg = rx.try_recv().expect("Expected stopped event on hit 3");
    assert!(matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"));
    assert_eq!(
        *resume_calls.lock().unwrap(),
        2,
        "resume() should not have been called on hit 3"
    );
}

#[tokio::test]
async fn test_condition_error_causes_stop_safe_default() {
    // A backend that returns an error from evaluate_in_frame.
    struct ErrorEvalBackend;

    impl MockTestBackend for ErrorEvalBackend {
        async fn add_breakpoint(
            &self,
            _: &str,
            _: &str,
            line: i32,
            column: Option<i32>,
        ) -> Result<BreakpointResult, BackendError> {
            Ok(BreakpointResult {
                vm_id: format!("bp/{line}"),
                resolved: true,
                line: Some(line),
                column,
            })
        }

        async fn evaluate_in_frame(
            &self,
            _: &str,
            _: i32,
            _: &str,
        ) -> Result<serde_json::Value, BackendError> {
            Err(BackendError::VmServiceError("evaluation failed".into()))
        }

        async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
            Ok(serde_json::json!({"isolates": []}))
        }
    }

    let (mut adapter, mut rx) = DapAdapter::new(ErrorEvalBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // Add a breakpoint with a condition.
    adapter.breakpoint_state.add_with_condition(
        "bp/vm/err",
        "file:///lib/main.dart",
        Some(10),
        None,
        true,
        breakpoints::BreakpointCondition {
            condition: Some("someCondition()".to_string()),
            hit_condition: None,
            log_message: None,
        },
    );

    // Pause at the breakpoint — evaluate_in_frame will error.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some("bp/vm/err".to_string()),
            exception: None,
        })
        .await;

    // Safe default: should emit stopped despite evaluation error.
    let msg = rx
        .try_recv()
        .expect("Expected stopped event on evaluation error");
    assert!(matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"));
}

#[tokio::test]
async fn test_unconditional_breakpoint_emits_stopped_without_resume() {
    // Breakpoint with no condition and no hit_condition → always stops.
    let (backend, resume_calls) = CondMockBackend::returning(serde_json::json!({}));
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    adapter.breakpoint_state.add_with_condition(
        "bp/unc/1",
        "file:///lib/main.dart",
        Some(5),
        None,
        true,
        breakpoints::BreakpointCondition::default(),
    );

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some("bp/unc/1".to_string()),
            exception: None,
        })
        .await;

    let msg = rx.try_recv().expect("Expected stopped event");
    assert!(matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"));
    assert_eq!(*resume_calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn test_no_breakpoint_id_emits_stopped_unconditionally() {
    // When breakpoint_id is None, no condition can be found — always stops.
    let (backend, resume_calls) =
        CondMockBackend::returning(serde_json::json!({"kind": "Bool", "valueAsString": "false"}));
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: None, // No breakpoint ID → no condition lookup
            exception: None,
        })
        .await;

    // Since there's no breakpoint_id, no condition evaluation happens.
    let msg = rx.try_recv().expect("Expected stopped event");
    assert!(matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"));
    assert_eq!(
        *resume_calls.lock().unwrap(),
        0,
        "Should not resume when no breakpoint_id"
    );
}

#[tokio::test]
async fn test_non_breakpoint_pause_emits_stopped_without_condition_check() {
    // Exception pause → no condition evaluation, always stops.
    let bool_false = serde_json::json!({"kind": "Bool", "valueAsString": "false"});
    let (backend, resume_calls) = CondMockBackend::returning(bool_false);
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Exception,
            breakpoint_id: None,
            exception: None,
        })
        .await;

    let msg = rx.try_recv().expect("Expected stopped event");
    assert!(
        matches!(msg, DapMessage::Event(ref e) if e.event == "stopped" && e.body.as_ref().map(|b| b["reason"] == "exception").unwrap_or(false))
    );
    assert_eq!(*resume_calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn test_combined_hit_and_expression_condition_both_must_pass() {
    // hit_condition ">= 2" AND condition "x > 5" (both truthy on hit 2)
    let bool_true = serde_json::json!({"kind": "Bool", "valueAsString": "true"});
    let (mut adapter, mut rx, resume_calls, vm_id) =
        make_conditional_adapter(bool_true, Some("x > 5"), Some(">= 2")).await;
    rx.try_recv().ok();

    // Hit 1: hit_condition fails — should resume silently without evaluating condition.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id.clone()),
            exception: None,
        })
        .await;
    assert!(rx.try_recv().is_err(), "Hit 1 should not stop");
    assert_eq!(*resume_calls.lock().unwrap(), 1);

    // Hit 2: both conditions pass — should emit stopped.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
            exception: None,
        })
        .await;
    let msg = rx.try_recv().expect("Expected stopped event on hit 2");
    assert!(matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"));
    assert_eq!(
        *resume_calls.lock().unwrap(),
        1,
        "Should not resume on hit 2"
    );
}

#[tokio::test]
async fn test_setbreakpoints_stores_condition_in_state() {
    // Verify that setBreakpoints handler stores condition from SourceBreakpoint.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    use crate::protocol::types::{DapSource, SourceBreakpoint};
    let req = DapRequest {
        seq: 1,
        command: "setBreakpoints".into(),
        arguments: Some(serde_json::json!({
            "source": DapSource {
                path: Some("/lib/main.dart".to_string()),
                ..Default::default()
            },
            "breakpoints": [
                SourceBreakpoint {
                    line: 10,
                    condition: Some("x > 5".to_string()),
                    hit_condition: Some(">= 2".to_string()),
                    ..Default::default()
                }
            ],
        })),
    };

    let resp = adapter.handle_request(&req).await;
    assert!(resp.success, "setBreakpoints should succeed");

    // Verify the stored breakpoint has the condition.
    let entry = adapter
        .breakpoint_state
        .iter()
        .next()
        .expect("One breakpoint should be tracked");
    assert_eq!(entry.condition.as_deref(), Some("x > 5"));
    assert_eq!(entry.hit_condition.as_deref(), Some(">= 2"));
}
