## Task: Rewrite tree_panel.rs to render guideline / branch tick / type-icon from `InspectorRow`

**Objective**: Replace the current `"  ".repeat(depth)` indent rendering with a DevTools-style tree: vertical `│` guidelines through ancestor columns that still have more siblings below, `├─` / `└─` branch ticks at each entry, and a per-widget-type icon glyph. Mouse-click regions must continue to honor existing invariants (row click selects; glyph click toggles; last-pushed-wins-at-same-z).

**Depends on**: 01-core-diagnostics-and-row-builder, 02-state-inspector-extensions

**Estimated Time**: 5–7 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`: Rewrite `render_tree_panel_inner` and helpers; introduce a glyph mapping table and a row-painter routine.
- `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs`: Add snapshot/buffer-based tests for guideline rendering, branch-tick rendering, group-leader rendering, mouse-region correctness; update any existing tests that depend on the old indent math.
- `crates/fdemon-tui/src/theme/palette.rs`: Add new palette constants for tree guideline / branch tick / chain member / group leader (small additive edit).

**Files Read (Dependencies):**
- `crates/fdemon-core/src/widget_tree.rs` (`InspectorRow`, `RowGroup`, `widget_runtime_type`).
- `crates/fdemon-app/src/state.rs` (`inspector_rows()`).
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` (current `visible_nodes()` call site at line 155 — task 09 changes this, but for this task it still reads `visible_nodes()` until 09 lands; the shim ensures correctness).

### Details

#### 1. Switch input from `(node, depth)` slice to `InspectorRow` slice — without touching `mod.rs`

The current signature is:

```rust
pub(super) fn render_tree_panel_inner(
    &self,
    area: Rect,
    buf: &mut Buffer,
    visible: &[(&DiagnosticsNode, usize)],
    selected: usize,
    mut ctx: Option<&mut MouseCtx<'_>>,
)
```

To keep `inspector/mod.rs` out of this task's write list (task 09 owns the mode-switch restructure of mod.rs), **do not change the signature**. Instead:

- Inside `render_tree_panel_inner`, ignore the `visible` parameter for guideline/branch/icon decisions and call `self.inspector_state.inspector_rows()` locally to get the rich row slice.
- Use `visible` only as the `selected_index → row_count` clamp (or stop using it entirely if you derive the count from `inspector_rows()`).

The unused-parameter warning will be silenced because the caller still passes `&visible`. If clippy complains about the unused `visible`, prefix with `_visible: …` or document with `#[allow(unused_variables)]` and a pointer to task 09 which will properly remove the parameter.

This means:
- Task 07's write list: **only** `tree_panel.rs` + `tests.rs` + `palette.rs`.
- Task 09's write list: **only** `inspector/mod.rs` (drops the now-unused `visible_nodes()` arg, adds the details mode branch).

The shim from task 02 makes mod.rs:155 (`let visible = self.inspector_state.visible_nodes();`) continue to compile; the layout panel keeps consuming the flat tuples.

#### 2. Indent column math

Define named constants:

```rust
/// Horizontal cells per depth level in the inspector tree.
/// Chosen as 2 cells: 1 for the guideline/branch-tick column, 1 for spacing
/// before the icon glyph at the next depth.
const TREE_INDENT_COLS: u16 = 2;

/// Cell index (relative to row start) where the type-icon glyph is drawn
/// for a row at depth `d`: `d * TREE_INDENT_COLS`.
fn glyph_col(depth: usize) -> u16 { (depth as u16).saturating_mul(TREE_INDENT_COLS) }
```

#### 3. Per-row rendering pipeline

For each `InspectorRow`:

1. **Background**: if selected, fill the row with `SELECTED_ROW_BG` (existing pattern).
2. **Guidelines**: for each `d in 0..row.depth`:
   - x position: `glyph_col(d)`.
   - Character: `│` IFF `d in row.ticks`, else space.
   - Style: `palette::TREE_GUIDELINE` (new color; see #5 below).
3. **Branch tick** at column `glyph_col(row.depth.saturating_sub(1))` (only when `row.depth > 0`):
   - Character: `├─` for non-last child (`line_to_parent == true`), `└─` for last child (`line_to_parent == false`).
   - Note: `├─` and `└─` are 2 columns wide (`├` + `─` glyphs). They occupy cells `[glyph_col(depth-1), glyph_col(depth)-1]` — exactly the 2-column TREE_INDENT_COLS width.
4. **Icon glyph** at column `glyph_col(row.depth)`:
   - 1 cell wide.
   - Lookup via `glyph_for_widget(node)` (see #4 below).
5. **Name + optional source-location hint**: at column `glyph_col(row.depth) + 2` onward, truncated to `tree_inner.width - glyph_col(row.depth) - 2`. Source-location hint logic for user-code rows is preserved from the existing renderer.

For `RowGroup::LeaderCollapsed { hidden_count }`:
- Draw the icon as a small "+" or "▶" indicating expandability.
- Draw the name as `+ {hidden_count} more widgets` in `palette::TEXT_MUTED`.
- Click region on the row → toggle `expanded_groups` for the leader id (new `Message::DevToolsInspectorToggleGroup { value_id }`? — see Notes for handling without a new Message variant).

For `RowGroup::LeaderExpanded`:
- Draw normally, with an icon that indicates "collapsible chain leader" (e.g. `▼` next to its type icon).

For `RowGroup::Member`:
- Draw at `row.depth` but with a subtle dim color to indicate it's part of an implementation chain.

#### 4. Type-icon glyph table

Add `fn glyph_for_widget(node: &DiagnosticsNode) -> char` mapping `widget_runtime_type()` → a single Unicode glyph. Suggested table (port the spirit of DevTools' `WidgetTheme.themeMap` but condensed to 1-cell glyphs):

| Widget type | Glyph | Rationale |
|---|---|---|
| Row, Column, Flex | `▦` | grid-like |
| Container, Padding, SizedBox | `▣` | filled box |
| Stack | `▤` | layered |
| Scaffold | `◯` | shell |
| MaterialApp, CupertinoApp | `▥` | app |
| Text, RichText | `T` | letter |
| Image, Icon | `▨` | media |
| Center, Align, Positioned | `+` | alignment |
| ListView, GridView, SingleChildScrollView | `≡` | list |
| Builder, StreamBuilder, ValueListenableBuilder | `B` | letter |
| BlocProvider, MultiBlocProvider | `B` | letter (or `▪` for dim/provider) |
| (default fallback) | first capital letter of the type | letter-in-circle equivalent |

Each glyph is 1 cell. Avoid combining characters and avoid East-Asian-wide glyphs that take 2 cells.

The mapping table should be a `const` `[(&str, char)]` for fast linear scan, OR a `match` block. Either is fine; the table is small.

#### 5. New palette entries

Add to `crates/fdemon-tui/src/theme/palette.rs` (read-only for the renderer, write site here):

```rust
pub const TREE_GUIDELINE: Color = Color::Rgb(60, 60, 70);
pub const TREE_BRANCH_TICK: Color = Color::Rgb(80, 80, 95);
pub const TREE_CHAIN_MEMBER_TEXT: Color = Color::Rgb(120, 120, 140);
pub const TREE_GROUP_LEADER_TEXT: Color = Color::Rgb(140, 140, 170);
```

(Coordinate naming with existing palette entries; consult `crates/fdemon-tui/src/theme/palette.rs` for the project's color convention.)

#### 6. Mouse-region math update

The existing tree_panel.rs:142–155 computes glyph X as `tree_inner.x + (depth * 2)`. After this task:

```rust
let glyph_x = tree_inner.x.checked_add(glyph_col(row.depth))
    .filter(|x| *x < tree_inner.right())?;
let glyph_rect = MouseRect::new(glyph_x, y, 1, 1);
```

The 1-cell width and the "row first, glyph second" push order MUST remain identical — these are the invariants documented in tests.rs `test_last_pushed_wins_at_same_z` and the docs at lines 30–37 of tree_panel.rs.

For group-leader rows, the glyph click should toggle the chain (expand/collapse the group). To avoid adding a new Message variant in this task, route the click through the existing `DevToolsInspectorToggleNode { index }` — the handler already knows the row's `value_id` via `inspector_rows()[index].node.value_id`, and task 02's handler can check whether the indexed row is a `LeaderCollapsed` / `LeaderExpanded` and toggle `expanded_groups` instead of `expanded`.

If that's awkward, add a new `Message::DevToolsInspectorToggleGroup { index }` and update task 04 + task 05 to handle it. **Recommend:** reuse `ToggleNode` and let the handler dispatch. Documented in task 05's notes as well.

#### 7. Tests

Add to tests.rs:

- `tree_renders_guidelines_for_nonlast_sibling_ancestors`.
- `tree_renders_branch_tick_last_child_uses_box_drawing_l`.
- `tree_renders_branch_tick_non_last_child_uses_box_drawing_t`.
- `tree_renders_collapsed_leader_with_plus_n_more_widgets`.
- `tree_renders_expanded_leader_then_member_rows`.
- `tree_renders_type_icon_for_known_widget_types` (Row/Column/Container/Stack/Text/fallback).
- `tree_mouse_glyph_rect_uses_new_indent_math` (depth 0 / depth 3 / depth pathological).
- `tree_mouse_row_rect_unchanged_full_width_of_tree_inner`.
- `tree_pushes_row_rect_then_glyph_rect_for_last_pushed_wins_invariant`.

These can be buffer-string snapshot tests (assert specific cell contents) rather than full image snapshots; that keeps them fast and readable.

### Acceptance Criteria

1. The user's screenshot 1 example (deep BlocProvider chain) now renders as a folded leader row under `MultiBlocProvider`. Visual structure matches DevTools screenshot 2 (vertical lines + branch ticks + per-type icon).
2. Toggling `hide_implementation_widgets` via Shift+H expands the chain into individual rows (with chain-member dim styling).
3. Existing tests in tests.rs continue to pass; the migration introduces no regressions in click handling.
4. `cargo test -p fdemon-tui` passes with new tests added.
5. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn tree_renders_branch_tick_last_child_uses_box_drawing_l() {
    let mut state = make_state_with_parent_two_children();
    let widget = WidgetInspector::new(&state);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    widget.render_tree_panel_inner(buf.area, &mut buf, &state.inspector_state.inspector_rows(), 0, None);
    // Second child (the last) should have └─ at depth 1's branch column
    let s = buf_to_string_row(&buf, 2);
    assert!(s.contains("└─"));
}
```

### Notes

- 1-cell width glyphs are required because the existing mouse `MouseRect::new(.., 1, 1)` uses 1 cell. East-Asian-wide glyphs would mis-align mouse hits.
- Avoid Nerd Font glyphs in this task — fdemon has a Nerd Font detection layer (`crates/fdemon-tui/src/...` — find with grep) and falls back gracefully. For Phase 1, use standard Unicode box-drawing + Latin letters so the rendering works in any terminal. Nerd-font enhancement can come later.
- The legacy `expand_icon()` helper currently returns `▶` / `▼` / `●` for collapsed/expanded/leaf. Keep the same three symbols for regular nodes; reuse the function. Only group-leader rendering needs new glyphs.
- The minor inline edit to `inspector/mod.rs:155` (switch from `visible_nodes()` to `inspector_rows()`) is owned by this task; task 09's larger restructure will then build on that change.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
