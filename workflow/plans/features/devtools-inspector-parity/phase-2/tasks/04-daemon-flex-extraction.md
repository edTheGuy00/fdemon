## Task: Extend `extract_layout_info` to populate flex children + axis/alignment

**Objective**: Extend the daemon's existing `extract_layout_info` function (`crates/fdemon-daemon/src/vm_service/extensions/layout.rs`) to populate the five new fields added to `LayoutInfo` in task 01: `direction`, `main_axis_alignment`, `cross_axis_alignment`, `main_axis_size`, and `children: Vec<FlexChild>`. No new RPC call — the data is already present in the `getLayoutExplorerNode` response we already fetch; we just weren't parsing it.

**Depends on**: 01 (FlexChild + axis/alignment enums must exist)

**Estimated Time**: 3–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/extensions/layout.rs`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/widget_tree.rs` — `FlexChild`, `FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize` (from task 01)
- `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/inspector_data_models.dart:457–482` — `FlexLayoutProperties._buildNode()` reference implementation
- `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart:139–154` — per-child JSON shape (`size`, `constraints`, `parentData`, `flexFactor`, `flexFit`)

### Details

#### JSON shape we're parsing

A `getLayoutExplorerNode` response for a `Column` looks roughly like:

```json
{
  "description": "Column",
  "valueId": "objects/100",
  "size": { "width": "180.0", "height": "872.0" },
  "constraints": { "description": "0.0<=w<=414.0, 0.0<=h<=896.0" },
  "renderObject": {
    "properties": [
      { "name": "direction", "description": "vertical" },
      { "name": "mainAxisAlignment", "description": "start" },
      { "name": "crossAxisAlignment", "description": "center" },
      { "name": "mainAxisSize", "description": "max" }
    ]
  },
  "children": [
    {
      "description": "Container",
      "valueId": "objects/101",
      "size": { "width": "180.0", "height": "341.0" },
      "constraints": { "description": "..." },
      "parentData": { "offsetX": "0.0", "offsetY": "0.0" },
      "flexFactor": null,
      "flexFit": null
    },
    {
      "description": "Expanded",
      "valueId": "objects/102",
      "size": { "width": "180.0", "height": "189.0" },
      "constraints": { "description": "..." },
      "parentData": { "offsetX": "0.0", "offsetY": "341.0" },
      "flexFactor": 1,
      "flexFit": "tight"
    }
  ]
}
```

#### 1. Parse axis / alignment / size from `renderObject.properties`

`renderObject.properties` is an array of `{name, description}` maps. We pluck the four named entries by name. Add a helper:

```rust
/// Look up a property by `name` in the `renderObject.properties` array of a
/// `getLayoutExplorerNode` response and return its `description` as a string.
fn render_property<'a>(raw: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    raw.get("renderObject")?
        .get("properties")?
        .as_array()?
        .iter()
        .find(|item| item.get("name").and_then(|v| v.as_str()) == Some(name))?
        .get("description")?
        .as_str()
}
```

Then, inside `extract_layout_info`, populate the four fields:

```rust
let direction = render_property(raw_json, "direction")
    .and_then(|s| serde_json::from_value::<Axis>(serde_json::Value::String(s.into())).ok());

let main_axis_alignment = render_property(raw_json, "mainAxisAlignment")
    .and_then(|s| serde_json::from_value::<MainAxisAlignment>(serde_json::Value::String(s.into())).ok());

let cross_axis_alignment = render_property(raw_json, "crossAxisAlignment")
    .and_then(|s| serde_json::from_value::<CrossAxisAlignment>(serde_json::Value::String(s.into())).ok());

let main_axis_size = render_property(raw_json, "mainAxisSize")
    .and_then(|s| serde_json::from_value::<MainAxisSize>(serde_json::Value::String(s.into())).ok());
```

The `serde_json::from_value` dance is necessary because the enum's `Deserialize` impl expects a JSON string. An alternative is to pattern-match the strings manually:

```rust
let direction = render_property(raw_json, "direction").map(|s| match s {
    "horizontal" => Axis::Horizontal,
    _ => Axis::Vertical,
});
```

Either pattern is acceptable; the implementor picks whichever reads better in context.

#### 2. Parse children → `Vec<FlexChild>`

Add a helper `extract_flex_child`:

```rust
fn extract_flex_child(child_json: &serde_json::Value) -> FlexChild {
    let id = child_json
        .get("valueId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let name = child_json
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| crate::sanitize::strip_ansi_codes(s).into_owned())
        .unwrap_or_default();

    let size = parse_widget_size(child_json);

    let constraints = child_json
        .get("constraints")
        .and_then(|c| c.get("description"))
        .and_then(|d| d.as_str())
        .and_then(BoxConstraints::parse);

    let flex_factor = child_json.get("flexFactor").and_then(|v| {
        v.as_u64()
            .map(|u| u as u32)
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    });

    let flex_fit = child_json
        .get("flexFit")
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "tight" => Some(FlexFit::Tight),
            "loose" => Some(FlexFit::Loose),
            _ => None,
        });

    let parent_offset = child_json.get("parentData").and_then(|pd| {
        let x = pd
            .get("offsetX")
            .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))?;
        let y = pd
            .get("offsetY")
            .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))?;
        Some((x, y))
    });

    FlexChild {
        id,
        name,
        size,
        constraints,
        flex_factor,
        flex_fit,
        parent_offset,
    }
}
```

Important:
- Use `strip_ansi_codes` on the `name` (description) to match the sanitization applied by `DiagnosticsNode` deserialization. The exact module path may differ — grep for `strip_ansi_codes` in the daemon crate; per the layout.rs precedent, sanitization is already applied to layout strings.
- Numbers in this JSON are sometimes serialized as strings (e.g. `"width": "180.0"`) and sometimes as numbers; the existing helpers `parse_widget_size` and the parent-offset extraction above both handle both forms.

Then, in `extract_layout_info`, populate `children`:

```rust
let children = raw_json
    .get("children")
    .and_then(|v| v.as_array())
    .map(|arr| arr.iter().map(extract_flex_child).collect::<Vec<_>>())
    .unwrap_or_default();
```

#### 3. Update the `LayoutInfo` literal returned by `extract_layout_info`

```rust
LayoutInfo {
    description: Some(node.description.clone()),
    constraints: /* existing */,
    size: /* existing */,
    flex_factor: /* existing */,
    flex_fit: /* existing */,
    padding: /* existing */,
    margin: /* existing */,
    direction,
    main_axis_alignment,
    cross_axis_alignment,
    main_axis_size,
    children,
}
```

### Acceptance Criteria

1. `extract_layout_info` now returns a `LayoutInfo` with the four new container-level fields populated when the response includes a `renderObject.properties` array (typically only for `Row`/`Column`/`Flex` widgets).
2. `extract_layout_info` populates `LayoutInfo.children` from the response's `children` array, with each `FlexChild` carrying its `name`, `size`, `constraints`, `flex_factor`, `flex_fit`, and `parent_offset` when present.
3. Non-flex widgets (e.g. `Container`, `Text`) continue to deserialize cleanly — all four axis/alignment fields are `None` and `children` is `vec![]`.
4. Existing `extensions/layout.rs` tests (per the research: ~30 tests at lines 242–831) continue to pass.

### Testing

Add new unit tests to the inline `mod tests` block in `layout.rs`. Pattern-match the existing JSON-mock style:

```rust
#[test]
fn extract_layout_info_column_with_flex_children() {
    let json = json!({
        "description": "Column",
        "valueId": "objects/100",
        "size": { "width": "180.0", "height": "872.0" },
        "constraints": { "description": "0.0<=w<=414.0, 0.0<=h<=896.0" },
        "renderObject": {
            "properties": [
                { "name": "direction", "description": "vertical" },
                { "name": "mainAxisAlignment", "description": "spaceBetween" },
                { "name": "crossAxisAlignment", "description": "stretch" },
                { "name": "mainAxisSize", "description": "max" }
            ]
        },
        "children": [
            {
                "description": "Container",
                "valueId": "objects/101",
                "size": { "width": "180.0", "height": "341.0" },
                "parentData": { "offsetX": "0.0", "offsetY": "0.0" }
            },
            {
                "description": "Expanded",
                "valueId": "objects/102",
                "size": { "width": "180.0", "height": "189.0" },
                "parentData": { "offsetX": "0.0", "offsetY": "341.0" },
                "flexFactor": 1,
                "flexFit": "tight"
            }
        ]
    });
    let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
    let info = extract_layout_info(&node, &json);

    assert_eq!(info.direction, Some(Axis::Vertical));
    assert_eq!(info.main_axis_alignment, Some(MainAxisAlignment::SpaceBetween));
    assert_eq!(info.cross_axis_alignment, Some(CrossAxisAlignment::Stretch));
    assert_eq!(info.main_axis_size, Some(MainAxisSize::Max));

    assert_eq!(info.children.len(), 2);
    assert_eq!(info.children[0].name, "Container");
    assert!(info.children[0].flex_factor.is_none());
    assert_eq!(info.children[0].parent_offset, Some((0.0, 0.0)));

    assert_eq!(info.children[1].name, "Expanded");
    assert_eq!(info.children[1].flex_factor, Some(1));
    assert_eq!(info.children[1].flex_fit, Some(FlexFit::Tight));
    assert_eq!(info.children[1].parent_offset, Some((0.0, 341.0)));
}

#[test]
fn extract_layout_info_non_flex_widget_has_empty_flex_fields() {
    let json = json!({
        "description": "Container",
        "valueId": "objects/200",
        "size": { "width": "100.0", "height": "50.0" }
    });
    let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
    let info = extract_layout_info(&node, &json);

    assert!(info.direction.is_none());
    assert!(info.main_axis_alignment.is_none());
    assert!(info.children.is_empty());
}

#[test]
fn extract_layout_info_strips_ansi_from_child_name() {
    let json = json!({
        "description": "Row",
        "children": [
            { "description": "\x1b[33mText\x1b[0m" }
        ]
    });
    let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
    let info = extract_layout_info(&node, &json);
    assert_eq!(info.children[0].name, "Text");
}

#[test]
fn extract_layout_info_handles_numeric_offsets() {
    let json = json!({
        "description": "Column",
        "children": [
            {
                "description": "A",
                "parentData": { "offsetX": 1.5, "offsetY": 2.5 }
            }
        ]
    });
    let node: DiagnosticsNode = serde_json::from_value(json.clone()).unwrap();
    let info = extract_layout_info(&node, &json);
    assert_eq!(info.children[0].parent_offset, Some((1.5, 2.5)));
}
```

### Notes

- This task does NOT issue a new RPC. The `children` array is already in the `getLayoutExplorerNode` response that Phase 1's `spawn_fetch_layout_data` fetches — we just weren't parsing past the top-level fields. No new spawn task, no new action, no message round-trip change for the flex explorer tab. Once this task lands, the data is available immediately to the Flex Explorer tab renderer (task 09).
- The `text_direction`, `vertical_direction`, `textBaseline` flex container fields (also listed in DevTools' `FlexLayoutProperties._buildNode`) are intentionally NOT parsed. They affect rendering of axis labels in DevTools but are not surfaced in the TUI flex visualization per parent PLAN §5.3. Add them only if task 09 explicitly needs them.
- The current `extract_layout_info` (per the research at `layout.rs:62–91`) is ~30 lines. After this task it grows to ~80–100 lines including helpers. Well under 500-line file budget.
- Existing tests in `layout.rs` (lines 242–831 per the research) include many `extract_layout_info_*` assertions; the new fields default to `None`/empty so they should still pass — but the implementor should run the full file's test suite to confirm.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a3c667889a4fbfbd0

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` | Added `render_property` and `extract_flex_child` helpers; extended `extract_layout_info` to populate `direction`, `main_axis_alignment`, `cross_axis_alignment`, `main_axis_size`, and `children`; added 18 new unit tests |

### Notable Decisions/Tradeoffs

1. **Match-based enum parsing vs `serde_json::from_value`**: Used explicit `match` strings for all enum variants instead of the serde roundtrip. This is more explicit, avoids a deserialization indirection, and makes the mapping unambiguous (especially for camelCase names like `spaceBetween` that differ from Rust variant names).
2. **`strip_ansi_codes` return type**: The function returns `String` (not `Cow<str>`), so `into_owned()` was not needed — used `map(strip_ansi_codes)` directly as a function pointer.
3. **Formatter reformatted parentData extraction**: `cargo fmt` wrapped the chained `offsetX`/`offsetY` extraction differently than the task's sample code; the formatted version is equivalent.

### Testing Performed

- `cargo test -p fdemon-daemon --lib vm_service::extensions::layout` — Passed (48 tests: 30 pre-existing + 18 new)
- `cargo test -p fdemon-daemon --lib` — Passed (800 tests, 0 failed)
- `cargo clippy -p fdemon-daemon` — Clean (0 warnings)
- `cargo fmt --all -- --check` — Clean

### Risks/Limitations

1. **Unknown enum variant fallback**: An unknown `direction` value defaults to `Axis::Vertical` (rather than `None`). This matches the Flutter default but could mask new direction values. The task spec and existing codebase use this pattern, so it's intentional.
2. **No new RPC call**: As specified, the `children` field is parsed directly from the existing `getLayoutExplorerNode` response. This means the flex explorer data is available without any additional network round-trips.
