//! Tests for the `updateDebugOptions` custom DAP request handler and the
//! `apply_library_debuggability` / `is_app_package` helpers.

use std::sync::{Arc, Mutex};

use crate::adapter::types::DebugEvent;
use crate::adapter::DapAdapter;
use crate::DapRequest;

use super::super::test_helpers::MockTestBackend;
use super::super::{BackendError, BreakpointResult};
use super::{make_request, register_isolate};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build an `updateDebugOptions` request with the given fields.
fn make_update_debug_options(
    seq: i64,
    debug_sdk: Option<bool>,
    debug_external: Option<bool>,
) -> DapRequest {
    let mut args = serde_json::json!({});
    if let Some(v) = debug_sdk {
        args["debugSdkLibraries"] = serde_json::json!(v);
    }
    if let Some(v) = debug_external {
        args["debugExternalPackageLibraries"] = serde_json::json!(v);
    }
    DapRequest {
        seq,
        command: "updateDebugOptions".into(),
        arguments: Some(args),
    }
}

/// A mock backend that records `set_library_debuggable` calls.
///
/// `calls` accumulates `(isolate_id, library_id, is_debuggable)` tuples for
/// each call. `isolate_libraries` is a map from isolate ID to the library list
/// returned by `get_isolate`.
struct LibraryDebuggableMock {
    /// Records every `set_library_debuggable(isolate_id, library_id, is_debuggable)` call.
    calls: Arc<Mutex<Vec<(String, String, bool)>>>,
    /// Canned library list for a single isolate, keyed by isolate ID.
    isolate_libraries: Vec<serde_json::Value>,
}

impl LibraryDebuggableMock {
    /// Create a backend with the given library list for all isolates.
    fn new(libraries: Vec<serde_json::Value>) -> (Self, Arc<Mutex<Vec<(String, String, bool)>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mock = Self {
            calls: calls.clone(),
            isolate_libraries: libraries,
        };
        (mock, calls)
    }
}

impl MockTestBackend for LibraryDebuggableMock {
    async fn get_isolate(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({
            "libraries": self.isolate_libraries,
        }))
    }

    async fn set_library_debuggable(
        &self,
        isolate_id: &str,
        library_id: &str,
        is_debuggable: bool,
    ) -> Result<(), BackendError> {
        self.calls.lock().unwrap().push((
            isolate_id.to_string(),
            library_id.to_string(),
            is_debuggable,
        ));
        Ok(())
    }

    async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
        Ok(serde_json::json!({ "isolates": [] }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: is_app_package
// ─────────────────────────────────────────────────────────────────────────────

/// When `app_package_name` is empty, ALL package: URIs are treated as app
/// packages (debuggable). This prevents silently marking app code as
/// non-debuggable when the IDE doesn't send packageName.
#[tokio::test]
async fn test_is_app_package_empty_name_returns_true() {
    let (adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    assert!(adapter.is_app_package("package:anything/lib.dart"));
    assert!(adapter.is_app_package("package:my_app/main.dart"));
}

/// URIs matching `package:<name>/` are app packages.
#[tokio::test]
async fn test_is_app_package_matching_prefix_returns_true() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    adapter.app_package_name = "my_app".to_string();

    assert!(adapter.is_app_package("package:my_app/main.dart"));
    assert!(adapter.is_app_package("package:my_app/src/widget.dart"));
}

/// URIs that don't match the app package are external.
#[tokio::test]
async fn test_is_app_package_different_package_returns_false() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    adapter.app_package_name = "my_app".to_string();

    assert!(!adapter.is_app_package("package:flutter/material.dart"));
    assert!(!adapter.is_app_package("package:provider/provider.dart"));
    assert!(!adapter.is_app_package("dart:core"));
}

/// Prefix-only match (no trailing slash) does not match sub-packages.
///
/// A package named "my_app" must NOT match "my_app_test" — the match requires
/// the trailing `/`.
#[tokio::test]
async fn test_is_app_package_does_not_match_prefixes_without_slash() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    adapter.app_package_name = "my_app".to_string();

    // "my_app_test" should NOT match even though it starts with "my_app".
    assert!(!adapter.is_app_package("package:my_app_test/test.dart"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: updateDebugOptions dispatch
// ─────────────────────────────────────────────────────────────────────────────

/// `updateDebugOptions` returns a success response with an empty body.
#[tokio::test]
async fn test_update_debug_options_returns_success() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    let req = make_update_debug_options(1, Some(true), Some(false));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success, "updateDebugOptions should succeed");
}

/// `updateDebugOptions` without arguments returns an error.
#[tokio::test]
async fn test_update_debug_options_without_arguments_returns_error() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    let req = make_request(1, "updateDebugOptions");
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success, "updateDebugOptions without args should fail");
}

/// `updateDebugOptions` sets `debug_sdk_libraries` on the adapter.
#[tokio::test]
async fn test_update_debug_options_sets_debug_sdk_libraries() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    assert!(!adapter.debug_sdk_libraries, "starts as false");

    let req = make_update_debug_options(1, Some(true), None);
    adapter.handle_request(&req).await;
    assert!(adapter.debug_sdk_libraries, "should be set to true");

    let req2 = make_update_debug_options(2, Some(false), None);
    adapter.handle_request(&req2).await;
    assert!(!adapter.debug_sdk_libraries, "should be set back to false");
}

/// `updateDebugOptions` sets `debug_external_package_libraries` on the adapter.
#[tokio::test]
async fn test_update_debug_options_sets_debug_external_package_libraries() {
    let (mut adapter, _rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    assert!(!adapter.debug_external_package_libraries, "starts as false");

    let req = make_update_debug_options(1, None, Some(true));
    adapter.handle_request(&req).await;
    assert!(
        adapter.debug_external_package_libraries,
        "should be set to true"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: apply_library_debuggability
// ─────────────────────────────────────────────────────────────────────────────

/// With `debug_sdk_libraries = true`, `dart:` libraries are set debuggable.
#[tokio::test]
async fn test_update_debug_options_toggles_sdk_libraries() {
    let libraries = vec![
        serde_json::json!({ "id": "libraries/1", "uri": "dart:core" }),
        serde_json::json!({ "id": "libraries/2", "uri": "file:///app/lib/main.dart" }),
    ];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);

    // Register an isolate so the adapter knows it exists.
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Enable SDK library debugging.
    let req = make_update_debug_options(1, Some(true), None);
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    let recorded = calls.lock().unwrap().clone();
    // Both libraries should have been processed.
    assert_eq!(
        recorded.len(),
        2,
        "two set_library_debuggable calls expected"
    );

    // dart:core should be debuggable = true.
    let sdk_call = recorded
        .iter()
        .find(|(_, lib, _)| lib == "libraries/1")
        .expect("dart:core call missing");
    assert!(
        sdk_call.2,
        "dart:core should be debuggable when debug_sdk_libraries=true"
    );

    // app file should always be debuggable.
    let app_call = recorded
        .iter()
        .find(|(_, lib, _)| lib == "libraries/2")
        .expect("app library call missing");
    assert!(app_call.2, "app code should always be debuggable");
}

/// With `debug_sdk_libraries = false`, `dart:` libraries are set non-debuggable.
#[tokio::test]
async fn test_update_debug_options_disables_sdk_libraries() {
    let libraries = vec![serde_json::json!({ "id": "libraries/1", "uri": "dart:core" })];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_update_debug_options(1, Some(false), None);
    adapter.handle_request(&req).await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert!(
        !recorded[0].2,
        "dart:core should be non-debuggable when debug_sdk_libraries=false"
    );
}

/// With `debug_external_package_libraries = true`, external packages are debuggable.
#[tokio::test]
async fn test_update_debug_options_toggles_external_package_libraries() {
    let libraries = vec![
        serde_json::json!({ "id": "libraries/1", "uri": "package:flutter/material.dart" }),
        serde_json::json!({ "id": "libraries/2", "uri": "package:provider/provider.dart" }),
    ];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_update_debug_options(1, None, Some(true));
    adapter.handle_request(&req).await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2);
    // All external packages should be debuggable.
    for (_, _, is_debuggable) in &recorded {
        assert!(
            is_debuggable,
            "external package should be debuggable when flag=true"
        );
    }
}

/// App code (`package:<app>/`) is always debuggable regardless of settings.
#[tokio::test]
async fn test_app_code_always_debuggable() {
    let libraries = vec![
        serde_json::json!({ "id": "libraries/1", "uri": "package:my_app/main.dart" }),
        serde_json::json!({ "id": "libraries/2", "uri": "dart:core" }),
    ];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);
    adapter.app_package_name = "my_app".to_string();
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // With both flags OFF (defaults), only SDK libraries should be non-debuggable.
    // App code should remain debuggable.
    let req = make_update_debug_options(1, Some(false), Some(false));
    adapter.handle_request(&req).await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2);

    let app_call = recorded
        .iter()
        .find(|(_, lib, _)| lib == "libraries/1")
        .expect("app library call missing");
    assert!(app_call.2, "app code should always be debuggable");

    let sdk_call = recorded
        .iter()
        .find(|(_, lib, _)| lib == "libraries/2")
        .expect("dart:core call missing");
    assert!(
        !sdk_call.2,
        "SDK code should be non-debuggable when debug_sdk=false"
    );
}

/// `apply_library_debuggability` is called for all registered isolates.
#[tokio::test]
async fn test_update_debug_options_applies_to_all_isolates() {
    let libraries = vec![serde_json::json!({ "id": "libraries/1", "uri": "dart:core" })];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);

    // Register two isolates.
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;
    register_isolate(&mut adapter, &mut rx, "isolates/2").await;

    let req = make_update_debug_options(1, Some(true), None);
    adapter.handle_request(&req).await;

    let recorded = calls.lock().unwrap().clone();
    // One call per isolate × one library each = 2 total.
    assert_eq!(
        recorded.len(),
        2,
        "should call set_library_debuggable once per isolate"
    );
    let isolate_ids: Vec<&str> = recorded.iter().map(|(iso, _, _)| iso.as_str()).collect();
    assert!(
        isolate_ids.contains(&"isolates/1"),
        "isolates/1 should be processed"
    );
    assert!(
        isolate_ids.contains(&"isolates/2"),
        "isolates/2 should be processed"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: IsolateRunnable ordering
// ─────────────────────────────────────────────────────────────────────────────

/// After `IsolateRunnable`, library debuggability is set before breakpoints.
///
/// This is verified by checking that `set_library_debuggable` was called before
/// any `add_breakpoint` call (using call ordering in the shared `calls` vec).
///
/// Setup:
/// 1. Register an isolate via `IsolateStart`
/// 2. Set breakpoints on the isolate (so desired_breakpoints is populated)
/// 3. Fire `IsolateExit` (simulates hot restart, clears active breakpoints)
/// 4. Fire `IsolateRunnable` for the new isolate
/// 5. Verify `set_library_debuggable` precedes `add_breakpoint` in the call order
#[tokio::test]
async fn test_isolate_runnable_applies_library_debuggability_before_breakpoints() {
    /// Backend that records the order in which methods are called.
    struct OrderedMock {
        /// Ordered log of method names called.
        call_order: Arc<Mutex<Vec<String>>>,
    }

    impl MockTestBackend for OrderedMock {
        async fn get_isolate(&self, _isolate_id: &str) -> Result<serde_json::Value, BackendError> {
            self.call_order
                .lock()
                .unwrap()
                .push("get_isolate".to_string());
            Ok(serde_json::json!({
                "libraries": [
                    { "id": "libraries/1", "uri": "dart:core" },
                ]
            }))
        }

        async fn set_library_debuggable(
            &self,
            _isolate_id: &str,
            _library_id: &str,
            _is_debuggable: bool,
        ) -> Result<(), BackendError> {
            self.call_order
                .lock()
                .unwrap()
                .push("set_library_debuggable".to_string());
            Ok(())
        }

        async fn add_breakpoint(
            &self,
            _isolate_id: &str,
            _uri: &str,
            line: i32,
            column: Option<i32>,
        ) -> Result<BreakpointResult, BackendError> {
            self.call_order
                .lock()
                .unwrap()
                .push("add_breakpoint".to_string());
            Ok(BreakpointResult {
                vm_id: format!("bp/line:{line}"),
                resolved: true,
                line: Some(line),
                column,
            })
        }

        async fn get_vm(&self) -> Result<serde_json::Value, BackendError> {
            Ok(serde_json::json!({ "isolates": [] }))
        }
    }

    let call_order = Arc::new(Mutex::new(Vec::<String>::new()));
    let mock = OrderedMock {
        call_order: call_order.clone(),
    };
    let (mut adapter, mut rx) = DapAdapter::new(mock);
    adapter.debug_sdk_libraries = true;

    // Step 1: Register an isolate via IsolateStart so that setBreakpoints can
    // place an active breakpoint (and therefore record a desired breakpoint
    // with a real DAP ID that survives the subsequent IsolateExit).
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".to_string(),
            name: "main".to_string(),
        })
        .await;
    super::drain_events(&mut rx);

    // Clear the call_order log from the IsolateStart phase so the ordering
    // assertion below is only about the IsolateRunnable phase.
    call_order.lock().unwrap().clear();

    // Step 2: Set a breakpoint with the isolate active — this ensures the
    // desired_breakpoints map is populated with a real DAP ID.
    let bp_req = super::make_set_breakpoints_request(1, "/app/lib/main.dart", &[10]);
    adapter.handle_request(&bp_req).await;
    super::drain_events(&mut rx);

    // Step 3: Simulate hot restart — isolate exits (clears active breakpoints,
    // desired_breakpoints survive).
    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".to_string(),
        })
        .await;
    super::drain_events(&mut rx);

    // Clear the call_order so only IsolateRunnable calls are recorded.
    call_order.lock().unwrap().clear();

    // Step 4: New isolate becomes runnable (hot restart complete).
    adapter
        .handle_debug_event(DebugEvent::IsolateRunnable {
            isolate_id: "isolates/2".to_string(),
        })
        .await;

    let order = call_order.lock().unwrap().clone();

    // Find positions of the critical methods.
    let lib_debug_pos = order
        .iter()
        .position(|s| s == "set_library_debuggable")
        .expect("set_library_debuggable should have been called");
    let add_bp_pos = order
        .iter()
        .position(|s| s == "add_breakpoint")
        .expect("add_breakpoint should have been called");

    assert!(
        lib_debug_pos < add_bp_pos,
        "set_library_debuggable ({lib_debug_pos}) must be called before add_breakpoint ({add_bp_pos})"
    );
}

/// New isolates created during hot restart inherit `debug_sdk_libraries` setting.
#[tokio::test]
async fn test_new_isolate_inherits_debug_sdk_libraries_setting() {
    let libraries = vec![serde_json::json!({ "id": "libraries/1", "uri": "dart:core" })];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);

    // Set SDK debugging on.
    adapter.debug_sdk_libraries = true;

    // Drain events from creation.
    super::drain_events(&mut rx);

    // Simulate a new isolate becoming runnable (e.g., from hot restart).
    adapter
        .handle_debug_event(DebugEvent::IsolateRunnable {
            isolate_id: "isolates/new".to_string(),
        })
        .await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(
        recorded.len(),
        1,
        "one set_library_debuggable call expected"
    );
    assert!(
        recorded[0].2,
        "dart:core should be debuggable because debug_sdk_libraries=true"
    );
}

/// `file://` libraries (app source) are always debuggable.
#[tokio::test]
async fn test_file_uri_libraries_always_debuggable() {
    let libraries = vec![
        serde_json::json!({ "id": "libraries/1", "uri": "file:///app/lib/main.dart" }),
        serde_json::json!({ "id": "libraries/2", "uri": "file:///app/lib/widget.dart" }),
    ];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    // Both SDK and external flags off.
    let req = make_update_debug_options(1, Some(false), Some(false));
    adapter.handle_request(&req).await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2);
    for (_, _, is_debuggable) in &recorded {
        assert!(
            is_debuggable,
            "file:// app libraries must always be debuggable"
        );
    }
}

/// `attach` request initializes `debug_sdk_libraries` from arguments.
#[tokio::test]
async fn test_attach_initializes_debug_sdk_libraries_from_args() {
    let (mut adapter, mut rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    assert!(!adapter.debug_sdk_libraries, "default is false");

    let attach_req = DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "debugSdkLibraries": true,
        })),
    };
    adapter.handle_request(&attach_req).await;
    // Drain any events.
    super::drain_events(&mut rx);

    assert!(
        adapter.debug_sdk_libraries,
        "debug_sdk_libraries should be true after attach"
    );
}

/// `attach` request initializes `debug_external_package_libraries` from arguments.
#[tokio::test]
async fn test_attach_initializes_debug_external_package_libraries_from_args() {
    let (mut adapter, mut rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    assert!(
        !adapter.debug_external_package_libraries,
        "default is false"
    );

    let attach_req = DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "debugExternalPackageLibraries": true,
        })),
    };
    adapter.handle_request(&attach_req).await;
    super::drain_events(&mut rx);

    assert!(
        adapter.debug_external_package_libraries,
        "debug_external_package_libraries should be true after attach"
    );
}

/// `attach` request initializes `app_package_name` from the `packageName` argument.
#[tokio::test]
async fn test_attach_initializes_app_package_name_from_args() {
    let (mut adapter, mut rx) = DapAdapter::new(super::super::test_helpers::MockBackend);
    assert!(adapter.app_package_name.is_empty(), "default is empty");

    let attach_req = DapRequest {
        seq: 1,
        command: "attach".into(),
        arguments: Some(serde_json::json!({
            "packageName": "my_flutter_app",
        })),
    };
    adapter.handle_request(&attach_req).await;
    super::drain_events(&mut rx);

    assert_eq!(
        adapter.app_package_name, "my_flutter_app",
        "app_package_name should be set from attach args"
    );
}

/// Libraries with no URI or empty library ID are silently skipped.
#[tokio::test]
async fn test_apply_library_debuggability_skips_empty_lib_id() {
    let libraries = vec![
        // No "id" field — should be skipped.
        serde_json::json!({ "uri": "dart:core" }),
        // Empty string id — should be skipped.
        serde_json::json!({ "id": "", "uri": "dart:async" }),
        // Valid entry.
        serde_json::json!({ "id": "libraries/3", "uri": "dart:io" }),
    ];
    let (mock, calls) = LibraryDebuggableMock::new(libraries);
    let (mut adapter, mut rx) = DapAdapter::new(mock);
    register_isolate(&mut adapter, &mut rx, "isolates/1").await;

    let req = make_update_debug_options(1, Some(true), None);
    adapter.handle_request(&req).await;

    let recorded = calls.lock().unwrap().clone();
    // Only one valid library should produce a call.
    assert_eq!(
        recorded.len(),
        1,
        "only valid (non-empty id) libraries should be processed"
    );
    assert_eq!(recorded[0].1, "libraries/3");
}
