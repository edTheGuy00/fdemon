//! Helpers for parsing `ext.flutter.inspector.getProperties` responses.
//!
//! The RPC returns a JSON object whose `result` key is an array of
//! `DiagnosticsNode`s — one per property of the queried widget. This module
//! provides two free functions:
//!
//! - [`parse_properties_response`] — deserializes the `result` array into a
//!   `Vec<DiagnosticsNode>`.
//! - [`split_widget_and_render_properties`] — partitions a property list into
//!   "widget properties" (rendered in the Properties tab) and "render-object
//!   properties" (rendered in the Render Object tab).
//!
//! The recursive second call (fetching sub-properties of each render-object
//! node) is performed by the action layer (`fdemon-app/actions/inspector`)
//! rather than here, because it owns the `VmRequestHandle` and timeout policy.
//!
//! Reference: `tmp/devtools/.../inspector_controller.dart:890–932`
//! (`_loadPropertiesForNode`).

use fdemon_core::prelude::*;
use fdemon_core::widget_tree::DiagnosticsNode;
use serde_json::Value;

/// Parse the JSON-RPC response object for `ext.flutter.inspector.getProperties`
/// into a list of `DiagnosticsNode`s.
///
/// The response shape is `{ "result": [<DiagnosticsNode>, ...] }`. Some
/// transports return the array directly without the outer wrapper; this helper
/// handles both forms (matching the pattern used by `extract_layout_info`).
///
/// Returns an empty vector if the response has no `result` key and is not
/// itself an array — this is rare but observed in practice when the widget
/// has no properties.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if any element in the array cannot be
/// deserialized as a [`DiagnosticsNode`].
pub fn parse_properties_response(raw: &Value) -> Result<Vec<DiagnosticsNode>> {
    let array = raw
        .get("result")
        .or(Some(raw))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    array
        .into_iter()
        .map(|node_json| {
            serde_json::from_value::<DiagnosticsNode>(node_json)
                .map_err(|e| Error::protocol(format!("getProperties deserialize: {e}")))
        })
        .collect()
}

/// Partition a property list into (widget-properties, render-object-properties).
///
/// Mirrors DevTools' split at `inspector_controller.dart:898–906`: any property
/// with `propertyType == "RenderObject"` moves to `render`; everything else
/// stays in `widget`. The original order within each bucket is preserved.
pub fn split_widget_and_render_properties(
    props: Vec<DiagnosticsNode>,
) -> (Vec<DiagnosticsNode>, Vec<DiagnosticsNode>) {
    let mut widget = Vec::with_capacity(props.len());
    let mut render = Vec::new();
    for node in props {
        if node.is_render_object_property() {
            render.push(node);
        } else {
            widget.push(node);
        }
    }
    (widget, render)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_properties_response_unwraps_result_key() {
        let raw = json!({
            "result": [
                {"description": "Color(0xff000000)", "propertyType": "Color"},
                {"description": "RenderFlex#abc123", "propertyType": "RenderObject"}
            ]
        });
        let props = parse_properties_response(&raw).unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].description, "Color(0xff000000)");
        assert_eq!(props[1].property_type.as_deref(), Some("RenderObject"));
    }

    #[test]
    fn parse_properties_response_accepts_bare_array() {
        let raw = json!([
            {"description": "EdgeInsets.zero", "propertyType": "EdgeInsetsGeometry"}
        ]);
        let props = parse_properties_response(&raw).unwrap();
        assert_eq!(props.len(), 1);
    }

    #[test]
    fn parse_properties_response_empty_when_no_result_and_not_array() {
        let raw = json!({"error": "no such widget"});
        let props = parse_properties_response(&raw).unwrap();
        assert!(props.is_empty());
    }

    #[test]
    fn split_partitions_by_property_type() {
        let nodes = vec![
            sample_node("color", Some("Color")),
            sample_node("render", Some("RenderObject")),
            sample_node("padding", Some("EdgeInsetsGeometry")),
            sample_node("render2", Some("RenderObject")),
        ];
        let (widget, render) = split_widget_and_render_properties(nodes);
        assert_eq!(widget.len(), 2);
        assert_eq!(widget[0].description, "color");
        assert_eq!(widget[1].description, "padding");
        assert_eq!(render.len(), 2);
        assert_eq!(render[0].description, "render");
        assert_eq!(render[1].description, "render2");
    }

    #[test]
    fn split_handles_empty_input() {
        let (widget, render) = split_widget_and_render_properties(vec![]);
        assert!(widget.is_empty());
        assert!(render.is_empty());
    }

    #[test]
    fn split_preserves_order_within_buckets() {
        let nodes = vec![
            sample_node("a", None),
            sample_node("b", Some("Color")),
            sample_node("c", Some("RenderObject")),
            sample_node("d", None),
        ];
        let (widget, _) = split_widget_and_render_properties(nodes);
        assert_eq!(widget[0].description, "a");
        assert_eq!(widget[1].description, "b");
        assert_eq!(widget[2].description, "d");
    }

    fn sample_node(desc: &str, prop_type: Option<&str>) -> DiagnosticsNode {
        let mut obj = json!({"description": desc});
        if let Some(pt) = prop_type {
            obj["propertyType"] = json!(pt);
        }
        serde_json::from_value(obj).unwrap()
    }
}
