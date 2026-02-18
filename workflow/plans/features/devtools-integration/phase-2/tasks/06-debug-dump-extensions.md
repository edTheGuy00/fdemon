## Task: Debug Dump Extensions

**Objective**: Implement typed wrappers for the 3 Flutter debug dump extensions that return formatted text output of the widget tree, render tree, and layer tree.

**Depends on**: 01-extension-framework

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions.rs`: Add debug dump extension methods
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-export new functions

### Details

#### 1. Debug Dump Methods

All 3 dump extensions follow the same simple pattern: send a request with only `isolateId`, receive a response with `{"data": "<formatted string>"}`.

```rust
/// Dump the widget tree as formatted text.
///
/// Returns the same output as `debugDumpApp()` — a multiline text dump of
/// all widgets in the tree with their properties.
///
/// Available in debug and profile mode.
pub async fn debug_dump_app(
    client: &VmServiceClient,
    isolate_id: &str,
) -> Result<String> {
    let result = client.call_extension(
        ext::DEBUG_DUMP_APP,
        isolate_id,
        None,
    ).await?;
    parse_data_extension_response(&result)
}

/// Dump the render tree as formatted text.
///
/// Returns the same output as `debugDumpRenderTree()` — a multiline text dump of
/// all render objects with their constraints, sizes, and painting details.
///
/// Available in debug and profile mode.
pub async fn debug_dump_render_tree(
    client: &VmServiceClient,
    isolate_id: &str,
) -> Result<String> {
    let result = client.call_extension(
        ext::DEBUG_DUMP_RENDER_TREE,
        isolate_id,
        None,
    ).await?;
    parse_data_extension_response(&result)
}

/// Dump the layer tree as formatted text.
///
/// Returns the same output as `debugDumpLayerTree()` — a multiline text dump of
/// all compositing layers with their properties.
///
/// Debug mode only (not available in profile mode).
pub async fn debug_dump_layer_tree(
    client: &VmServiceClient,
    isolate_id: &str,
) -> Result<String> {
    let result = client.call_extension(
        ext::DEBUG_DUMP_LAYER_TREE,
        isolate_id,
        None,
    ).await?;
    parse_data_extension_response(&result)
}
```

**Wire format (same for all 3):**
```json
// Request
{
    "method": "ext.flutter.debugDumpApp",
    "params": {
        "isolateId": "isolates/..."
    }
}

// Response
{
    "result": {
        "type": "_extensionType",
        "method": "ext.flutter.debugDumpApp",
        "data": "MyApp\n└─MaterialApp\n  └─Scaffold\n    ├─AppBar\n    │ └─Text(\"Title\")\n    └─Center\n      └─Text(\"Hello World\")\n"
    }
}
```

#### 2. Debug Dump Enum

A convenience enum for specifying which dump to run:

```rust
/// Which debug tree to dump as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugDumpKind {
    /// Widget tree (`debugDumpApp`) — debug + profile mode
    WidgetTree,
    /// Render tree (`debugDumpRenderTree`) — debug + profile mode
    RenderTree,
    /// Layer tree (`debugDumpLayerTree`) — debug mode only
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
    pub fn available_in_profile(&self) -> bool {
        match self {
            Self::WidgetTree | Self::RenderTree => true,
            Self::LayerTree => false,
        }
    }
}

/// Run a debug dump by kind.
pub async fn debug_dump(
    client: &VmServiceClient,
    isolate_id: &str,
    kind: DebugDumpKind,
) -> Result<String> {
    let result = client.call_extension(kind.method(), isolate_id, None).await?;
    parse_data_extension_response(&result)
}
```

### Acceptance Criteria

1. All 3 dump functions send correct JSON-RPC with only `isolateId` param
2. Response text correctly extracted from `{"data": "..."}` field
3. `DebugDumpKind` enum maps to correct method names
4. `available_in_profile()` correctly reports mode availability
5. Empty dump output handled (returns empty string, not error)
6. Extension-not-available errors propagated correctly
7. All new functions and types re-exported

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
        // Dumps can be very large for complex apps
        let large_tree = "Widget\n".repeat(10_000);
        let json = json!({"data": large_tree});
        let result = parse_data_extension_response(&json).unwrap();
        assert_eq!(result.lines().count(), 10_000);
    }

    #[test]
    fn test_debug_dump_kind_methods() {
        assert_eq!(DebugDumpKind::WidgetTree.method(), "ext.flutter.debugDumpApp");
        assert_eq!(DebugDumpKind::RenderTree.method(), "ext.flutter.debugDumpRenderTree");
        assert_eq!(DebugDumpKind::LayerTree.method(), "ext.flutter.debugDumpLayerTree");
    }

    #[test]
    fn test_debug_dump_kind_profile_availability() {
        assert!(DebugDumpKind::WidgetTree.available_in_profile());
        assert!(DebugDumpKind::RenderTree.available_in_profile());
        assert!(!DebugDumpKind::LayerTree.available_in_profile());
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
```

### Notes

- **Dumps can produce very large output** for complex apps (thousands of lines). The functions return the full string — truncation or pagination is the caller's responsibility (Phase 4 TUI rendering).
- **`debugDumpApp` and `debugDumpRenderTree` work in profile mode**, while `debugDumpLayerTree` is debug-only. The `available_in_profile()` method communicates this, but the actual mode check happens at the Flutter side (the extension simply won't exist in the wrong mode).
- **No TEA integration in this task.** Adding a keybinding or menu option to trigger dumps and display them in the TUI belongs to Phase 4.
- **These dumps are useful for text-based debugging** even without a tree widget — they can be logged directly to the session's log view as info-level messages, which would be a simple Phase 4 integration.
- This is the simplest task in Phase 2 since all 3 functions follow an identical pattern with trivial response parsing.

---

## Completion Summary

**Status:** Not started
