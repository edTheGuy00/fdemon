## Task: Create Layout Panel Widget

**Objective**: Create `inspector/layout_panel.rs` — a new widget that replaces the details panel with an enhanced Layout Explorer showing widget name, source location, box model visualization, dimensions, padding, constraints, and flex properties.

**Depends on**: Task 01 (add-edge-insets-core-types), Task 02 (merge-layout-state-into-inspector)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs`: **NEW**

### Details

#### File structure

Create `layout_panel.rs` as a new file in the `inspector/` directory module. Add `mod layout_panel;` declaration to `inspector/mod.rs` (but do NOT wire it into rendering yet — that's Task 06).

The file adds methods to `WidgetInspector<'_>` via a separate `impl` block, following the same pattern as `tree_panel.rs` and `details_panel.rs`.

#### Main entry point

```rust
impl WidgetInspector<'_> {
    /// Render the layout panel showing box model, dimensions, and constraints
    /// for the currently selected widget tree node.
    pub(super) fn render_layout_panel(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
    ) {
        // ...
    }
}
```

#### Rendering sections (top to bottom)

The panel renders inside a `Block` with title `" Layout Explorer "` and `BORDER_DIM` border style (matching the old `layout_explorer.rs` outer block).

**Section 1: Widget name and source location (2-3 lines)**

At the top, show the selected widget's name and creation location:

```
  Column                        ← ACCENT + BOLD, from node.display_name()
  lib/screens/home.dart:42      ← STATUS_BLUE, from short_path(location.file):location.line
```

If no `creation_location`, show only the name.

**Section 2: Box model visualization (variable height)**

When `layout.padding` is `Some(padding)` AND `layout.size` is `Some(size)`:

```
  ┌─ padding ────────────────────────┐
  │  top: 8.0                        │
  │  ┌─ widget ──────────────────┐   │
  │  │                           │   │
  │  │   200.0 x 48.0            │   │
  │  │                           │   │
  │  └───────────────────────────┘   │
  │  bottom: 8.0                     │
  └──────────────────────────────────┘
```

- Outer block: `Block::bordered()` with title `" padding "`, `STATUS_YELLOW` border
- Padding values: `top: {v}` above the inner block, `bottom: {v}` below, `left: {v}` on left, `right: {v}` on right
- Inner block: `Block::bordered()` with title `" widget "`, `TEXT_MUTED` border
- Size centered inside inner block: `"{w} x {h}"` in `STATUS_GREEN` + `BOLD`
- Only render when available height >= 7 lines (outer border + padding lines + inner border + content)

When padding is `None` but size is available — show simplified size box (from old `render_size_box`):

```
  ┌─ Size ───────────────────────┐
  │       200.0 x 48.0           │
  │   ┌───────────────────┐     │
  │   │                   │     │
  │   └───────────────────┘     │
  └──────────────────────────────┘
```

**Section 3: Dimensions row (1 line)**

Always shown when `layout.size` is `Some`:

```
  W: 200.0  H: 48.0
```

Style: `STATUS_GREEN` for values, `TEXT_MUTED` for labels.

**Section 4: Constraints (2-3 lines)**

When `layout.constraints` is `Some`:

```
  Constraints
    min: 0.0 x 0.0  max: 414.0 x 896.0
```

If constraints are tight (`min == max` for both dimensions), append `(tight)` indicator.

Use `format_constraint_value()` to show "Inf" for infinity values (reimplement from old `layout_explorer.rs` or import as a shared helper).

**Section 5: Flex properties (1 line)**

When any of `flex_factor`, `flex_fit`, `description` are present:

```
  flex: 1  fit: tight
```

Style: `STATUS_INDIGO`. Parts joined with `"  "`.

#### State handling

| State | What to render |
|-------|---------------|
| `inspector_state.layout_loading == true` | Loading spinner: `"Loading layout..."` centered |
| `inspector_state.layout_error.is_some()` | Error message with hint, similar to inspector error box |
| `inspector_state.layout.is_some()` | Full layout visualization (sections above) |
| No layout data, no error, no loading | `"Select a widget to see layout details"` centered |

#### Minimum height handling

If available height is very small (< 5 lines), show only the dimensions row:
```
  Column  200.0 x 48.0  min: 0x0 max: 414x896
```

#### Key helpers to implement

```rust
/// Format a constraint value: "Inf" for infinity, "{:.1}" otherwise.
fn format_constraint_value(value: f64) -> String

/// Render the box model visualization with nested padding/widget blocks.
fn render_box_model(area: Rect, buf: &mut Buffer, size: &WidgetSize, padding: &EdgeInsets)

/// Render the simplified size box without padding.
fn render_size_box(area: Rect, buf: &mut Buffer, size: &WidgetSize)
```

### Acceptance Criteria

1. `inspector/layout_panel.rs` exists with `render_layout_panel` method on `WidgetInspector<'_>`
2. Shows widget name in `ACCENT` + `BOLD` at top
3. Shows source location in `STATUS_BLUE` when available
4. Box model visualization renders correctly with padding values
5. Dimensions row shows `W: {w}  H: {h}`
6. Constraints display shows min/max with "Inf" for infinity and "(tight)" indicator
7. Flex properties shown when available
8. Loading, error, and empty states handled gracefully
9. Compact mode for small terminal heights (< 5 lines)
10. File is under 400 lines
11. `cargo check -p fdemon-tui` passes
12. Unit tests for all rendering states (15+ tests)

### Testing

Add tests inline in `layout_panel.rs` or in a `layout_panel_tests.rs` sibling file. Use `TestTerminal` / buffer-based rendering tests matching the pattern in `inspector/tests.rs`:

```rust
#[test]
fn test_layout_panel_shows_widget_name() {
    // Create InspectorState with selected node + layout data
    // Render WidgetInspector
    // Assert widget name appears in buffer
}

#[test]
fn test_layout_panel_shows_box_model_with_padding() {
    // Layout with padding: EdgeInsets { top: 8, right: 16, bottom: 8, left: 16 }
    // Assert padding border block and values rendered
}

#[test]
fn test_layout_panel_shows_size_box_without_padding() {
    // Layout with size but no padding
    // Assert size box without padding wrapper
}

#[test]
fn test_layout_panel_shows_constraints() {
    // Layout with BoxConstraints
    // Assert "min: 0.0 x 0.0  max: 414.0 x Inf" rendered
}

#[test]
fn test_layout_panel_shows_tight_indicator() {
    // Tight constraints (min == max)
    // Assert "(tight)" appears
}

#[test]
fn test_layout_panel_shows_flex_properties() {
    // Layout with flex_factor and flex_fit
    // Assert "flex: 1  fit: tight" rendered
}

#[test]
fn test_layout_panel_loading_state() {
    // layout_loading = true
    // Assert "Loading layout..." shown
}

#[test]
fn test_layout_panel_error_state() {
    // layout_error = Some(DevToolsError { ... })
    // Assert error message and hint shown
}

#[test]
fn test_layout_panel_empty_state() {
    // No layout data, not loading, no error
    // Assert "Select a widget to see layout details" shown
}

#[test]
fn test_layout_panel_compact_mode() {
    // Render in area with height < 5
    // Assert single-line compact summary
}

#[test]
fn test_layout_panel_source_location() {
    // Node with creation_location
    // Assert "lib/foo.dart:42" rendered in STATUS_BLUE
}

#[test]
fn test_format_constraint_value_infinity() {
    assert_eq!(format_constraint_value(f64::INFINITY), "Inf");
    assert_eq!(format_constraint_value(1e10), "Inf");
    assert_eq!(format_constraint_value(414.0), "414.0");
}
```

### Notes

- This file is created but **not wired into the inspector rendering** yet — the `render_tree` method in `inspector/mod.rs` still calls `render_details`. Wiring happens in Task 06.
- The box model visualization uses ratatui's `Block` widget with borders for the nested rectangles. No custom drawing — leverage the existing `Block::bordered()` with `title()`.
- `format_constraint_value` was `pub` in the old `layout_explorer.rs`. In the new file, it can be `pub(super)` or just `fn` (private) if only used internally.
- The proportional size box from the old `layout_explorer.rs` (aspect-ratio-preserving inner rectangle) is a nice touch — preserve that logic in `render_size_box` if there's enough vertical space.
- Follow the existing pattern: methods on `WidgetInspector<'_>` via `impl` block, imports from `super::` for shared helpers.

---

## Completion Summary

**Status:** Not started
