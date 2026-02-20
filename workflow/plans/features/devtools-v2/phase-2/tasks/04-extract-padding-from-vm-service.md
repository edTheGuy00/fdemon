## Task: Extract Padding Data from VM Service

**Objective**: Extend the VM Service layout extension to extract padding and margin `EdgeInsets` data from Flutter's diagnostic node properties, populating the new `LayoutInfo.padding` and `LayoutInfo.margin` fields added in Task 01.

**Depends on**: Task 01 (add-edge-insets-core-types)

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions/layout.rs`: Extend `extract_layout_info` to parse padding/margin

### Details

#### Where padding data lives in Flutter's VM Service response

When `ext.flutter.inspector.getLayoutExplorerNode` is called, the response JSON includes a `properties` array on each diagnostics node. Padding widgets (like `Padding`, `EdgeInsets`, `Container` with padding) expose padding as properties:

```json
{
  "description": "Padding",
  "properties": [
    {
      "name": "padding",
      "description": "EdgeInsets(8.0, 16.0, 8.0, 16.0)",
      "type": "DiagnosticsProperty<EdgeInsetsGeometry>"
    }
  ],
  "renderObject": {
    "properties": [
      {
        "name": "padding",
        "description": "EdgeInsets(8.0, 16.0, 8.0, 16.0)"
      }
    ]
  }
}
```

The padding may appear in either the widget's `properties` or the `renderObject.properties`. Check both locations, preferring the render object (more reliable).

#### Extend `extract_layout_info` (layout.rs, lines 67-87)

The current function signature:
```rust
pub fn extract_layout_info(node: &DiagnosticsNode, raw_json: &serde_json::Value) -> LayoutInfo
```

After extracting `constraints`, `size`, `flex_factor`, `flex_fit`, and `description` (existing logic), add:

```rust
// Extract padding from properties
let padding = extract_edge_insets(raw_json, "padding");
let margin = extract_edge_insets(raw_json, "margin");

LayoutInfo {
    constraints,
    size,
    flex_factor,
    flex_fit,
    description,
    padding,
    margin,
}
```

#### Add `extract_edge_insets` helper

```rust
/// Search for a named EdgeInsets property in the node's properties arrays.
///
/// Checks both top-level `properties` and `renderObject.properties`.
/// Returns `None` if the property is not found or cannot be parsed.
fn extract_edge_insets(raw_json: &serde_json::Value, name: &str) -> Option<EdgeInsets> {
    // 1. Check renderObject.properties first (more reliable)
    if let Some(props) = raw_json.get("renderObject")
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
```

#### Graceful degradation

- If no padding property exists on the node (most widgets don't have padding), `padding` is `None` — this is the common case
- If the EdgeInsets string format is unrecognized, `EdgeInsets::parse` returns `None` — graceful fallback
- The layout panel widget (Task 05) already handles `padding: None` by hiding the padding section

### Acceptance Criteria

1. `extract_layout_info` populates `padding` field when padding data is available in the JSON
2. `extract_layout_info` populates `margin` field when margin data is available
3. Both fields remain `None` when no padding/margin properties exist (majority of widgets)
4. Parser handles at least these EdgeInsets formats: `"EdgeInsets(T, R, B, L)"`, `"EdgeInsets.all(N)"`, `"EdgeInsets.zero"`
5. No panics on malformed property data
6. `cargo check -p fdemon-daemon` passes
7. `cargo test -p fdemon-daemon` passes

### Testing

Add tests in `layout.rs` (or a sibling test module):

```rust
#[test]
fn test_extract_edge_insets_from_render_object() {
    let json = serde_json::json!({
        "renderObject": {
            "properties": [
                { "name": "padding", "description": "EdgeInsets(8.0, 16.0, 8.0, 16.0)" }
            ]
        }
    });
    let ei = extract_edge_insets(&json, "padding").unwrap();
    assert_eq!(ei.top, 8.0);
    assert_eq!(ei.right, 16.0);
}

#[test]
fn test_extract_edge_insets_from_top_level_properties() {
    let json = serde_json::json!({
        "properties": [
            { "name": "padding", "description": "EdgeInsets.all(8.0)" }
        ]
    });
    let ei = extract_edge_insets(&json, "padding").unwrap();
    assert_eq!(ei, EdgeInsets { top: 8.0, right: 8.0, bottom: 8.0, left: 8.0 });
}

#[test]
fn test_extract_edge_insets_missing_returns_none() {
    let json = serde_json::json!({ "properties": [] });
    assert!(extract_edge_insets(&json, "padding").is_none());
}

#[test]
fn test_extract_layout_info_with_padding() {
    // Full integration test: construct a raw JSON with constraints, size, AND padding,
    // verify all fields are populated in the returned LayoutInfo
}

#[test]
fn test_extract_layout_info_without_padding_still_works() {
    // Regression: existing JSON without padding properties still produces valid LayoutInfo
}
```

### Notes

- The `EdgeInsets::parse` implementation lives in `fdemon-core` (Task 01). This task only calls it — it does not implement the parser.
- Not all Flutter widgets expose padding. `Padding`, `Container`, `SizedBox` with padding do. `Text`, `Column`, `Row` generally don't. The `None` case is the happy path for most widgets.
- The `renderObject.properties` source is preferred because render objects reflect the actual rendered layout, while widget properties may include default/inherited values.
- Future enhancement: also extract `alignment` and `transform` properties if they become useful for the layout panel visualization.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` | Added `EdgeInsets` to imports; replaced `..Default::default()` in `extract_layout_info` with explicit `padding` and `margin` fields; added `extract_edge_insets` helper function; added 12 new unit tests |

### Notable Decisions/Tradeoffs

1. **Explicit struct fields over `..Default::default()`**: Replaced `..Default::default()` with explicit `padding` and `margin` fields in `extract_layout_info`. This makes the struct construction exhaustive and ensures the compiler catches any future `LayoutInfo` field additions that need to be handled in this function.

2. **`renderObject.properties` preferred over top-level `properties`**: As specified in the task, `renderObject.properties` is checked first because it reflects actual rendered layout, while top-level `properties` may include default or inherited values. The fallback order is clearly documented in the `extract_edge_insets` doc comment.

3. **`None` is the happy path**: `extract_edge_insets` returns `None` silently for missing or unparseable properties — no panics, no logging noise. Only `EdgeInsets::parse` returning a value triggers a `Some` return.

4. **No public API surface added**: `extract_edge_insets` is a private helper (`fn`, not `pub fn`), matching the pattern of other helpers in this file (`parse_widget_size`).

### Testing Performed

- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon` — Passed (348 tests, 3 ignored integration tests)

New tests added (all in `vm_service::extensions::layout::tests`):
- `test_extract_edge_insets_from_render_object`
- `test_extract_edge_insets_from_top_level_properties`
- `test_extract_edge_insets_missing_returns_none`
- `test_extract_edge_insets_no_properties_field_returns_none`
- `test_extract_edge_insets_prefers_render_object_over_top_level`
- `test_extract_edge_insets_zero_format`
- `test_extract_edge_insets_malformed_description_returns_none`
- `test_extract_edge_insets_wrong_property_name_returns_none`
- `test_extract_layout_info_with_padding` (integration)
- `test_extract_layout_info_without_padding_still_works` (regression)
- `test_extract_layout_info_with_margin`

### Risks/Limitations

1. **No `margin` in real Flutter responses**: Flutter's layout explorer generally does not emit a `margin` property in `renderObject.properties`. The margin extraction logic is correct and tested, but in practice it will almost always return `None`. This is acceptable — the field is future-compatible.
2. **`properties` array in `DiagnosticsNode` vs raw JSON `properties`**: The raw JSON `properties` array (read by `extract_edge_insets`) is distinct from the `DiagnosticsNode.properties` field already parsed by serde. Both exist as parallel data sources; this implementation reads directly from `raw_json` (the unparsed JSON value) as specified in the task.
