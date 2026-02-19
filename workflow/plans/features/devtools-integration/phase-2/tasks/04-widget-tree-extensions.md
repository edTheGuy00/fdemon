## Task: Widget Tree Inspector Extensions

**Objective**: Implement typed wrappers for the Flutter Widget Inspector extensions — fetching the widget summary tree, detail subtrees, and selected widget. Parse JSON responses into the `DiagnosticsNode` types from Task 02.

**Depends on**: 01-extension-framework, 02-widget-tree-types

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions.rs`: Add widget tree extension methods
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-export new types
- `crates/fdemon-daemon/Cargo.toml`: May need `fdemon-core` widget_tree types (already a dependency)

### Details

#### 1. Get Root Widget Tree

Fetch the widget summary tree using the newer `getRootWidgetTree` extension (available since Flutter 3.22+), falling back to `getRootWidgetSummaryTree` for older versions.

```rust
/// Fetch the root widget summary tree.
///
/// Uses `ext.flutter.inspector.getRootWidgetTree` (newer API) with fallback to
/// `ext.flutter.inspector.getRootWidgetSummaryTree` for older Flutter versions.
///
/// Returns the root DiagnosticsNode with children populated.
/// Debug mode only.
pub async fn get_root_widget_tree(
    client: &VmServiceClient,
    isolate_id: &str,
    object_group: &str,
) -> Result<DiagnosticsNode> {
    // Try newer API first
    let result = client.call_extension(
        ext::GET_ROOT_WIDGET_TREE,
        isolate_id,
        Some(hashmap! {
            "objectGroup" => object_group,
            "isSummaryTree" => "true",
            "withPreviews" => "false",
        }),
    ).await;

    match result {
        Ok(value) => parse_diagnostics_node_response(&value),
        Err(_) => {
            // Fallback to older API
            let value = client.call_extension(
                ext::GET_ROOT_WIDGET_SUMMARY_TREE,
                isolate_id,
                Some(hashmap! {
                    "objectGroup" => object_group,
                }),
            ).await?;
            parse_diagnostics_node_response(&value)
        }
    }
}
```

**Wire format:**
```json
// Request
{
    "method": "ext.flutter.inspector.getRootWidgetTree",
    "params": {
        "isolateId": "isolates/...",
        "objectGroup": "fdemon-inspector-1",
        "isSummaryTree": "true",
        "withPreviews": "false"
    }
}

// Response
{
    "result": {
        "description": "MyApp",
        "type": "_WidgetDiagnosticableNode",
        "hasChildren": true,
        "valueId": "objects/1",
        "children": [ ... ],
        ...
    }
}
```

#### 2. Get Details Subtree

Fetch detailed information for a specific widget node by its `valueId`.

```rust
/// Fetch a detailed subtree for a specific widget.
///
/// `value_id` is the `valueId` from a previously fetched DiagnosticsNode.
/// `subtree_depth` controls how many levels of children to include (default: 2).
///
/// Returns a DiagnosticsNode with full properties and children up to the specified depth.
/// Debug mode only.
pub async fn get_details_subtree(
    client: &VmServiceClient,
    isolate_id: &str,
    value_id: &str,
    object_group: &str,
    subtree_depth: u32,
) -> Result<DiagnosticsNode> {
    let result = client.call_extension(
        ext::GET_DETAILS_SUBTREE,
        isolate_id,
        Some(hashmap! {
            "arg" => value_id,
            "objectGroup" => object_group,
            "subtreeDepth" => &subtree_depth.to_string(),
        }),
    ).await?;
    parse_diagnostics_node_response(&result)
}
```

**Important parameter keys:** This extension uses `arg` for the widget ID and `objectGroup` for the group name.

#### 3. Get Selected Widget

Fetch the currently selected widget (tapped in the inspector overlay on the device).

```rust
/// Fetch the currently selected widget in the inspector.
///
/// Returns `Ok(Some(node))` if a widget is selected, `Ok(None)` if nothing is selected.
/// Debug mode only.
pub async fn get_selected_widget(
    client: &VmServiceClient,
    isolate_id: &str,
    object_group: &str,
) -> Result<Option<DiagnosticsNode>> {
    let result = client.call_extension(
        ext::GET_SELECTED_WIDGET,
        isolate_id,
        Some(hashmap! {
            "objectGroup" => object_group,
        }),
    ).await?;
    parse_optional_diagnostics_node_response(&result)
}
```

#### 4. Response Parsing

Parse the DiagnosticsNode JSON from extension responses:

```rust
/// Parse a DiagnosticsNode from an extension response's `result` field.
///
/// The response may have the node directly in `result`, or nested under `result.result`.
fn parse_diagnostics_node_response(value: &Value) -> Result<DiagnosticsNode> {
    // The VM Service wraps extension responses. The actual node may be in:
    // 1. The `result` field directly (if request() already unwrapped it)
    // 2. A nested `result` field
    let node_value = value.get("result").unwrap_or(value);
    serde_json::from_value(node_value.clone())
        .map_err(|e| Error::protocol(format!("failed to parse DiagnosticsNode: {}", e)))
}

/// Parse an optional DiagnosticsNode (for getSelectedWidget which may return null).
fn parse_optional_diagnostics_node_response(value: &Value) -> Result<Option<DiagnosticsNode>> {
    let node_value = value.get("result").unwrap_or(value);
    if node_value.is_null() {
        return Ok(None);
    }
    parse_diagnostics_node_response(value).map(Some)
}
```

#### 5. Object Group Integration

The widget tree functions require an `objectGroup` parameter. Integrate with the `ObjectGroupManager` from Task 01:

```rust
/// High-level widget inspector that manages object groups automatically.
pub struct WidgetInspector {
    object_group: ObjectGroupManager,
    isolate_id: String,
}

impl WidgetInspector {
    pub fn new(isolate_id: String) -> Self { ... }

    /// Fetch the widget tree, creating a new object group.
    /// Disposes the previous group to free old references.
    pub async fn fetch_tree(&mut self, client: &VmServiceClient) -> Result<DiagnosticsNode> {
        let group = self.object_group.create_group().await?;
        get_root_widget_tree(client, &self.isolate_id, &group).await
    }

    /// Fetch details for a widget node (uses current active group).
    pub async fn fetch_details(
        &self,
        client: &VmServiceClient,
        value_id: &str,
    ) -> Result<DiagnosticsNode> {
        let group = self.object_group.active_group()
            .ok_or_else(|| Error::vm_service("no active object group"))?;
        get_details_subtree(client, &self.isolate_id, value_id, group, 2).await
    }

    /// Get the currently selected widget (uses current active group).
    pub async fn fetch_selected(
        &self,
        client: &VmServiceClient,
    ) -> Result<Option<DiagnosticsNode>> {
        let group = self.object_group.active_group()
            .ok_or_else(|| Error::vm_service("no active object group"))?;
        get_selected_widget(client, &self.isolate_id, group).await
    }

    /// Dispose all object groups and clean up.
    pub async fn dispose(&mut self, client: &VmServiceClient) -> Result<()> {
        self.object_group.dispose_all(client).await
    }
}
```

### Acceptance Criteria

1. `get_root_widget_tree()` fetches and parses the widget summary tree into `DiagnosticsNode`
2. Falls back from `getRootWidgetTree` to `getRootWidgetSummaryTree` for older Flutter versions
3. `get_details_subtree()` fetches detailed node info with correct `arg` and `objectGroup` params
4. `get_selected_widget()` returns `None` when no widget is selected
5. All responses parsed into `DiagnosticsNode` structs (from `fdemon-core`)
6. `WidgetInspector` manages object groups automatically
7. Object references are invalidated when groups are disposed
8. Extension-not-available errors handled gracefully

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
        // VM Service may add new fields — ensure we don't fail on unknown fields
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
    }
}
```

### Notes

- **`#[serde(deny_unknown_fields)]` must NOT be used** on `DiagnosticsNode` — the VM Service may return additional fields in newer Flutter versions, and we must gracefully ignore them.
- **The `result` unwrapping logic** depends on how `VmServiceClient.request()` returns data. If `request()` already extracts the `result` field from the JSON-RPC response, then the extension functions receive the result directly. If not, they need to unwrap `result` themselves. Check the existing Phase 1 `request()` implementation.
- **Object group names must be unique** per inspector session. The `fdemon-inspector-{counter}` format ensures this.
- **The fallback from `getRootWidgetTree` to `getRootWidgetSummaryTree`** is important for supporting older Flutter versions that don't have the newer API. The fallback should only be attempted once — if the newer API works, don't try the older one.
- **Parameter key `arg`** (not `valueId` or `id`) is used for `getDetailsSubtree` — this is a quirk of the Flutter inspector service extension implementation.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/extensions.rs` | Added `DiagnosticsNode` import, `parse_diagnostics_node_response()`, `parse_optional_diagnostics_node_response()`, `get_root_widget_tree()` (with fallback), `get_details_subtree()`, `get_selected_widget()`, `WidgetInspector` struct, `dispose_all()` on `ObjectGroupManager`, and 15 new unit tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Re-exported all new public items: `get_root_widget_tree`, `get_details_subtree`, `get_selected_widget`, `parse_diagnostics_node_response`, `parse_optional_diagnostics_node_response`, `WidgetInspector` |

### Notable Decisions/Tradeoffs

1. **`dispose_all` added to `ObjectGroupManager`**: The `WidgetInspector::dispose()` method needs to dispose the active group without creating a new one. Rather than adding logic to `WidgetInspector`, the `dispose_all` method was added to `ObjectGroupManager` to keep group lifecycle logic centralized. The `_client` parameter is unused in `dispose_all` (the client is already stored on `ObjectGroupManager`) — this was kept for API symmetry with `WidgetInspector::dispose(&mut self, client)`.

2. **`parse_diagnostics_node_response` tries `result` key first**: The function checks for a nested `"result"` key and falls back to the value itself. This handles both the case where `call_extension` has already unwrapped the JSON-RPC result (returns the node directly) and any future Flutter versions that might add an extra wrapper. This matches the task spec's design intent.

3. **`WidgetInspector::new` takes a `VmServiceClient`**: The struct stores an `ObjectGroupManager` which requires a client clone. The task spec showed only `isolate_id` as a parameter, but an `ObjectGroupManager` needs a client reference at construction time (to call `disposeGroup`). The constructor signature was updated to include the client.

4. **No `hashmap!` macro**: The codebase has no `hashmap!` convenience macro. All `HashMap` construction uses explicit insertion, consistent with existing code in `ObjectGroupManager::dispose_group()`.

### Testing Performed

- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon` — Passed (284 tests: 0 failed, 3 ignored)
- `cargo clippy -p fdemon-daemon -- -D warnings` — Passed (0 warnings)
- `cargo fmt --all` — Applied (minor formatting fix to `dispose_all` signature)

### Risks/Limitations

1. **`WidgetInspector` not tested end-to-end**: The struct requires a live VM Service connection for integration testing, which is out of scope for unit tests. The parsing and group management logic are unit-tested; the async extension calls are tested through `parse_diagnostics_node_response` and the existing `ObjectGroupManager` tests.

2. **`dispose_all` ignores the client parameter**: The `VmServiceClient` passed to `WidgetInspector::dispose()` is forwarded but not used (the stored client in `ObjectGroupManager` handles the RPC call). The `_client` parameter maintains a consistent API surface for callers.
