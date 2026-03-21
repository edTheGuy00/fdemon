//! Tests for `log_level_to_category`, `LogOutput` event handling,
//! `DapEvent::output` constructor, `BackendError` type safety, and `emit_output`.

use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;

// ── log_level_to_category ─────────────────────────────────────────────

#[test]
fn test_log_level_to_category_error_is_stderr() {
    assert_eq!(log_level_to_category("error"), "stderr");
}

#[test]
fn test_log_level_to_category_info_is_stdout() {
    assert_eq!(log_level_to_category("info"), "stdout");
}

#[test]
fn test_log_level_to_category_debug_is_console() {
    assert_eq!(log_level_to_category("debug"), "console");
}

#[test]
fn test_log_level_to_category_warning_is_console() {
    assert_eq!(log_level_to_category("warning"), "console");
}

#[test]
fn test_log_level_to_category_unknown_is_console() {
    assert_eq!(log_level_to_category("verbose"), "console");
    assert_eq!(log_level_to_category("trace"), "console");
    assert_eq!(log_level_to_category(""), "console");
}

// ── LogOutput event handling ───────────────────────────────────────────

#[tokio::test]
async fn test_log_output_error_emits_stderr_category() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "Something went wrong".to_string(),
            level: "error".to_string(),
            source_uri: None,
            line: None,
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        assert_eq!(ev.event, "output");
        let body = ev.body.unwrap();
        assert_eq!(body["category"], "stderr");
        assert_eq!(body["output"], "Something went wrong\n");
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

#[tokio::test]
async fn test_log_output_info_emits_stdout_category() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "App started".to_string(),
            level: "info".to_string(),
            source_uri: None,
            line: None,
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        assert_eq!(ev.event, "output");
        let body = ev.body.unwrap();
        assert_eq!(body["category"], "stdout");
        assert_eq!(body["output"], "App started\n");
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

#[tokio::test]
async fn test_log_output_debug_emits_console_category() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "debug message".to_string(),
            level: "debug".to_string(),
            source_uri: None,
            line: None,
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        assert_eq!(ev.event, "output");
        let body = ev.body.unwrap();
        assert_eq!(body["category"], "console");
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

#[tokio::test]
async fn test_log_output_message_ends_with_newline() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    // Message without trailing newline — adapter must add one.
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "no newline".to_string(),
            level: "info".to_string(),
            source_uri: None,
            line: None,
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        let body = ev.body.unwrap();
        let output = body["output"].as_str().unwrap();
        assert!(output.ends_with('\n'), "output must end with newline");
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

#[tokio::test]
async fn test_log_output_message_already_has_newline_not_doubled() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "has newline\n".to_string(),
            level: "info".to_string(),
            source_uri: None,
            line: None,
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        let body = ev.body.unwrap();
        let output = body["output"].as_str().unwrap();
        assert_eq!(output, "has newline\n", "newline should not be doubled");
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

#[tokio::test]
async fn test_log_output_with_source_location() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "Error: null check".to_string(),
            level: "error".to_string(),
            source_uri: Some("file:///home/user/app/lib/main.dart".to_string()),
            line: Some(42),
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        let body = ev.body.unwrap();
        assert_eq!(body["category"], "stderr");
        assert_eq!(body["output"], "Error: null check\n");
        assert_eq!(body["source"]["path"], "/home/user/app/lib/main.dart");
        assert_eq!(body["source"]["name"], "main.dart");
        assert_eq!(body["line"], 42);
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

#[tokio::test]
async fn test_log_output_without_source_has_no_source_field() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::LogOutput {
            message: "plain log".to_string(),
            level: "info".to_string(),
            source_uri: None,
            line: None,
        })
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        let body = ev.body.unwrap();
        assert!(
            body.get("source").is_none() || body["source"].is_null(),
            "source should not be present when source_uri is None"
        );
        assert!(
            body.get("line").is_none() || body["line"].is_null(),
            "line should not be present when source_uri is None"
        );
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}

// ── DapEvent::output constructor ───────────────────────────────────────

#[test]
fn test_output_event_structure() {
    use crate::{DapEvent, DapMessage};
    let event = DapEvent::output("stderr", "Error: null check\n");
    let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
    assert_eq!(json["type"], "event");
    assert_eq!(json["event"], "output");
    assert_eq!(json["body"]["category"], "stderr");
    assert_eq!(json["body"]["output"], "Error: null check\n");
}

#[test]
fn test_output_event_with_source_in_body() {
    use crate::{DapEvent, DapMessage};
    let body = serde_json::json!({
        "category": "stderr",
        "output": "Error: null check\n",
        "source": {
            "name": "main.dart",
            "path": "/home/user/app/lib/main.dart"
        },
        "line": 42
    });
    let event = DapEvent::new("output", Some(body));
    let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
    assert_eq!(
        json["body"]["source"]["path"],
        "/home/user/app/lib/main.dart"
    );
    assert_eq!(json["body"]["line"], 42);
}

// ── BackendError type safety ───────────────────────────────────────────

#[tokio::test]
async fn test_backend_error_not_connected_produces_dap_error_for_pause() {
    use crate::DapRequest;
    let (mut adapter, _rx) = DapAdapter::new(NotConnectedBackend);
    adapter.thread_map.get_or_create("isolates/1");

    let req = DapRequest {
        seq: 1,
        command: "pause".into(),
        arguments: Some(serde_json::json!({ "threadId": 1 })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "BackendError::NotConnected should produce a DAP error response"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("not connected"),
        "Error message should include BackendError display text, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_backend_error_not_connected_for_attach_produces_dap_error() {
    use crate::DapRequest;
    let (mut adapter, _rx) = DapAdapter::new(NotConnectedBackend);

    let req = DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: None,
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "BackendError::NotConnected on attach should produce a DAP error"
    );
}

// ── on_resume state cleanup ────────────────────────────────────────────

#[tokio::test]
async fn test_on_resume_clears_exception_refs() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    // Register an isolate so we have a valid thread ID.
    let thread_id = super::register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Simulate an exception pause which inserts an exception ref.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Exception,
            breakpoint_id: None,
            exception: Some(serde_json::json!({"kind": "PlainInstance", "id": "obj/1"})),
        })
        .await;
    rx.try_recv().ok(); // drain the stopped event

    // Verify the exception ref was inserted.
    assert!(
        adapter.exception_refs.contains_key(&thread_id),
        "exception_refs should contain an entry after a pause-at-exception event"
    );

    // Call on_resume directly — it must clear exception_refs.
    adapter.on_resume();

    assert!(
        adapter.exception_refs.is_empty(),
        "on_resume() must clear exception_refs"
    );
}

// ── emit_output helper ─────────────────────────────────────────────────

#[tokio::test]
async fn test_emit_output_sends_output_event() {
    let (adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .emit_output("console", "Flutter Demon: Attached to VM Service\n")
        .await;

    let msg = rx.try_recv().expect("Should have received an event");
    if let DapMessage::Event(ev) = msg {
        assert_eq!(ev.event, "output");
        let body = ev.body.unwrap();
        assert_eq!(body["category"], "console");
        assert_eq!(body["output"], "Flutter Demon: Attached to VM Service\n");
    } else {
        panic!("Expected Event, got {:?}", msg);
    }
}
