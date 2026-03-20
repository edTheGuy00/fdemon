//! Unit tests for the `completions` DAP request handler.
//!
//! Covers:
//! - Completions include local variable names from the current frame
//! - Completions include Dart keywords
//! - Results filtered by the text fragment being typed
//! - `supportsCompletionsRequest: true` in capabilities
//! - Works without a frame context (keywords only)
//! - Empty prefix returns all candidates
//! - Result capped at 50 items
//! - `extract_last_identifier` helper edge cases

use super::super::test_helpers::MockTestBackend;
use super::*;
use crate::adapter::handlers::extract_last_identifier;
use crate::adapter::stack::FrameRef;
use crate::adapter::BackendError;
use crate::protocol::types::Capabilities;
use crate::DapRequest;

// ─────────────────────────────────────────────────────────────────────────────
// Helper: build a completions request
// ─────────────────────────────────────────────────────────────────────────────

fn make_completions_request(
    seq: i64,
    text: &str,
    column: i64,
    frame_id: Option<i64>,
) -> DapRequest {
    let mut args = serde_json::json!({
        "text": text,
        "column": column,
    });
    if let Some(fid) = frame_id {
        args["frameId"] = serde_json::json!(fid);
    }
    DapRequest {
        seq,
        command: "completions".into(),
        arguments: Some(args),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends
// ─────────────────────────────────────────────────────────────────────────────

/// Backend that returns a stack with two named local variables: "counter" and "widget".
struct CompletionsMockBackend;

impl MockTestBackend for CompletionsMockBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "frames": [
                {
                    "kind": "Regular",
                    "code": { "name": "main" },
                    "location": {
                        "script": { "uri": "file:///app/lib/main.dart" },
                        "line": 10,
                        "column": 1,
                    },
                    "vars": [
                        {
                            "name": "counter",
                            "value": {
                                "type": "InstanceRef",
                                "kind": "Int",
                                "valueAsString": "0"
                            }
                        },
                        {
                            "name": "widget",
                            "value": {
                                "type": "InstanceRef",
                                "kind": "PlainInstance",
                                "valueAsString": ""
                            }
                        }
                    ]
                }
            ]
        }))
    }
}

/// Backend that returns a stack with many variables (to test the 50-item cap).
struct ManyVarsMockBackend;

impl MockTestBackend for ManyVarsMockBackend {
    async fn get_stack(
        &self,
        _isolate_id: &str,
        _limit: Option<i32>,
    ) -> Result<serde_json::Value, BackendError> {
        // Produce 60 variables named "var0" .. "var59".
        let vars: Vec<serde_json::Value> = (0..60)
            .map(|i| {
                serde_json::json!({
                    "name": format!("var{}", i),
                    "value": { "type": "InstanceRef", "kind": "Int", "valueAsString": "0" }
                })
            })
            .collect();
        Ok(serde_json::json!({
            "frames": [
                {
                    "kind": "Regular",
                    "code": { "name": "main" },
                    "location": { "script": { "uri": "file:///app/lib/main.dart" }, "line": 1 },
                    "vars": vars
                }
            ]
        }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: capabilities advertisement
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_capabilities_advertises_completions() {
    let caps = Capabilities::fdemon_defaults();
    assert_eq!(
        caps.supports_completions_request,
        Some(true),
        "supportsCompletionsRequest must be true in fdemon_defaults()"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: completions handler
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completions_includes_locals_with_prefix() {
    // Register an isolate and set up a paused frame.
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Allocate a frame ref for frame index 0.
    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    // Request completions with prefix "cou" (column = 4 is 1-based after "cou").
    let req = make_completions_request(1, "cou", 4, Some(frame_id));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "completions should succeed");
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();

    let labels: Vec<&str> = targets
        .iter()
        .map(|t| t["label"].as_str().unwrap())
        .collect();

    assert!(
        labels.contains(&"counter"),
        "Should include 'counter' for prefix 'cou'; got {:?}",
        labels
    );
    // "widget" does not start with "cou" — should not appear.
    assert!(
        !labels.contains(&"widget"),
        "Should not include 'widget' for prefix 'cou'"
    );
}

#[tokio::test]
async fn test_completions_includes_keywords() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Request completions with prefix "tr" — should match "true".
    let req = make_completions_request(1, "tr", 3, None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();
    let labels: Vec<&str> = targets
        .iter()
        .map(|t| t["label"].as_str().unwrap())
        .collect();

    assert!(
        labels.contains(&"true"),
        "Should include 'true' for prefix 'tr'; got {:?}",
        labels
    );
    assert!(
        !labels.contains(&"false"),
        "Should not include 'false' for prefix 'tr'"
    );
}

#[tokio::test]
async fn test_completions_empty_prefix_returns_all() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    // Empty text, column 1 (cursor at start) — empty fragment.
    let req = make_completions_request(1, "", 1, Some(frame_id));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();
    let labels: Vec<&str> = targets
        .iter()
        .map(|t| t["label"].as_str().unwrap())
        .collect();

    // All locals should appear.
    assert!(labels.contains(&"counter"), "Should include 'counter'");
    assert!(labels.contains(&"widget"), "Should include 'widget'");
    // All keywords should appear.
    assert!(labels.contains(&"true"), "Should include 'true'");
    assert!(labels.contains(&"false"), "Should include 'false'");
    assert!(labels.contains(&"null"), "Should include 'null'");
    assert!(labels.contains(&"this"), "Should include 'this'");
}

#[tokio::test]
async fn test_completions_keywords_only_without_frame() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // No frameId — should only return keywords.
    let req = make_completions_request(1, "", 1, None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();
    let labels: Vec<&str> = targets
        .iter()
        .map(|t| t["label"].as_str().unwrap())
        .collect();

    // Locals should not appear (no frame).
    assert!(
        !labels.contains(&"counter"),
        "Should not include locals without frame"
    );
    // All keywords should appear.
    assert!(labels.contains(&"true"), "Should include 'true'");
    assert!(labels.contains(&"false"), "Should include 'false'");
    assert!(labels.contains(&"null"), "Should include 'null'");
    assert!(labels.contains(&"this"), "Should include 'this'");
}

#[tokio::test]
async fn test_completions_capped_at_50_items() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(ManyVarsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    // Empty prefix — all 60 vars + 4 keywords would normally be 64.
    let req = make_completions_request(1, "", 1, Some(frame_id));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();

    assert!(
        targets.len() <= 50,
        "completions should be capped at 50; got {}",
        targets.len()
    );
}

#[tokio::test]
async fn test_completions_locals_have_variable_type() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    let req = make_completions_request(1, "", 1, Some(frame_id));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();

    // Find the "counter" item and verify its type.
    let counter = targets
        .iter()
        .find(|t| t["label"].as_str() == Some("counter"))
        .expect("'counter' should be in targets");
    assert_eq!(
        counter["type"].as_str(),
        Some("variable"),
        "'counter' should have type 'variable'"
    );
}

#[tokio::test]
async fn test_completions_keywords_have_keyword_type() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_completions_request(1, "fal", 4, None);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();

    let false_item = targets
        .iter()
        .find(|t| t["label"].as_str() == Some("false"))
        .expect("'false' should match prefix 'fal'");
    assert_eq!(
        false_item["type"].as_str(),
        Some("keyword"),
        "'false' should have type 'keyword'"
    );
}

#[tokio::test]
async fn test_completions_locals_sort_before_keywords() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let (mut adapter, _rx2) = DapAdapter::new_with_tx(CompletionsMockBackend, tx);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let frame_id = adapter.frame_store.allocate(FrameRef::new("isolates/1", 0));

    let req = make_completions_request(1, "", 1, Some(frame_id));
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let body = resp.body.unwrap();
    let targets = body["targets"].as_array().unwrap();

    // Locals have sort_text "0_<name>", keywords have "2_<name>".
    // Find a local and a keyword and compare their sort_text prefixes.
    let counter_sort = targets
        .iter()
        .find(|t| t["label"].as_str() == Some("counter"))
        .and_then(|t| t["sortText"].as_str())
        .expect("'counter' should have sortText");

    let true_sort = targets
        .iter()
        .find(|t| t["label"].as_str() == Some("true"))
        .and_then(|t| t["sortText"].as_str())
        .expect("'true' should have sortText");

    assert!(
        counter_sort < true_sort,
        "Local 'counter' (sort_text={:?}) should sort before keyword 'true' (sort_text={:?})",
        counter_sort,
        true_sort
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: extract_last_identifier helper
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_last_identifier_simple_word() {
    assert_eq!(extract_last_identifier("counter"), "counter");
}

#[test]
fn test_extract_last_identifier_after_dot() {
    assert_eq!(extract_last_identifier("obj.field"), "field");
}

#[test]
fn test_extract_last_identifier_empty_string() {
    assert_eq!(extract_last_identifier(""), "");
}

#[test]
fn test_extract_last_identifier_ends_with_non_ident() {
    // "myList[" ends with "[" which is not an identifier char.
    assert_eq!(extract_last_identifier("myList["), "");
}

#[test]
fn test_extract_last_identifier_dollar_sign() {
    // Dart identifiers may include "$".
    assert_eq!(extract_last_identifier("_$myVar"), "_$myVar");
}

#[test]
fn test_extract_last_identifier_partial_prefix() {
    assert_eq!(extract_last_identifier("tru"), "tru");
}

#[test]
fn test_extract_last_identifier_spaces() {
    // Space is a non-identifier character.
    assert_eq!(extract_last_identifier("var x"), "x");
}
