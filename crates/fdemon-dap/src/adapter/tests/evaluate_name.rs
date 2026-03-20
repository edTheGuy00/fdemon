//! Tests for `evaluateName` construction on DAP variables.
//!
//! Covers:
//! - Local variables receive their name as `evaluateName`
//! - Exception root receives `"$_threadException"` as `evaluateName`
//! - Global statics receive their field name as `evaluateName`
//! - Object fields receive `parent.fieldName` as `evaluateName`
//! - List elements receive `parent[index]` as `evaluateName`
//! - Map string-keyed entries receive `parent["key"]` as `evaluateName`
//! - Map int-keyed entries receive `parent[n]` as `evaluateName`
//! - Nested fields carry `evaluateName` across expansion levels
//! - Primitive variables (no expansion) still receive `evaluateName`
//! - Variables with no evaluate_name have `evaluateName == None`
//! - `evaluate_name_map` is cleared on resume

use super::register_isolate;
use crate::adapter::test_helpers::*;
use crate::adapter::*;

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends
// ─────────────────────────────────────────────────────────────────────────────

/// Backend with a one-variable locals frame (PlainInstance) and object expansion.
struct EvalNameMockBackend;

impl MockTestBackend for EvalNameMockBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "frames": [
                {
                    "kind": "Regular",
                    "code": { "name": "myFunc" },
                    "location": {
                        "script": { "uri": "file:///app/lib/main.dart" },
                        "line": 10,
                        "column": 1
                    },
                    "vars": [
                        {
                            "name": "myObj",
                            "value": {
                                "type": "InstanceRef",
                                "kind": "PlainInstance",
                                "id": "objects/obj1",
                                "classRef": { "name": "MyClass", "id": "classes/MyClass" }
                            }
                        },
                        {
                            "name": "myList",
                            "value": {
                                "type": "InstanceRef",
                                "kind": "List",
                                "id": "objects/list1",
                                "length": 3
                            }
                        },
                        {
                            "name": "myMap",
                            "value": {
                                "type": "InstanceRef",
                                "kind": "Map",
                                "id": "objects/map1",
                                "length": 2
                            }
                        },
                        {
                            "name": "myInt",
                            "value": {
                                "type": "InstanceRef",
                                "kind": "Int",
                                "valueAsString": "42"
                            }
                        }
                    ]
                }
            ]
        }))
    }

    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/obj1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "fields": [
                    {
                        "name": "width",
                        "value": { "kind": "Double", "valueAsString": "3.14", "id": "objects/f1" }
                    },
                    {
                        "name": "label",
                        "value": { "kind": "String", "valueAsString": "hello", "id": "objects/f2" }
                    }
                ]
            })),
            "objects/list1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "List",
                "elements": [
                    { "kind": "Int", "valueAsString": "10", "id": "objects/e0" },
                    { "kind": "Int", "valueAsString": "20", "id": "objects/e1" },
                    { "kind": "Int", "valueAsString": "30", "id": "objects/e2" }
                ]
            })),
            "objects/map1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "Map",
                "associations": [
                    {
                        "key": { "kind": "String", "valueAsString": "alpha" },
                        "value": { "kind": "Int", "valueAsString": "1", "id": "objects/mv0" }
                    },
                    {
                        "key": { "kind": "Int", "valueAsString": "99" },
                        "value": { "kind": "Int", "valueAsString": "2", "id": "objects/mv1" }
                    }
                ]
            })),
            "objects/nested_obj" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "fields": [
                    {
                        "name": "value",
                        "value": { "kind": "Int", "valueAsString": "7", "id": "objects/inner_v" }
                    }
                ]
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Register an isolate, pause it at a breakpoint, call stackTrace + scopes +
/// variables for Locals scope, and return the variable list as JSON.
async fn get_local_variables(
    adapter: &mut DapAdapter<impl DebugBackend>,
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
    isolate_id: &str,
) -> Vec<serde_json::Value> {
    let thread_id = register_isolate(adapter, rx, isolate_id).await;

    // Trigger a Paused event so scopes are available.
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: isolate_id.into(),
            reason: PauseReason::Breakpoint,
            breakpoint_id: None,
            exception: None,
        })
        .await;
    rx.try_recv().ok(); // drain stopped event

    // stackTrace
    let st_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        })
        .await;
    let frame_id = st_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // scopes
    let sc_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 2,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    let scopes = sc_resp.body.unwrap()["scopes"].as_array().unwrap().clone();
    let locals_ref = scopes.iter().find(|s| s["name"] == "Locals").unwrap()["variablesReference"]
        .as_i64()
        .unwrap();

    // variables
    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 3,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": locals_ref })),
        })
        .await;
    vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone()
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: Local variables receive their name as evaluateName
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_local_variable_evaluate_name_set_to_var_name() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    // Find myObj (PlainInstance)
    let my_obj = vars
        .iter()
        .find(|v| v["name"] == "myObj")
        .expect("myObj missing");
    assert_eq!(
        my_obj.get("evaluateName").and_then(|v| v.as_str()),
        Some("myObj"),
        "myObj should have evaluateName == 'myObj'"
    );
}

#[tokio::test]
async fn test_local_primitive_variable_has_evaluate_name() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    let my_int = vars
        .iter()
        .find(|v| v["name"] == "myInt")
        .expect("myInt missing");
    assert_eq!(
        my_int.get("evaluateName").and_then(|v| v.as_str()),
        Some("myInt"),
        "Primitive local variables should have evaluateName set"
    );
}

#[tokio::test]
async fn test_local_list_variable_has_evaluate_name() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    let my_list = vars
        .iter()
        .find(|v| v["name"] == "myList")
        .expect("myList missing");
    assert_eq!(
        my_list.get("evaluateName").and_then(|v| v.as_str()),
        Some("myList"),
        "List local variable should have evaluateName set"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: Object fields receive parent.fieldName
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_field_evaluate_name_is_parent_dot_field() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    // Get the variablesReference for myObj and store the evaluate_name.
    let my_obj = vars
        .iter()
        .find(|v| v["name"] == "myObj")
        .expect("myObj missing");
    let obj_ref = my_obj["variablesReference"].as_i64().unwrap();
    assert_ne!(obj_ref, 0, "myObj should be expandable");

    // Store evaluate_name in the map so expand_object can use it.
    // (In the full flow this is done automatically — we verify here that the
    // evaluate_name_map was populated when the local was fetched.)
    assert!(
        adapter.evaluate_name_map.contains_key(&obj_ref),
        "evaluate_name_map should contain myObj's var_ref after fetching locals"
    );
    assert_eq!(
        adapter.evaluate_name_map.get(&obj_ref).map(|s| s.as_str()),
        Some("myObj"),
        "evaluate_name_map[myObj_ref] should be 'myObj'"
    );

    // Expand myObj — fields should have evaluateName 'myObj.width', 'myObj.label'.
    let fields_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 10,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": obj_ref })),
        })
        .await;
    assert!(fields_resp.success, "Expanding myObj should succeed");

    let fields = fields_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();

    let width = fields
        .iter()
        .find(|v| v["name"] == "width")
        .expect("width missing");
    assert_eq!(
        width.get("evaluateName").and_then(|v| v.as_str()),
        Some("myObj.width"),
        "width field should have evaluateName 'myObj.width'"
    );

    let label = fields
        .iter()
        .find(|v| v["name"] == "label")
        .expect("label missing");
    assert_eq!(
        label.get("evaluateName").and_then(|v| v.as_str()),
        Some("myObj.label"),
        "label field should have evaluateName 'myObj.label'"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: List elements receive parent[index]
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_element_evaluate_name_is_parent_bracket_index() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    let my_list = vars
        .iter()
        .find(|v| v["name"] == "myList")
        .expect("myList missing");
    let list_ref = my_list["variablesReference"].as_i64().unwrap();
    assert_ne!(list_ref, 0, "myList should be expandable");

    // Expand list.
    let elems_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 10,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": list_ref })),
        })
        .await;
    assert!(elems_resp.success, "Expanding myList should succeed");

    let elems = elems_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(elems.len(), 3);

    // Element [0] should have evaluateName "myList[0]"
    assert_eq!(
        elems[0].get("evaluateName").and_then(|v| v.as_str()),
        Some("myList[0]"),
        "Element [0] should have evaluateName 'myList[0]'"
    );
    // Element [1] should have evaluateName "myList[1]"
    assert_eq!(
        elems[1].get("evaluateName").and_then(|v| v.as_str()),
        Some("myList[1]"),
        "Element [1] should have evaluateName 'myList[1]'"
    );
    // Element [2] should have evaluateName "myList[2]"
    assert_eq!(
        elems[2].get("evaluateName").and_then(|v| v.as_str()),
        Some("myList[2]"),
        "Element [2] should have evaluateName 'myList[2]'"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4: Map entries — string key uses parent["key"], int key uses parent[n]
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_map_string_key_evaluate_name() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    let my_map = vars
        .iter()
        .find(|v| v["name"] == "myMap")
        .expect("myMap missing");
    let map_ref = my_map["variablesReference"].as_i64().unwrap();

    let map_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 10,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": map_ref })),
        })
        .await;
    assert!(map_resp.success, "Expanding myMap should succeed");

    let entries = map_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(entries.len(), 2);

    // String key "alpha" → evaluateName should be 'myMap["alpha"]'
    let string_entry = entries
        .iter()
        .find(|e| e["name"] == "[alpha]")
        .expect("[alpha] entry missing");
    assert_eq!(
        string_entry.get("evaluateName").and_then(|v| v.as_str()),
        Some("myMap[\"alpha\"]"),
        "String-keyed map entry should have evaluateName 'myMap[\"alpha\"]'"
    );
}

#[tokio::test]
async fn test_map_int_key_evaluate_name() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    let my_map = vars
        .iter()
        .find(|v| v["name"] == "myMap")
        .expect("myMap missing");
    let map_ref = my_map["variablesReference"].as_i64().unwrap();

    let map_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 10,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": map_ref })),
        })
        .await;

    let entries = map_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();

    // Int key 99 → evaluateName should be 'myMap[99]'
    let int_entry = entries
        .iter()
        .find(|e| e["name"] == "[99]")
        .expect("[99] entry missing");
    assert_eq!(
        int_entry.get("evaluateName").and_then(|v| v.as_str()),
        Some("myMap[99]"),
        "Int-keyed map entry should have evaluateName 'myMap[99]'"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5: No evaluateName when variable ref is allocated without one
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_no_evaluate_name_when_none_passed() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Int",
        "valueAsString": "5"
    });
    // instance_ref_to_variable (3-arg) delegates with None.
    let var = adapter.instance_ref_to_variable("x", &instance_ref, "isolates/1");
    assert_eq!(
        var.evaluate_name, None,
        "evaluate_name should be None when not provided"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 6: Exception root has evaluateName "$_threadException"
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_exception_root_evaluate_name_is_thread_exception() {
    let (mut adapter, mut rx) = DapAdapter::new(StackMockBackend);

    let thread_id = register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Pause at an exception.
    let exc_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "PlainInstance",
        "id": "objects/exc1",
        "classRef": { "name": "StateError", "id": "classes/StateError" }
    });
    adapter
        .handle_debug_event(DebugEvent::Paused {
            isolate_id: "isolates/1".into(),
            reason: PauseReason::Exception,
            breakpoint_id: None,
            exception: Some(exc_ref),
        })
        .await;
    rx.try_recv().ok();

    // stackTrace
    let st_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 1,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        })
        .await;
    let frame_id = st_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    // scopes — find the Exceptions scope.
    let sc_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 2,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    let scopes = sc_resp.body.unwrap()["scopes"].as_array().unwrap().clone();
    let exc_scope = scopes
        .iter()
        .find(|s| s["name"] == "Exceptions")
        .expect("Exceptions scope missing");
    let exc_ref_id = exc_scope["variablesReference"].as_i64().unwrap();

    // variables for Exceptions scope.
    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 3,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": exc_ref_id })),
        })
        .await;
    assert!(vars_resp.success);

    let vars = vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(vars.len(), 1, "Exceptions scope should have 1 variable");

    let exc_var = &vars[0];
    assert_eq!(
        exc_var.get("evaluateName").and_then(|v| v.as_str()),
        Some("$_threadException"),
        "Exception root should have evaluateName '$_threadException'"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 7: evaluate_name_map is cleared on resume
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_evaluate_name_map_cleared_on_resume() {
    let (mut adapter, mut rx) = DapAdapter::new(EvalNameMockBackend);
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;

    // There should be some entries in the evaluate_name_map after fetching locals.
    let has_expandable = vars
        .iter()
        .any(|v| v["variablesReference"].as_i64().unwrap_or(0) != 0);
    assert!(
        has_expandable,
        "Should have at least one expandable variable"
    );
    assert!(
        !adapter.evaluate_name_map.is_empty(),
        "evaluate_name_map should be populated after fetching locals"
    );

    // Resume should clear the map.
    adapter.on_resume();
    assert!(
        adapter.evaluate_name_map.is_empty(),
        "evaluate_name_map should be empty after on_resume()"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 8: Nested field has correct nested evaluateName (obj.field.nested)
// ─────────────────────────────────────────────────────────────────────────────

/// Backend that returns a nested object structure: myObj.inner (PlainInstance)
/// which itself has a field `value`.
struct NestedEvalNameBackend;

impl MockTestBackend for NestedEvalNameBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "frames": [{
                "kind": "Regular",
                "code": { "name": "test" },
                "location": {
                    "script": { "uri": "file:///app/lib/main.dart" },
                    "line": 1,
                    "column": 1
                },
                "vars": [{
                    "name": "outer",
                    "value": {
                        "type": "InstanceRef",
                        "kind": "PlainInstance",
                        "id": "objects/outer",
                        "classRef": { "name": "Outer", "id": "classes/Outer" }
                    }
                }]
            }]
        }))
    }

    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/outer" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "fields": [{
                    "name": "inner",
                    "value": {
                        "kind": "PlainInstance",
                        "id": "objects/inner",
                        "classRef": { "name": "Inner", "id": "classes/Inner" }
                    }
                }]
            })),
            "objects/inner" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "fields": [{
                    "name": "value",
                    "value": { "kind": "Int", "valueAsString": "42" }
                }]
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }
}

#[tokio::test]
async fn test_nested_field_evaluate_name_chained() {
    let (mut adapter, mut rx) = DapAdapter::new(NestedEvalNameBackend);

    // Get locals: outer → evaluateName "outer"
    let vars = get_local_variables(&mut adapter, &mut rx, "isolates/1").await;
    let outer = vars
        .iter()
        .find(|v| v["name"] == "outer")
        .expect("outer missing");
    let outer_ref = outer["variablesReference"].as_i64().unwrap();
    assert_ne!(outer_ref, 0, "outer should be expandable");
    assert_eq!(
        outer.get("evaluateName").and_then(|v| v.as_str()),
        Some("outer"),
        "outer should have evaluateName 'outer'"
    );

    // Expand outer → inner field should have evaluateName "outer.inner"
    let outer_fields_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 10,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": outer_ref })),
        })
        .await;
    assert!(outer_fields_resp.success);
    let outer_fields = outer_fields_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    let inner_field = outer_fields
        .iter()
        .find(|v| v["name"] == "inner")
        .expect("inner field missing");
    assert_eq!(
        inner_field.get("evaluateName").and_then(|v| v.as_str()),
        Some("outer.inner"),
        "inner field should have evaluateName 'outer.inner'"
    );

    // Expand inner → value field should have evaluateName "outer.inner.value"
    let inner_ref = inner_field["variablesReference"].as_i64().unwrap();
    assert_ne!(inner_ref, 0, "inner should be expandable");

    let inner_fields_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 11,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": inner_ref })),
        })
        .await;
    assert!(inner_fields_resp.success);
    let inner_fields = inner_fields_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone();
    let value_field = inner_fields
        .iter()
        .find(|v| v["name"] == "value")
        .expect("value field missing");
    assert_eq!(
        value_field.get("evaluateName").and_then(|v| v.as_str()),
        Some("outer.inner.value"),
        "Nested field should have chained evaluateName 'outer.inner.value'"
    );
}
