//! Tests for global time budgets on toString() and getter evaluation.
//!
//! These tests cover:
//! - `enrich_with_to_string` completes within `TO_STRING_TOTAL_BUDGET` (3s)
//!   even when each individual candidate would take longer than the budget
//! - Getter evaluation completes within `GETTER_EVAL_TOTAL_BUDGET` (5s)
//!   even when an object has many slow getters
//! - Budget-exceeded getters appear as lazy items (expandable on demand)
//! - Per-call timeouts still apply within the budget

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::adapter::test_helpers::MockTestBackend;
use crate::adapter::*;

// ─────────────────────────────────────────────────────────────────────────────
// Mock backend: many slow toString() candidates
// ─────────────────────────────────────────────────────────────────────────────

/// A backend that returns N `PlainInstance` variables in a single frame, with
/// each `toString()` call sleeping for `sleep_per_call_ms` milliseconds.
struct SlowToStringMockBackend {
    /// How long each `evaluate("toString()")` call sleeps.
    sleep_per_call: Duration,
    /// Number of PlainInstance variables to return in the stack frame.
    num_instances: usize,
    /// Count of evaluate() calls made.
    call_count: Arc<Mutex<u32>>,
}

impl SlowToStringMockBackend {
    fn new(num_instances: usize, sleep_per_call: Duration) -> (Self, Arc<Mutex<u32>>) {
        let call_count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            sleep_per_call,
            num_instances,
            call_count: call_count.clone(),
        };
        (backend, call_count)
    }
}

impl MockTestBackend for SlowToStringMockBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        let n = self.num_instances;
        let vars: Vec<serde_json::Value> = (0..n)
            .map(|i| {
                serde_json::json!({
                    "name": format!("obj{}", i),
                    "value": {
                        "type": "InstanceRef",
                        "kind": "PlainInstance",
                        "classRef": { "name": "MyClass" },
                        "id": format!("objects/inst{}", i)
                    }
                })
            })
            .collect();
        Ok(serde_json::json!({
            "frames": [{
                "kind": "Regular",
                "code": { "name": "main" },
                "location": {
                    "script": { "uri": "file:///app/lib/main.dart" },
                    "line": 10
                },
                "vars": vars
            }]
        }))
    }

    async fn evaluate(
        &self,
        _isolate_id: &str,
        _target_id: &str,
        _expression: &str,
    ) -> Result<serde_json::Value, BackendError> {
        *self.call_count.lock().unwrap() += 1;
        tokio::time::sleep(self.sleep_per_call).await;
        Ok(serde_json::json!({
            "kind": "String",
            "valueAsString": "MyClass(enriched)"
        }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock backend: many slow getters
// ─────────────────────────────────────────────────────────────────────────────

/// Backend that returns a `PlainInstance` with `num_getters` getters,
/// each `evaluate()` call sleeping for `sleep_per_call` milliseconds.
struct SlowGetterBudgetMockBackend {
    sleep_per_call: Duration,
    num_getters: usize,
    call_count: Arc<Mutex<u32>>,
}

impl SlowGetterBudgetMockBackend {
    fn new(num_getters: usize, sleep_per_call: Duration) -> (Self, Arc<Mutex<u32>>) {
        let call_count = Arc::new(Mutex::new(0u32));
        let backend = Self {
            sleep_per_call,
            num_getters,
            call_count: call_count.clone(),
        };
        (backend, call_count)
    }
}

impl MockTestBackend for SlowGetterBudgetMockBackend {
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
            "classes/slow" => {
                let n = self.num_getters;
                let functions: Vec<serde_json::Value> = (0..n)
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
                    "name": "SlowClass",
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
        *self.call_count.lock().unwrap() += 1;
        tokio::time::sleep(self.sleep_per_call).await;
        Ok(serde_json::json!({ "kind": "Int", "valueAsString": "42" }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

use super::register_isolate;

/// Set up adapter, run stackTrace → scopes → variables for frame 0.
/// Returns the list of variable JSON objects.
async fn get_locals(
    adapter: &mut DapAdapter<impl DebugBackend>,
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
) -> Vec<serde_json::Value> {
    let thread_id = register_isolate(adapter, rx, "isolates/1").await;

    let stack_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 2,
            command: "stackTrace".into(),
            arguments: Some(serde_json::json!({ "threadId": thread_id })),
        })
        .await;
    assert!(
        stack_resp.success,
        "stackTrace failed: {:?}",
        stack_resp.message
    );
    let frame_id = stack_resp.body.unwrap()["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();

    let scopes_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 3,
            command: "scopes".into(),
            arguments: Some(serde_json::json!({ "frameId": frame_id })),
        })
        .await;
    assert!(
        scopes_resp.success,
        "scopes failed: {:?}",
        scopes_resp.message
    );
    let locals_ref = scopes_resp.body.unwrap()["scopes"][0]["variablesReference"]
        .as_i64()
        .unwrap();

    let vars_resp = adapter
        .handle_request(&crate::DapRequest {
            seq: 4,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": locals_ref })),
        })
        .await;
    assert!(
        vars_resp.success,
        "variables failed: {:?}",
        vars_resp.message
    );
    vars_resp.body.unwrap()["variables"]
        .as_array()
        .unwrap()
        .clone()
}

/// Expand an object via a `variables` request using a `VariableRef::Object`.
/// Returns the resulting variables array.
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
// toString total budget tests
// ─────────────────────────────────────────────────────────────────────────────

/// When each toString() call sleeps longer than the per-call timeout (1s),
/// the entire enrichment pass completes within `TO_STRING_TOTAL_BUDGET` (3s)
/// even with 10 candidates.
///
/// Each call will time out at 1s, so 10 calls would take ~10s without the
/// total budget. The budget should cut this to at most 3s.
#[tokio::test(flavor = "multi_thread")]
async fn test_tostring_enrichment_respects_total_budget() {
    // 10 candidates, each sleeping 2s (longer than per-call 1s timeout).
    // Without budget: 10 * 1s = 10s (due to per-call timeouts).
    // With 3s total budget: should complete in ~3s.
    let (backend, call_count) = SlowToStringMockBackend::new(10, Duration::from_secs(2));
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let start = std::time::Instant::now();

    // The variables call should complete in ~3s (total budget), not ~10s.
    let vars = tokio::time::timeout(Duration::from_secs(8), get_locals(&mut adapter, &mut rx))
        .await
        .expect("variables request should not hang (expected total budget to kick in)");

    let elapsed = start.elapsed();

    // Should have all 10 variables (budget does not drop variables, only skips enrichment).
    assert_eq!(
        vars.len(),
        10,
        "All 10 variables should be present, got: {}",
        vars.len()
    );

    // Elapsed should be less than 6s (generous headroom above the 3s budget,
    // accounting for test infrastructure overhead).
    assert!(
        elapsed < Duration::from_secs(6),
        "Expected completion within 6s (3s budget + headroom), but took {:?}",
        elapsed
    );

    // Not all candidates should have been enriched (budget was exhausted).
    // Some variables should have been enriched (class name only, no appended toString).
    let calls = *call_count.lock().unwrap();
    assert!(
        calls < 10,
        "Expected fewer than 10 evaluate calls due to budget exhaustion, got: {}",
        calls
    );
}

/// Variables that are NOT enriched due to budget keep their original display
/// value (class name only), not a partial or corrupt value.
#[tokio::test(flavor = "multi_thread")]
async fn test_tostring_budget_exhausted_variables_keep_class_name() {
    // 10 candidates, each sleeping 1.5s (longer than per-call timeout of 1s).
    // Budget of 3s means at most ~3 calls can start before deadline.
    let (backend, _call_count) = SlowToStringMockBackend::new(10, Duration::from_secs(2));
    let (mut adapter, mut rx) = DapAdapter::new(backend);

    let vars = tokio::time::timeout(Duration::from_secs(10), get_locals(&mut adapter, &mut rx))
        .await
        .expect("should complete within timeout");

    // All 10 variables should be present.
    assert_eq!(vars.len(), 10, "All variables should be present");

    // Every variable should have a non-empty value (class name at minimum).
    for (i, var) in vars.iter().enumerate() {
        let value = var["value"].as_str().unwrap_or("");
        assert!(
            !value.is_empty(),
            "Variable {} should have a non-empty value, got empty string",
            i
        );
        // Value should contain the class name or "instance" — not an error string.
        assert!(
            !value.contains("<error") && !value.contains("timed out"),
            "Variable {} should not show error/timeout in value, got: {:?}",
            i,
            value
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Getter total budget tests
// ─────────────────────────────────────────────────────────────────────────────

/// When each getter sleeps longer than the per-call timeout (1s), the entire
/// getter expansion completes within `GETTER_EVAL_TOTAL_BUDGET` (5s) even
/// with 20 getters.
///
/// Each call will time out at 1s, so 20 getters would take ~20s without the
/// total budget. The budget should cut this to at most 5s.
#[tokio::test(flavor = "multi_thread")]
async fn test_getter_evaluation_respects_total_budget() {
    // 20 getters, each sleeping 2s (longer than the per-call 1s timeout).
    // Without budget: 20 * 1s = 20s. With 5s budget: should complete in ~5s.
    let (backend, call_count) = SlowGetterBudgetMockBackend::new(20, Duration::from_secs(2));
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let start = std::time::Instant::now();

    let vars = tokio::time::timeout(
        Duration::from_secs(12),
        expand_instance(&mut adapter, "objects/inst1"),
    )
    .await
    .expect("getter expansion should not hang (expected budget to kick in)");

    let elapsed = start.elapsed();

    // Should complete within 8s (5s budget + generous headroom).
    assert!(
        elapsed < Duration::from_secs(10),
        "Expected completion within 10s (5s budget + headroom), took {:?}",
        elapsed
    );

    // Should have 20 variables total (some eager, rest as lazy).
    assert_eq!(
        vars.len(),
        20,
        "All 20 getter variables should be present (some eager, some lazy), got: {}",
        vars.len()
    );

    // Fewer than 20 eager evaluations should have been made (budget exhausted).
    let calls = *call_count.lock().unwrap();
    assert!(
        calls < 20,
        "Expected fewer than 20 evaluate calls due to budget exhaustion, got: {}",
        calls
    );
}

/// When the getter budget is exhausted, remaining getters are added as lazy
/// items (not silently dropped). The user can still expand them individually.
#[tokio::test(flavor = "multi_thread")]
async fn test_getter_budget_exhausted_remaining_are_lazy() {
    // 20 getters, each sleeping 2s. Budget of 5s means ~5 getters evaluated
    // before the deadline is hit, leaving ~15 as lazy.
    let (backend, _call_count) = SlowGetterBudgetMockBackend::new(20, Duration::from_secs(2));
    let (mut adapter, _rx) = DapAdapter::new(backend);

    let vars = tokio::time::timeout(
        Duration::from_secs(12),
        expand_instance(&mut adapter, "objects/inst1"),
    )
    .await
    .expect("should complete within timeout");

    // All 20 getters should be present.
    assert_eq!(vars.len(), 20, "All 20 getter variables should be present");

    // At least some getters should be lazy (budget was exhausted).
    let lazy_count = vars
        .iter()
        .filter(|v| v["presentationHint"]["lazy"] == true)
        .count();
    assert!(
        lazy_count > 0,
        "Expected some lazy getters after budget exhaustion, but found none"
    );

    // Lazy getters must have a non-zero variablesReference (expandable on demand).
    for var in vars
        .iter()
        .filter(|v| v["presentationHint"]["lazy"] == true)
    {
        let var_ref = var["variablesReference"].as_i64().unwrap_or(0);
        assert!(
            var_ref > 0,
            "Lazy getter '{}' should have a non-zero variablesReference for expansion",
            var["name"].as_str().unwrap_or("?")
        );
    }

    // Lazy getters must have an empty value (not yet evaluated).
    for var in vars
        .iter()
        .filter(|v| v["presentationHint"]["lazy"] == true)
    {
        let value = var["value"].as_str().unwrap_or("NOT_EMPTY");
        assert_eq!(
            value,
            "",
            "Lazy getter '{}' should have empty value, got: {:?}",
            var["name"].as_str().unwrap_or("?"),
            value
        );
    }
}

/// A lazy getter added due to budget exhaustion can still be expanded by the
/// user (via its `GetterEval` variable reference).
#[tokio::test(flavor = "multi_thread")]
async fn test_budget_lazy_getter_can_be_expanded() {
    // 20 slow getters. After budget exhaust, remaining are lazy.
    let (backend, _call_count) = SlowGetterBudgetMockBackend::new(20, Duration::from_secs(2));
    let (mut adapter, _rx) = DapAdapter::new(backend);

    // Expand the instance to get the variable list.
    let vars = tokio::time::timeout(
        Duration::from_secs(12),
        expand_instance(&mut adapter, "objects/inst1"),
    )
    .await
    .expect("initial expansion should complete");

    // Find a lazy getter.
    let lazy_var = vars
        .iter()
        .find(|v| v["presentationHint"]["lazy"] == true)
        .cloned();

    // There must be at least one lazy getter (budget exhausted).
    let lazy_var = lazy_var.expect("Expected at least one lazy getter after budget exhaustion");
    let lazy_var_ref = lazy_var["variablesReference"].as_i64().unwrap();
    assert!(lazy_var_ref > 0, "Lazy getter must have non-zero ref");

    // Expanding the lazy getter should succeed quickly (instant evaluate mock
    // is not used here — the backend sleeps 2s, but the per-call timeout
    // kicks in at 1s). The expansion should return a single variable.
    let expand_req = crate::DapRequest {
        seq: 99,
        command: "variables".into(),
        arguments: Some(serde_json::json!({ "variablesReference": lazy_var_ref })),
    };
    let expand_resp =
        tokio::time::timeout(Duration::from_secs(3), adapter.handle_request(&expand_req))
            .await
            .expect("expanding lazy getter should complete within 3s");

    assert!(
        expand_resp.success,
        "Expanding a budget-lazy getter should succeed: {:?}",
        expand_resp.message
    );

    let expanded = expand_resp.body.unwrap()["variables"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    assert_eq!(
        expanded.len(),
        1,
        "GetterEval expansion should return exactly one variable"
    );
    // The timed-out evaluation shows "<timed out>" (per-call timeout applies).
    let val = expanded[0]["value"].as_str().unwrap_or("");
    assert_eq!(
        val, "<timed out>",
        "Lazy getter expansion should show '<timed out>' when backend is slow"
    );
}
