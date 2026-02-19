//! Debug dump extensions.
//!
//! Provides [`DebugDumpKind`] and the debug dump RPC wrappers
//! (`debug_dump_app`, `debug_dump_render_tree`, `debug_dump_layer_tree`, `debug_dump`).

use fdemon_core::prelude::*;

use super::ext;
use super::parse_data_extension_response;
use super::VmServiceClient;

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
pub async fn debug_dump_app(client: &VmServiceClient, isolate_id: &str) -> Result<String> {
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
pub async fn debug_dump_render_tree(client: &VmServiceClient, isolate_id: &str) -> Result<String> {
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
pub async fn debug_dump_layer_tree(client: &VmServiceClient, isolate_id: &str) -> Result<String> {
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
    client: &VmServiceClient,
    isolate_id: &str,
    kind: DebugDumpKind,
) -> Result<String> {
    let result = client
        .call_extension(kind.method(), isolate_id, None)
        .await?;
    parse_data_extension_response(&result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        use super::super::ext;
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
}
