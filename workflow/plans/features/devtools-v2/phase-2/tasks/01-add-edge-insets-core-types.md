## Task: Add EdgeInsets Type and Extend LayoutInfo

**Objective**: Add an `EdgeInsets` struct to `fdemon-core` and extend `LayoutInfo` with `padding` and `margin` fields, preparing the domain model for the merged Inspector+Layout panel's box model visualization.

**Depends on**: None

### Scope

- `crates/fdemon-core/src/widget_tree.rs`: Add `EdgeInsets` struct, extend `LayoutInfo`

### Details

#### Add `EdgeInsets` struct

Add near the other layout types (`BoxConstraints`, `WidgetSize`) in `widget_tree.rs`:

```rust
/// Edge insets representing padding or margin on four sides.
///
/// Parsed from Flutter's diagnostic property format:
/// `"EdgeInsets(8.0, 0.0, 8.0, 0.0)"` or individual LTRB values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}
```

Add methods:

```rust
impl EdgeInsets {
    pub fn zero() -> Self {
        Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 }
    }

    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }

    /// Parse from Flutter's EdgeInsets string format.
    ///
    /// Supported formats:
    /// - `"EdgeInsets(8.0, 0.0, 8.0, 0.0)"` — (top, right, bottom, left)
    /// - `"EdgeInsets.all(8.0)"` — uniform on all sides
    /// - `"EdgeInsets.zero"` — all zeros
    /// - `"EdgeInsets(0.0, 16.0, 0.0, 16.0)"` — TRBL ordering from Flutter
    pub fn parse(s: &str) -> Option<Self> {
        // Implementation: strip "EdgeInsets" prefix, parse values
        // See Testing section for expected behavior
    }
}
```

#### Extend `LayoutInfo`

Add two new optional fields to the existing `LayoutInfo` struct (lines 163-179 of `widget_tree.rs`):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutInfo {
    pub constraints: Option<BoxConstraints>,
    pub size: Option<WidgetSize>,
    pub flex_factor: Option<f64>,
    pub flex_fit: Option<String>,
    pub description: Option<String>,
    pub padding: Option<EdgeInsets>,    // NEW
    pub margin: Option<EdgeInsets>,     // NEW
}
```

Both fields default to `None` via `#[derive(Default)]`. Existing code that constructs `LayoutInfo` without these fields will continue to compile since they're `Option` with default `None`.

#### Export from lib.rs

Ensure `EdgeInsets` is exported from `fdemon-core/src/lib.rs` alongside the existing `LayoutInfo`, `BoxConstraints`, `WidgetSize` types.

### Acceptance Criteria

1. `EdgeInsets` struct exists with `top`, `right`, `bottom`, `left` fields (all `f64`)
2. `EdgeInsets::parse()` handles at least: `"EdgeInsets(T, R, B, L)"`, `"EdgeInsets.all(N)"`, `"EdgeInsets.zero"`
3. `EdgeInsets::is_zero()` returns `true` when all values are `0.0`
4. `LayoutInfo` has `padding: Option<EdgeInsets>` and `margin: Option<EdgeInsets>` fields
5. All existing `LayoutInfo` tests pass unchanged (new fields default to `None`)
6. `EdgeInsets` derives `Debug, Clone, PartialEq, Serialize, Deserialize`
7. `cargo check -p fdemon-core` and `cargo check -p fdemon-daemon` pass (downstream crates unaffected)

### Testing

Add tests in `widget_tree.rs` (inline `#[cfg(test)] mod tests` or adjacent):

```rust
#[test]
fn test_edge_insets_parse_trbl() {
    let ei = EdgeInsets::parse("EdgeInsets(8.0, 16.0, 8.0, 16.0)").unwrap();
    assert_eq!(ei, EdgeInsets { top: 8.0, right: 16.0, bottom: 8.0, left: 16.0 });
}

#[test]
fn test_edge_insets_parse_all() {
    let ei = EdgeInsets::parse("EdgeInsets.all(8.0)").unwrap();
    assert_eq!(ei, EdgeInsets { top: 8.0, right: 8.0, bottom: 8.0, left: 8.0 });
}

#[test]
fn test_edge_insets_parse_zero() {
    let ei = EdgeInsets::parse("EdgeInsets.zero").unwrap();
    assert!(ei.is_zero());
}

#[test]
fn test_edge_insets_parse_invalid_returns_none() {
    assert!(EdgeInsets::parse("not an edge insets").is_none());
    assert!(EdgeInsets::parse("").is_none());
}

#[test]
fn test_layout_info_default_has_no_padding() {
    let info = LayoutInfo::default();
    assert!(info.padding.is_none());
    assert!(info.margin.is_none());
}
```

### Notes

- `EdgeInsets` follows the same pattern as `BoxConstraints` (parse from string, optional in `LayoutInfo`)
- The actual VM Service parsing (extracting padding from JSON) is Task 04 — this task only adds the types
- Flutter's `EdgeInsets` string format may vary by Dart version; the parser should be lenient and return `None` on unrecognized formats
- `margin` may never be populated by current Flutter diagnostics, but adding the field now avoids a future schema change

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/widget_tree.rs` | Added `EdgeInsets` struct with `zero()`, `is_zero()`, `parse()` methods; added `padding` and `margin` fields to `LayoutInfo`; added 9 new unit tests |
| `crates/fdemon-core/src/lib.rs` | Exported `EdgeInsets` alongside existing widget tree types |
| `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` | Added `..Default::default()` to `LayoutInfo` struct literal to compile with new optional fields |
| `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs` | Added `..Default::default()` to 4 `LayoutInfo` struct literals in test helpers |

### Notable Decisions/Tradeoffs

1. **Struct update syntax for downstream literals**: Rather than explicitly writing `padding: None, margin: None` in every existing struct literal, used `..Default::default()` which is more maintainable as new optional fields are added in future tasks. This is consistent with Rust idiom for structs with `Default`.

2. **Pre-existing `fdemon-app` failures are out-of-scope**: The workspace has pre-existing broken changes in `fdemon-app/src/handler/update.rs` that reference a removed `layout_explorer` field (from earlier phase-2 work merging `LayoutExplorerState` into `InspectorState`). These are unrelated to Task 01 and existed before this task started.

3. **Parser is lenient**: `EdgeInsets::parse()` returns `None` for unrecognised formats rather than erroring, matching the task spec ("lenient") and the `BoxConstraints::parse()` pattern already in the file.

### Testing Performed

- `cargo check -p fdemon-core` — Passed
- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-core` — Passed (331 unit tests + 5 doc tests)
- `cargo clippy -p fdemon-core -- -D warnings` — Passed

### Risks/Limitations

1. **`fdemon-app` pre-existing errors**: The workspace does not fully compile due to pre-existing phase-2 work in progress (`handler/update.rs` still references `layout_explorer` which was removed from `DevToolsViewState` by an earlier task). This does not affect Task 01's deliverables (`fdemon-core` compiles and tests cleanly).
