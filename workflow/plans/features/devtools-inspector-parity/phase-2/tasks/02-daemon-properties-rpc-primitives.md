## Task: Add `getProperties` RPC constant + response parsing helpers

**Objective**: Wire the new `ext.flutter.inspector.getProperties` extension into the fdemon-daemon VM service layer by declaring the method constant and providing free-function helpers that parse the response array and split widget vs. render-object properties. These primitives are consumed by task 05's `spawn_fetch_inspector_properties` background task.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — add `GET_PROPERTIES` constant and `pub mod properties;` declaration
- `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` **NEW** — response parsing helpers + tests

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` — the existing precedent for "free-function parser + inline JSON-mocked tests" module layout
- `crates/fdemon-core/src/widget_tree.rs` — `DiagnosticsNode` struct + the `is_render_object_property()` helper
- `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/inspector_service.dart:552–569, 915–925` — `getProperties` RPC request/response shape
- `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/inspector_controller.dart:890–932` — `_loadPropertiesForNode` algorithm

### Details

#### 1. Add the `GET_PROPERTIES` constant

In `crates/fdemon-daemon/src/vm_service/extensions/mod.rs`, the inspector RPC constants block (currently around lines 61–146 per the research, the "Widget inspector" sub-section) lists:

```rust
pub const GET_ROOT_WIDGET_TREE: &str = "ext.flutter.inspector.getRootWidgetTree";
pub const GET_ROOT_WIDGET_SUMMARY_TREE: &str = "ext.flutter.inspector.getRootWidgetSummaryTree";
pub const GET_DETAILS_SUBTREE: &str = "ext.flutter.inspector.getDetailsSubtree";
pub const GET_SELECTED_WIDGET: &str = "ext.flutter.inspector.getSelectedWidget";
pub const DISPOSE_GROUP: &str = "ext.flutter.inspector.disposeGroup";
```

Add immediately after `DISPOSE_GROUP`:

```rust
/// `ext.flutter.inspector.getProperties` — returns a list of `DiagnosticsNode`
/// describing each property of the widget identified by `arg = valueId`.
///
/// Request: `{ "arg": "<valueId>", "objectGroup": "<groupName>" }`
/// Response: `{ "result": [<DiagnosticsNode>, …] }`
///
/// Used by Phase 2's `FetchInspectorProperties` action. Properties whose
/// `propertyType == "RenderObject"` are recursively expanded by a second
/// `getProperties` call on the render object's `valueId` to surface the
/// render object's own properties (constraints, size, layer, semantics, etc.).
pub const GET_PROPERTIES: &str = "ext.flutter.inspector.getProperties";
```

Also add the module declaration in `extensions/mod.rs` (next to the existing `pub mod inspector;` and `pub mod layout;` declarations):

```rust
pub mod properties;
```

#### 2. Create `extensions/properties.rs`

```rust
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

use fdemon_core::DiagnosticsNode;
use serde_json::Value;

use crate::Error;

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
pub fn parse_properties_response(raw: &Value) -> Result<Vec<DiagnosticsNode>, Error> {
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
                .map_err(|e| Error::recoverable(format!("getProperties deserialize: {e}")))
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
```

If `Error` is not the right import path (e.g., the daemon crate's error type is named differently), follow whatever `extensions/layout.rs` uses for its error returns — adjust accordingly.

### Acceptance Criteria

1. `GET_PROPERTIES` is declared in `extensions/mod.rs` next to the other inspector RPC constants, with a doc comment describing the request/response shape.
2. `extensions/properties` module is declared in `extensions/mod.rs` and compiles.
3. `parse_properties_response` correctly deserializes both wrapping forms (`{"result": [...]}` and bare `[...]`) and an empty/missing response (returns `Ok(vec![])`).
4. `split_widget_and_render_properties` partitions correctly based on `is_render_object_property()` and preserves intra-bucket order.

### Testing

Add an inline `#[cfg(test)]` module in `properties.rs` using `serde_json::json!()` mocks, following the precedent in `extensions/layout.rs:242–831`:

```rust
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
```

### Notes

- This task does NOT add a method to `WidgetInspector` (the higher-level wrapper in `extensions/inspector.rs:308`). Per the spawn-task precedent (`spawn_fetch_layout_data` at `actions/inspector/mod.rs:332–487`), the action layer calls `handle.call_extension(ext::GET_PROPERTIES, args)` directly via `VmRequestHandle`, not through `WidgetInspector`. Free functions in `properties.rs` are the right pattern here.
- The recursion (second `getProperties` call for each render-object node) is intentionally NOT in this module — it requires the `VmRequestHandle`, timeout policy, and `tokio::spawn` context owned by task 05. Keep this module pure / sync / easily testable.
- The error type used by `Result<_, Error>` should match whatever `extensions/layout.rs` returns. If the implementor finds the error path differs (e.g. some helpers return `serde_json::Error` directly), follow the layout-extraction precedent.
- The line count for the new `properties.rs` is expected to be ~150–200 lines including tests; well under the 500-line CODE_STANDARDS threshold.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a51ec10e94edd46d2

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` | Added `GET_PROPERTIES` constant with doc comment in `ext` mod; added `pub mod properties;` declaration; extended `test_inspector_extension_constants_use_inspector_prefix` test to cover `GET_PROPERTIES` |
| `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` | NEW — `parse_properties_response` and `split_widget_and_render_properties` free functions with 6 inline tests |

### Notable Decisions/Tradeoffs

1. **Error constructor**: The task's code snippet used `Error::recoverable()` which does not exist in the codebase. Used `Error::protocol()` instead, matching the pattern in `extensions/layout.rs`. This is consistent with how deserialization errors are handled across all VM service extension helpers.
2. **No `pub use` re-export in mod.rs**: The `properties` module exports are not re-exported at the `extensions` top level (unlike `layout::` and `inspector::`). This is intentional — `parse_properties_response` and `split_widget_and_render_properties` are internal parsing helpers that task 05's action layer will reference via `extensions::properties::*` rather than needing a flat API surface.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (786 tests, including 6 new properties tests)
- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` - Passed
- `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` - Passed (all crates clean)

### Risks/Limitations

1. **No re-export**: The `properties` module is not re-exported at the `extensions` level. Task 05 (`spawn_fetch_inspector_properties`) will need to import via `crate::vm_service::extensions::properties::{parse_properties_response, split_widget_and_render_properties}`. If a flat API surface is later needed, add the re-exports in `mod.rs`.
