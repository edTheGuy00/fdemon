//! Flutter service extension call infrastructure.
//!
//! This module provides the building blocks for calling Flutter service extensions
//! via the VM Service protocol, including:
//!
//! - [`ext`] — Constants for all known Flutter extension method names.
//! - [`parse_bool_extension_response`] — Parse `{"enabled": "true"|"false"}` responses.
//! - [`parse_data_extension_response`] — Parse `{"data": "..."}` responses.
//! - [`is_extension_not_available`] — Detect "method not found" VM Service errors.
//! - [`ObjectGroupManager`] — Manage Widget Inspector object group lifecycle.
//!
//! ## Protocol Notes
//!
//! All Flutter service extension calls follow the same pattern:
//! - Method name: `ext.flutter.<name>` or `ext.flutter.inspector.<name>`
//! - Required param: `isolateId` (the main UI isolate ID)
//! - All additional param values must be strings (VM Service protocol requirement)
//!
//! When an extension is not available (profile/release mode, or not yet registered),
//! the VM Service returns a JSON-RPC error with code `-32601` ("Method not found").
//! Use [`is_extension_not_available`] to distinguish this from connection errors.

use std::collections::HashMap;

use fdemon_core::prelude::*;
use fdemon_core::widget_tree::DiagnosticsNode;
use serde_json::{json, Value};

use super::protocol::VmServiceError;

// Re-export VmServiceClient so submodules can use `super::VmServiceClient`
// instead of the longer `super::super::client::VmServiceClient` path.
pub(super) use super::client::VmServiceClient;

pub mod dumps;
pub mod inspector;
pub mod layout;
pub mod overlays;

// Re-export all public items from submodules so the public API is flat.
pub use dumps::{
    debug_dump, debug_dump_app, debug_dump_layer_tree, debug_dump_render_tree, DebugDumpKind,
};
pub use inspector::{
    get_details_subtree, get_root_widget_tree, get_selected_widget, ObjectGroupManager,
    WidgetInspector,
};
pub use layout::{extract_layout_info, extract_layout_tree, fetch_layout_data, get_layout_node};
pub use overlays::{
    debug_paint, flip_overlay, performance_overlay, query_all_overlays, repaint_rainbow,
    toggle_bool_extension, widget_inspector, DebugOverlayState,
};

// ---------------------------------------------------------------------------
// Extension method name constants
// ---------------------------------------------------------------------------

/// Constants for Flutter service extension method names.
///
/// All extension names follow the `ext.flutter.*` namespace convention.
pub mod ext {
    // ── Debug overlays ──────────────────────────────────────────────────────

    /// Toggle the repaint rainbow debug overlay.
    pub const REPAINT_RAINBOW: &str = "ext.flutter.repaintRainbow";

    /// Toggle the debug paint overlay (widget boundaries, padding, etc.).
    pub const DEBUG_PAINT: &str = "ext.flutter.debugPaint";

    /// Toggle the performance overlay (frame rendering timings).
    pub const SHOW_PERFORMANCE_OVERLAY: &str = "ext.flutter.showPerformanceOverlay";

    /// Toggle the Widget Inspector show mode.
    pub const INSPECTOR_SHOW: &str = "ext.flutter.inspector.show";

    // ── Widget inspector ────────────────────────────────────────────────────

    /// Get the full widget tree from the root.
    pub const GET_ROOT_WIDGET_TREE: &str = "ext.flutter.inspector.getRootWidgetTree";

    /// Get the root widget summary tree (collapsed subtrees).
    pub const GET_ROOT_WIDGET_SUMMARY_TREE: &str = "ext.flutter.inspector.getRootWidgetSummaryTree";

    /// Get the details subtree for a specific node.
    pub const GET_DETAILS_SUBTREE: &str = "ext.flutter.inspector.getDetailsSubtree";

    /// Get the currently selected widget in the inspector.
    pub const GET_SELECTED_WIDGET: &str = "ext.flutter.inspector.getSelectedWidget";

    /// Dispose a named object group, releasing all its references.
    pub const DISPOSE_GROUP: &str = "ext.flutter.inspector.disposeGroup";

    // ── Layout explorer ─────────────────────────────────────────────────────

    /// Get the layout explorer node data for a widget.
    pub const GET_LAYOUT_EXPLORER_NODE: &str = "ext.flutter.inspector.getLayoutExplorerNode";

    /// Check whether the widget tree is ready to be fetched.
    ///
    /// Returns `{"result": true}` when the framework has completed its first
    /// frame and the widget tree can be safely introspected. Polling this
    /// before `getRootWidgetTree` avoids transient null-check failures on
    /// complex or freshly-reloaded widget trees.
    pub const IS_WIDGET_TREE_READY: &str = "ext.flutter.inspector.isWidgetTreeReady";

    // ── Debug dumps ─────────────────────────────────────────────────────────

    /// Dump the widget tree to a string.
    pub const DEBUG_DUMP_APP: &str = "ext.flutter.debugDumpApp";

    /// Dump the render tree to a string.
    pub const DEBUG_DUMP_RENDER_TREE: &str = "ext.flutter.debugDumpRenderTree";

    /// Dump the layer tree to a string.
    pub const DEBUG_DUMP_LAYER_TREE: &str = "ext.flutter.debugDumpLayerTree";

    // ── Network Profiling (ext.dart.io) ──────────────────────────────────────

    /// Enable or disable HTTP timeline logging.
    ///
    /// Must be set to `true` before `getHttpProfile` returns data.
    pub const HTTP_ENABLE_TIMELINE_LOGGING: &str = "ext.dart.io.httpEnableTimelineLogging";

    /// Fetch the HTTP profile (list of recorded HTTP requests).
    ///
    /// Supports an optional `updatedSince` parameter (microseconds since epoch)
    /// for incremental polling.
    pub const GET_HTTP_PROFILE: &str = "ext.dart.io.getHttpProfile";

    /// Fetch full details for a single HTTP request (headers, bodies, events).
    ///
    /// Requires an `id` parameter matching the request ID from `getHttpProfile`.
    pub const GET_HTTP_PROFILE_REQUEST: &str = "ext.dart.io.getHttpProfileRequest";

    /// Clear all recorded HTTP profile data.
    pub const CLEAR_HTTP_PROFILE: &str = "ext.dart.io.clearHttpProfile";

    /// Fetch socket profiling statistics.
    pub const GET_SOCKET_PROFILE: &str = "ext.dart.io.getSocketProfile";

    /// Enable or disable socket profiling.
    pub const SOCKET_PROFILING_ENABLED: &str = "ext.dart.io.socketProfilingEnabled";

    /// Get the dart:io version string.
    pub const GET_DART_IO_VERSION: &str = "ext.dart.io.getVersion";
}

// ---------------------------------------------------------------------------
// Response parsing helpers
// ---------------------------------------------------------------------------

/// Parse a boolean toggle response from a Flutter service extension.
///
/// Flutter toggle extensions return `{"enabled": "true"}` or `{"enabled": "false"}`.
/// Note: the value is always a **string**, not a JSON boolean.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the `"enabled"` field is missing or not a string.
pub fn parse_bool_extension_response(result: &Value) -> Result<bool> {
    result
        .get("enabled")
        .and_then(|v| v.as_str())
        .map(|s| s == "true")
        .ok_or_else(|| Error::protocol("missing 'enabled' field in extension response"))
}

/// Parse a string data response from a Flutter service extension.
///
/// Debug dump extensions return `{"data": "<string content>"}`.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the `"data"` field is missing or not a string.
pub fn parse_data_extension_response(result: &Value) -> Result<String> {
    result
        .get("data")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::protocol("missing 'data' field in extension response"))
}

// ---------------------------------------------------------------------------
// Extension availability detection
// ---------------------------------------------------------------------------

/// JSON-RPC error code for "Method not found".
///
/// The VM Service returns this code when an extension method is not registered,
/// which happens in profile/release builds or before the extension is activated.
const METHOD_NOT_FOUND_CODE: i32 = -32601;

/// VM Service error code for "Extension not available" (non-standard, used by some implementations).
const EXTENSION_NOT_AVAILABLE_CODE: i32 = 113;

/// Check whether a [`VmServiceError`] indicates an unavailable extension.
///
/// Returns `true` when the VM Service error indicates that the requested extension
/// method is not registered. This is distinct from connection errors and typically
/// means the app was built in profile or release mode, or the extension has not
/// yet been registered by the framework.
///
/// # Example
///
/// ```ignore
/// match client.call_extension("ext.flutter.repaintRainbow", &isolate_id, None).await {
///     Ok(result) => { /* handle result */ }
///     Err(Error::Protocol(msg)) => {
///         // Parse the VmServiceError from the message to check availability
///     }
///     Err(e) => return Err(e),
/// }
/// ```
pub fn is_extension_not_available(error: &VmServiceError) -> bool {
    if error.code == METHOD_NOT_FOUND_CODE {
        return true;
    }

    // Some VM Service implementations return code 113 ("Extension not available")
    // or code -32000 with a "method not found" message.
    if error.code == EXTENSION_NOT_AVAILABLE_CODE {
        return true;
    }

    // Fallback: check the message text for known "not found" patterns.
    let msg = error.message.to_lowercase();
    msg.contains("method not found") || msg.contains("extension not available")
}

// ---------------------------------------------------------------------------
// DiagnosticsNode response parsing
// ---------------------------------------------------------------------------

/// Parse a [`DiagnosticsNode`] from an extension response value.
///
/// The VM Service wraps extension responses. After `call_extension` unwraps
/// the JSON-RPC envelope, the node may be:
/// 1. The value itself (direct result from the extension)
/// 2. Nested under a `"result"` field (some Flutter versions add this extra wrapper)
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the value cannot be deserialized as a
/// [`DiagnosticsNode`].
pub fn parse_diagnostics_node_response(value: &Value) -> Result<DiagnosticsNode> {
    let node_value = value.get("result").unwrap_or(value);
    serde_json::from_value(node_value.clone())
        .map_err(|e| Error::protocol(format!("failed to parse DiagnosticsNode: {e}")))
}

/// Parse an optional [`DiagnosticsNode`] from an extension response value.
///
/// Returns `Ok(None)` if the result is JSON null (e.g., no widget is currently
/// selected). Otherwise delegates to [`parse_diagnostics_node_response`].
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the value is non-null but cannot be
/// deserialized as a [`DiagnosticsNode`].
pub fn parse_optional_diagnostics_node_response(value: &Value) -> Result<Option<DiagnosticsNode>> {
    let node_value = value.get("result").unwrap_or(value);
    if node_value.is_null() {
        return Ok(None);
    }
    // Use `node_value` directly — it already has the "result" wrapper stripped.
    // Delegating to `parse_diagnostics_node_response(value)` would cause a
    // double-extraction attempt when the wrapper is present.
    serde_json::from_value(node_value.clone())
        .map(Some)
        .map_err(|e| Error::protocol(format!("failed to parse DiagnosticsNode: {e}")))
}

// ---------------------------------------------------------------------------
// call_extension params builder
// ---------------------------------------------------------------------------

/// Build a `serde_json::Value::Object` params map for a `call_extension` call.
///
/// Inserts `isolateId` and all entries from `args`.
pub(super) fn build_extension_params(
    isolate_id: &str,
    args: Option<HashMap<String, String>>,
) -> serde_json::Value {
    let mut params = serde_json::Map::new();
    params.insert("isolateId".to_string(), json!(isolate_id));
    if let Some(extra) = args {
        for (k, v) in extra {
            params.insert(k, json!(v));
        }
    }
    serde_json::Value::Object(params)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── parse_bool_extension_response ───────────────────────────────────────

    #[test]
    fn test_parse_bool_response_true() {
        let json = json!({"enabled": "true", "type": "_extensionType"});
        assert_eq!(parse_bool_extension_response(&json).unwrap(), true);
    }

    #[test]
    fn test_parse_bool_response_false() {
        let json = json!({"enabled": "false", "type": "_extensionType"});
        assert_eq!(parse_bool_extension_response(&json).unwrap(), false);
    }

    #[test]
    fn test_parse_bool_response_missing_field_returns_error() {
        let json = json!({"other": "value"});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_bool_response_null_field_returns_error() {
        let json = json!({"enabled": null});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_bool_response_numeric_returns_error() {
        // enabled must be a string, not a JSON bool or number
        let json = json!({"enabled": true});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_bool_response_arbitrary_string_is_false() {
        // Only "true" maps to true; anything else is false
        let json = json!({"enabled": "yes"});
        assert_eq!(parse_bool_extension_response(&json).unwrap(), false);
    }

    // ── parse_data_extension_response ───────────────────────────────────────

    #[test]
    fn test_parse_data_response_returns_string() {
        let json = json!({"data": "Widget tree dump..."});
        assert_eq!(
            parse_data_extension_response(&json).unwrap(),
            "Widget tree dump..."
        );
    }

    #[test]
    fn test_parse_data_response_empty_string() {
        let json = json!({"data": ""});
        assert_eq!(parse_data_extension_response(&json).unwrap(), "");
    }

    #[test]
    fn test_parse_data_response_missing_field_returns_error() {
        let json = json!({"other": "value"});
        assert!(parse_data_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_data_response_null_returns_error() {
        let json = json!({"data": null});
        assert!(parse_data_extension_response(&json).is_err());
    }

    // ── is_extension_not_available ──────────────────────────────────────────

    #[test]
    fn test_is_extension_not_available_method_not_found_code() {
        let error = VmServiceError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        };
        assert!(is_extension_not_available(&error));
    }

    #[test]
    fn test_is_extension_not_available_code_113() {
        let error = VmServiceError {
            code: 113,
            message: "Extension not available".to_string(),
            data: None,
        };
        assert!(is_extension_not_available(&error));
    }

    #[test]
    fn test_is_extension_not_available_message_fallback() {
        let error = VmServiceError {
            code: -32000,
            message: "Method not found: ext.flutter.repaintRainbow".to_string(),
            data: None,
        };
        assert!(is_extension_not_available(&error));
    }

    #[test]
    fn test_is_extension_not_available_false_for_other_errors() {
        let error = VmServiceError {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        };
        assert!(!is_extension_not_available(&error));
    }

    #[test]
    fn test_is_extension_not_available_false_for_channel_closed() {
        let error = VmServiceError {
            code: 100,
            message: "Server Error".to_string(),
            data: None,
        };
        assert!(!is_extension_not_available(&error));
    }

    // ── extension constants ─────────────────────────────────────────────────

    #[test]
    fn test_extension_constants_use_correct_prefix() {
        // All constants must start with "ext.flutter."
        assert!(ext::REPAINT_RAINBOW.starts_with("ext.flutter."));
        assert!(ext::DEBUG_PAINT.starts_with("ext.flutter."));
        assert!(ext::SHOW_PERFORMANCE_OVERLAY.starts_with("ext.flutter."));
        assert!(ext::INSPECTOR_SHOW.starts_with("ext.flutter."));
        assert!(ext::DEBUG_DUMP_APP.starts_with("ext.flutter."));
        assert!(ext::DEBUG_DUMP_RENDER_TREE.starts_with("ext.flutter."));
        assert!(ext::DEBUG_DUMP_LAYER_TREE.starts_with("ext.flutter."));
    }

    #[test]
    fn test_inspector_extension_constants_use_inspector_prefix() {
        assert!(ext::GET_ROOT_WIDGET_TREE.starts_with("ext.flutter.inspector."));
        assert!(ext::GET_ROOT_WIDGET_SUMMARY_TREE.starts_with("ext.flutter.inspector."));
        assert!(ext::GET_DETAILS_SUBTREE.starts_with("ext.flutter.inspector."));
        assert!(ext::GET_SELECTED_WIDGET.starts_with("ext.flutter.inspector."));
        assert!(ext::DISPOSE_GROUP.starts_with("ext.flutter.inspector."));
        assert!(ext::GET_LAYOUT_EXPLORER_NODE.starts_with("ext.flutter.inspector."));
        assert!(ext::IS_WIDGET_TREE_READY.starts_with("ext.flutter.inspector."));
    }

    // ── parse_bool_extension_response (task-specified tests) ────────────────

    #[test]
    fn test_parse_bool_response_enabled_true() {
        let json = json!({
            "type": "_extensionType",
            "method": "ext.flutter.repaintRainbow",
            "enabled": "true"
        });
        assert!(parse_bool_extension_response(&json).unwrap());
    }

    #[test]
    fn test_parse_bool_response_enabled_false() {
        let json = json!({
            "type": "_extensionType",
            "method": "ext.flutter.repaintRainbow",
            "enabled": "false"
        });
        assert!(!parse_bool_extension_response(&json).unwrap());
    }

    #[test]
    fn test_parse_bool_response_missing_enabled() {
        let json = json!({"type": "_extensionType"});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_bool_response_json_bool_true_returns_error() {
        // VM Service protocol requires strings, not JSON booleans.
        // A JSON boolean `true` is not a string, so as_str() returns None.
        let json = json!({"enabled": true});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_bool_response_json_bool_false_returns_error() {
        // VM Service protocol requires strings, not JSON booleans.
        let json = json!({"enabled": false});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    // ── build_extension_params ──────────────────────────────────────────────

    #[test]
    fn test_build_extension_params_includes_isolate_id() {
        let params = build_extension_params("isolates/123", None);
        assert_eq!(params["isolateId"], "isolates/123");
    }

    #[test]
    fn test_build_extension_params_includes_extra_args() {
        let mut args = HashMap::new();
        args.insert("objectGroup".to_string(), "test-group".to_string());
        let params = build_extension_params("isolates/123", Some(args));
        assert_eq!(params["isolateId"], "isolates/123");
        assert_eq!(params["objectGroup"], "test-group");
    }

    #[test]
    fn test_build_extension_params_no_args() {
        let params = build_extension_params("isolates/456", None);
        let obj = params.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert!(obj.contains_key("isolateId"));
    }
}
