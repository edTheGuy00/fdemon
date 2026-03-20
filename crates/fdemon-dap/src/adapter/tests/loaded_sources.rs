//! Unit tests for the `loadedSources` DAP request handler.
//!
//! Verifies:
//! - All user-visible scripts are returned as `Source` objects
//! - SDK sources have `sourceReference > 0` and `presentationHint: "deemphasize"`
//! - Package sources resolve correctly (sourceReference when no project root)
//! - Internal/generated scripts (`eval:`, `dart:_*`) are filtered out
//! - The handler errors when no isolate is registered
//! - `supportsLoadedSourcesRequest: true` is advertised in capabilities

use super::super::test_helpers::MockTestBackend;
use super::super::{BackendError, DapAdapter};
use super::register_isolate;
use crate::DapRequest;

// ─────────────────────────────────────────────────────────────────────────────
// Mock backends
// ─────────────────────────────────────────────────────────────────────────────

/// A backend returning a mix of script URI types.
struct LoadedSourcesMockBackend {
    scripts: serde_json::Value,
}

impl LoadedSourcesMockBackend {
    fn with_scripts(scripts: Vec<serde_json::Value>) -> Self {
        Self {
            scripts: serde_json::json!({ "scripts": scripts }),
        }
    }
}

impl MockTestBackend for LoadedSourcesMockBackend {
    async fn get_scripts(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        Ok(self.scripts.clone())
    }
}

/// A backend whose `get_scripts` always fails.
struct FailingScriptsMockBackend;

impl MockTestBackend for FailingScriptsMockBackend {
    async fn get_scripts(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        Err(BackendError::VmServiceError(
            "scripts unavailable".to_string(),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

fn make_loaded_sources_request(seq: i64) -> DapRequest {
    DapRequest {
        seq,
        command: "loadedSources".into(),
        arguments: None,
    }
}

fn script(uri: &str, id: &str) -> serde_json::Value {
    serde_json::json!({ "uri": uri, "id": id })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_loaded_sources_returns_scripts() {
    let backend = LoadedSourcesMockBackend::with_scripts(vec![
        script("file:///app/lib/main.dart", "scripts/1"),
        script("file:///app/lib/home.dart", "scripts/2"),
    ]);
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success, "loadedSources should succeed");
    let sources = resp.body.as_ref().unwrap()["sources"]
        .as_array()
        .expect("sources must be an array");
    assert_eq!(sources.len(), 2);

    // Both file:// scripts should have paths, no sourceReference.
    let paths: Vec<&str> = sources.iter().filter_map(|s| s["path"].as_str()).collect();
    assert!(paths.contains(&"/app/lib/main.dart"));
    assert!(paths.contains(&"/app/lib/home.dart"));

    // No sourceReference for file:// scripts.
    for src in sources {
        assert!(
            src.get("sourceReference").is_none()
                || src["sourceReference"].as_i64().unwrap_or(0) == 0,
            "file:// scripts should not have a sourceReference"
        );
    }
}

#[tokio::test]
async fn test_loaded_sources_filters_eval_and_internal() {
    let backend = LoadedSourcesMockBackend::with_scripts(vec![
        script("file:///app/lib/main.dart", "scripts/1"),
        script("eval:source/1", "scripts/eval1"),
        script("dart:_internal", "scripts/dartinternal"),
        script("dart:_runtime", "scripts/drtruntime"),
        script("dart:core", "scripts/dartcore"),
    ]);
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let sources = resp.body.as_ref().unwrap()["sources"].as_array().unwrap();

    // eval: and dart:_* must be filtered out. Only main.dart and dart:core remain.
    assert_eq!(
        sources.len(),
        2,
        "eval: and dart:_ scripts must be filtered"
    );

    let uris: Vec<&str> = sources.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(!uris.contains(&"eval:source/1"), "eval: must be filtered");
    // dart:_internal and dart:_runtime must not appear
    assert!(
        !uris
            .iter()
            .any(|u| u.contains("_internal") || u.contains("_runtime")),
        "dart:_ scripts must be filtered"
    );
}

#[tokio::test]
async fn test_loaded_sources_deemphasizes_sdk() {
    let backend = LoadedSourcesMockBackend::with_scripts(vec![
        script("dart:core", "scripts/dartcore"),
        script("dart:async", "scripts/dartasync"),
        script("org-dartlang-sdk:///sdk/lib/core/core.dart", "scripts/sdk1"),
    ]);
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let sources = resp.body.as_ref().unwrap()["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 3);

    for src in sources {
        let hint = src["presentationHint"].as_str();
        assert_eq!(
            hint,
            Some("deemphasize"),
            "SDK sources must have presentationHint: deemphasize, got {:?}",
            hint
        );

        let source_ref = src["sourceReference"].as_i64().unwrap_or(0);
        assert!(
            source_ref > 0,
            "SDK sources must have sourceReference > 0, got {}",
            source_ref
        );
    }
}

#[tokio::test]
async fn test_loaded_sources_package_gets_source_reference_without_project_root() {
    // Without a project root, package: URIs cannot be resolved locally
    // and must be assigned a sourceReference.
    let backend = LoadedSourcesMockBackend::with_scripts(vec![
        script("package:flutter/material.dart", "scripts/flutter1"),
        script("package:myapp/src/utils.dart", "scripts/myapp1"),
    ]);
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let sources = resp.body.as_ref().unwrap()["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2);

    for src in sources {
        let source_ref = src["sourceReference"].as_i64().unwrap_or(0);
        assert!(
            source_ref > 0,
            "package: scripts without project root must have sourceReference > 0"
        );
    }

    // Flutter packages should also be deemphasized.
    let flutter_src = sources
        .iter()
        .find(|s| s["name"].as_str() == Some("material.dart"))
        .expect("flutter source not found");
    assert_eq!(
        flutter_src["presentationHint"].as_str(),
        Some("deemphasize")
    );
}

#[tokio::test]
async fn test_loaded_sources_no_isolate_returns_error() {
    use super::super::test_helpers::MockBackend;

    let (mut adapter, _rx) = DapAdapter::new(MockBackend);
    // No isolate registered.

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(!resp.success, "loadedSources without an isolate must fail");
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("no active isolate"),
        "error message should mention no isolate, got: {msg}"
    );
}

#[tokio::test]
async fn test_loaded_sources_backend_failure_returns_error() {
    let (mut adapter, mut rx) = DapAdapter::new(FailingScriptsMockBackend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(
        !resp.success,
        "loadedSources should fail when backend fails"
    );
    let msg = resp.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("get_scripts failed"),
        "error message should mention get_scripts, got: {msg}"
    );
}

#[tokio::test]
async fn test_loaded_sources_empty_scripts_returns_empty_array() {
    let backend = LoadedSourcesMockBackend::with_scripts(vec![]);
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_loaded_sources_request(1);
    let resp = adapter.handle_request(&req).await;

    assert!(resp.success);
    let sources = resp.body.as_ref().unwrap()["sources"].as_array().unwrap();
    assert!(
        sources.is_empty(),
        "empty scripts list must yield empty sources"
    );
}

#[tokio::test]
async fn test_loaded_sources_source_references_are_stable() {
    // Two requests for the same scripts must produce identical sourceReference IDs.
    let backend =
        LoadedSourcesMockBackend::with_scripts(vec![script("dart:core", "scripts/dartcore")]);
    let (mut adapter, mut rx) = DapAdapter::new(backend);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req1 = make_loaded_sources_request(1);
    let resp1 = adapter.handle_request(&req1).await;
    let ref1 = resp1.body.as_ref().unwrap()["sources"][0]["sourceReference"]
        .as_i64()
        .unwrap();

    let req2 = make_loaded_sources_request(2);
    let resp2 = adapter.handle_request(&req2).await;
    let ref2 = resp2.body.as_ref().unwrap()["sources"][0]["sourceReference"]
        .as_i64()
        .unwrap();

    assert_eq!(
        ref1, ref2,
        "sourceReference IDs must be stable across calls"
    );
    assert!(ref1 > 0, "sourceReference must be > 0");
}

#[test]
fn test_supports_loaded_sources_request_in_capabilities() {
    use crate::protocol::types::Capabilities;
    let caps = Capabilities::fdemon_defaults();
    assert_eq!(
        caps.supports_loaded_sources_request,
        Some(true),
        "supportsLoadedSourcesRequest must be true in fdemon_defaults()"
    );
}
