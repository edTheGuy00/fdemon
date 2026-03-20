//! Tests for `handle_stack_trace`, `handle_scopes`, `instance_ref_to_variable`,
//! and `handle_variables`.

use super::register_isolate;
use crate::adapter::test_helpers::*;
use crate::adapter::*;

// ── stackTrace tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_stack_trace_unknown_thread_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(StackMockBackend);
    let req = crate::DapRequest {
        seq: 1,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": 999 })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Unknown thread ID"),
        "Expected unknown thread error, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_stack_trace_no_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(StackMockBackend);
    let req = crate::DapRequest {
        seq: 1,
        command: "stackTrace".into(),
        arguments: None,
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
}

#[tokio::test]
async fn test_stack_trace_returns_all_frames() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;

    assert!(
        resp.success,
        "stackTrace should succeed: {:?}",
        resp.message
    );
    let body = resp.body.unwrap();
    let frames = body["stackFrames"].as_array().unwrap();
    // StackMockBackend returns 3 frames.
    assert_eq!(frames.len(), 3);
    assert_eq!(body["totalFrames"], 3);
}

#[tokio::test]
async fn test_stack_trace_frame_ids_are_unique_and_monotonic() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let frames = resp.body.unwrap()["stackFrames"]
        .as_array()
        .unwrap()
        .clone();

    let ids: Vec<i64> = frames.iter().map(|f| f["id"].as_i64().unwrap()).collect();
    // IDs are monotonically increasing starting at 1.
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(id, (i as i64) + 1, "Frame IDs must be monotonic from 1");
    }
    // All IDs are unique.
    let mut deduped = ids.clone();
    deduped.dedup();
    assert_eq!(deduped.len(), ids.len(), "Frame IDs must be unique");
}

#[tokio::test]
async fn test_stack_trace_user_code_has_path_and_no_hint() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let frames = resp.body.unwrap()["stackFrames"]
        .as_array()
        .unwrap()
        .clone();

    // Frame 0 is "main" — user code at file:///app/lib/main.dart.
    let frame0 = &frames[0];
    assert_eq!(frame0["name"], "main");
    assert_eq!(frame0["line"], 42);
    assert_eq!(frame0["column"], 5);
    assert_eq!(frame0["source"]["path"], "/app/lib/main.dart");
    // User code: no presentation hint.
    assert!(
        frame0["source"].get("presentationHint").is_none()
            || frame0["source"]["presentationHint"].is_null(),
        "User code should have no presentation hint"
    );
}

#[tokio::test]
async fn test_stack_trace_flutter_framework_frame_deemphasized() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let frames = resp.body.unwrap()["stackFrames"]
        .as_array()
        .unwrap()
        .clone();

    // Frame 1 is "runApp" — Flutter framework source.
    let frame1 = &frames[1];
    assert_eq!(frame1["name"], "runApp");
    assert_eq!(
        frame1["source"]["presentationHint"], "deemphasize",
        "Flutter framework frames should be de-emphasized"
    );
}

#[tokio::test]
async fn test_stack_trace_async_suspension_marker_frame() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let frames = resp.body.unwrap()["stackFrames"]
        .as_array()
        .unwrap()
        .clone();

    // Frame 2 is an async gap marker.
    let frame2 = &frames[2];
    assert_eq!(frame2["name"], "<asynchronous gap>");
    assert_eq!(
        frame2["presentationHint"], "label",
        "AsyncSuspensionMarker must have presentation_hint: label"
    );
}

#[tokio::test]
async fn test_stack_trace_start_frame_offsets_results() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Request frames starting at index 1 (skip frame 0).
    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({
            "threadId": thread_id,
            "startFrame": 1,
        })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let body = resp.body.unwrap();
    let frames = body["stackFrames"].as_array().unwrap();
    // 3 total frames, skip 1 → 2 returned.
    assert_eq!(frames.len(), 2, "startFrame=1 should skip the first frame");
    // Total is still the full count.
    assert_eq!(body["totalFrames"], 3);
    // First returned frame should now be the flutter framework frame.
    assert_eq!(frames[0]["name"], "runApp");
}

#[tokio::test]
async fn test_stack_trace_frame_ids_stored_in_frame_store() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let frames = resp.body.unwrap()["stackFrames"]
        .as_array()
        .unwrap()
        .clone();

    // Every frame ID returned should be lookupable in the frame_store.
    for frame in &frames {
        let id = frame["id"].as_i64().unwrap();
        assert!(
            adapter.frame_store.lookup(id).is_some(),
            "Frame ID {} should be in frame_store",
            id
        );
    }
}

#[tokio::test]
async fn test_stack_trace_empty_frames_returns_success() {
    // MockBackend returns {} with no "frames" key — should succeed with 0 frames.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    let body = resp.body.unwrap();
    let frames = body["stackFrames"].as_array().unwrap();
    assert!(frames.is_empty());
    assert_eq!(body["totalFrames"], 0);
}

// ── scopes tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_scopes_no_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = crate::DapRequest {
        seq: 1,
        command: "scopes".into(),
        arguments: None,
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
}

#[tokio::test]
async fn test_scopes_invalid_frame_id_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = crate::DapRequest {
        seq: 1,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": 999 })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Invalid frame ID"),
        "Expected invalid frame error, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_scopes_returns_locals_and_globals() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // First get a frame ID via stackTrace.
    let stack_req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    assert!(stack_resp.success);
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // Now request scopes for that frame.
    let scopes_req = crate::DapRequest {
        seq: 3,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let resp = adapter.handle_request(&scopes_req).await;
    assert!(resp.success, "scopes should succeed: {:?}", resp.message);

    let body = resp.body.unwrap();
    let scopes = body["scopes"].as_array().unwrap();
    assert_eq!(scopes.len(), 2, "Should return exactly 2 scopes");

    // First scope: Locals.
    assert_eq!(scopes[0]["name"], "Locals");
    assert_eq!(scopes[0]["presentationHint"], "locals");
    assert_eq!(scopes[0]["expensive"], false);
    let locals_ref = scopes[0]["variablesReference"].as_i64().unwrap();
    assert!(locals_ref > 0, "Locals variablesReference must be positive");

    // Second scope: Globals.
    assert_eq!(scopes[1]["name"], "Globals");
    assert_eq!(scopes[1]["presentationHint"], "globals");
    assert_eq!(scopes[1]["expensive"], true);
    let globals_ref = scopes[1]["variablesReference"].as_i64().unwrap();
    assert!(
        globals_ref > 0,
        "Globals variablesReference must be positive"
    );

    // References must be distinct.
    assert_ne!(
        locals_ref, globals_ref,
        "Locals and Globals must have different variablesReference values"
    );
}

#[tokio::test]
async fn test_scopes_variable_references_stored_in_var_store() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Get a frame ID.
    let stack_req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // Get scopes.
    let scopes_req = crate::DapRequest {
        seq: 3,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let scopes_resp = adapter.handle_request(&scopes_req).await;
    assert!(scopes_resp.success);
    let scopes = scopes_resp.body.unwrap()["scopes"]
        .as_array()
        .unwrap()
        .clone();

    for scope in &scopes {
        let var_ref = scope["variablesReference"].as_i64().unwrap();
        assert!(
            adapter.var_store.lookup(var_ref).is_some(),
            "variablesReference {} should be in var_store",
            var_ref
        );
    }
}

#[tokio::test]
async fn test_scopes_locals_scope_has_correct_var_ref_kind() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let stack_req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_req = crate::DapRequest {
        seq: 3,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let scopes_resp = adapter.handle_request(&scopes_req).await;
    let scopes = scopes_resp.body.unwrap()["scopes"]
        .as_array()
        .unwrap()
        .clone();

    let locals_ref = scopes[0]["variablesReference"].as_i64().unwrap();
    let var_ref = adapter.var_store.lookup(locals_ref).unwrap();
    assert!(
        matches!(
            var_ref,
            VariableRef::Scope {
                scope_kind: ScopeKind::Locals,
                ..
            }
        ),
        "Locals scope should store a VariableRef::Scope(Locals)"
    );

    let globals_ref = scopes[1]["variablesReference"].as_i64().unwrap();
    let var_ref = adapter.var_store.lookup(globals_ref).unwrap();
    assert!(
        matches!(
            var_ref,
            VariableRef::Scope {
                scope_kind: ScopeKind::Globals,
                ..
            }
        ),
        "Globals scope should store a VariableRef::Scope(Globals)"
    );
}

#[tokio::test]
async fn test_scopes_stale_frame_id_after_resume_returns_error() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Get a frame ID while stopped.
    let stack_req = crate::DapRequest {
        seq: 2,
        command: "stackTrace".into(),
        arguments: Some(serde_json::json!({ "threadId": thread_id })),
    };
    let stack_resp = adapter.handle_request(&stack_req).await;
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // Simulate a resume — invalidates all frame IDs.
    adapter.on_resume();

    // The previously valid frame ID should now be stale.
    let scopes_req = crate::DapRequest {
        seq: 3,
        command: "scopes".into(),
        arguments: Some(serde_json::json!({ "frameId": frame_id })),
    };
    let resp = adapter.handle_request(&scopes_req).await;
    assert!(
        !resp.success,
        "Stale frame ID after resume should return error"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Invalid frame ID"),
        "Error should mention invalid frame ID, got: {:?}",
        msg
    );
}

// ── instance_ref_to_variable (unit tests) ─────────────────────────────

#[test]
fn test_primitive_null_no_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable("x", &serde_json::json!({"kind": "Null"}), "i/1");
    assert_eq!(var.value, "null");
    assert_eq!(var.variables_reference, 0);
    assert_eq!(var.type_field.as_deref(), Some("Null"));
}

#[test]
fn test_primitive_bool_no_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "flag",
        &serde_json::json!({"kind": "Bool", "valueAsString": "true"}),
        "i/1",
    );
    assert_eq!(var.value, "true");
    assert_eq!(var.variables_reference, 0);
    assert_eq!(var.type_field.as_deref(), Some("bool"));
}

#[test]
fn test_primitive_int_no_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "n",
        &serde_json::json!({"kind": "Int", "valueAsString": "42"}),
        "i/1",
    );
    assert_eq!(var.value, "42");
    assert_eq!(var.variables_reference, 0);
    assert_eq!(var.type_field.as_deref(), Some("int"));
}

#[test]
fn test_primitive_double_no_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "x",
        &serde_json::json!({"kind": "Double", "valueAsString": "3.14"}),
        "i/1",
    );
    assert_eq!(var.value, "3.14");
    assert_eq!(var.variables_reference, 0);
    assert_eq!(var.type_field.as_deref(), Some("double"));
}

#[test]
fn test_string_quoted() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "name",
        &serde_json::json!({"kind": "String", "valueAsString": "hello"}),
        "i/1",
    );
    assert_eq!(var.value, "\"hello\"");
    assert_eq!(var.variables_reference, 0);
    assert_eq!(var.type_field.as_deref(), Some("String"));
}

#[test]
fn test_string_empty_value_as_string_produces_empty_quotes() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable("s", &serde_json::json!({"kind": "String"}), "i/1");
    assert_eq!(var.value, "\"\"");
    assert_eq!(var.variables_reference, 0);
}

#[test]
fn test_list_shows_length_and_is_expandable() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "items",
        &serde_json::json!({
            "kind": "List", "length": 3, "id": "objects/1",
            "class": {"name": "List"}
        }),
        "i/1",
    );
    assert!(
        var.value.contains("length: 3"),
        "Expected 'length: 3' in value, got: {:?}",
        var.value
    );
    assert!(
        var.variables_reference > 0,
        "List must have a positive variablesReference"
    );
    assert_eq!(var.indexed_variables, Some(3));
    assert_eq!(var.type_field.as_deref(), Some("List"));
}

#[test]
fn test_list_without_id_has_zero_ref() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "items",
        &serde_json::json!({"kind": "List", "length": 2}),
        "i/1",
    );
    assert_eq!(var.variables_reference, 0);
}

#[test]
fn test_plain_instance_expandable() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "widget",
        &serde_json::json!({
            "kind": "PlainInstance", "id": "objects/2",
            "class": {"name": "Container"}
        }),
        "i/1",
    );
    assert!(
        var.value.contains("Container"),
        "Expected 'Container' in value, got: {:?}",
        var.value
    );
    assert!(
        var.variables_reference > 0,
        "PlainInstance must have a positive variablesReference"
    );
    assert_eq!(var.type_field.as_deref(), Some("Container"));
}

#[test]
fn test_plain_instance_without_class_uses_kind() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "closure",
        &serde_json::json!({"kind": "Closure", "id": "objects/3"}),
        "i/1",
    );
    assert_eq!(var.type_field.as_deref(), Some("Closure"));
    assert!(var.variables_reference > 0);
}

#[test]
fn test_fallback_unknown_kind_no_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "mystery",
        &serde_json::json!({"kind": "FutureSomething", "valueAsString": "future"}),
        "i/1",
    );
    assert_eq!(var.value, "future");
    assert_eq!(var.variables_reference, 0);
}

#[test]
fn test_each_collection_type_is_expandable() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    for kind in &[
        "Map",
        "Set",
        "Uint8List",
        "Uint8ClampedList",
        "Int32List",
        "Float64List",
    ] {
        let var = adapter.instance_ref_to_variable(
            "col",
            &serde_json::json!({"kind": kind, "id": "objects/col", "length": 0}),
            "i/1",
        );
        assert!(
            var.variables_reference > 0,
            "Collection kind '{}' should be expandable",
            kind
        );
    }
}

#[test]
fn test_var_store_grows_for_each_expandable_instance() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    assert!(adapter.var_store.is_empty());

    adapter.instance_ref_to_variable(
        "a",
        &serde_json::json!({"kind": "PlainInstance", "id": "objects/1"}),
        "i/1",
    );
    adapter.instance_ref_to_variable(
        "b",
        &serde_json::json!({"kind": "List", "id": "objects/2", "length": 0}),
        "i/1",
    );
    assert_eq!(adapter.var_store.len(), 2);
}

// ── handle_variables dispatch tests ───────────────────────────────────

#[tokio::test]
async fn test_variables_stale_reference_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": 9999 })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("9999"),
        "Error should mention the invalid reference, got: {:?}",
        msg
    );
}

#[tokio::test]
async fn test_variables_no_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: None,
    };
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
}

#[tokio::test]
async fn test_variables_globals_scope_returns_empty_list() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var_ref = adapter.var_store.allocate(VariableRef::Scope {
        frame_index: 0,
        scope_kind: ScopeKind::Globals,
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "Globals scope should succeed with empty list: {:?}",
        resp.message
    );
    let body = resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert!(
        vars.is_empty(),
        "Globals should return empty list in Phase 3"
    );
}

#[tokio::test]
async fn test_variables_locals_scope_returns_frame_vars() {
    let (mut adapter, mut rx) = DapAdapter::new(VarMockBackend);
    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // 1. Get the stack to populate the frame store.
    let stack_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        })
        .await;
    assert!(stack_resp.success);
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // 2. Get scopes to get the locals variable reference.
    let scopes_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    assert!(scopes_resp.success);
    let locals_ref = scopes_resp.body.unwrap()["scopes"][0]["variablesReference"]
        .as_i64()
        .unwrap();

    // 3. Request variables for the locals scope.
    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 4,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": locals_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "Variables for locals should succeed: {:?}",
        vars_resp.message
    );

    let body = vars_resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    // VarMockBackend returns 2 variables: "count" (Int) and "label" (String).
    assert_eq!(vars.len(), 2, "Expected 2 local variables");

    let count_var = &vars[0];
    assert_eq!(count_var["name"], "count");
    assert_eq!(count_var["value"], "42");
    assert_eq!(count_var["variablesReference"], 0);

    let label_var = &vars[1];
    assert_eq!(label_var["name"], "label");
    assert_eq!(label_var["value"], "\"hello\"");
    assert_eq!(label_var["variablesReference"], 0);
}

#[tokio::test]
async fn test_variables_expand_list_object() {
    let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/list1".into(),
    });

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "Expanding list should succeed: {:?}",
        vars_resp.message
    );

    let body = vars_resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(vars.len(), 2, "Expected 2 list elements");
    assert_eq!(vars[0]["name"], "[0]");
    assert_eq!(vars[0]["value"], "10");
    assert_eq!(vars[1]["name"], "[1]");
    assert_eq!(vars[1]["value"], "20");
}

#[tokio::test]
async fn test_variables_expand_map_object() {
    let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/map1".into(),
    });

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "Expanding map should succeed: {:?}",
        vars_resp.message
    );

    let body = vars_resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(vars.len(), 1, "Expected 1 map entry");
    assert_eq!(vars[0]["name"], "[a]");
    assert_eq!(vars[0]["value"], "1");
}

#[tokio::test]
async fn test_variables_expand_instance_fields() {
    let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/inst1".into(),
    });

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "Expanding instance should succeed: {:?}",
        vars_resp.message
    );

    let body = vars_resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(vars.len(), 1, "Expected 1 field");
    assert_eq!(vars[0]["name"], "width");
    assert_eq!(vars[0]["value"], "3.14");
}

#[tokio::test]
async fn test_variables_stale_after_resume() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Scope {
        frame_index: 0,
        scope_kind: ScopeKind::Locals,
    });

    // Simulate resume (invalidates all references).
    adapter.on_resume();

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        !resp.success,
        "Stale reference should return error after resume"
    );
}

#[tokio::test]
async fn test_variables_nested_expansion_allocates_unique_refs() {
    // Each expandable object gets its own unique variablesReference.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);

    let ref_a = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/a".into(),
    });
    let ref_b = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/b".into(),
    });
    assert_ne!(ref_a, ref_b, "Each expansion should get a unique reference");
}

#[tokio::test]
async fn test_variables_list_with_start_offset() {
    let (mut adapter, _rx) = DapAdapter::new(VarMockBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/list1".into(),
    });

    // start=1 → index label should be [1] for the first returned element.
    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref, "start": 1 })),
        })
        .await;
    assert!(vars_resp.success);
    let body = vars_resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(
        vars[0]["name"], "[1]",
        "First element with start=1 should be labeled [1]"
    );
}

#[tokio::test]
async fn test_variables_unknown_object_type_returns_empty() {
    // If getObject returns an unrecognized type, return empty variables list.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    // MockBackend returns {} (no "type" field) for any object.
    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".into(),
        object_id: "objects/unknown".into(),
    });

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "Unknown object type should succeed with empty list"
    );
    let body = vars_resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert!(
        vars.is_empty(),
        "Unknown object type should return empty list"
    );
}

// ── classRef / class dual-path lookup tests ────────────────────────────
//
// These tests verify Bug 1 fix: VmServiceBackend::get_stack() serializes
// through typed Rust structs with #[serde(rename_all = "camelCase")], so
// InstanceRef.class_ref becomes "classRef" in JSON. The old code read
// ".get("class")" which always returned None for typed-stack responses.
// The fix reads "classRef" first, falling back to "class" for raw VM wire.

#[test]
fn test_instance_ref_to_variable_uses_class_ref_camel_case() {
    // Simulate typed Stack serialization (camelCase "classRef"):
    // This is what get_stack() produces when the InstanceRef struct is
    // serialized via serde with #[serde(rename_all = "camelCase")].
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "widget",
        &serde_json::json!({
            "kind": "PlainInstance",
            "classRef": {"name": "MyWidget", "id": "classes/1"},
            "id": "objects/123"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("MyWidget"),
        "Expected 'MyWidget' from classRef, got: {:?}",
        var.type_field
    );
    assert!(
        var.value.contains("MyWidget"),
        "Value should contain class name, got: {:?}",
        var.value
    );
}

#[test]
fn test_instance_ref_to_variable_uses_class_raw_wire() {
    // Simulate raw VM wire format (uses "class", not "classRef"):
    // This is the format returned by get_object() (expand_object path).
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "widget",
        &serde_json::json!({
            "kind": "PlainInstance",
            "class": {"name": "MyWidget", "id": "classes/1"},
            "id": "objects/123"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("MyWidget"),
        "Expected 'MyWidget' from class fallback, got: {:?}",
        var.type_field
    );
}

#[test]
fn test_instance_ref_to_variable_class_ref_takes_priority_over_class() {
    // When both "classRef" and "class" are present, "classRef" wins.
    // This verifies the priority of the dual-path lookup.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "x",
        &serde_json::json!({
            "kind": "PlainInstance",
            "classRef": {"name": "CorrectClass", "id": "classes/correct"},
            "class": {"name": "WrongClass", "id": "classes/wrong"},
            "id": "objects/1"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("CorrectClass"),
        "classRef should take priority over class"
    );
}

#[test]
fn test_list_variable_shows_class_ref_name_in_type() {
    // {"kind": "List", "classRef": {"name": "List<int>"}, "length": 3, "id": "objects/456"}
    // Verify variable.type == "List<int>", not just "List" (the kind fallback).
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "numbers",
        &serde_json::json!({
            "kind": "List",
            "classRef": {"name": "List<int>", "id": "classes/list_int"},
            "length": 3,
            "id": "objects/456"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("List<int>"),
        "List should use classRef name, got: {:?}",
        var.type_field
    );
    assert!(
        var.value.contains("List<int>"),
        "Value should contain class name, got: {:?}",
        var.value
    );
    assert!(
        var.value.contains("length: 3"),
        "Value should show length, got: {:?}",
        var.value
    );
}

#[test]
fn test_list_variable_shows_class_raw_wire_name_in_type() {
    // Same as above but with raw wire "class" field.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "strings",
        &serde_json::json!({
            "kind": "List",
            "class": {"name": "List<String>", "id": "classes/list_str"},
            "length": 2,
            "id": "objects/789"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("List<String>"),
        "List should use class name from raw wire, got: {:?}",
        var.type_field
    );
}

#[test]
fn test_plain_instance_without_either_class_field_falls_back_to_kind() {
    // When neither "classRef" nor "class" is present, fall back to the kind.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "unknown",
        &serde_json::json!({
            "kind": "PlainInstance",
            "id": "objects/99"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("PlainInstance"),
        "Should fall back to kind when no class info, got: {:?}",
        var.type_field
    );
}

#[test]
fn test_map_variable_shows_class_ref_name_in_type() {
    // Map with a parameterized type from classRef.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "dict",
        &serde_json::json!({
            "kind": "Map",
            "classRef": {"name": "_Map<String, int>", "id": "classes/map_si"},
            "length": 5,
            "id": "objects/map1"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("_Map<String, int>"),
        "Map should use classRef name, got: {:?}",
        var.type_field
    );
}

#[test]
fn test_plain_instance_class_ref_used_in_instance_value_display() {
    // Verifies the display value for a PlainInstance uses the class name
    // from classRef, not the kind string.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "counter",
        &serde_json::json!({
            "kind": "PlainInstance",
            "classRef": {"name": "Counter", "id": "classes/counter"},
            "id": "objects/c1"
        }),
        "isolates/1",
    );
    // Without valueAsString, display is "<ClassName> instance"
    assert!(
        var.value.contains("Counter"),
        "PlainInstance value should contain class name, got: {:?}",
        var.value
    );
    assert_eq!(var.type_field.as_deref(), Some("Counter"));
}

#[test]
fn test_closure_uses_class_ref_when_present() {
    // Closures can also have classRef providing a more descriptive type.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let var = adapter.instance_ref_to_variable(
        "fn",
        &serde_json::json!({
            "kind": "Closure",
            "classRef": {"name": "_Closure@12345", "id": "classes/clos"},
            "id": "objects/fn1"
        }),
        "isolates/1",
    );
    assert_eq!(
        var.type_field.as_deref(),
        Some("_Closure@12345"),
        "Closure should use classRef name, got: {:?}",
        var.type_field
    );
}
