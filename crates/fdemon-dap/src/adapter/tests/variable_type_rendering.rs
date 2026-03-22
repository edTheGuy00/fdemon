//! Tests for variable type rendering improvements.
//!
//! Covers:
//! - String truncation indicator (`valueAsStringIsTruncated`)
//! - Record type display and expansion
//! - WeakReference display and expansion
//! - Sentinel display (optimized-out variables)
//! - Set expansion using the `elements` array (not `fields`)
//! - TypeArguments filtering from field expansion
//! - `is_expandable` for Record and WeakReference

use crate::adapter::evaluate::is_expandable;
use crate::adapter::test_helpers::*;
use crate::adapter::*;

// ─────────────────────────────────────────────────────────────────────────────
// String truncation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_string_truncation_indicator_shows_ellipsis() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "String",
        "valueAsString": "hello world",
        "valueAsStringIsTruncated": true,
        "id": "objects/str1"
    });
    let var = adapter.instance_ref_to_variable("msg", &instance_ref, "isolates/1");
    assert!(
        var.value.contains("..."),
        "Truncated string should contain '...', got: {:?}",
        var.value
    );
    assert!(
        var.value.starts_with('"'),
        "String value should be quoted, got: {:?}",
        var.value
    );
}

#[test]
fn test_string_truncation_indicator_has_var_ref_for_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "String",
        "valueAsString": "hello world",
        "valueAsStringIsTruncated": true,
        "id": "objects/str1"
    });
    let var = adapter.instance_ref_to_variable("msg", &instance_ref, "isolates/1");
    assert!(
        var.variables_reference != 0,
        "Truncated string should have a non-zero variables_reference for expansion"
    );
}

#[test]
fn test_string_not_truncated_has_no_var_ref() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "String",
        "valueAsString": "hello",
        "valueAsStringIsTruncated": false,
        "id": "objects/str2"
    });
    let var = adapter.instance_ref_to_variable("msg", &instance_ref, "isolates/1");
    assert_eq!(
        var.variables_reference, 0,
        "Non-truncated string should have variables_reference == 0"
    );
    assert!(
        !var.value.contains("..."),
        "Non-truncated string should not contain '...'"
    );
    assert_eq!(var.value, "\"hello\"");
}

#[test]
fn test_string_truncated_without_id_has_no_var_ref() {
    // No object ID means we can't expand — var_ref must be 0.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "String",
        "valueAsString": "hello world",
        "valueAsStringIsTruncated": true
        // No "id" key
    });
    let var = adapter.instance_ref_to_variable("msg", &instance_ref, "isolates/1");
    assert_eq!(
        var.variables_reference, 0,
        "Truncated string without object ID should still have variables_reference == 0"
    );
    assert!(
        var.value.contains("..."),
        "Truncated string should still show ellipsis"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Record type display
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_record_type_display_shows_field_count() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Record",
        "length": 3,
        "id": "objects/rec1"
    });
    let var = adapter.instance_ref_to_variable("r", &instance_ref, "isolates/1");
    assert_eq!(var.value, "Record (3 fields)");
    assert_eq!(var.type_field.as_deref(), Some("Record"));
}

#[test]
fn test_record_type_is_expandable_with_id() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Record",
        "length": 2,
        "id": "objects/rec2"
    });
    let var = adapter.instance_ref_to_variable("r", &instance_ref, "isolates/1");
    assert!(
        var.variables_reference != 0,
        "Record with object ID should be expandable"
    );
}

#[test]
fn test_record_type_zero_fields() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Record",
        "length": 0,
        "id": "objects/rec3"
    });
    let var = adapter.instance_ref_to_variable("r", &instance_ref, "isolates/1");
    assert_eq!(var.value, "Record (0 fields)");
}

#[test]
fn test_is_expandable_record() {
    assert!(is_expandable(&serde_json::json!({"kind": "Record"})));
}

// ─────────────────────────────────────────────────────────────────────────────
// Record expansion via expand_object
// ─────────────────────────────────────────────────────────────────────────────

struct RecordExpandBackend;

impl MockTestBackend for RecordExpandBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        if object_id == "objects/rec1" {
            Ok(serde_json::json!({
                "type": "Instance",
                "kind": "Record",
                "fields": [
                    { "name": "$1", "value": { "kind": "Int", "valueAsString": "10", "id": "obj/f1" } },
                    { "name": "$2", "value": { "kind": "String", "valueAsString": "hello", "id": "obj/f2" } },
                    { "name": "label", "value": { "kind": "Bool", "valueAsString": "true", "id": "obj/f3" } }
                ]
            }))
        } else {
            Ok(serde_json::json!({ "type": "Instance", "kind": "Null" }))
        }
    }

    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "frames": [] }))
    }
}

#[tokio::test]
async fn test_record_expansion_shows_positional_and_named_fields() {
    let (mut adapter, _rx) = DapAdapter::new(RecordExpandBackend);

    // Allocate an Object variable reference for the record.
    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".to_string(),
        object_id: "objects/rec1".to_string(),
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "Record expand should succeed: {:?}",
        resp.message
    );

    let body = resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(vars.len(), 3, "Record should expand to 3 fields");
    assert_eq!(vars[0]["name"], "$1");
    assert_eq!(vars[0]["value"], "10");
    assert_eq!(vars[1]["name"], "$2");
    assert_eq!(vars[1]["value"], "\"hello\"");
    assert_eq!(vars[2]["name"], "label");
    assert_eq!(vars[2]["value"], "true");
}

// ─────────────────────────────────────────────────────────────────────────────
// WeakReference type display
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_weak_reference_display() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "WeakReference",
        "id": "objects/wr1"
    });
    let var = adapter.instance_ref_to_variable("wr", &instance_ref, "isolates/1");
    assert_eq!(var.value, "WeakReference");
    assert_eq!(var.type_field.as_deref(), Some("WeakReference"));
}

#[test]
fn test_weak_reference_is_expandable() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "WeakReference",
        "id": "objects/wr1"
    });
    let var = adapter.instance_ref_to_variable("wr", &instance_ref, "isolates/1");
    assert!(
        var.variables_reference != 0,
        "WeakReference with object ID should be expandable"
    );
}

#[test]
fn test_is_expandable_weak_reference() {
    assert!(is_expandable(&serde_json::json!({"kind": "WeakReference"})));
}

// ─────────────────────────────────────────────────────────────────────────────
// WeakReference expansion via expand_object
// ─────────────────────────────────────────────────────────────────────────────

struct WeakRefExpandBackend {
    target_alive: bool,
}

impl MockTestBackend for WeakRefExpandBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        if object_id == "objects/wr1" {
            let target = if self.target_alive {
                serde_json::json!({ "kind": "PlainInstance", "id": "objects/target1", "classRef": { "name": "Foo" } })
            } else {
                serde_json::Value::Null
            };
            Ok(serde_json::json!({
                "type": "Instance",
                "kind": "WeakReference",
                "target": target
            }))
        } else {
            Ok(serde_json::json!({ "type": "Instance", "kind": "Null" }))
        }
    }

    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "frames": [] }))
    }
}

#[tokio::test]
async fn test_weak_reference_expansion_shows_target_when_alive() {
    let (mut adapter, _rx) = DapAdapter::new(WeakRefExpandBackend { target_alive: true });

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".to_string(),
        object_id: "objects/wr1".to_string(),
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "WeakReference expand should succeed: {:?}",
        resp.message
    );

    let body = resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(
        vars.len(),
        1,
        "WeakReference should expand to one 'target' variable"
    );
    assert_eq!(vars[0]["name"], "target");
}

#[tokio::test]
async fn test_weak_reference_expansion_target_null_when_gc_collected() {
    let (mut adapter, _rx) = DapAdapter::new(WeakRefExpandBackend {
        target_alive: false,
    });

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".to_string(),
        object_id: "objects/wr1".to_string(),
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "WeakReference expand should succeed even if target is null"
    );

    let body = resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(vars.len(), 1, "Should still have a 'target' variable");
    assert_eq!(vars[0]["name"], "target");
    // target is null — should display as "null" with no expansion
    assert_eq!(vars[0]["value"], "null");
    assert_eq!(vars[0]["variablesReference"], 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Sentinel display
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_sentinel_displays_value_as_string() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Sentinel",
        "valueAsString": "<collected>"
    });
    let var = adapter.instance_ref_to_variable("x", &instance_ref, "isolates/1");
    assert_eq!(var.value, "<collected>");
    assert_eq!(var.type_field.as_deref(), Some("Sentinel"));
    assert_eq!(var.variables_reference, 0, "Sentinels are not expandable");
}

#[test]
fn test_sentinel_fallback_when_no_value_as_string() {
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Sentinel"
        // No valueAsString
    });
    let var = adapter.instance_ref_to_variable("x", &instance_ref, "isolates/1");
    assert_eq!(
        var.value, "<optimized out>",
        "Missing valueAsString should fall back to '<optimized out>'"
    );
}

#[test]
fn test_sentinel_is_not_expandable() {
    assert!(
        !is_expandable(&serde_json::json!({"kind": "Sentinel"})),
        "Sentinel should not be expandable"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Set expansion
// ─────────────────────────────────────────────────────────────────────────────

struct SetExpandBackend;

impl MockTestBackend for SetExpandBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        if object_id == "objects/set1" {
            Ok(serde_json::json!({
                "type": "Instance",
                "kind": "Set",
                "elements": [
                    { "kind": "Int", "valueAsString": "1", "id": "obj/e0" },
                    { "kind": "Int", "valueAsString": "2", "id": "obj/e1" },
                    { "kind": "Int", "valueAsString": "3", "id": "obj/e2" }
                ]
            }))
        } else {
            Ok(serde_json::json!({ "type": "Instance", "kind": "Null" }))
        }
    }

    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "frames": [] }))
    }
}

#[tokio::test]
async fn test_set_expansion_uses_elements_array() {
    let (mut adapter, _rx) = DapAdapter::new(SetExpandBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".to_string(),
        object_id: "objects/set1".to_string(),
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "Set expansion should succeed: {:?}",
        resp.message
    );

    let body = resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(vars.len(), 3, "Set should expand to 3 indexed elements");
    // Verify indexed names and values.
    assert_eq!(vars[0]["name"], "[0]");
    assert_eq!(vars[0]["value"], "1");
    assert_eq!(vars[1]["name"], "[1]");
    assert_eq!(vars[1]["value"], "2");
    assert_eq!(vars[2]["name"], "[2]");
    assert_eq!(vars[2]["value"], "3");
}

#[test]
fn test_set_display_in_instance_ref_to_variable() {
    // Set is rendered the same as List in instance_ref_to_variable.
    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    let instance_ref = serde_json::json!({
        "type": "InstanceRef",
        "kind": "Set",
        "length": 3,
        "id": "objects/set1",
        "classRef": { "name": "LinkedHashSet<int>" }
    });
    let var = adapter.instance_ref_to_variable("s", &instance_ref, "isolates/1");
    assert!(
        var.value.contains("length: 3"),
        "Set display should include length, got: {:?}",
        var.value
    );
    assert!(var.variables_reference != 0, "Set should be expandable");
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeArguments filtering
// ─────────────────────────────────────────────────────────────────────────────

struct TypeArgsExpandBackend;

impl MockTestBackend for TypeArgsExpandBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        if object_id == "objects/inst1" {
            Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "fields": [
                    // TypeArguments entry — should be filtered out.
                    { "type": "@TypeArguments", "name": "typeArguments", "value": { "kind": "Null" } },
                    // Regular field — should be kept.
                    { "name": "count", "value": { "kind": "Int", "valueAsString": "5", "id": "obj/f1" } }
                ]
            }))
        } else {
            Ok(serde_json::json!({ "type": "Instance", "kind": "Null" }))
        }
    }

    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "frames": [] }))
    }
}

#[tokio::test]
async fn test_type_arguments_fields_are_filtered_from_expansion() {
    let (mut adapter, _rx) = DapAdapter::new(TypeArgsExpandBackend);

    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".to_string(),
        object_id: "objects/inst1".to_string(),
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };
    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "PlainInstance expand should succeed: {:?}",
        resp.message
    );

    let body = resp.body.unwrap();
    let vars = body["variables"].as_array().unwrap();
    assert_eq!(
        vars.len(),
        1,
        "TypeArguments field should be filtered; only 'count' should remain"
    );
    assert_eq!(vars[0]["name"], "count");
    assert_eq!(vars[0]["value"], "5");
}
