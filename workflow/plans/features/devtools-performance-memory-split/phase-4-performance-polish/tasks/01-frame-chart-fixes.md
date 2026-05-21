# Task 01 — Frame Chart Fixes (bar height + selection highlight + scroll behavior)

**Status:** Not Started
**Wave:** 1
**Agent:** implementor
**Estimated Effort:** 3–4 hours
**Depends On:** —

## Problem

Three concrete user complaints about the Performance tab's Frame Chart widget:

1. **Bars not proportional / disappear at small heights.** `ms_to_half_blocks` rounds short frames to 0 with no minimum floor. Result: in shallow terminal windows, fast frames (~1 ms) become invisible.
2. **Selection highlight invisible.** A single-character `▔` painted at the top of the chart area is detached from the visible bar and too small to notice.
3. **Selection always pinned to right edge.** Pressing Left/Right moves the global `selected_frame` index, but `handle_select_performance_frame` unconditionally resets `frame_chart_scroll_offset = 0`, and `compute_visible_range` then re-anchors the window such that the selected frame is at the right edge. Visually the chart appears to scroll with selection trailing.

## Files (Write)

- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs`
- `crates/fdemon-app/src/handler/devtools/performance/frame.rs`

## Files (Read)

- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` — orchestration; verify entry points
- `crates/fdemon-app/src/session/performance.rs` — verify field names (`frame_chart_scroll_offset`, `frame_chart_visible_width`, `selected_frame`)

## Approach Hints

### Fix 1 — Bar height minimum

In `bars.rs::ms_to_half_blocks`:

```rust
pub(crate) fn ms_to_half_blocks(ms: f64, y_range_ms: f64, total_half_blocks: f64) -> u16 {
    if ms <= 0.0 || y_range_ms <= 0.0 {
        return 0;
    }
    let raw = ((ms / y_range_ms) * total_half_blocks).round() as u16;
    // Never let a nonzero frame become invisible — clamp to at least 1 half-block.
    raw.max(1)
}
```

Add a unit test: `ms_to_half_blocks_clamps_nonzero_to_at_least_one` covering very-small-ms / very-short-area cases.

### Fix 2 — Full-column selection highlight

Replace the single `▔` overlay at `area.y` with a column highlight spanning the full chart height. Two options to evaluate:

**Option A (recommended):** Adjacent-column side markers using `▏` (U+258F left-eighth) on the left of the selected pair of columns and `▕` (U+2595 right-eighth) on the right, painted on every row of the chart area. This frames the selected bar without obscuring its content.

**Option B (fallback):** Paint a different background color across the selected columns (`buf.cell_mut(x, y).set_bg(Color::DarkGray)` for every row).

Pick Option A by default; if it visually clashes with the bar colors, fall back to Option B. Add a tests for both row-coverage and that adjacent (non-selected) columns are not affected.

### Fix 3 — Selection-within-viewport scrolling

Two coordinated changes:

**Handler side** (`frame.rs::handle_select_performance_frame`):

```rust
// Remove the unconditional reset:
//   handle.session.performance.frame_chart_scroll_offset = 0;
//
// Replace with viewport-aware logic:
let visible_width = handle.session.performance.frame_chart_visible_width.get();
let scroll = &mut handle.session.performance.frame_chart_scroll_offset;
if let Some(sel_idx) = index {
    let total = handle.session.performance.frames.len();
    let visible_start = total.saturating_sub(*scroll + visible_width);
    let visible_end = total.saturating_sub(*scroll);
    if sel_idx < visible_start {
        // Selection moved off the left edge — scroll left to keep it visible
        *scroll = total.saturating_sub(sel_idx + visible_width);
    } else if sel_idx >= visible_end {
        // Selection moved off the right edge — scroll right
        *scroll = total.saturating_sub(sel_idx + 1);
    }
    // Otherwise the selection is within the viewport — leave scroll_offset alone
}
```

**Render side** (`bars.rs::compute_visible_range`): Stop using `selected_frame.is_some()` to anchor the window. Always honor `scroll_offset` as the viewport authority:

```rust
fn compute_visible_range(
    frame_count: usize,
    visible_width: usize,
    scroll_offset: usize,
) -> (usize, usize) {
    let end = frame_count.saturating_sub(scroll_offset);
    let start = end.saturating_sub(visible_width);
    (start, end)
}
```

Drop the `selected_frame: Option<usize>` parameter from `compute_visible_range` (or keep the signature for compatibility but make it ignore that argument with a justifying comment — prefer removal if no other callers depend on it).

### Constants

Per CODE_STANDARDS Principle 4, name any new magic numbers:

```rust
/// Minimum half-block height for a nonzero frame, prevents fast frames from
/// vanishing at small terminal heights.
const MIN_BAR_HALF_BLOCKS: u16 = 1;
```

## Acceptance Criteria

1. **Bug 1 — Bar height minimum**
   - `ms_to_half_blocks(0.5, 20.0, 4.0)` returns `1` (previously `0`).
   - `ms_to_half_blocks(0.0, 20.0, 4.0)` returns `0` (zero-duration stays zero).
   - New test `ms_to_half_blocks_clamps_nonzero_to_at_least_one` passes.
2. **Bug 2 — Full-column selection highlight**
   - Selected bar columns are visually distinct across every row of the chart area (not just the top row).
   - Adjacent unselected bars are not affected (no bleed-over).
   - New test `selection_highlight_paints_full_column` asserts a side-marker character (or distinct bg color) is present at every chart row for the selected column(s).
   - New test `selection_highlight_does_not_paint_adjacent_columns` asserts the column to the right of the selection is unmodified.
3. **Bug 3 — Selection-within-viewport scrolling**
   - With `frames.len() = 200`, `visible_width = 30`, `scroll_offset = 70` (visible range 100–130), `selected_frame = 130`: pressing Left (selection → 129) leaves `scroll_offset` unchanged at 70.
   - Pressing Left until `selected_frame = 100`: `scroll_offset` still 70 (no scroll yet — selection at leftmost visible).
   - Pressing Left once more (selection → 99): `scroll_offset` becomes 71 (or equivalent — viewport shifts left by 1 to keep selection visible).
   - Pressing Right past the right edge mirror-scrolls the viewport right.
   - New tests: `test_select_within_viewport_does_not_scroll`, `test_select_at_left_edge_scrolls_viewport_left`, `test_select_at_right_edge_scrolls_viewport_right`.
4. **Quality gate** — `cargo fmt --all -- --check`, `cargo check -p fdemon-tui -p fdemon-app --all-targets`, `cargo test -p fdemon-tui -p fdemon-app`, `cargo clippy -p fdemon-tui -p fdemon-app --all-targets -- -D warnings` all pass.
5. **No regression** — existing `frame_chart` and `frame.rs` tests still pass; the Phase-3 selection-from-mouse-click test still works; the existing visible-range tests get updated to assert the new viewport authority but still pass.

## Notes

- This task is bundled (three sub-bugs) because all three live in the same two files. Splitting would force three sequential merges; bundling keeps the diff coherent.
- The render-time `frame_chart_visible_width: Cell<usize>` (CODE_STANDARDS Principle 3) is read in the handler — annotate the read site with the standard `// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md` comment if not already present.
- Selection highlight color (Option A vs B) — pick whichever is more visible in practice; document the choice in the Completion Summary.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` | Added `MIN_BAR_HALF_BLOCKS` constant; updated `ms_to_half_blocks` to clamp nonzero values to at least 1; replaced single-row `▔` selection highlight with full-column Option-A side-markers (`▏`/`▕`); simplified `compute_visible_range` to remove the `selected_frame` parameter (scroll_offset is now the sole viewport authority); updated the call site to match the new signature |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/tests.rs` | Updated all `compute_visible_range` call sites to the 3-argument form; renamed/updated the old selection-anchor tests; added `ms_to_half_blocks_clamps_nonzero_to_at_least_one`, `ms_to_half_blocks_zero_ms_stays_zero`, `selection_highlight_paints_full_column`, and `selection_highlight_does_not_paint_adjacent_columns`; updated `test_selected_frame_shows_highlight` to expect the new marker characters |
| `crates/fdemon-app/src/handler/devtools/performance/frame.rs` | Replaced unconditional `frame_chart_scroll_offset = 0` in `handle_select_performance_frame` with viewport-aware logic that only adjusts the offset when the selected frame falls outside the current visible window; updated the `left_right_arrow_clears_scroll_offset` test comment to explain the viewport-aware outcome; added three new tests: `test_select_within_viewport_does_not_scroll`, `test_select_at_left_edge_scrolls_viewport_left`, `test_select_at_right_edge_scrolls_viewport_right` |

### Notable Decisions/Tradeoffs

1. **Selection highlight: Option A (side markers)**: Chose Option A (`▏` U+258F left-eighth on the column before the UI bar, `▕` U+2595 right-eighth on the column after the Raster bar) painted across every row of the chart area. This frames the selected bar pair without obscuring bar content. Option B (background color) was not needed — Option A is clearly visible and integrates well with the existing block-character bar aesthetics.

2. **`compute_visible_range` signature simplification**: Removed the `selected_frame: Option<usize>` parameter entirely. The old selection-anchor mode (mode 2: "keep selected frame at right edge") conflicted with the viewport-aware scrolling goal of Fix 3. The handler now manages the scroll_offset to keep the selection visible, making render-time anchoring redundant. This is a clean separation of concerns: the handler owns scrolling policy, the renderer owns display.

3. **Existing `left_right_arrow_clears_scroll_offset` test**: The test assertion (`offset == 0`) continues to hold under the new logic because pressing Left from `selected=None`, `scroll_offset=50`, `visible_width=50`, `frames=200` selects index 199, which falls to the right of the viewport `[100, 150)`, triggering a right-scroll to `200 - (199+1) = 0`. The test body comment was updated to explain this.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check -p fdemon-tui -p fdemon-app --all-targets` - Passed
- `cargo test -p fdemon-tui -p fdemon-app` - Passed (1207 + 2454 tests)
- `cargo clippy -p fdemon-tui -p fdemon-app --all-targets -- -D warnings` - Passed
- `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` - Passed (full quality gate)

### Risks/Limitations

1. **Left-marker only appears when selection is not in the first slot**: When the selected frame is slot 0 (x=0), `left_marker_x = 0.saturating_sub(1) = 0 = x`, so `has_left_marker` is false — no left marker is painted. This is intentional: there is no column to the left of the leftmost slot within the chart area. The right marker (`▕` at x+2) is always painted if within bounds.
