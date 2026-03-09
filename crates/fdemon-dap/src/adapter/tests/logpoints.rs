//! Tests for logpoint evaluation: output events, auto-resume, and interpolation.

use std::sync::{Arc, Mutex};

use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;
use crate::DapRequest;

/// Set up an adapter with a logpoint breakpoint registered.
///
/// Returns: (adapter, event_rx, resume_calls, vm_id)
async fn make_logpoint_adapter(
    backend: LogpointMockBackend,
    resume_calls: Arc<Mutex<u32>>,
    log_message: &str,
) -> (
    DapAdapter<LogpointMockBackend>,
    tokio::sync::mpsc::Receiver<DapMessage>,
    Arc<Mutex<u32>>,
    String,
) {
    let (mut adapter, rx) = DapAdapter::new(backend);

    // Register an isolate.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;

    // Add a logpoint directly into the breakpoint state (bypasses the RPC).
    adapter.breakpoint_state.add_with_condition(
        "bp/vm/lp1",
        "file:///lib/main.dart",
        Some(10),
        None,
        true,
        breakpoints::BreakpointCondition {
            condition: None,
            hit_condition: None,
            log_message: Some(log_message.to_string()),
        },
    );

    (adapter, rx, resume_calls, "bp/vm/lp1".to_string())
}

#[tokio::test]
async fn test_logpoint_emits_output_event_not_stopped() {
    // A logpoint with log_message should emit `output`, not `stopped`.
    let (backend, resume_calls) = LogpointMockBackend::new_returning_value("x", "42");
    let (mut adapter, mut rx, _resume, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "x = {x}").await;
    // Drain the IsolateStart thread event.
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    let msg = rx
        .try_recv()
        .expect("Expected an output event from logpoint");
    match msg {
        DapMessage::Event(ref e) => {
            assert_eq!(e.event, "output", "Should emit 'output', not 'stopped'");
            let body = e.body.as_ref().unwrap();
            assert_eq!(body["category"], "console");
            let output = body["output"].as_str().unwrap();
            assert!(
                output.contains("x = 42"),
                "Output should contain interpolated value, got: {:?}",
                output
            );
            assert!(output.ends_with('\n'), "Output should end with newline");
        }
        other => panic!("Expected Event(output), got: {:?}", other),
    }

    // No `stopped` event should follow.
    assert!(
        rx.try_recv().is_err(),
        "Logpoint must not emit a stopped event"
    );
}

#[tokio::test]
async fn test_logpoint_auto_resumes_isolate() {
    // After emitting output, the adapter must call resume().
    let (backend, resume_calls) = LogpointMockBackend::new_returning_value("x", "42");
    let (mut adapter, mut rx, resume_calls, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "x = {x}").await;
    rx.try_recv().ok(); // Drain IsolateStart event.

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    // Drain the output event.
    rx.try_recv().ok();

    assert_eq!(
        *resume_calls.lock().unwrap(),
        1,
        "Logpoint must call resume() exactly once"
    );
}

#[tokio::test]
async fn test_logpoint_literal_only_message() {
    // No expressions in template — just a literal message.
    let (backend, _resume) = LogpointMockBackend::new_failing();
    let resume_calls = _resume.clone();
    let (mut adapter, mut rx, _rc, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "Hello, world!").await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    let msg = rx.try_recv().expect("Expected output event");
    if let DapMessage::Event(ref e) = msg {
        assert_eq!(e.event, "output");
        let output = e.body.as_ref().unwrap()["output"].as_str().unwrap();
        assert!(
            output.starts_with("Hello, world!"),
            "Output should be the literal message, got: {:?}",
            output
        );
    } else {
        panic!("Expected Event(output), got: {:?}", msg);
    }
}

#[tokio::test]
async fn test_logpoint_expression_evaluation_error_produces_error_placeholder() {
    // If expression evaluation fails, output contains `<error>`.
    let (backend, _resume) = LogpointMockBackend::new_failing();
    let resume_calls = _resume.clone();
    // "missingVar" is not in eval_map → evaluate_in_frame returns Err.
    let (mut adapter, mut rx, _rc, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "val = {missingVar}").await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    let msg = rx.try_recv().expect("Expected output event");
    if let DapMessage::Event(ref e) = msg {
        assert_eq!(e.event, "output");
        let output = e.body.as_ref().unwrap()["output"].as_str().unwrap();
        assert!(
            output.contains("<error>"),
            "Failed expression should produce <error>, got: {:?}",
            output
        );
    } else {
        panic!("Expected Event(output), got: {:?}", msg);
    }
}

#[tokio::test]
async fn test_logpoint_output_includes_source_location() {
    // The output event should include source name, path, and line.
    let (backend, resume_calls) = LogpointMockBackend::new_returning_value("x", "1");
    let (mut adapter, mut rx, _rc, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "{x}").await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    let msg = rx.try_recv().expect("Expected output event");
    if let DapMessage::Event(ref e) = msg {
        let body = e.body.as_ref().unwrap();
        // Source should be populated (breakpoint was at file:///lib/main.dart, line 10).
        assert!(
            body.get("source").is_some() && !body["source"].is_null(),
            "output event should include source, got body: {:?}",
            body
        );
        let source = &body["source"];
        assert!(
            source["name"].as_str().unwrap_or("").contains("main.dart"),
            "source name should mention the file, got: {:?}",
            source
        );
        // Line should be 10.
        assert_eq!(body["line"], 10);
    } else {
        panic!("Expected Event(output), got: {:?}", msg);
    }
}

#[tokio::test]
async fn test_logpoint_multiple_expressions_interpolated() {
    // "({a}, {b})" with a=1, b=2 should produce "(1, 2)".
    let (backend, resume_calls) = LogpointMockBackend::new_with_map(&[("a", "1"), ("b", "2")]);
    let (mut adapter, mut rx, _rc, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "({a}, {b})").await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    let msg = rx.try_recv().expect("Expected output event");
    if let DapMessage::Event(ref e) = msg {
        let output = e.body.as_ref().unwrap()["output"].as_str().unwrap();
        assert!(
            output.starts_with("(1, 2)"),
            "Should interpolate both expressions, got: {:?}",
            output
        );
    } else {
        panic!("Expected Event(output), got: {:?}", msg);
    }
}

#[tokio::test]
async fn test_regular_breakpoint_is_not_affected_by_logpoint_logic() {
    // A breakpoint without log_message should still emit `stopped`.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // Add a regular (non-logpoint) breakpoint.
    adapter.breakpoint_state.add_with_condition(
        "bp/regular",
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
            breakpoint_id: Some("bp/regular".to_string()),
        })
        .await;

    let msg = rx.try_recv().expect("Expected stopped event");
    assert!(
        matches!(msg, DapMessage::Event(ref e) if e.event == "stopped"),
        "Regular breakpoint should emit stopped, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_setbreakpoints_stores_log_message_in_state() {
    // Verify that setBreakpoints handler stores log_message from SourceBreakpoint.
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
                    line: 15,
                    log_message: Some("counter = {counter}".to_string()),
                    ..Default::default()
                }
            ],
        })),
    };

    let resp = adapter.handle_request(&req).await;
    assert!(resp.success, "setBreakpoints should succeed");

    let entry = adapter
        .breakpoint_state
        .iter()
        .next()
        .expect("One breakpoint should be tracked");
    assert_eq!(
        entry.log_message.as_deref(),
        Some("counter = {counter}"),
        "log_message should be stored in breakpoint entry"
    );
}

#[tokio::test]
async fn test_logpoint_with_condition_falsy_does_not_log() {
    // Logpoint with condition "x > 5" that evaluates to false — should not log.
    // The backend returns false for ALL evaluations (including the condition).
    let (backend, resume_calls) =
        CondMockBackend::returning(serde_json::json!({"kind": "Bool", "valueAsString": "false"}));
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // Add a breakpoint with BOTH condition and log_message.
    adapter.breakpoint_state.add_with_condition(
        "bp/cond_lp",
        "file:///lib/main.dart",
        Some(20),
        None,
        true,
        breakpoints::BreakpointCondition {
            condition: Some("x > 5".to_string()),
            hit_condition: None,
            log_message: Some("x = {x}".to_string()),
        },
    );

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some("bp/cond_lp".to_string()),
        })
        .await;

    // Condition is falsy → should resume silently with no output or stopped event.
    assert!(
        rx.try_recv().is_err(),
        "Falsy condition logpoint should emit no events"
    );
    assert_eq!(
        *resume_calls.lock().unwrap(),
        1,
        "Should have resumed once (silently)"
    );
}

#[tokio::test]
async fn test_logpoint_output_ends_with_newline() {
    // Output event must always end with '\n'.
    let (backend, resume_calls) = LogpointMockBackend::new_returning_value("x", "no_newline");
    let (mut adapter, mut rx, _rc, vm_id) =
        make_logpoint_adapter(backend, resume_calls, "val={x}").await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: Some(vm_id),
        })
        .await;

    let msg = rx.try_recv().expect("Expected output event");
    if let DapMessage::Event(ref e) = msg {
        let output = e.body.as_ref().unwrap()["output"].as_str().unwrap();
        assert!(
            output.ends_with('\n'),
            "Output must always end with newline, got: {:?}",
            output
        );
    } else {
        panic!("Expected Event(output), got: {:?}", msg);
    }
}
