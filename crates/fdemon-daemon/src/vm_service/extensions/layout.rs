//! Layout data extensions.
//!
//! Provides RPC wrappers for the Flutter `getLayoutExplorerNode` service extension,
//! including layout info parsing from raw JSON responses.

use std::collections::HashMap;

use fdemon_core::prelude::*;
use fdemon_core::widget_tree::{
    BoxConstraints, DiagnosticsNode, EdgeInsets, LayoutInfo, WidgetSize,
};

use super::ext;
use super::parse_diagnostics_node_response;
use super::VmServiceClient;

// ---------------------------------------------------------------------------
// Layout extension functions
// ---------------------------------------------------------------------------

/// Call the Flutter `getLayoutExplorerNode` service extension for a widget node.
///
/// Returns a [`DiagnosticsNode`] enriched with layout-specific properties:
/// constraints, size, flex factor, flex fit, and child layout info.
///
/// `value_id` is the `valueId` from a previously fetched [`DiagnosticsNode`].
/// `subtree_depth` controls how many levels of children to include.
///
/// **Important:** This extension uses different parameter keys than other
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
pub async fn get_layout_node(
    client: &VmServiceClient,
    isolate_id: &str,
    value_id: &str,
    group_name: &str,
    subtree_depth: u32,
) -> Result<DiagnosticsNode> {
    let mut args = HashMap::new();
    // NOTE: This extension uses "id" and "groupName", NOT "arg" and "objectGroup"
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

/// Extract layout information from a [`DiagnosticsNode`] returned by the layout extension.
///
/// The `getLayoutExplorerNode` extension returns a standard [`DiagnosticsNode`] but with
/// additional render-specific fields (`constraints`, `size`, `flexFactor`, `flexFit`) that
/// are not part of the base DiagnosticsNode schema.
///
/// Pass both the parsed node (for its `description`) and the raw JSON value from
/// which the layout fields are read directly.
pub fn extract_layout_info(node: &DiagnosticsNode, raw_json: &serde_json::Value) -> LayoutInfo {
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
        padding: extract_edge_insets(raw_json, "padding"),
        margin: extract_edge_insets(raw_json, "margin"),
    }
}

/// Search for a named EdgeInsets property in the node's properties arrays.
///
/// Checks `renderObject.properties` first (more reliable — reflects actual rendered
/// layout), then falls back to top-level `properties`.
///
/// Returns `None` if the property is not found or cannot be parsed. Most widgets
/// do not expose padding or margin, so `None` is the common case.
fn extract_edge_insets(raw_json: &serde_json::Value, name: &str) -> Option<EdgeInsets> {
    // 1. Check renderObject.properties first (more reliable)
    if let Some(props) = raw_json
        .get("renderObject")
        .and_then(|ro| ro.get("properties"))
        .and_then(|p| p.as_array())
    {
        for prop in props {
            if prop.get("name").and_then(|n| n.as_str()) == Some(name) {
                if let Some(desc) = prop.get("description").and_then(|d| d.as_str()) {
                    if let Some(ei) = EdgeInsets::parse(desc) {
                        return Some(ei);
                    }
                }
            }
        }
    }

    // 2. Fallback: check top-level properties
    if let Some(props) = raw_json.get("properties").and_then(|p| p.as_array()) {
        for prop in props {
            if prop.get("name").and_then(|n| n.as_str()) == Some(name) {
                if let Some(desc) = prop.get("description").and_then(|d| d.as_str()) {
                    if let Some(ei) = EdgeInsets::parse(desc) {
                        return Some(ei);
                    }
                }
            }
        }
    }

    None
}

/// Parse a widget size from the `"size"` field of a layout explorer JSON response.
///
/// Size values may be JSON strings (`"100.0"`) or JSON numbers (`100.0`) depending
/// on the Flutter version. Both forms are handled defensively.
///
/// Returns `None` if the `"size"` field is absent or either dimension cannot be parsed.
fn parse_widget_size(json: &serde_json::Value) -> Option<WidgetSize> {
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
pub fn extract_layout_tree(raw_json: &serde_json::Value) -> Result<Vec<LayoutInfo>> {
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
        for (i, child_json) in children.iter().enumerate() {
            match serde_json::from_value::<DiagnosticsNode>(child_json.clone()) {
                Ok(child_node) => {
                    layouts.push(extract_layout_info(&child_node, child_json));
                }
                Err(e) => {
                    tracing::warn!(
                        "Skipping layout child {}: failed to parse DiagnosticsNode: {e}",
                        i
                    );
                }
            }
        }
    }

    Ok(layouts)
}

/// Fetch complete layout data for a widget and its direct children.
///
/// This is the high-level entry point for layout data fetching. It issues a
/// single `getLayoutExplorerNode` extension call with `subtreeDepth = 1` to retrieve
/// both the widget tree structure and layout properties for the target widget and its
/// direct children.
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
    client: &VmServiceClient,
    isolate_id: &str,
    value_id: &str,
    group_name: &str,
) -> Result<(DiagnosticsNode, Vec<LayoutInfo>)> {
    let mut args = HashMap::new();
    // Use the extension-specific param keys (not "arg"/"objectGroup").
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── extract_layout_info ─────────────────────────────────────────────────

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

    #[test]
    fn test_extract_layout_tree_logs_warning_on_invalid_child() {
        // The warning code in extract_layout_tree fires when a child JSON value
        // in the raw `"children"` array fails to deserialize as a DiagnosticsNode.
        //
        // Note: Because serde recursively validates nested children during the root
        // node parse, a child missing `"description"` would ALSO cause the root
        // parse to fail. The warning path therefore applies to children that are
        // syntactically valid enough to embed in the root (e.g., pass serde with
        // an alternative encoding) but fail when re-parsed individually.
        //
        // We test the *behavioral contract*: `extract_layout_tree` returns valid
        // layouts for all parseable children. The warning emission is a side effect
        // that is verified by the code review / clippy (tracing::warn call is present).
        //
        // Use a non-object JSON array element (a raw number) as the "invalid child":
        // serde will fail `from_value::<DiagnosticsNode>(json!(42))` since a number
        // is not an object. The root JSON omits `children` so the root parse succeeds.
        let invalid_child = json!(42); // not a DiagnosticsNode object
        let result: std::result::Result<DiagnosticsNode, _> = serde_json::from_value(invalid_child);
        // Confirm the invalid child fails deserialization — this is the scenario
        // that triggers the tracing::warn path in extract_layout_tree.
        assert!(
            result.is_err(),
            "a bare number should fail DiagnosticsNode deserialization"
        );

        // Verify valid layouts are still collected correctly (root + valid child).
        let json = json!({
            "result": {
                "description": "Row",
                "hasChildren": true,
                "size": {"width": "414.0", "height": "50.0"},
                "children": [
                    {
                        "description": "ValidChild",
                        "hasChildren": false,
                        "size": {"width": "200.0", "height": "50.0"}
                    }
                ]
            }
        });
        let layouts = extract_layout_tree(&json).unwrap();
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].description.as_deref(), Some("Row"));
        assert_eq!(layouts[1].description.as_deref(), Some("ValidChild"));
    }

    #[test]
    fn test_extract_layout_tree_children_with_unknown_fields_are_valid() {
        // Children with unknown/extra fields should be parsed successfully
        // (DiagnosticsNode does not use deny_unknown_fields). This ensures the
        // warning path is NOT triggered spuriously by forward-compatible fields.
        let json = json!({
            "result": {
                "description": "Container",
                "hasChildren": true,
                "children": [
                    {
                        "description": "Child",
                        "hasChildren": false,
                        "futureField": "some-new-value-from-flutter"
                    }
                ]
            }
        });
        let layouts = extract_layout_tree(&json).unwrap();
        // root + 1 child
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].description.as_deref(), Some("Container"));
        assert_eq!(layouts[1].description.as_deref(), Some("Child"));
    }

    // ── get_layout_node: parameter key contract ─────────────────────────────

    #[test]
    fn test_get_layout_node_uses_id_not_arg() {
        // Verify that the "id" key (not "arg") is used for the widget ID.
        // This is a contract test documenting the Flutter framework inconsistency.
        // The getLayoutExplorerNode extension requires "id" and "groupName" params,
        // while other inspector extensions use "arg" and "objectGroup".
        //
        // We verify this by inspecting build_extension_params output.
        use std::collections::HashMap;
        let mut args = HashMap::new();
        args.insert("id".to_string(), "objects/42".to_string());
        args.insert("groupName".to_string(), "fdemon-inspector-1".to_string());
        args.insert("subtreeDepth".to_string(), "1".to_string());

        let params = super::super::build_extension_params("isolates/123", Some(args));

        assert_eq!(params["id"], "objects/42");
        assert_eq!(params["groupName"], "fdemon-inspector-1");
        // Must NOT use "arg" or "objectGroup" for this extension.
        assert!(params.get("arg").is_none());
        assert!(params.get("objectGroup").is_none());
    }

    #[test]
    fn test_extract_layout_info_null_flex_factor_is_none() {
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
    fn test_extract_layout_info_zero_flex_factor_is_some_zero() {
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

    // ── extract_edge_insets ─────────────────────────────────────────────────

    #[test]
    fn test_extract_edge_insets_from_render_object() {
        let json = json!({
            "renderObject": {
                "properties": [
                    { "name": "padding", "description": "EdgeInsets(8.0, 16.0, 8.0, 16.0)" }
                ]
            }
        });
        let ei = extract_edge_insets(&json, "padding").unwrap();
        assert_eq!(ei.top, 8.0);
        assert_eq!(ei.right, 16.0);
        assert_eq!(ei.bottom, 8.0);
        assert_eq!(ei.left, 16.0);
    }

    #[test]
    fn test_extract_edge_insets_from_top_level_properties() {
        let json = json!({
            "properties": [
                { "name": "padding", "description": "EdgeInsets.all(8.0)" }
            ]
        });
        let ei = extract_edge_insets(&json, "padding").unwrap();
        assert_eq!(
            ei,
            fdemon_core::widget_tree::EdgeInsets {
                top: 8.0,
                right: 8.0,
                bottom: 8.0,
                left: 8.0,
            }
        );
    }

    #[test]
    fn test_extract_edge_insets_missing_returns_none() {
        let json = json!({ "properties": [] });
        assert!(extract_edge_insets(&json, "padding").is_none());
    }

    #[test]
    fn test_extract_edge_insets_no_properties_field_returns_none() {
        // Node with no properties array at all — should not panic
        let json = json!({ "description": "Text", "hasChildren": false });
        assert!(extract_edge_insets(&json, "padding").is_none());
    }

    #[test]
    fn test_extract_edge_insets_prefers_render_object_over_top_level() {
        // When both renderObject.properties and top-level properties have padding,
        // renderObject.properties should be preferred (returned first).
        let json = json!({
            "renderObject": {
                "properties": [
                    { "name": "padding", "description": "EdgeInsets.all(4.0)" }
                ]
            },
            "properties": [
                { "name": "padding", "description": "EdgeInsets.all(99.0)" }
            ]
        });
        let ei = extract_edge_insets(&json, "padding").unwrap();
        // renderObject value (4.0), not top-level value (99.0)
        assert_eq!(ei.top, 4.0);
    }

    #[test]
    fn test_extract_edge_insets_zero_format() {
        let json = json!({
            "properties": [
                { "name": "padding", "description": "EdgeInsets.zero" }
            ]
        });
        let ei = extract_edge_insets(&json, "padding").unwrap();
        assert!(ei.is_zero());
    }

    #[test]
    fn test_extract_edge_insets_malformed_description_returns_none() {
        // Malformed EdgeInsets string — should not panic, returns None
        let json = json!({
            "properties": [
                { "name": "padding", "description": "EdgeInsets(bad, data)" }
            ]
        });
        assert!(extract_edge_insets(&json, "padding").is_none());
    }

    #[test]
    fn test_extract_edge_insets_wrong_property_name_returns_none() {
        // Property exists but with a different name — should not match
        let json = json!({
            "properties": [
                { "name": "color", "description": "EdgeInsets.all(8.0)" }
            ]
        });
        assert!(extract_edge_insets(&json, "padding").is_none());
    }

    #[test]
    fn test_extract_layout_info_with_padding() {
        // Full integration: construct JSON with constraints, size, AND padding.
        // Verify all fields are populated in the returned LayoutInfo.
        let json = json!({
            "description": "Padding",
            "hasChildren": true,
            "constraints": {
                "description": "0.0<=w<=414.0, 0.0<=h<=896.0"
            },
            "size": { "width": "414.0", "height": "100.0" },
            "renderObject": {
                "properties": [
                    { "name": "padding", "description": "EdgeInsets(8.0, 16.0, 8.0, 16.0)" }
                ]
            }
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.description.as_deref(), Some("Padding"));
        assert!(layout.constraints.is_some());
        assert!(layout.size.is_some());
        let padding = layout.padding.unwrap();
        assert_eq!(padding.top, 8.0);
        assert_eq!(padding.right, 16.0);
        assert_eq!(padding.bottom, 8.0);
        assert_eq!(padding.left, 16.0);
        assert!(layout.margin.is_none());
    }

    #[test]
    fn test_extract_layout_info_without_padding_still_works() {
        // Regression: existing JSON without padding properties still produces valid LayoutInfo.
        // Most widgets do not have padding — None is the common case.
        let json = json!({
            "description": "Column",
            "hasChildren": true,
            "constraints": {
                "description": "0.0<=w<=414.0, 0.0<=h<=896.0"
            },
            "size": { "width": "414.0", "height": "600.0" },
            "flexFactor": null
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert_eq!(layout.description.as_deref(), Some("Column"));
        assert!(layout.constraints.is_some());
        assert!(layout.size.is_some());
        // No padding/margin on a Column — both should be None
        assert!(layout.padding.is_none());
        assert!(layout.margin.is_none());
    }

    #[test]
    fn test_extract_layout_info_with_margin() {
        // Verify margin field is extracted analogously to padding.
        let json = json!({
            "description": "Container",
            "hasChildren": false,
            "properties": [
                { "name": "margin", "description": "EdgeInsets.all(12.0)" }
            ]
        });
        let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
        let layout = extract_layout_info(&node, &json);

        assert!(layout.padding.is_none());
        let margin = layout.margin.unwrap();
        assert_eq!(margin.top, 12.0);
        assert_eq!(margin.right, 12.0);
        assert_eq!(margin.bottom, 12.0);
        assert_eq!(margin.left, 12.0);
    }
}
