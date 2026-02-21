## Task: Extract frame_chart.rs into submodules

**Objective**: Split `frame_chart.rs` (544 lines) into a directory-based module structure with each submodule under 500 lines.

**Depends on**: None (independent of other tasks; can run in Wave 2 for logical grouping)

**Source**: Review Major Issue #3 (Code Quality Inspector)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs` → `frame_chart/mod.rs`
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` — **NEW**
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` — **NEW**

### Details

#### Current State

`frame_chart.rs` (544 lines) has tests already extracted to `frame_chart/tests.rs` (400 lines). The main source file remains as a flat `.rs` file with a `mod tests;` declaration.

#### Extraction Plan

**`frame_chart/mod.rs`** (~120 lines) — keep:
- Module doc, imports, constants, color definitions (lines 1-51)
- `FrameChart` struct + `Widget` impl + `render()` dispatch (lines 52-120)
- `mod` declarations for `bars`, `detail`, `tests`

**`frame_chart/bars.rs`** (~290 lines) — extract:
- `render_bar_chart()` (lines 122-234, ~113 lines) — main bar rendering loop
- `compute_visible_range()` (lines 236-260, ~25 lines) — scroll window calculation
- `render_budget_line()` (lines 262-292, ~31 lines) — 16ms budget line
- `render_bar()` (lines 461-503, ~43 lines) — single bar column renderer
- `ms_to_half_blocks()` (lines 441-459, ~19 lines) — milliseconds to half-block height conversion
- `bar_colors()` (lines 421-439, ~19 lines) — color selection for frame status

**`frame_chart/detail.rs`** (~244 lines) — extract:
- `render_detail_panel()` (lines 294-340, ~47 lines) — detail panel layout
- `render_frame_detail()` (lines 342-400, ~59 lines) — selected frame breakdown
- `render_summary_line()` (lines 402-419, ~18 lines) — frame summary when none selected
- `frame_status_label_and_style()` (lines 505-524, ~20 lines) — status label rendering
- `render_ui_phase_line()` (lines 526-538, ~13 lines) — UI phase timing line

#### Module Visibility

Same approach as the memory_chart extraction — use `impl FrameChart<'_>` blocks in submodule files with `pub(super)` visibility:

```rust
// frame_chart/bars.rs
use super::*;

impl FrameChart<'_> {
    pub(super) fn render_bar_chart(&self, area: Rect, buf: &mut Buffer) { ... }
    pub(super) fn compute_visible_range(&self, available_width: u16) -> Range<usize> { ... }
    pub(super) fn render_budget_line(&self, area: Rect, buf: &mut Buffer) { ... }
    // Pure helpers can be free functions with pub(super) if they don't use self
}
```

For pure helper functions (`bar_colors`, `ms_to_half_blocks`, `render_bar`) that don't use `self`, keep them as `pub(super) fn` free functions in `bars.rs`.

#### File Conversion Steps

1. Rename `frame_chart.rs` to `frame_chart/mod.rs`
2. Move bar chart rendering functions into `frame_chart/bars.rs`
3. Move detail panel functions into `frame_chart/detail.rs`
4. Add `mod bars;` and `mod detail;` to `mod.rs`
5. Ensure all types needed by submodules are accessible
6. Verify `cargo check -p fdemon-tui` and `cargo test -p fdemon-tui`

### Acceptance Criteria

1. `frame_chart/mod.rs` is under 500 lines (target ~120 lines)
2. Each new submodule (`bars.rs`, `detail.rs`) is under 500 lines
3. Existing `tests.rs` continues to work from the `frame_chart/` directory
4. All existing rendering tests pass without modification
5. No public API changes
6. `cargo check --workspace && cargo test --workspace` passes

### Testing

No new tests needed — this is a pure refactor. All existing tests in `frame_chart/tests.rs` (400 lines) should continue to pass.

Verify:
```bash
cargo test -p fdemon-tui -- frame_chart
```

### Notes

- The `frame_chart/tests.rs` file already exists in the directory. When converting `frame_chart.rs` to `frame_chart/mod.rs`, the `mod tests;` declaration will continue to find `frame_chart/tests.rs` without changes.
- Pure helper functions like `bar_colors()` and `ms_to_half_blocks()` are used by both `render_bar_chart()` and potentially by tests. Keep them in `bars.rs` with `pub(super)` visibility so both `mod.rs` and `tests.rs` can access them via `super::bars::function_name()`.
- If tests import from `super::*`, verify that re-exports in `mod.rs` cover everything the tests need. May need to add `pub(super) use bars::*;` or `pub(super) use detail::*;` in `mod.rs`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs` | Deleted (converted to directory module) |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` | NEW — module entry point: imports, constants, colour helpers, `FrameChart` struct + `Widget` impl (131 lines) |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` | NEW — bar chart rendering: `render_bar_chart`, `compute_visible_range`, `render_budget_line`, plus pure helpers `bar_colors`, `ms_to_half_blocks`, `render_bar` (260 lines) |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` | NEW — detail panel: `render_detail_panel`, `render_frame_detail`, `render_summary_line`, `frame_status_label_and_style`, `render_ui_phase_line` (193 lines) |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/tests.rs` | Unchanged (399 lines) |

### Notable Decisions/Tradeoffs

1. **pub(crate) for re-exported helpers**: `bar_colors` and `ms_to_half_blocks` in `bars.rs` are marked `pub(crate)` (not `pub(super)`) to allow `mod.rs` to re-export them with `pub(super) use`. If both were `pub(super)`, the re-export would widen visibility from `frame_chart` scope to `performance` scope, triggering E0364.

2. **#[cfg(test)] on re-exports**: The re-export `pub(super) use bars::{bar_colors, ms_to_half_blocks}` in `mod.rs` is gated with `#[cfg(test)]` to avoid an "unused imports" warning in non-test builds. The functions are only needed by `tests.rs` via `use super::*`.

3. **super::super::styles in detail.rs**: `detail.rs` accesses `fps_style` and `jank_style` via `use super::super::styles::{fps_style, jank_style}` (navigating up to the `performance` module) because those functions are `pub(super)` in `styles.rs` where `super` = `performance`.

4. **FrameChart fields made pub(super)**: Struct fields were changed from private to `pub(super)` so that `bars.rs` and `detail.rs` (as children of `frame_chart`) can access them via `self.frame_history`, etc. in `impl FrameChart` blocks.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed (no warnings)
- `cargo test -p fdemon-tui --lib -- frame_chart` - Passed (25 tests)
- `cargo test --workspace --lib` - Passed (2264 tests total: 963 + 340 + 357 + 604)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

1. **None identified**: This is a pure refactor with no logic changes. The test suite provides full coverage of the extracted code.
