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
use fdemon_core::widget_tree::{BoxConstraints, DiagnosticsNode, LayoutInfo, WidgetSize};
use serde_json::{json, Value};

use super::protocol::VmServiceError;

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

    // ── Debug dumps ─────────────────────────────────────────────────────────

    /// Dump the widget tree to a string.
    pub const DEBUG_DUMP_APP: &str = "ext.flutter.debugDumpApp";

    /// Dump the render tree to a string.
    pub const DEBUG_DUMP_RENDER_TREE: &str = "ext.flutter.debugDumpRenderTree";

    /// Dump the layer tree to a string.
    pub const DEBUG_DUMP_LAYER_TREE: &str = "ext.flutter.debugDumpLayerTree";
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
    if error.code == 113 {
        return true;
    }

    // Fallback: check the message text for known "not found" patterns.
    let msg = error.message.to_lowercase();
    msg.contains("method not found") || msg.contains("extension not available")
}

// ---------------------------------------------------------------------------
// Object Group Manager
// ---------------------------------------------------------------------------

/// Manages object groups for the Widget Inspector.
///
/// The Widget Inspector uses *object groups* to scope the lifetime of object
/// references returned by inspector calls. References (`valueId`) are only
/// valid while their group exists. When a group is disposed, all references
/// fetched under that group become invalid.
///
/// [`ObjectGroupManager`] tracks a single active group and automatically
/// disposes the previous group when a new one is created, preventing reference
/// leaks.
///
/// ## Usage
///
/// ```ignore
/// let mut group_mgr = ObjectGroupManager::new(client.clone(), isolate_id.clone());
///
/// // Create a group before fetching widget tree data
/// let group_name = group_mgr.create_group().await?;
///
/// // Pass group_name in extension params
/// let result = client.call_extension(
///     ext::GET_ROOT_WIDGET_SUMMARY_TREE,
///     &isolate_id,
///     Some([("groupName".to_string(), group_name)].into()),
/// ).await?;
///
/// // Next create_group() automatically disposes the previous group
/// let _new_group = group_mgr.create_group().await?;
/// ```
pub struct ObjectGroupManager {
    client: super::client::VmServiceClient,
    isolate_id: String,
    active_group: Option<String>,
    group_counter: u32,
}

impl ObjectGroupManager {
    /// Create a new [`ObjectGroupManager`] for the given client and isolate.
    pub fn new(client: super::client::VmServiceClient, isolate_id: String) -> Self {
        Self {
            client,
            isolate_id,
            active_group: None,
            group_counter: 0,
        }
    }

    /// Create a new object group and return its name.
    ///
    /// If a previous active group exists, it is disposed before creating the
    /// new one. The new group name is stored as the active group.
    ///
    /// # Errors
    ///
    /// Returns an error if the previous group cannot be disposed (non-fatal
    /// in most cases — the caller may choose to continue).
    pub async fn create_group(&mut self) -> Result<String> {
        if let Some(old) = self.active_group.take() {
            self.dispose_group(&old).await?;
        }
        self.group_counter += 1;
        let name = format!("fdemon-inspector-{}", self.group_counter);
        self.active_group = Some(name.clone());
        Ok(name)
    }

    /// Dispose a named object group via `ext.flutter.inspector.disposeGroup`.
    ///
    /// This releases all object references that were fetched while the group
    /// was active. After this call, any `valueId` obtained under `group_name`
    /// is no longer valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension call fails (e.g., transport error).
    /// An "extension not available" error is treated as non-fatal and logged.
    pub async fn dispose_group(&self, group_name: &str) -> Result<()> {
        let mut args = HashMap::new();
        args.insert("objectGroup".to_string(), group_name.to_string());

        match self
            .client
            .call_extension(ext::DISPOSE_GROUP, &self.isolate_id, Some(args))
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::debug!(
                    "ObjectGroupManager: failed to dispose group '{}': {}",
                    group_name,
                    e
                );
                Err(e)
            }
        }
    }

    /// Return the current active group name, if any.
    pub fn active_group(&self) -> Option<&str> {
        self.active_group.as_deref()
    }

    /// Return the number of groups created so far (monotonically increasing).
    pub fn group_counter(&self) -> u32 {
        self.group_counter
    }

    /// Dispose the active object group (if any) and clear the active group.
    ///
    /// After this call [`active_group`][Self::active_group] returns `None`.
    /// All `valueId` references fetched under the disposed group are invalid.
    ///
    /// A no-op if there is no active group.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension call to dispose the group fails.
    pub async fn dispose_all(&mut self, _client: &super::client::VmServiceClient) -> Result<()> {
        if let Some(group) = self.active_group.take() {
            self.dispose_group(&group).await?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Debug overlay state
// ---------------------------------------------------------------------------

/// Current state of all Flutter debug overlay extensions.
///
/// Each field is `Option<bool>` because the state is unknown until the first
/// query. `None` means the state has not yet been queried or the extension is
/// unavailable (e.g., profile/release build).
#[derive(Debug, Clone, Default)]
pub struct DebugOverlayState {
    /// Whether the repaint rainbow overlay is enabled.
    pub repaint_rainbow: Option<bool>,
    /// Whether the debug paint overlay is enabled.
    pub debug_paint: Option<bool>,
    /// Whether the performance overlay is enabled.
    pub performance_overlay: Option<bool>,
    /// Whether the widget inspector overlay is enabled.
    pub widget_inspector: Option<bool>,
}

// ---------------------------------------------------------------------------
// Toggle helpers
// ---------------------------------------------------------------------------

/// Toggle or query a boolean debug overlay extension.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error (e.g.,
///   the extension is not available in profile/release mode).
/// - [`Error::ChannelClosed`] if the VM Service client connection is closed.
pub async fn toggle_bool_extension(
    client: &super::client::VmServiceClient,
    method: &str,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    let args = enabled.map(|e| {
        let mut m = HashMap::new();
        m.insert("enabled".to_string(), e.to_string());
        m
    });
    let result = client.call_extension(method, isolate_id, args).await?;
    parse_bool_extension_response(&result)
}

/// Toggle or query the repaint rainbow overlay.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn repaint_rainbow(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::REPAINT_RAINBOW, isolate_id, enabled).await
}

/// Toggle or query the debug paint overlay.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn debug_paint(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::DEBUG_PAINT, isolate_id, enabled).await
}

/// Toggle or query the performance overlay on the device.
///
/// Available in debug and profile mode.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn performance_overlay(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::SHOW_PERFORMANCE_OVERLAY, isolate_id, enabled).await
}

/// Toggle or query the widget inspector overlay.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn widget_inspector(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::INSPECTOR_SHOW, isolate_id, enabled).await
}

// ---------------------------------------------------------------------------
// Bulk query
// ---------------------------------------------------------------------------

/// Query the current state of all debug overlays.
///
/// Queries all 4 overlay extensions concurrently. Individual failures are
/// silently captured as `None` (extension unavailable), allowing partial
/// results in mixed-mode builds (e.g., performance overlay is available in
/// profile mode while the others are debug-only).
///
/// # Returns
///
/// A [`DebugOverlayState`] where each field is `Some(bool)` if the extension
/// responded successfully, or `None` if the extension is unavailable or the
/// call failed.
pub async fn query_all_overlays(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
) -> DebugOverlayState {
    DebugOverlayState {
        repaint_rainbow: repaint_rainbow(client, isolate_id, None).await.ok(),
        debug_paint: debug_paint(client, isolate_id, None).await.ok(),
        performance_overlay: performance_overlay(client, isolate_id, None).await.ok(),
        widget_inspector: widget_inspector(client, isolate_id, None).await.ok(),
    }
}

// ---------------------------------------------------------------------------
// Convenience flip
// ---------------------------------------------------------------------------

/// Toggle an overlay to the opposite of its current state.
///
/// Makes two RPC calls: one to read the current state, one to set the
/// opposite. Returns the new state after the flip.
///
/// # Errors
///
/// - Returns an error if the read call fails (e.g., extension unavailable).
/// - Returns an error if the write call fails.
pub async fn flip_overlay(
    client: &super::client::VmServiceClient,
    method: &str,
    isolate_id: &str,
) -> Result<bool> {
    let current = toggle_bool_extension(client, method, isolate_id, None).await?;
    toggle_bool_extension(client, method, isolate_id, Some(!current)).await
}

// ---------------------------------------------------------------------------
// Debug dump extensions
// ---------------------------------------------------------------------------

/// Which debug tree to dump as text.
///
/// Each variant corresponds to a Flutter service extension that formats an
/// internal Flutter tree as a multi-line string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugDumpKind {
    /// Widget tree (`debugDumpApp`) — available in debug and profile mode.
    WidgetTree,
    /// Render tree (`debugDumpRenderTree`) — available in debug and profile mode.
    RenderTree,
    /// Layer tree (`debugDumpLayerTree`) — debug mode only.
    LayerTree,
}

impl DebugDumpKind {
    /// Get the extension method name for this dump kind.
    pub fn method(&self) -> &'static str {
        match self {
            Self::WidgetTree => ext::DEBUG_DUMP_APP,
            Self::RenderTree => ext::DEBUG_DUMP_RENDER_TREE,
            Self::LayerTree => ext::DEBUG_DUMP_LAYER_TREE,
        }
    }

    /// Whether this dump is available in profile mode.
    ///
    /// `WidgetTree` and `RenderTree` are available in both debug and profile
    /// mode. `LayerTree` is debug mode only.
    pub fn available_in_profile(&self) -> bool {
        match self {
            Self::WidgetTree | Self::RenderTree => true,
            Self::LayerTree => false,
        }
    }
}

/// Dump the widget tree as formatted text.
///
/// Returns the same output as `debugDumpApp()` — a multiline text dump of
/// all widgets in the tree with their properties.
///
/// Available in debug and profile mode.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn debug_dump_app(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
) -> Result<String> {
    let result = client
        .call_extension(ext::DEBUG_DUMP_APP, isolate_id, None)
        .await?;
    parse_data_extension_response(&result)
}

/// Dump the render tree as formatted text.
///
/// Returns the same output as `debugDumpRenderTree()` — a multiline text dump
/// of all render objects with their constraints, sizes, and painting details.
///
/// Available in debug and profile mode.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn debug_dump_render_tree(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
) -> Result<String> {
    let result = client
        .call_extension(ext::DEBUG_DUMP_RENDER_TREE, isolate_id, None)
        .await?;
    parse_data_extension_response(&result)
}

/// Dump the layer tree as formatted text.
///
/// Returns the same output as `debugDumpLayerTree()` — a multiline text dump
/// of all compositing layers with their properties.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn debug_dump_layer_tree(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
) -> Result<String> {
    let result = client
        .call_extension(ext::DEBUG_DUMP_LAYER_TREE, isolate_id, None)
        .await?;
    parse_data_extension_response(&result)
}

/// Run a debug dump by kind.
///
/// A convenience wrapper around the three individual dump functions. The
/// `kind` argument selects which tree is dumped. Truncation or pagination of
/// the returned string is the caller's responsibility — complex apps can
/// produce thousands of lines.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn debug_dump(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    kind: DebugDumpKind,
) -> Result<String> {
    let result = client
        .call_extension(kind.method(), isolate_id, None)
        .await?;
    parse_data_extension_response(&result)
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
    parse_diagnostics_node_response(value).map(Some)
}

// ---------------------------------------------------------------------------
// Widget inspector extension functions
// ---------------------------------------------------------------------------

/// Fetch the root widget summary tree.
///
/// Uses `ext.flutter.inspector.getRootWidgetTree` (Flutter 3.22+) with an
/// automatic fallback to `ext.flutter.inspector.getRootWidgetSummaryTree` for
/// older Flutter versions.
///
/// Returns the root [`DiagnosticsNode`] with children populated.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// Returns an error if both the newer and the older API fail, or if the
/// response cannot be parsed as a [`DiagnosticsNode`].
pub async fn get_root_widget_tree(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    object_group: &str,
) -> Result<DiagnosticsNode> {
    // Build args for the newer getRootWidgetTree API.
    let mut newer_args = HashMap::new();
    newer_args.insert("objectGroup".to_string(), object_group.to_string());
    newer_args.insert("isSummaryTree".to_string(), "true".to_string());
    newer_args.insert("withPreviews".to_string(), "false".to_string());

    let result = client
        .call_extension(ext::GET_ROOT_WIDGET_TREE, isolate_id, Some(newer_args))
        .await;

    match result {
        Ok(value) => parse_diagnostics_node_response(&value),
        Err(_) => {
            // Fallback: try the older getRootWidgetSummaryTree API.
            let mut older_args = HashMap::new();
            older_args.insert("objectGroup".to_string(), object_group.to_string());

            let value = client
                .call_extension(
                    ext::GET_ROOT_WIDGET_SUMMARY_TREE,
                    isolate_id,
                    Some(older_args),
                )
                .await?;
            parse_diagnostics_node_response(&value)
        }
    }
}

/// Fetch a detailed subtree for a specific widget node.
///
/// `value_id` is the `valueId` field from a previously fetched
/// [`DiagnosticsNode`]. The `subtree_depth` controls how many levels of
/// children to include (recommended: `2`).
///
/// Returns a [`DiagnosticsNode`] with full properties and children populated
/// up to the specified depth.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// Returns an error if the extension call fails or the response cannot be
/// parsed as a [`DiagnosticsNode`].
pub async fn get_details_subtree(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    value_id: &str,
    object_group: &str,
    subtree_depth: u32,
) -> Result<DiagnosticsNode> {
    // Note: this extension uses "arg" (not "valueId" or "id") for the widget ID.
    let mut args = HashMap::new();
    args.insert("arg".to_string(), value_id.to_string());
    args.insert("objectGroup".to_string(), object_group.to_string());
    args.insert("subtreeDepth".to_string(), subtree_depth.to_string());

    let result = client
        .call_extension(ext::GET_DETAILS_SUBTREE, isolate_id, Some(args))
        .await?;
    parse_diagnostics_node_response(&result)
}

/// Fetch the currently selected widget in the inspector overlay.
///
/// Returns `Ok(Some(node))` if a widget is selected, `Ok(None)` if nothing
/// is currently selected (e.g., the user has not tapped a widget in the
/// inspector overlay).
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// Returns an error if the extension call fails or the response cannot be
/// parsed.
pub async fn get_selected_widget(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    object_group: &str,
) -> Result<Option<DiagnosticsNode>> {
    let mut args = HashMap::new();
    args.insert("objectGroup".to_string(), object_group.to_string());

    let result = client
        .call_extension(ext::GET_SELECTED_WIDGET, isolate_id, Some(args))
        .await?;
    parse_optional_diagnostics_node_response(&result)
}

// ---------------------------------------------------------------------------
// Layout Explorer extension functions
// ---------------------------------------------------------------------------

/// Fetch layout explorer data for a widget node.
///
/// Returns a [`DiagnosticsNode`] enriched with layout-specific properties:
/// constraints, size, flex factor, flex fit, and child layout info.
///
/// `value_id` is the `valueId` from a previously fetched [`DiagnosticsNode`].
/// `subtree_depth` controls how many levels of children to include.
///
/// **Important:** The layout explorer uses different parameter keys than other
/// inspector extensions. It uses `id` (not `arg`) for the widget ID and
/// `groupName` (not `objectGroup`) for the object group. This is a quirk of
/// the Flutter framework that must be matched exactly.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// Returns an error if the extension call fails or the response cannot be
/// parsed as a [`DiagnosticsNode`].
pub async fn get_layout_explorer_node(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    value_id: &str,
    group_name: &str,
    subtree_depth: u32,
) -> Result<DiagnosticsNode> {
    let mut args = HashMap::new();
    // NOTE: Layout explorer uses "id" and "groupName", NOT "arg" and "objectGroup"
    // like other inspector extensions. This is a real inconsistency in the Flutter
    // framework and the keys must match exactly.
    args.insert("id".to_string(), value_id.to_string());
    args.insert("groupName".to_string(), group_name.to_string());
    args.insert("subtreeDepth".to_string(), subtree_depth.to_string());

    let result = client
        .call_extension(ext::GET_LAYOUT_EXPLORER_NODE, isolate_id, Some(args))
        .await?;
    parse_diagnostics_node_response(&result)
}

/// Extract layout information from a [`DiagnosticsNode`] returned by the layout explorer.
///
/// The layout explorer returns a standard [`DiagnosticsNode`] but with additional
/// render-specific fields (`constraints`, `size`, `flexFactor`, `flexFit`) that are
/// not part of the base DiagnosticsNode schema.
///
/// Pass both the parsed node (for its `description`) and the raw JSON value from
/// which the layout fields are read directly.
pub fn extract_layout_info(node: &DiagnosticsNode, raw_json: &Value) -> LayoutInfo {
    LayoutInfo {
        description: Some(node.description.clone()),
        constraints: raw_json
            .get("constraints")
            .and_then(|c| c.get("description"))
            .and_then(|d| d.as_str())
            .and_then(BoxConstraints::parse),
        size: parse_widget_size(raw_json),
        flex_factor: raw_json.get("flexFactor").and_then(|v| {
            // flexFactor may be a JSON number or string depending on Flutter version.
            // null means the widget is not a flex child — return None (not 0).
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        }),
        flex_fit: raw_json
            .get("flexFit")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

/// Parse a widget size from the `"size"` field of a layout explorer JSON response.
///
/// Size values may be JSON strings (`"100.0"`) or JSON numbers (`100.0`) depending
/// on the Flutter version. Both forms are handled defensively.
///
/// Returns `None` if the `"size"` field is absent or either dimension cannot be parsed.
fn parse_widget_size(json: &Value) -> Option<WidgetSize> {
    let size = json.get("size")?;
    let width = size.get("width").and_then(|v| {
        v.as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| v.as_f64())
    })?;
    let height = size.get("height").and_then(|v| {
        v.as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| v.as_f64())
    })?;
    Some(WidgetSize { width, height })
}

/// Extract layout info for a node and all its direct children from a layout explorer response.
///
/// The raw JSON may be the complete response envelope (with a `"result"` wrapper) or the
/// result value directly. Both forms are handled.
///
/// Returns a [`Vec<LayoutInfo>`] where index 0 is the root node and subsequent entries
/// correspond to its children in order.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the root node JSON cannot be deserialized as a
/// [`DiagnosticsNode`].
pub fn extract_layout_tree(raw_json: &Value) -> Result<Vec<LayoutInfo>> {
    let result_value = raw_json.get("result").unwrap_or(raw_json);

    // Parse and collect root node layout info.
    let root_node: DiagnosticsNode = serde_json::from_value(result_value.clone()).map_err(|e| {
        Error::protocol(format!(
            "failed to parse root DiagnosticsNode in layout tree: {e}"
        ))
    })?;
    let mut layouts = vec![extract_layout_info(&root_node, result_value)];

    // Collect layout info for each direct child.
    if let Some(children) = result_value.get("children").and_then(|c| c.as_array()) {
        for child_json in children {
            if let Ok(child_node) = serde_json::from_value::<DiagnosticsNode>(child_json.clone()) {
                layouts.push(extract_layout_info(&child_node, child_json));
            }
        }
    }

    Ok(layouts)
}

/// Fetch complete layout data for a widget and its direct children.
///
/// This is the high-level entry point for the Layout Explorer feature. It issues a
/// single extension call with `subtreeDepth = 1` to retrieve both the widget tree
/// structure and layout properties for the target widget and its direct children.
///
/// Returns a tuple of:
/// - The [`DiagnosticsNode`] tree (structure, description, `valueId` etc.)
/// - A [`Vec<LayoutInfo>`] with layout properties for the root node and each child
///   (index 0 = root, subsequent entries = children in order)
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// Returns an error if the extension call fails, or if the response cannot be
/// parsed.
pub async fn fetch_layout_data(
    client: &super::client::VmServiceClient,
    isolate_id: &str,
    value_id: &str,
    group_name: &str,
) -> Result<(DiagnosticsNode, Vec<LayoutInfo>)> {
    let mut args = HashMap::new();
    // Use the layout-explorer-specific param keys (not "arg"/"objectGroup").
    args.insert("id".to_string(), value_id.to_string());
    args.insert("groupName".to_string(), group_name.to_string());
    args.insert("subtreeDepth".to_string(), "1".to_string());

    let raw_result = client
        .call_extension(ext::GET_LAYOUT_EXPLORER_NODE, isolate_id, Some(args))
        .await?;

    let node = parse_diagnostics_node_response(&raw_result)?;
    let layouts = extract_layout_tree(&raw_result)?;

    Ok((node, layouts))
}

// ---------------------------------------------------------------------------
// WidgetInspector
// ---------------------------------------------------------------------------

/// High-level widget inspector that manages object groups automatically.
///
/// Object groups scope the lifetime of references returned by inspector calls.
/// When a group is disposed, all `valueId` references fetched under that group
/// become invalid. [`WidgetInspector`] automates group lifecycle so callers
/// don't need to manage this manually.
///
/// ## Usage
///
/// ```ignore
/// let client = VmServiceClient::connect("ws://127.0.0.1:8181/ws").await?;
/// let isolate_id = client.main_isolate_id().await?;
/// let mut inspector = WidgetInspector::new(isolate_id);
///
/// // Fetch tree (creates a new object group, disposes the previous one)
/// let tree = inspector.fetch_tree(&client).await?;
///
/// // Fetch details for a node (uses the current active group)
/// if let Some(value_id) = &tree.value_id {
///     let details = inspector.fetch_details(&client, value_id).await?;
/// }
///
/// // Clean up all references
/// inspector.dispose(&client).await?;
/// ```
pub struct WidgetInspector {
    object_group: ObjectGroupManager,
    isolate_id: String,
}

impl WidgetInspector {
    /// Create a new [`WidgetInspector`] for the given isolate.
    ///
    /// The provided `client` is cloned for use by the [`ObjectGroupManager`].
    pub fn new(client: super::client::VmServiceClient, isolate_id: String) -> Self {
        let object_group = ObjectGroupManager::new(client, isolate_id.clone());
        Self {
            object_group,
            isolate_id,
        }
    }

    /// Fetch the widget summary tree, creating a new object group.
    ///
    /// Disposes the previous group before creating the new one, which
    /// invalidates all `valueId` references from the previous fetch.
    ///
    /// # Errors
    ///
    /// Returns an error if the group cannot be created or the extension call
    /// fails.
    pub async fn fetch_tree(
        &mut self,
        client: &super::client::VmServiceClient,
    ) -> Result<DiagnosticsNode> {
        let group = self.object_group.create_group().await?;
        get_root_widget_tree(client, &self.isolate_id, &group).await
    }

    /// Fetch details for a specific widget node.
    ///
    /// Uses the current active object group. `value_id` must be a `valueId`
    /// from a node fetched under the current active group (i.e., after the
    /// most recent [`fetch_tree`][Self::fetch_tree] call).
    ///
    /// # Errors
    ///
    /// Returns [`Error::VmService`] if no active object group exists (call
    /// [`fetch_tree`][Self::fetch_tree] first), or a transport/protocol error
    /// if the extension call fails.
    pub async fn fetch_details(
        &self,
        client: &super::client::VmServiceClient,
        value_id: &str,
    ) -> Result<DiagnosticsNode> {
        let group = self
            .object_group
            .active_group()
            .ok_or_else(|| Error::vm_service("no active object group"))?;
        get_details_subtree(client, &self.isolate_id, value_id, group, 2).await
    }

    /// Get the currently selected widget in the inspector overlay.
    ///
    /// Uses the current active object group. Returns `Ok(None)` when no
    /// widget is currently selected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VmService`] if no active object group exists (call
    /// [`fetch_tree`][Self::fetch_tree] first), or a transport/protocol error
    /// if the extension call fails.
    pub async fn fetch_selected(
        &self,
        client: &super::client::VmServiceClient,
    ) -> Result<Option<DiagnosticsNode>> {
        let group = self
            .object_group
            .active_group()
            .ok_or_else(|| Error::vm_service("no active object group"))?;
        get_selected_widget(client, &self.isolate_id, group).await
    }

    /// Dispose all object groups and release all held references.
    ///
    /// After this call, all previously obtained `valueId` references are
    /// invalid. The inspector may be reused after calling
    /// [`fetch_tree`][Self::fetch_tree] again.
    ///
    /// # Errors
    ///
    /// Returns an error if the dispose extension call fails.
    pub async fn dispose(&mut self, client: &super::client::VmServiceClient) -> Result<()> {
        self.object_group.dispose_all(client).await
    }
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
    }

    // ── DebugOverlayState ───────────────────────────────────────────────────

    #[test]
    fn test_debug_overlay_state_default_all_none() {
        let state = DebugOverlayState::default();
        assert_eq!(state.repaint_rainbow, None);
        assert_eq!(state.debug_paint, None);
        assert_eq!(state.performance_overlay, None);
        assert_eq!(state.widget_inspector, None);
    }

    #[test]
    fn test_debug_overlay_state_clone() {
        let state = DebugOverlayState {
            repaint_rainbow: Some(true),
            debug_paint: Some(false),
            performance_overlay: None,
            widget_inspector: Some(true),
        };
        let cloned = state.clone();
        assert_eq!(cloned.repaint_rainbow, Some(true));
        assert_eq!(cloned.debug_paint, Some(false));
        assert_eq!(cloned.performance_overlay, None);
        assert_eq!(cloned.widget_inspector, Some(true));
    }

    #[test]
    fn test_debug_overlay_state_partial_update() {
        let mut state = DebugOverlayState::default();
        state.repaint_rainbow = Some(true);
        assert_eq!(state.repaint_rainbow, Some(true));
        // Other fields should remain None.
        assert_eq!(state.debug_paint, None);
        assert_eq!(state.performance_overlay, None);
        assert_eq!(state.widget_inspector, None);
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

    // ── DebugDumpKind ────────────────────────────────────────────────────────

    #[test]
    fn test_debug_dump_kind_methods() {
        assert_eq!(
            DebugDumpKind::WidgetTree.method(),
            "ext.flutter.debugDumpApp"
        );
        assert_eq!(
            DebugDumpKind::RenderTree.method(),
            "ext.flutter.debugDumpRenderTree"
        );
        assert_eq!(
            DebugDumpKind::LayerTree.method(),
            "ext.flutter.debugDumpLayerTree"
        );
    }

    #[test]
    fn test_debug_dump_kind_profile_availability() {
        assert!(DebugDumpKind::WidgetTree.available_in_profile());
        assert!(DebugDumpKind::RenderTree.available_in_profile());
        assert!(!DebugDumpKind::LayerTree.available_in_profile());
    }

    #[test]
    fn test_debug_dump_kind_method_matches_ext_constants() {
        assert_eq!(DebugDumpKind::WidgetTree.method(), ext::DEBUG_DUMP_APP);
        assert_eq!(
            DebugDumpKind::RenderTree.method(),
            ext::DEBUG_DUMP_RENDER_TREE
        );
        assert_eq!(
            DebugDumpKind::LayerTree.method(),
            ext::DEBUG_DUMP_LAYER_TREE
        );
    }

    #[test]
    fn test_debug_dump_kind_clone_and_eq() {
        let kind = DebugDumpKind::WidgetTree;
        let cloned = kind;
        assert_eq!(kind, cloned);
        assert_ne!(DebugDumpKind::WidgetTree, DebugDumpKind::RenderTree);
        assert_ne!(DebugDumpKind::WidgetTree, DebugDumpKind::LayerTree);
        assert_ne!(DebugDumpKind::RenderTree, DebugDumpKind::LayerTree);
    }

    // ── parse_data_extension_response (dump-specific tests) ─────────────────

    #[test]
    fn test_parse_dump_response() {
        let json = json!({
            "type": "_extensionType",
            "method": "ext.flutter.debugDumpApp",
            "data": "MyApp\n  MaterialApp\n    Scaffold\n"
        });
        let result = parse_data_extension_response(&json).unwrap();
        assert!(result.contains("MyApp"));
        assert!(result.contains("MaterialApp"));
    }

    #[test]
    fn test_parse_dump_response_empty() {
        let json = json!({
            "type": "_extensionType",
            "data": ""
        });
        let result = parse_data_extension_response(&json).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_parse_dump_response_missing_data() {
        let json = json!({"type": "_extensionType"});
        assert!(parse_data_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_dump_response_large_output() {
        // Dumps can be very large for complex apps.
        let large_tree = "Widget\n".repeat(10_000);
        let json = json!({"data": large_tree});
        let result = parse_data_extension_response(&json).unwrap();
        assert_eq!(result.lines().count(), 10_000);
    }

    #[test]
    fn test_parse_dump_response_with_special_characters() {
        let json = json!({
            "data": "Widget<String>\n  Text(\"Hello \\\"World\\\"\")\n  Icon(Icons.add)"
        });
        let result = parse_data_extension_response(&json).unwrap();
        assert!(result.contains("Widget<String>"));
        assert!(result.contains("Hello"));
    }

    // ── parse_diagnostics_node_response ─────────────────────────────────────

    #[test]
    fn test_parse_diagnostics_node_response_simple() {
        let json = json!({
            "result": {
                "description": "MyApp",
                "hasChildren": true,
                "valueId": "objects/1",
                "children": []
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert_eq!(node.description, "MyApp");
        assert_eq!(node.value_id.as_deref(), Some("objects/1"));
    }

    #[test]
    fn test_parse_diagnostics_node_response_direct_value() {
        // When the client has already unwrapped the JSON-RPC result, the
        // value is the node directly (no extra "result" wrapper).
        let json = json!({
            "description": "MyApp",
            "hasChildren": false,
            "valueId": "objects/2"
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert_eq!(node.description, "MyApp");
        assert_eq!(node.value_id.as_deref(), Some("objects/2"));
    }

    #[test]
    fn test_parse_diagnostics_node_response_nested_tree() {
        let json = json!({
            "result": {
                "description": "MaterialApp",
                "hasChildren": true,
                "valueId": "objects/1",
                "createdByLocalProject": true,
                "children": [
                    {
                        "description": "Scaffold",
                        "hasChildren": true,
                        "valueId": "objects/2",
                        "children": [
                            {
                                "description": "AppBar",
                                "hasChildren": false,
                                "valueId": "objects/3"
                            }
                        ]
                    }
                ]
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert_eq!(node.description, "MaterialApp");
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].description, "Scaffold");
        assert_eq!(node.children[0].children.len(), 1);
        assert_eq!(node.children[0].children[0].description, "AppBar");
    }

    #[test]
    fn test_parse_diagnostics_node_with_properties() {
        let json = json!({
            "result": {
                "description": "Container",
                "hasChildren": false,
                "valueId": "objects/5",
                "properties": [
                    {"name": "width", "description": "100.0", "level": "info"},
                    {"name": "height", "description": "200.0", "level": "info"},
                    {"name": "color", "description": "Color(0xff2196f3)", "level": "info"}
                ]
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert_eq!(node.properties.len(), 3);
        assert_eq!(node.properties[0].name.as_deref(), Some("width"));
    }

    #[test]
    fn test_parse_optional_null_response() {
        let json = json!({"result": null});
        let result = parse_optional_diagnostics_node_response(&json).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_optional_null_direct_value() {
        // A null value without a "result" wrapper also counts as None.
        let json = json!(null);
        let result = parse_optional_diagnostics_node_response(&json).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_optional_returns_some_for_valid_node() {
        let json = json!({
            "result": {
                "description": "Container",
                "hasChildren": false
            }
        });
        let result = parse_optional_diagnostics_node_response(&json).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().description, "Container");
    }

    #[test]
    fn test_parse_diagnostics_node_with_creation_location() {
        let json = json!({
            "result": {
                "description": "MyWidget",
                "hasChildren": false,
                "creationLocation": {
                    "file": "file:///app/lib/main.dart",
                    "line": 42,
                    "column": 8,
                    "name": "MyWidget"
                }
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        let loc = node.creation_location.unwrap();
        assert_eq!(loc.file, "file:///app/lib/main.dart");
        assert_eq!(loc.line, 42);
    }

    #[test]
    fn test_parse_diagnostics_node_unknown_fields_ignored() {
        // VM Service may add new fields — ensure we don't fail on unknown fields.
        let json = json!({
            "result": {
                "description": "Widget",
                "hasChildren": false,
                "futureField": "some value",
                "anotherNew": 42
            }
        });
        let node = parse_diagnostics_node_response(&json);
        assert!(node.is_ok());
        assert_eq!(node.unwrap().description, "Widget");
    }

    #[test]
    fn test_parse_diagnostics_node_response_missing_description_returns_error() {
        // "description" is required by DiagnosticsNode — missing it should fail.
        let json = json!({
            "result": {
                "hasChildren": false,
                "valueId": "objects/1"
            }
        });
        assert!(parse_diagnostics_node_response(&json).is_err());
    }

    #[test]
    fn test_parse_diagnostics_node_has_children_defaults_to_false() {
        let json = json!({
            "result": {
                "description": "Container"
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert!(!node.has_children);
        assert!(node.children.is_empty());
        assert!(node.properties.is_empty());
    }

    #[test]
    fn test_parse_diagnostics_node_created_by_local_project() {
        let json = json!({
            "result": {
                "description": "MyWidget",
                "hasChildren": false,
                "createdByLocalProject": true
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert!(node.created_by_local_project);
    }

    // ── parse_diagnostics_node_response: edge cases ──────────────────────────

    #[test]
    fn test_parse_diagnostics_node_summary_tree_field() {
        let json = json!({
            "result": {
                "description": "MyApp",
                "hasChildren": false,
                "summaryTree": true
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert!(node.summary_tree);
    }

    #[test]
    fn test_parse_diagnostics_node_node_type_field() {
        let json = json!({
            "result": {
                "description": "Container",
                "type": "_WidgetDiagnosticableNode",
                "hasChildren": false
            }
        });
        let node = parse_diagnostics_node_response(&json).unwrap();
        assert_eq!(node.node_type.as_deref(), Some("_WidgetDiagnosticableNode"));
    }

    // ── Layout Explorer: extract_layout_info ────────────────────────────────

    #[test]
    fn test_extract_layout_info_full() {
        let json = json!({
            "description": "Column",
            "hasChildren": true,
            "valueId": "objects/42",
            "constraints": {
                "type": "BoxConstraints",
                "description": "0.0<=w<=414.0, 0.0<=h<=896.0"
            },
            "size": {
                "width": "414.0",
                "height": "600.0"
            },
            "flexFactor": null,
            "flexFit": null
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.description.as_deref(), Some("Column"));
        let constraints = layout.constraints.unwrap();
        assert_eq!(constraints.max_width, 414.0);
        assert_eq!(constraints.max_height, 896.0);
        let size = layout.size.unwrap();
        assert_eq!(size.width, 414.0);
        assert_eq!(size.height, 600.0);
        // flexFactor: null should map to None, not 0
        assert_eq!(layout.flex_factor, None);
        assert_eq!(layout.flex_fit, None);
    }

    #[test]
    fn test_extract_layout_info_with_flex() {
        let json = json!({
            "description": "Expanded",
            "hasChildren": true,
            "flexFactor": 2.0,
            "flexFit": "tight"
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.flex_factor, Some(2.0));
        assert_eq!(layout.flex_fit.as_deref(), Some("tight"));
    }

    #[test]
    fn test_extract_layout_info_with_flex_string_factor() {
        // Some Flutter versions encode flexFactor as a string.
        let json = json!({
            "description": "Flexible",
            "hasChildren": true,
            "flexFactor": "3.0",
            "flexFit": "loose"
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.flex_factor, Some(3.0));
        assert_eq!(layout.flex_fit.as_deref(), Some("loose"));
    }

    #[test]
    fn test_extract_layout_info_minimal() {
        let json = json!({
            "description": "SizedBox",
            "hasChildren": false
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.constraints, None);
        assert_eq!(layout.size, None);
        assert_eq!(layout.flex_factor, None);
        assert_eq!(layout.flex_fit, None);
    }

    #[test]
    fn test_extract_layout_info_description_comes_from_node() {
        let json = json!({
            "description": "Row",
            "hasChildren": false
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.description.as_deref(), Some("Row"));
    }

    #[test]
    fn test_extract_layout_info_constraints_box_constraints_prefix() {
        // Constraints description may include "BoxConstraints(...)" wrapper
        let json = json!({
            "description": "Container",
            "hasChildren": false,
            "constraints": {
                "type": "BoxConstraints",
                "description": "BoxConstraints(0.0<=w<=200.0, 0.0<=h<=Infinity)"
            }
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        let constraints = layout.constraints.unwrap();
        assert_eq!(constraints.min_width, 0.0);
        assert_eq!(constraints.max_width, 200.0);
        assert_eq!(constraints.min_height, 0.0);
        assert!(constraints.max_height.is_infinite());
    }

    // ── Layout Explorer: parse_widget_size ──────────────────────────────────

    #[test]
    fn test_parse_widget_size_string_values() {
        let json = json!({"size": {"width": "100.0", "height": "200.0"}});
        let size = parse_widget_size(&json).unwrap();
        assert_eq!(size.width, 100.0);
        assert_eq!(size.height, 200.0);
    }

    #[test]
    fn test_parse_widget_size_numeric_values() {
        let json = json!({"size": {"width": 100.0, "height": 200.0}});
        let size = parse_widget_size(&json).unwrap();
        assert_eq!(size.width, 100.0);
        assert_eq!(size.height, 200.0);
    }

    #[test]
    fn test_parse_widget_size_missing_size_field_returns_none() {
        let json = json!({"description": "Widget"});
        assert_eq!(parse_widget_size(&json), None);
    }

    #[test]
    fn test_parse_widget_size_missing_width_returns_none() {
        let json = json!({"size": {"height": "100.0"}});
        assert_eq!(parse_widget_size(&json), None);
    }

    #[test]
    fn test_parse_widget_size_missing_height_returns_none() {
        let json = json!({"size": {"width": "100.0"}});
        assert_eq!(parse_widget_size(&json), None);
    }

    #[test]
    fn test_parse_widget_size_zero_dimensions() {
        let json = json!({"size": {"width": 0.0, "height": 0.0}});
        let size = parse_widget_size(&json).unwrap();
        assert_eq!(size.width, 0.0);
        assert_eq!(size.height, 0.0);
    }

    #[test]
    fn test_parse_widget_size_mixed_string_and_numeric() {
        // width is a string, height is a number — both should parse
        let json = json!({"size": {"width": "50.5", "height": 75.0}});
        let size = parse_widget_size(&json).unwrap();
        assert_eq!(size.width, 50.5);
        assert_eq!(size.height, 75.0);
    }

    // ── Layout Explorer: extract_layout_tree ────────────────────────────────

    #[test]
    fn test_extract_layout_tree_with_children() {
        let json = json!({
            "result": {
                "description": "Row",
                "hasChildren": true,
                "constraints": {"description": "0.0<=w<=414.0, 0.0<=h<=50.0"},
                "size": {"width": "414.0", "height": "50.0"},
                "children": [
                    {
                        "description": "Text",
                        "hasChildren": false,
                        "flexFactor": 1.0,
                        "flexFit": "loose",
                        "size": {"width": "200.0", "height": "50.0"}
                    },
                    {
                        "description": "Icon",
                        "hasChildren": false,
                        "size": {"width": "24.0", "height": "24.0"}
                    }
                ]
            }
        });
        let layouts = extract_layout_tree(&json).unwrap();
        // parent + 2 children = 3
        assert_eq!(layouts.len(), 3);
        assert_eq!(layouts[0].description.as_deref(), Some("Row"));
        assert_eq!(layouts[1].description.as_deref(), Some("Text"));
        assert_eq!(layouts[1].flex_factor, Some(1.0));
        assert_eq!(layouts[1].flex_fit.as_deref(), Some("loose"));
        assert_eq!(layouts[2].description.as_deref(), Some("Icon"));
    }

    #[test]
    fn test_extract_layout_tree_no_children() {
        let json = json!({
            "result": {
                "description": "Text",
                "hasChildren": false,
                "size": {"width": "100.0", "height": "20.0"}
            }
        });
        let layouts = extract_layout_tree(&json).unwrap();
        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].description.as_deref(), Some("Text"));
    }

    #[test]
    fn test_extract_layout_tree_direct_value_no_result_wrapper() {
        // The "result" wrapper may or may not be present — handle both forms.
        let json = json!({
            "description": "Column",
            "hasChildren": true,
            "size": {"width": "200.0", "height": "400.0"},
            "children": [
                {
                    "description": "Text",
                    "hasChildren": false,
                    "size": {"width": "200.0", "height": "20.0"}
                }
            ]
        });
        let layouts = extract_layout_tree(&json).unwrap();
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].description.as_deref(), Some("Column"));
        assert_eq!(layouts[1].description.as_deref(), Some("Text"));
    }

    #[test]
    fn test_extract_layout_tree_extra_fields_on_children_are_ignored() {
        // Children with unknown/extra fields are still parsed correctly
        // because DiagnosticsNode uses deny_unknown_fields: false.
        let json = json!({
            "result": {
                "description": "Row",
                "hasChildren": true,
                "children": [
                    {
                        "description": "ValidChild",
                        "hasChildren": false,
                        "unknownExtraField": "some-value"
                    },
                    {
                        "description": "AnotherChild",
                        "hasChildren": false,
                        "flexFactor": 1.0
                    }
                ]
            }
        });
        let layouts = extract_layout_tree(&json).unwrap();
        // root + 2 children
        assert_eq!(layouts.len(), 3);
        assert_eq!(layouts[0].description.as_deref(), Some("Row"));
        assert_eq!(layouts[1].description.as_deref(), Some("ValidChild"));
        assert_eq!(layouts[2].description.as_deref(), Some("AnotherChild"));
        assert_eq!(layouts[2].flex_factor, Some(1.0));
    }

    #[test]
    fn test_extract_layout_tree_missing_description_returns_error() {
        // The root node must have a description field — this should fail.
        let json = json!({
            "result": {
                "hasChildren": false
            }
        });
        assert!(extract_layout_tree(&json).is_err());
    }

    // ── Layout Explorer: parameter key contract ─────────────────────────────

    #[test]
    fn test_layout_explorer_uses_id_not_arg() {
        // Verify that the "id" key (not "arg") is used for the widget ID.
        // This is a contract test documenting the Flutter framework inconsistency.
        // The layout explorer extension requires "id" and "groupName" params,
        // while other inspector extensions use "arg" and "objectGroup".
        //
        // We verify this by inspecting build_extension_params output.
        let mut args = HashMap::new();
        args.insert("id".to_string(), "objects/42".to_string());
        args.insert("groupName".to_string(), "fdemon-inspector-1".to_string());
        args.insert("subtreeDepth".to_string(), "1".to_string());

        let params = build_extension_params("isolates/123", Some(args));

        assert_eq!(params["id"], "objects/42");
        assert_eq!(params["groupName"], "fdemon-inspector-1");
        // Must NOT use "arg" or "objectGroup" for the layout explorer.
        assert!(params.get("arg").is_none());
        assert!(params.get("objectGroup").is_none());
    }

    #[test]
    fn test_layout_explorer_null_flex_factor_is_none() {
        // flexFactor: null is the common case for non-flex children.
        // It must be represented as None, not Some(0.0).
        let json = json!({
            "description": "Container",
            "hasChildren": false,
            "flexFactor": null
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        // null flexFactor → None (not 0.0)
        assert_eq!(layout.flex_factor, None);
    }

    #[test]
    fn test_layout_explorer_zero_flex_factor_is_some_zero() {
        // flexFactor: 0 is a real flex factor value (distinct from null).
        let json = json!({
            "description": "Container",
            "hasChildren": false,
            "flexFactor": 0.0
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.flex_factor, Some(0.0));
    }
}
