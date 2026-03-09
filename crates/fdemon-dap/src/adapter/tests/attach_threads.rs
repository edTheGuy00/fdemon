//! Tests for `handle_attach`, `handle_threads`, and thread name lifecycle.

use super::make_request;
use crate::adapter::test_helpers::*;
use crate::adapter::*;
use crate::DapMessage;

// ── handle_attach tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_handle_attach_success_populates_thread_map() {
    let (mut adapter, _rx) = DapAdapter::new(AttachMockBackend);
    let resp = adapter.handle_request(&make_request(1, "attach")).await;
    assert!(resp.success, "attach should succeed when VM is reachable");
    assert_eq!(
        adapter.thread_map.len(),
        2,
        "Both isolates should be registered"
    );
}

#[tokio::test]
async fn test_handle_attach_emits_thread_started_events() {
    let (mut adapter, mut rx) = DapAdapter::new(AttachMockBackend);
    adapter.handle_request(&make_request(1, "attach")).await;

    let mut started_count = 0;
    while let Ok(msg) = rx.try_recv() {
        if let DapMessage::Event(e) = msg {
            // After Task 08, attach also emits flutter.appStart.
            // Only count the thread started events for this test.
            if e.event == "thread" {
                let body = e.body.unwrap();
                assert_eq!(body["reason"], "started");
                started_count += 1;
            }
        }
    }
    assert_eq!(
        started_count, 2,
        "Should emit one started event per isolate"
    );
}

#[tokio::test]
async fn test_handle_attach_stores_thread_names() {
    let (mut adapter, _rx) = DapAdapter::new(AttachMockBackend);
    adapter.handle_request(&make_request(1, "attach")).await;

    let name1 = adapter.thread_names.get(&1).map(String::as_str);
    let name2 = adapter.thread_names.get(&2).map(String::as_str);
    assert_eq!(name1, Some("main"));
    assert_eq!(name2, Some("background"));
}

#[tokio::test]
async fn test_handle_attach_vm_failure_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(FailingVmBackend);
    let resp = adapter.handle_request(&make_request(1, "attach")).await;
    assert!(!resp.success, "attach should fail when VM is unreachable");
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Failed to attach"),
        "Error should mention attach failure, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_handle_attach_empty_vm_response_succeeds() {
    // MockBackend.get_vm() returns {} with no "isolates" key.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let resp = adapter.handle_request(&make_request(1, "attach")).await;
    assert!(
        resp.success,
        "attach should succeed even with empty VM response"
    );
    assert_eq!(
        adapter.thread_map.len(),
        0,
        "No threads should be registered when VM has no isolates"
    );
}

// ── handle_threads tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_handle_threads_returns_success_with_empty_list() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let resp = adapter.handle_request(&make_request(1, "threads")).await;
    assert!(resp.success);
    let body = resp.body.as_ref().unwrap();
    let threads = body["threads"].as_array().unwrap();
    assert!(
        threads.is_empty(),
        "Should return empty list when no threads registered"
    );
}

#[tokio::test]
async fn test_handle_threads_returns_all_registered_threads() {
    let (mut adapter, _rx) = DapAdapter::new(AttachMockBackend);
    adapter.handle_request(&make_request(1, "attach")).await;

    let resp = adapter.handle_request(&make_request(2, "threads")).await;
    assert!(resp.success);
    let body = resp.body.as_ref().unwrap();
    let threads = body["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 2);
    // Threads are sorted by ID.
    assert_eq!(threads[0]["id"], 1);
    assert_eq!(threads[0]["name"], "main");
    assert_eq!(threads[1]["id"], 2);
    assert_eq!(threads[1]["name"], "background");
}

#[tokio::test]
async fn test_handle_threads_uses_default_name_when_missing() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    // Manually register a thread without inserting a name — fallback to "Thread N".
    let thread_id = adapter.thread_map.get_or_create("isolates/7");

    let resp = adapter.handle_request(&make_request(1, "threads")).await;
    assert!(resp.success);
    let body = resp.body.as_ref().unwrap();
    let threads = body["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["name"], format!("Thread {thread_id}"));
}

// ── thread name lifecycle ─────────────────────────────────────────────

#[tokio::test]
async fn test_isolate_start_stores_thread_name() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/42".into(),
            name: "worker".into(),
        })
        .await;
    rx.try_recv().ok();

    let thread_id = adapter.thread_map.thread_id_for("isolates/42").unwrap();
    assert_eq!(
        adapter.thread_names.get(&thread_id).map(String::as_str),
        Some("worker"),
        "IsolateStart should store the thread name"
    );
}

#[tokio::test]
async fn test_isolate_exit_removes_thread_name() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/42".into(),
            name: "worker".into(),
        })
        .await;
    rx.try_recv().ok();

    let thread_id = adapter.thread_map.thread_id_for("isolates/42").unwrap();
    assert!(adapter.thread_names.contains_key(&thread_id));

    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/42".into(),
        })
        .await;
    rx.try_recv().ok();

    assert!(
        !adapter.thread_names.contains_key(&thread_id),
        "IsolateExit should remove the thread name"
    );
}

#[tokio::test]
async fn test_isolate_exit_removes_thread_from_map() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    assert_eq!(adapter.thread_map.len(), 1);

    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".into(),
        })
        .await;
    rx.try_recv().ok();

    assert_eq!(
        adapter.thread_map.len(),
        0,
        "IsolateExit should remove the thread from the thread map"
    );
}
