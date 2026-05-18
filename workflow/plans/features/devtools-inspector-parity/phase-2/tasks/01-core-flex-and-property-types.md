## Task: Add flex layout types + sanitize `property_type` in fdemon-core

**Objective**: Extend `crates/fdemon-core/src/widget_tree.rs` with the Rust types needed to model flex layout children + container axis/alignment, and wire ANSI sanitization into the existing `DiagnosticsNode.property_type` field. These types are the foundation for Phase 2's Render Object tab (consumes `DiagnosticsNode.property_type`) and Flex Explorer tab (consumes `LayoutInfo.children` + axis/alignment fields).

**Depends on**: None

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs`

**Files Read (Dependencies):**
- `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/inspector_data_models.dart:457–482` — flex enum values and parsing
- `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart:78–81, 106–108, 139–154` — flex child JSON shape
- `crates/fdemon-core/src/ansi.rs` — `strip_ansi_codes()` (already a dependency of existing sanitized fields)

### Details

#### 1. Add `FlexFit` enum

```rust
/// `FlexFit` for a `Flexible`/`Expanded` child.
///
/// Mirrors Flutter's `FlexFit` enum:
/// - `Tight` — the child is forced to fill the available main-axis space.
/// - `Loose` — the child takes its intrinsic main-axis size up to the available space.
///
/// Source: `tmp/devtools/.../diagnostics_node.dart:78–81`. The JSON field is a
/// string (`"tight"` / `"loose"`). Default per DevTools when missing or
/// unrecognized is `Loose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlexFit {
    Tight,
    #[default]
    Loose,
}
```

#### 2. Add `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize` enums

```rust
/// Flex container's primary direction.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:466`. Parsed from the
/// `direction` property in `renderObject.properties` (`"horizontal"` /
/// `"vertical"`). Default per DevTools: `Vertical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    Horizontal,
    #[default]
    Vertical,
}

/// Flex container's main-axis alignment.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:467–469`.
/// Field name in `renderObject.properties`: `mainAxisAlignment`. Default: `Start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MainAxisAlignment {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Flex container's cross-axis alignment.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:470–472`.
/// Field name in `renderObject.properties`: `crossAxisAlignment`. Default: `Center`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CrossAxisAlignment {
    Start,
    End,
    #[default]
    Center,
    Stretch,
    Baseline,
}

/// Flex container's main-axis size policy.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:473`.
/// Field name in `renderObject.properties`: `mainAxisSize`. Default: `Max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MainAxisSize {
    Min,
    #[default]
    Max,
}
```

All four enums use `Default` so that missing JSON fields produce a sensible default at parse time (Phase 2 task 04 does this parsing).

#### 3. Add `FlexChild` struct

```rust
/// One child of a `Row`/`Column`/`Flex` container, as surfaced by
/// `ext.flutter.inspector.getLayoutExplorerNode`.
///
/// Source: `tmp/devtools/.../diagnostics_node.dart:139–154` describes the
/// per-child JSON shape:
/// - `size: { width, height }`
/// - `constraints: { minWidth, maxWidth, minHeight, maxHeight }`
/// - `parentData: { offsetX, offsetY }`
/// - `flexFactor: int | null`
/// - `flexFit: "tight" | "loose" | null`
///
/// Phase 2's `extract_layout_info` (task 04) walks `node.children` and emits
/// one `FlexChild` per child (whether or not it has flex; a child with
/// `flex_factor: None` is a fixed-size child).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlexChild {
    /// `valueId` of the child node (for cross-reference back into the widget
    /// tree). Optional because some children may not carry a valueId.
    pub id: Option<String>,
    /// Description string (widget runtime type, e.g. `"Container"`,
    /// `"Padding"`). Sanitized at extract time.
    pub name: String,
    /// Child's measured size, if available.
    pub size: Option<WidgetSize>,
    /// Constraints the parent passed to this child.
    pub constraints: Option<BoxConstraints>,
    /// `flexFactor` from `parentData`. `None` means non-flex (fixed-size).
    pub flex_factor: Option<u32>,
    /// `flexFit`. `None` when the child is not a `Flexible`/`Expanded`.
    pub flex_fit: Option<FlexFit>,
    /// Pixel offset of the child within the parent (from `parentData`).
    /// Stored as `(offset_x, offset_y)` in logical pixels.
    pub parent_offset: Option<(f64, f64)>,
}
```

#### 4. Extend `LayoutInfo`

Add five fields after the existing `margin` field (currently at `widget_tree.rs:817`):

```rust
pub struct LayoutInfo {
    // ... existing fields: constraints, size, flex_factor, flex_fit,
    //                       description, padding, margin

    /// Flex container's primary axis (only meaningful for `Row`/`Column`/`Flex`).
    /// `None` for non-flex widgets.
    pub direction: Option<Axis>,

    /// Main-axis alignment (only meaningful for flex containers).
    pub main_axis_alignment: Option<MainAxisAlignment>,

    /// Cross-axis alignment (only meaningful for flex containers).
    pub cross_axis_alignment: Option<CrossAxisAlignment>,

    /// Main-axis size policy (only meaningful for flex containers).
    pub main_axis_size: Option<MainAxisSize>,

    /// Children of the flex container, in render order. Empty for non-flex
    /// widgets or when the layout response did not include children.
    pub children: Vec<FlexChild>,
}
```

All five are `Option` / `Vec` so non-flex widgets continue to deserialize cleanly with `Default::default()` semantics.

#### 5. Wire ANSI sanitization for `property_type`

Locate the existing `property_type` field at `widget_tree.rs:97–102`:

```rust
/// Property type metadata for diagnostics-as-properties nodes ...
#[serde(default, rename = "propertyType")]
pub property_type: Option<String>,
```

Replace the attribute with:

```rust
#[serde(
    default,
    rename = "propertyType",
    deserialize_with = "deserialize_sanitized_option_string"
)]
pub property_type: Option<String>,
```

This matches the existing pattern used by `LayoutInfo.flex_fit` (`widget_tree.rs:806–807`) and `LayoutInfo.description` (`widget_tree.rs:810–811`).

### Acceptance Criteria

1. The five new enums (`FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize`) compile, derive `Debug + Clone + Copy + PartialEq + Eq + Default + Serialize + Deserialize`, and deserialize from the camelCase / lowercase JSON strings used by Flutter's `getLayoutExplorerNode` response.
2. `FlexChild` compiles with the field list above and derives `Debug + Clone + PartialEq + Default`.
3. `LayoutInfo` gains the five new fields; existing tests in `widget_tree.rs:1395–1500` (LayoutInfo / EdgeInsets section) continue to pass after the additions (they should — the new fields default to `None` / empty `Vec`).
4. `DiagnosticsNode.property_type` passes through `strip_ansi_codes()` at deserialize time.

### Testing

Add new unit tests inside the existing `mod tests` block. Group them under new comment banners after the existing `LayoutInfo / EdgeInsets tests` banner (~line 1500).

```rust
// -----------------------------------------------------------------------
// Flex enums
// -----------------------------------------------------------------------

#[test]
fn flex_fit_deserializes_tight_and_loose() {
    let tight: FlexFit = serde_json::from_str("\"tight\"").unwrap();
    let loose: FlexFit = serde_json::from_str("\"loose\"").unwrap();
    assert_eq!(tight, FlexFit::Tight);
    assert_eq!(loose, FlexFit::Loose);
}

#[test]
fn flex_fit_default_is_loose() {
    assert_eq!(FlexFit::default(), FlexFit::Loose);
}

#[test]
fn main_axis_alignment_deserializes_space_between() {
    let v: MainAxisAlignment = serde_json::from_str("\"spaceBetween\"").unwrap();
    assert_eq!(v, MainAxisAlignment::SpaceBetween);
}

#[test]
fn cross_axis_alignment_deserializes_stretch_and_baseline() {
    let stretch: CrossAxisAlignment = serde_json::from_str("\"stretch\"").unwrap();
    let baseline: CrossAxisAlignment = serde_json::from_str("\"baseline\"").unwrap();
    assert_eq!(stretch, CrossAxisAlignment::Stretch);
    assert_eq!(baseline, CrossAxisAlignment::Baseline);
}

#[test]
fn axis_default_is_vertical() {
    assert_eq!(Axis::default(), Axis::Vertical);
}

// -----------------------------------------------------------------------
// FlexChild struct
// -----------------------------------------------------------------------

#[test]
fn flex_child_default_is_empty() {
    let c = FlexChild::default();
    assert!(c.id.is_none());
    assert_eq!(c.name, "");
    assert!(c.flex_factor.is_none());
}

// -----------------------------------------------------------------------
// property_type sanitization (Phase 2)
// -----------------------------------------------------------------------

#[test]
fn property_type_strips_ansi_codes() {
    let json = serde_json::json!({
        "description": "Color",
        "propertyType": "\x1b[31mRenderObject\x1b[0m"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.property_type.as_deref(), Some("RenderObject"));
}

#[test]
fn property_type_passes_through_clean_strings() {
    let json = serde_json::json!({
        "description": "Color",
        "propertyType": "RenderObject"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.property_type.as_deref(), Some("RenderObject"));
}
```

### Notes

- The four flex-direction/alignment fields on `LayoutInfo` are added here but populated by task 04 (daemon extraction). Until task 04 lands, they remain `None` at runtime; that's fine — task 09 (Flex Explorer renderer) only reads them after task 04 wires them up.
- The `FlexChild.parent_offset` uses a tuple `(f64, f64)` rather than a named struct to keep the file from accumulating yet another small type. If task 04's parser finds the tuple awkward, the implementor of task 01 may revisit this choice — but per CODE_STANDARDS, prefer named types only when they earn their keep.
- Do NOT add `Serialize`/`Deserialize` derives to `FlexChild` itself. Per the existing precedent on `LayoutInfo`, the struct is populated manually by `extract_layout_info` from raw JSON, not via serde derive on the whole struct.
- The file is already at ~2228 lines (Phase 1.5 deferred splitting it — review item m1). This task adds ~150 lines of types + tests. The file split (m1) is explicitly deferred to a separate pre-Phase-2 task per the Phase 1.5 plan's "Out of Scope" table — do not attempt to split here.
- All new `pub` items require `///` doc comments per CODE_STANDARDS.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/widget_tree.rs` | Added `FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize` enums; added `FlexChild` struct; extended `LayoutInfo` with 5 new fields; wired ANSI sanitization for `property_type`; added 15 new unit tests |
| `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` | Extended `extract_layout_info` struct literal to include the 5 new `LayoutInfo` fields (all `None`/empty pending task 04) |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` | Extended `LayoutInfo` struct literal in test helper to include new fields |
| `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel_tests.rs` | Extended `LayoutInfo` struct literal in test helper to include new fields |

### Notable Decisions/Tradeoffs

1. **`children: Vec<FlexChild>` uses `#[serde(skip)]` on `LayoutInfo`**: `FlexChild` intentionally has no `Serialize`/`Deserialize` derives (per task note), so the `children` field would prevent `LayoutInfo` from auto-deriving `Deserialize`. Using `#[serde(skip)]` keeps `LayoutInfo`'s existing serde derives intact while allowing task 04 to populate `children` manually.
2. **Propagated `None`/empty to existing callers**: Two `LayoutInfo` struct literals in daemon and two in tui tests needed new fields added. All set to `None`/empty since task 04 handles real population.
3. **Rust unicode escape in test**: The `serde_json::json!` macro uses Rust string syntax, so `` must be written as `\u{001b}` — matched the pattern used in task specification but corrected to valid Rust syntax.

### Testing Performed

- `cargo check -p fdemon-core` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-core` - Passed (438 tests, 15 new)
- `cargo test --workspace --lib` - Passed (1091 tests)
- `cargo clippy --workspace` - Passed (no errors)
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

1. **Task 04 dependency**: The four flex alignment/direction fields on `LayoutInfo` and `FlexChild` population are left `None`/empty until task 04 (daemon flex extraction) wires the real parsing from the `getLayoutExplorerNode` response.
