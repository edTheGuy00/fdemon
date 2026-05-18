## Task: Flex Explorer tab — ASCII flex diagram

**Objective**: Replace the "Coming soon — Phase 2" stub in `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` with a real ASCII flex visualization consuming `LayoutInfo.children`, `direction`, `main_axis_alignment`, `cross_axis_alignment`, and `main_axis_size` populated by task 04.

**Depends on**: 04 (daemon flex extraction)

**Estimated Time**: 3–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/widget_tree.rs` — `LayoutInfo`, `FlexChild`, `FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize`
- `crates/fdemon-app/src/state.rs` — `InspectorState.layout`, `layout_loading`, `layout_error`
- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` — existing `format_constraint_value` and other formatting helpers
- Parent PLAN.md §5.3 — the ASCII layout intent (constants for box-min-height, axis-arrow chars)

### Details

#### Visual intent (parent PLAN §5.3)

```
┌─ Cross Axis: center ────────────────────────────────────────────┐
│ Column                                          Total Flex: 0    │
│ ┌────────────────────────────────────────────────────────────┐ ▲│
│ │  w=180  h=341                                              │M ││
│ │     [child #1] flex=0 fit=tight                            │a ││
│ │                                                            │i ││
│ ├────────────────────────────────────────────────────────────┤n ││
│ │  w=180  h=189                                              │  ││
│ │     [child #2] flex=0 fit=tight                            │A ││
│ ├────────────────────────────────────────────────────────────┤x ││
│ │  w=180  h=341                                              │i ││
│ │     [child #3] flex=0 fit=tight                            │s ││
│ └────────────────────────────────────────────────────────────┘ ▼│
│ constraints: 0 ≤ w ≤ 392, 0 ≤ h ≤ 872   size: 180 × 872          │
└──────────────────────────────────────────────────────────────────┘
```

**Important simplification (parent PLAN §7.1, cross-cutting constraint #9)**: child boxes are **fixed-height stacked equal-size boxes**. They do NOT scale proportionally to actual flex factor or measured dimensions. Hierarchy is communicated entirely through labels.

#### Layout structure

The Flex Explorer tab is one bordered block (`Borders::ALL`, title varies by axis — see below). Inside:

1. **Top border**: cross-axis label embedded in the border line: `"┌─ Cross Axis: center ──...──┐"`. The label varies with `cross_axis_alignment`.
2. **Header row**: widget type name (e.g. `"Column"`, `"Row"`) on the left + "Total Flex: N" on the right.
3. **Main content**: child boxes drawn vertically (for `direction == Vertical`) or horizontally (for `direction == Horizontal`). Each box has a top border (`┌─...─┐`), a label row (`│  w=W  h=H                  │`), a child name row (`│     [child #N] flex=F fit=T │`), and a shared bottom border with the next box (`├─...─┤` for non-last, `└─...─┘` for last).
4. **Main-axis indicator strip**: on the right (vertical) or bottom (horizontal). For vertical: a 1-cell-wide column reading `"▲M a i n  A x i s ▼"` with main-axis alignment label included.
5. **Bottom row**: `"constraints: <c>   size: <s>"`.

This is a moderately complex draw. Recommend a step-by-step approach:

- Step A: Render the bordered outer block (full area) with title showing cross-axis alignment.
- Step B: Render the header row inside the block.
- Step C: Render the child-box stack (or row).
- Step D: Render the main-axis indicator strip on the appropriate side.
- Step E: Render the footer row.

Use named constants for all magic numbers:

```rust
/// Minimum visible height inside the tab block (excluding outer borders) for
/// the visualization to fit. Below this, render a "Terminal too narrow"
/// fallback message.
const MIN_FLEX_VIZ_HEIGHT: u16 = 12;

/// Minimum visible width inside the tab block. Below this, fallback message.
const MIN_FLEX_VIZ_WIDTH: u16 = 40;

/// Height of one child box in cells. Constant — boxes do NOT scale with
/// flex factor or actual size (per parent PLAN §7.1).
const CHILD_BOX_HEIGHT: u16 = 4;

/// Width of the main-axis indicator strip (in cells) on the right or bottom.
const MAIN_AXIS_STRIP_WIDTH: u16 = 3;

/// Up/down arrow chars used in the main-axis strip.
const MAIN_AXIS_ARROW_UP: char = '▲';
const MAIN_AXIS_ARROW_DOWN: char = '▼';
const MAIN_AXIS_ARROW_LEFT: char = '◀';
const MAIN_AXIS_ARROW_RIGHT: char = '▶';
```

#### States

1. **Layout missing** (`inspector.layout.is_none()` && `!layout_loading`): muted line `"No layout data — press Enter to fetch."`.
2. **Loading** (`layout_loading == true`): muted `"Loading layout..."`.
3. **Error** (`layout_error.is_some()`): error summary + hint.
4. **Not a flex container** (`layout.children.is_empty() && layout.direction.is_none()`): muted line `"This widget is not a Row, Column, or Flex container."` (Phase 3 will hide the tab entirely in this case; for Phase 2 we render an explanatory message.)
5. **Flex container, children present**: render the visualization.

#### Child rendering rules

For each `FlexChild` in `layout.children`:

- Label row 1: `format!("  w={}  h={}", child.size.width, child.size.height)` (if size present), else `"  (unmeasured)"`.
- Label row 2: `format!("     [{}] flex={} fit={}", child.name, child.flex_factor.unwrap_or(0), child.flex_fit.unwrap_or(FlexFit::Loose).short_label())`.

Helper: `fn short_label(f: FlexFit) -> &'static str` returns `"tight"` / `"loose"`.

For a `Row` (`direction == Horizontal`), boxes are placed side-by-side rather than stacked. The label rows render inside narrower boxes (so the labels get truncated or wrap). This case is less common; implement it but accept that wider terminals are needed for legibility.

#### Axis label helpers

```rust
fn cross_axis_label(direction: Axis, alignment: CrossAxisAlignment) -> String {
    let axis_name = match direction {
        Axis::Vertical => "Cross Axis",  // perpendicular to vertical → horizontal label
        Axis::Horizontal => "Cross Axis",
    };
    format!("{}: {}", axis_name, cross_axis_value(alignment))
}

fn cross_axis_value(a: CrossAxisAlignment) -> &'static str {
    match a {
        CrossAxisAlignment::Start => "start",
        CrossAxisAlignment::End => "end",
        CrossAxisAlignment::Center => "center",
        CrossAxisAlignment::Stretch => "stretch",
        CrossAxisAlignment::Baseline => "baseline",
    }
}

fn main_axis_value(a: MainAxisAlignment) -> &'static str {
    match a {
        MainAxisAlignment::Start => "start",
        MainAxisAlignment::End => "end",
        MainAxisAlignment::Center => "center",
        MainAxisAlignment::SpaceBetween => "spaceBetween",
        MainAxisAlignment::SpaceAround => "spaceAround",
        MainAxisAlignment::SpaceEvenly => "spaceEvenly",
    }
}
```

#### Total flex

```rust
fn total_flex(children: &[FlexChild]) -> u32 {
    children.iter().map(|c| c.flex_factor.unwrap_or(0)).sum()
}
```

### Acceptance Criteria

1. Selecting a `Column` widget, pressing Enter, switching to Flex Explorer shows the ASCII visualization with the column's children stacked vertically.
2. Each child renders `w=W`, `h=H`, child name, `flex=N`, `fit=tight/loose`.
3. Cross-axis label appears in the top border (e.g., `"┌─ Cross Axis: stretch ──...─┐"`).
4. Main-axis label appears in the rightside (vertical column) or bottom (horizontal row) indicator strip.
5. `"Total Flex: N"` appears in the header row.
6. Constraints + measured size of the flex container appear in the footer row.
7. Selecting a `Row`: visualization rotates to horizontal stacking.
8. Selecting a non-flex widget (e.g. `Container`): tab shows `"This widget is not a Row, Column, or Flex container."`.
9. Loading / error states render with consistent muted styling.
10. The "Coming soon — Phase 2" stub and its local `render_centered_text` helper are removed.
11. Below `MIN_FLEX_VIZ_HEIGHT` or `MIN_FLEX_VIZ_WIDTH`, the tab renders a muted "Terminal too small for flex visualization" message instead of a broken layout.

### Testing

Use `TestBackend` snapshot-style tests, following the inspector test pattern:

```rust
#[test]
fn flex_explorer_renders_column_with_two_children() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::FlexExplorer;
    state.layout = Some(LayoutInfo {
        description: Some("Column".into()),
        direction: Some(Axis::Vertical),
        main_axis_alignment: Some(MainAxisAlignment::Start),
        cross_axis_alignment: Some(CrossAxisAlignment::Stretch),
        main_axis_size: Some(MainAxisSize::Max),
        children: vec![
            FlexChild {
                name: "Container".into(),
                size: Some(WidgetSize { width: 180.0, height: 341.0 }),
                flex_factor: None,
                ..Default::default()
            },
            FlexChild {
                name: "Expanded".into(),
                size: Some(WidgetSize { width: 180.0, height: 189.0 }),
                flex_factor: Some(1),
                flex_fit: Some(FlexFit::Tight),
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let buf = render_flex_explorer_tab(&state, (80, 24));
    let s = buffer_to_string(&buf);
    assert!(s.contains("Column"));
    assert!(s.contains("Cross Axis: stretch"));
    assert!(s.contains("Container"));
    assert!(s.contains("Expanded"));
    assert!(s.contains("flex=1"));
    assert!(s.contains("fit=tight"));
    assert!(s.contains("Total Flex: 1"));
}

#[test]
fn flex_explorer_non_flex_widget_shows_explanation() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::FlexExplorer;
    state.layout = Some(LayoutInfo {
        description: Some("Container".into()),
        // direction == None; children == empty.
        ..Default::default()
    });
    let buf = render_flex_explorer_tab(&state, (60, 12));
    assert!(buffer_to_string(&buf).contains("not a Row, Column, or Flex"));
}

#[test]
fn flex_explorer_loading_state() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::FlexExplorer;
    state.layout_loading = true;
    let buf = render_flex_explorer_tab(&state, (60, 12));
    assert!(buffer_to_string(&buf).contains("Loading"));
}

#[test]
fn flex_explorer_too_small_fallback() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::FlexExplorer;
    state.layout = Some(LayoutInfo {
        description: Some("Column".into()),
        direction: Some(Axis::Vertical),
        children: vec![FlexChild { name: "A".into(), ..Default::default() }],
        ..Default::default()
    });
    // Below MIN_FLEX_VIZ_HEIGHT.
    let buf = render_flex_explorer_tab(&state, (60, 5));
    assert!(buffer_to_string(&buf).contains("too small"));
}
```

### Notes

- This is the most rendering-heavy task in Phase 2. Plan for ~3–5h end-to-end including the test suite. If the implementor finds the file growing past 500 lines, factor sub-functions into a `flex_explorer/draw.rs` sub-module — but the CODE_STANDARDS 500-line ceiling is a soft target; 600 with tests is acceptable per the project's existing `layout_panel.rs` (537 lines).
- **Do not attempt proportional sizing**. This was explicitly weighed in parent PLAN §7.1 — it aliases badly in terminals. Fixed-height boxes labeled with real values is the agreed compromise.
- The `direction == Horizontal` case (`Row`) is implementable but secondary in priority. If time-constrained, the implementor may render `Row` as a degenerate "stacked vertically with a note that it's actually horizontal" — but this is a fallback, not the goal. Aim for proper horizontal stacking with narrower boxes.
- The `parent_offset` field on `FlexChild` is parsed by task 04 but NOT visualized in this task — the offset is implied by the box order. Phase 3 polish could add an explicit "offset: (x, y)" label per child.
- Footer constraint + size formatting can reuse `format_constraint_value` from `layout_panel.rs` (visibility allows it).
- The `render_centered_text` helper at `flex_explorer_tab.rs:17–30` is deleted — replaced by inline state-specific renderers.

---

## Completion Summary

**Status:** Pending

(To be filled in by the implementor.)
