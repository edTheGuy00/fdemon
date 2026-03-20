//! Tests for getter evaluation in the variables panel.
//!
//! These tests cover:
//! - Eager getter evaluation (`evaluateGettersInDebugViews == true`, the default)
//! - Lazy getter mode (`evaluateGettersInDebugViews == false`)
//! - Getter error handling
//! - Getter timeout handling
//! - Internal getter filtering
//! - Superclass hierarchy traversal
//! - `GetterEval` variable reference lazy expansion

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::adapter::test_helpers::MockTestBackend;
use crate::adapter::*;

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends
// ─────────────────────────────────────────────────────────────────────────────

/// A `get_object` mock that returns:
/// - `"objects/inst1"`: a PlainInstance with a class reference and one field.
/// - `"classes/person"`: a Class with two Getter functions ("name", "age").
/// - Anything else: empty object.
struct GetterEvalMockBackend {
    /// Map of getter name → return value for `evaluate()`.
    getter_values: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl GetterEvalMockBackend {
    fn new(getter_values: HashMap<String, serde_json::Value>) -> Self {
        Self {
            getter_values: Arc::new(Mutex::new(getter_values)),
        }
    }
}

impl MockTestBackend for GetterEvalMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/person", "name": "Person" },
                "fields": [
                    {
                        "name": "_name",
                        "value": {
                            "kind": "String",
                            "valueAsString": "Alice",
                            "id": "objects/str1"
                        }
                    }
                ]
            })),
            "classes/person" => Ok(serde_json::json!({
                "type": "Class",
                "name": "Person",
                "functions": [
                    { "name": "name", "kind": "ImplicitGetter", "static": false },
                    { "name": "age", "kind": "Getter", "static": false }
                ],
                "super": null
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        let map = self.getter_values.lock().unwrap();
        match map.get(expression) {
            Some(v) => Ok(v.clone()),
            None => Err(BackendError::VmServiceError(format!(
                "no getter '{}' defined",
                expression
            ))),
        }
    }
}

/// Backend that returns an object with internal getters (_identityHashCode,
/// hashCode, runtimeType) plus one user getter ("name").
struct InternalGetterMockBackend;

impl MockTestBackend for InternalGetterMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/widget", "name": "Widget" },
                "fields": []
            })),
            "classes/widget" => Ok(serde_json::json!({
                "type": "Class",
                "name": "Widget",
                "functions": [
                    { "name": "_identityHashCode", "kind": "ImplicitGetter", "static": false },
                    { "name": "hashCode", "kind": "Getter", "static": false },
                    { "name": "runtimeType", "kind": "Getter", "static": false },
                    { "name": "name", "kind": "ImplicitGetter", "static": false }
                ],
                "super": null
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        if expression == "name" {
            Ok(serde_json::json!({ "kind": "String", "valueAsString": "MyWidget" }))
        } else {
            Err(BackendError::VmServiceError(format!(
                "unexpected getter: {}",
                expression
            )))
        }
    }
}

/// Backend that returns an instance with a class that has one failing getter.
struct FailingGetterMockBackend;

impl MockTestBackend for FailingGetterMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/bad", "name": "BadClass" },
                "fields": []
            })),
            "classes/bad" => Ok(serde_json::json!({
                "type": "Class",
                "name": "BadClass",
                "functions": [
                    { "name": "brokenGetter", "kind": "Getter", "static": false }
                ],
                "super": null
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        Err(BackendError::VmServiceError(
            "Getter threw: Null check operator used on a null value".to_string(),
        ))
    }
}

/// Backend whose `evaluate()` hangs indefinitely (to test timeout).
struct SlowGetterMockBackend;

impl MockTestBackend for SlowGetterMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/slow", "name": "SlowClass" },
                "fields": []
            })),
            "classes/slow" => Ok(serde_json::json!({
                "type": "Class",
                "name": "SlowClass",
                "functions": [
                    { "name": "heavyGetter", "kind": "Getter", "static": false }
                ],
                "super": null
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        // Simulate a getter that takes much longer than the 1s timeout.
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok(serde_json::json!({ "kind": "String", "valueAsString": "never" }))
    }
}

/// Backend with a two-level class hierarchy (Person subclasses Animal).
struct HierarchyGetterMockBackend;

impl MockTestBackend for HierarchyGetterMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/person", "name": "Person" },
                "fields": []
            })),
            "classes/person" => Ok(serde_json::json!({
                "type": "Class",
                "name": "Person",
                "functions": [
                    { "name": "name", "kind": "ImplicitGetter", "static": false }
                ],
                "super": { "id": "classes/animal", "name": "Animal" }
            })),
            "classes/animal" => Ok(serde_json::json!({
                "type": "Class",
                "name": "Animal",
                "functions": [
                    { "name": "sound", "kind": "Getter", "static": false }
                ],
                "super": null
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        match expression {
            "name" => Ok(serde_json::json!({ "kind": "String", "valueAsString": "Bob" })),
            "sound" => Ok(serde_json::json!({ "kind": "String", "valueAsString": "Woof" })),
            _ => Err(BackendError::VmServiceError(format!(
                "unknown getter: {}",
                expression
            ))),
        }
    }
}

/// Backend that returns a class with many getters to test the 50-getter limit.
struct ManyGettersMockBackend;

impl MockTestBackend for ManyGettersMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/big", "name": "BigClass" },
                "fields": []
            })),
            "classes/big" => {
                // Build 60 getters — more than the limit of 50.
                let functions: Vec<serde_json::Value> = (0..60)
                    .map(|i| {
                        serde_json::json!({
                            "name": format!("getter{}", i),
                            "kind": "ImplicitGetter",
                            "static": false
                        })
                    })
                    .collect();
                Ok(serde_json::json!({
                    "type": "Class",
                    "name": "BigClass",
                    "functions": functions,
                    "super": null
                }))
            }
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "kind": "Int", "valueAsString": "42" }))
    }
}

/// Backend that returns a non-PlainInstance (List).
struct ListInstanceMockBackend;

impl MockTestBackend for ListInstanceMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/list1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "List",
                "elements": [
                    { "kind": "Int", "valueAsString": "1", "id": "e1" },
                    { "kind": "Int", "valueAsString": "2", "id": "e2" }
                ]
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }
}

/// Backend that returns a static getter (should be excluded).
struct StaticGetterMockBackend;

impl MockTestBackend for StaticGetterMockBackend {
    async fn get_object(
        &self,
        _isolate_id: &str,
        object_id: &str,
        _offset: Option<i64>,
        _count: Option<i64>,
    ) -> Result<serde_json::Value, BackendError> {
        match object_id {
            "objects/inst1" => Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                "class": { "id": "classes/foo", "name": "Foo" },
                "fields": []
            })),
            "classes/foo" => Ok(serde_json::json!({
                "type": "Class",
                "name": "Foo",
                "functions": [
                    // Static getter — should be filtered.
                    { "name": "staticCount", "kind": "Getter", "static": true },
                    // Instance getter — should be included.
                    { "name": "id", "kind": "ImplicitGetter", "static": false }
                ],
                "super": null
            })),
            _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
        }
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        if expression == "id" {
            Ok(serde_json::json!({ "kind": "Int", "valueAsString": "7" }))
        } else {
            Err(BackendError::VmServiceError(format!(
                "unexpected getter call: {}",
                expression
            )))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper to expand an object directly through the variables request pathway.
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: construct a variables reference pointing to `object_id` in
/// `"isolates/1"`, then send a `variables` request and return the resulting
/// variables array.
async fn expand_instance(
    adapter: &mut DapAdapter<impl DebugBackend>,
    object_id: &str,
) -> Vec<serde_json::Value> {
    let var_ref = adapter.var_store.allocate(VariableRef::Object {
        isolate_id: "isolates/1".to_string(),
        object_id: object_id.to_string(),
    });

    let req = crate::DapRequest {
        seq: 1,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": var_ref })),
    };

    let resp = adapter.handle_request(&req).await;
    assert!(
        resp.success,
        "variables request should succeed: {:?}",
        resp.message
    );
    resp.body.unwrap()["variables"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Expanding a PlainInstance returns both fields and eagerly evaluated getters.
#[tokio::test]
async fn test_expand_object_includes_getters() {
    let mut getter_values = HashMap::new();
    getter_values.insert(
        "name".to_string(),
        serde_json::json!({ "kind": "String", "valueAsString": "Alice" }),
    );
    getter_values.insert(
        "age".to_string(),
        serde_json::json!({ "kind": "Int", "valueAsString": "30" }),
    );

    let (mut adapter, _rx) = DapAdapter::new(GetterEvalMockBackend::new(getter_values));
    // Default: evaluate_getters_in_debug_views == true.

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // Should have: 1 field (_name) + 2 getters (name, age)
    assert_eq!(
        vars.len(),
        3,
        "Expected 1 field + 2 getters, got: {:?}",
        vars
    );

    // The field.
    let field = vars.iter().find(|v| v["name"] == "_name");
    assert!(field.is_some(), "Field '_name' not found");
    assert_eq!(field.unwrap()["value"], "\"Alice\"");

    // The "name" getter.
    let name_var = vars.iter().find(|v| v["name"] == "name");
    assert!(name_var.is_some(), "Getter 'name' not found");
    assert_eq!(name_var.unwrap()["value"], "\"Alice\"");

    // The "age" getter.
    let age_var = vars.iter().find(|v| v["name"] == "age");
    assert!(age_var.is_some(), "Getter 'age' not found");
    assert_eq!(age_var.unwrap()["value"], "30");
}

/// Eagerly evaluated getters have `presentationHint.attributes: ["hasSideEffects"]`.
#[tokio::test]
async fn test_eager_getters_have_has_side_effects_attribute() {
    let mut getter_values = HashMap::new();
    getter_values.insert(
        "name".to_string(),
        serde_json::json!({ "kind": "String", "valueAsString": "Bob" }),
    );
    getter_values.insert(
        "age".to_string(),
        serde_json::json!({ "kind": "Int", "valueAsString": "25" }),
    );

    let (mut adapter, _rx) = DapAdapter::new(GetterEvalMockBackend::new(getter_values));

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    for var in &vars {
        // Only check getter variables (not the field "_name").
        let name = var["name"].as_str().unwrap_or("");
        if name == "name" || name == "age" {
            let attrs = &var["presentationHint"]["attributes"];
            assert!(
                attrs
                    .as_array()
                    .map(|a| a.iter().any(|v| v == "hasSideEffects"))
                    .unwrap_or(false),
                "Getter '{}' should have 'hasSideEffects' attribute, got hint: {:?}",
                name,
                var["presentationHint"]
            );
        }
    }
}

/// When a getter evaluation fails, the value is shown as `"<error: message>"`.
#[tokio::test]
async fn test_getter_error_shows_error_string() {
    let (mut adapter, _rx) = DapAdapter::new(FailingGetterMockBackend);

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // One getter: brokenGetter.
    assert_eq!(vars.len(), 1, "Expected 1 getter variable");
    let var = &vars[0];
    assert_eq!(var["name"], "brokenGetter");
    let value = var["value"].as_str().unwrap_or("");
    assert!(
        value.starts_with("<error:"),
        "Expected error string, got: {:?}",
        value
    );
    assert!(
        value.contains("Null check operator"),
        "Error should contain the original message, got: {:?}",
        value
    );
    // No expansion (variablesReference == 0).
    assert_eq!(var["variablesReference"], 0);
}

/// Internal getters (_identityHashCode, hashCode, runtimeType) are filtered out.
#[tokio::test]
async fn test_internal_getters_filtered() {
    let (mut adapter, _rx) = DapAdapter::new(InternalGetterMockBackend);

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // Should only contain "name", not the three internal getters.
    let names: Vec<&str> = vars
        .iter()
        .map(|v| v["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        !names.contains(&"_identityHashCode"),
        "_identityHashCode should be filtered, names: {:?}",
        names
    );
    assert!(
        !names.contains(&"hashCode"),
        "hashCode should be filtered, names: {:?}",
        names
    );
    assert!(
        !names.contains(&"runtimeType"),
        "runtimeType should be filtered, names: {:?}",
        names
    );
    assert!(
        names.contains(&"name"),
        "'name' getter should be present, names: {:?}",
        names
    );
    assert_eq!(vars.len(), 1, "Only 'name' should be present");
}

/// When `evaluateGettersInDebugViews == false`, getters appear as lazy items.
#[tokio::test]
async fn test_lazy_getters_when_setting_false() {
    let mut getter_values = HashMap::new();
    getter_values.insert(
        "name".to_string(),
        serde_json::json!({ "kind": "String", "valueAsString": "Lazy" }),
    );
    getter_values.insert(
        "age".to_string(),
        serde_json::json!({ "kind": "Int", "valueAsString": "99" }),
    );

    let (mut adapter, _rx) = DapAdapter::new(GetterEvalMockBackend::new(getter_values));
    // Disable eager evaluation.
    adapter.evaluate_getters_in_debug_views = false;

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // Should have: 1 field + 2 lazy getters.
    assert_eq!(vars.len(), 3, "Expected 1 field + 2 lazy getters");

    // Getters should have lazy: true in presentationHint.
    for var in &vars {
        let name = var["name"].as_str().unwrap_or("");
        if name == "name" || name == "age" {
            assert_eq!(
                var["presentationHint"]["lazy"],
                serde_json::json!(true),
                "Getter '{}' should have lazy: true, got: {:?}",
                name,
                var["presentationHint"]
            );
            // Value should be empty (not yet evaluated).
            assert_eq!(
                var["value"].as_str().unwrap_or("NOT_EMPTY"),
                "",
                "Lazy getter '{}' should have empty value",
                name
            );
            // Should have a non-zero variablesReference for expansion.
            assert!(
                var["variablesReference"].as_i64().unwrap_or(0) > 0,
                "Lazy getter '{}' should have non-zero variablesReference",
                name
            );
        }
    }
}

/// Lazy getters evaluate on explicit expansion via `GetterEval` reference.
#[tokio::test]
async fn test_lazy_getters_evaluate_on_expansion() {
    let mut getter_values = HashMap::new();
    getter_values.insert(
        "name".to_string(),
        serde_json::json!({ "kind": "String", "valueAsString": "Expanded" }),
    );
    getter_values.insert(
        "age".to_string(),
        serde_json::json!({ "kind": "Int", "valueAsString": "55" }),
    );

    let (mut adapter, _rx) = DapAdapter::new(GetterEvalMockBackend::new(getter_values));
    adapter.evaluate_getters_in_debug_views = false;

    // First, expand the object to get lazy getter variable references.
    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // Find the "name" lazy getter.
    let name_var = vars.iter().find(|v| v["name"] == "name").cloned();
    assert!(name_var.is_some(), "Lazy getter 'name' not found");
    let name_var_ref = name_var.unwrap()["variablesReference"].as_i64().unwrap();
    assert!(name_var_ref > 0, "Lazy getter should have non-zero ref");

    // Now expand the lazy getter reference.
    let expand_req = crate::DapRequest {
        seq: 2,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": name_var_ref })),
    };
    let expand_resp = adapter.handle_request(&expand_req).await;
    assert!(
        expand_resp.success,
        "Expanding lazy getter should succeed: {:?}",
        expand_resp.message
    );

    let expanded_vars = expand_resp.body.unwrap()["variables"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Should return a single variable with the evaluated value.
    assert_eq!(
        expanded_vars.len(),
        1,
        "GetterEval should return exactly one variable"
    );
    assert_eq!(expanded_vars[0]["name"], "name");
    assert_eq!(expanded_vars[0]["value"], "\"Expanded\"");
}

/// Getter evaluation timeout shows `"<timed out>"` after 1 second.
#[tokio::test(flavor = "multi_thread")]
async fn test_getter_timeout_shows_timed_out() {
    let (mut adapter, _rx) = DapAdapter::new(SlowGetterMockBackend);

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // One getter: heavyGetter.
    assert_eq!(vars.len(), 1, "Expected 1 getter variable");
    let var = &vars[0];
    assert_eq!(var["name"], "heavyGetter");
    assert_eq!(
        var["value"].as_str().unwrap_or(""),
        "<timed out>",
        "Timed-out getter should show '<timed out>'"
    );
    assert_eq!(var["variablesReference"], 0);
}

/// Static getters are not included in the expansion results.
#[tokio::test]
async fn test_static_getters_excluded() {
    let (mut adapter, _rx) = DapAdapter::new(StaticGetterMockBackend);

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    let names: Vec<&str> = vars
        .iter()
        .map(|v| v["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        !names.contains(&"staticCount"),
        "Static getter should not be included, names: {:?}",
        names
    );
    assert!(
        names.contains(&"id"),
        "Instance getter 'id' should be included, names: {:?}",
        names
    );
    assert_eq!(vars.len(), 1, "Only 'id' should be present");
}

/// The superclass hierarchy is traversed for getter collection.
#[tokio::test]
async fn test_superclass_getters_are_collected() {
    let (mut adapter, _rx) = DapAdapter::new(HierarchyGetterMockBackend);

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    let names: Vec<&str> = vars
        .iter()
        .map(|v| v["name"].as_str().unwrap_or(""))
        .collect();

    // "name" from Person (direct class).
    assert!(
        names.contains(&"name"),
        "'name' getter from Person should be present, names: {:?}",
        names
    );
    // "sound" from Animal (superclass).
    assert!(
        names.contains(&"sound"),
        "'sound' getter from Animal should be present, names: {:?}",
        names
    );
    assert_eq!(vars.len(), 2, "Should have 2 getters: name + sound");
}

/// At most 50 getters are collected per object, even when the class has more.
#[tokio::test]
async fn test_getter_count_capped_at_50() {
    let (mut adapter, _rx) = DapAdapter::new(ManyGettersMockBackend);

    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // The class has 60 getters, but at most 50 should be included.
    assert!(
        vars.len() <= 50,
        "Should be capped at 50 getters, got: {}",
        vars.len()
    );
    assert!(
        vars.len() >= 50,
        "Should collect exactly 50 getters, got: {}",
        vars.len()
    );
}

/// Non-PlainInstance objects (e.g., List) do not trigger getter evaluation.
#[tokio::test]
async fn test_list_instance_has_no_getters() {
    let (mut adapter, _rx) = DapAdapter::new(ListInstanceMockBackend);

    let vars = expand_instance(&mut adapter, "objects/list1").await;

    // List should have exactly 2 indexed elements, no getters.
    assert_eq!(vars.len(), 2, "List should have 2 elements, no getters");
    assert_eq!(vars[0]["name"], "[0]");
    assert_eq!(vars[1]["name"], "[1]");
}

/// When the object has no class ID, getter collection is skipped gracefully.
#[tokio::test]
async fn test_plain_instance_without_class_id_has_no_getters() {
    struct NoClassIdMockBackend;
    impl MockTestBackend for NoClassIdMockBackend {
        async fn get_object(
            &self,
            _isolate_id: &str,
            _object_id: &str,
            _offset: Option<i64>,
            _count: Option<i64>,
        ) -> Result<serde_json::Value, BackendError> {
            Ok(serde_json::json!({
                "type": "Instance",
                "kind": "PlainInstance",
                // No "class" field — class ID absent.
                "fields": [
                    {
                        "name": "x",
                        "value": { "kind": "Int", "valueAsString": "42" }
                    }
                ]
            }))
        }
    }

    let (mut adapter, _rx) = DapAdapter::new(NoClassIdMockBackend);
    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // Just the field, no getters attempted.
    assert_eq!(vars.len(), 1, "Should have 1 field, no getters");
    assert_eq!(vars[0]["name"], "x");
}

/// When the class `get_object` call fails, getter collection is skipped gracefully.
#[tokio::test]
async fn test_getter_collection_skips_when_class_fetch_fails() {
    struct ClassFetchFailsMockBackend;
    impl MockTestBackend for ClassFetchFailsMockBackend {
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
                    "class": { "id": "classes/unreachable", "name": "Unreachable" },
                    "fields": [
                        {
                            "name": "y",
                            "value": { "kind": "Int", "valueAsString": "7" }
                        }
                    ]
                }))
            } else {
                // Fail to fetch the class.
                Err(BackendError::VmServiceError("class not found".to_string()))
            }
        }
    }

    let (mut adapter, _rx) = DapAdapter::new(ClassFetchFailsMockBackend);
    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    // Should have the field but no getters (class fetch failed).
    assert_eq!(
        vars.len(),
        1,
        "Should have 1 field, no getters after class fetch failure"
    );
    assert_eq!(vars[0]["name"], "y");
}

/// `evaluateGettersInDebugViews` setting is applied from the attach request.
#[tokio::test]
async fn test_attach_sets_evaluate_getters_flag() {
    let (mut adapter, _rx) = DapAdapter::new(crate::adapter::test_helpers::MockBackend);

    // Default should be true.
    assert!(
        adapter.evaluate_getters_in_debug_views,
        "Default should be true"
    );

    // Send attach with evaluateGettersInDebugViews: false.
    let req = crate::DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "evaluateGettersInDebugViews": false
        })),
    };
    let _resp = adapter.handle_request(&req).await;

    assert!(
        !adapter.evaluate_getters_in_debug_views,
        "Setting should be false after attach with false"
    );
}

/// Setting not present in attach args leaves the default (true) unchanged.
#[tokio::test]
async fn test_attach_without_flag_keeps_default() {
    let (mut adapter, _rx) = DapAdapter::new(crate::adapter::test_helpers::MockBackend);

    // Send attach without evaluateGettersInDebugViews.
    let req = crate::DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({})),
    };
    let _resp = adapter.handle_request(&req).await;

    assert!(
        adapter.evaluate_getters_in_debug_views,
        "Default should remain true when not specified"
    );
}

/// Getters from both fields and methods of `Getter` kind are collected.
#[tokio::test]
async fn test_both_implicit_getter_and_getter_kind_collected() {
    struct MixedGetterKindBackend;
    impl MockTestBackend for MixedGetterKindBackend {
        async fn get_object(
            &self,
            _isolate_id: &str,
            object_id: &str,
            _offset: Option<i64>,
            _count: Option<i64>,
        ) -> Result<serde_json::Value, BackendError> {
            match object_id {
                "objects/inst1" => Ok(serde_json::json!({
                    "type": "Instance",
                    "kind": "PlainInstance",
                    "class": { "id": "classes/mixed", "name": "Mixed" },
                    "fields": []
                })),
                "classes/mixed" => Ok(serde_json::json!({
                    "type": "Class",
                    "name": "Mixed",
                    "functions": [
                        // ImplicitGetter from a field.
                        { "name": "x", "kind": "ImplicitGetter", "static": false },
                        // Explicit Getter method.
                        { "name": "computed", "kind": "Getter", "static": false },
                        // Regular method — should be excluded.
                        { "name": "doSomething", "kind": "RegularFunction", "static": false }
                    ],
                    "super": null
                })),
                _ => Ok(serde_json::json!({ "type": "Instance", "kind": "Null" })),
            }
        }

        async fn evaluate(
            &self,
            _isolate_id: &str,
            _target_id: &str,
            expression: &str,
        ) -> Result<serde_json::Value, BackendError> {
            match expression {
                "x" => Ok(serde_json::json!({ "kind": "Int", "valueAsString": "1" })),
                "computed" => Ok(serde_json::json!({ "kind": "Int", "valueAsString": "2" })),
                _ => Err(BackendError::VmServiceError(format!(
                    "unexpected: {}",
                    expression
                ))),
            }
        }
    }

    let (mut adapter, _rx) = DapAdapter::new(MixedGetterKindBackend);
    let vars = expand_instance(&mut adapter, "objects/inst1").await;

    let names: Vec<&str> = vars
        .iter()
        .map(|v| v["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        names.contains(&"x"),
        "'x' (ImplicitGetter) should be included"
    );
    assert!(
        names.contains(&"computed"),
        "'computed' (Getter) should be included"
    );
    assert!(
        !names.contains(&"doSomething"),
        "Regular function should not be included"
    );
    assert_eq!(vars.len(), 2);
}
