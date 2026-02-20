## Task: Extract memory_chart.rs into submodules

**Objective**: Split `memory_chart.rs` (711 lines) into a directory-based module structure with each submodule under 500 lines.

**Depends on**: Task 01 (layout fix), Task 02 (UTF-8 fix) — both modify `memory_chart.rs`

**Source**: Review Major Issue #3 (Code Quality Inspector)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` → `memory_chart/mod.rs`
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs` — **NEW**
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` — **NEW**

### Details

#### Current State

`memory_chart.rs` is already partially extracted:
- `memory_chart/braille_canvas.rs` (99 lines) — extracted
- `memory_chart/tests.rs` (489 lines) — extracted
- `memory_chart.rs` itself (711 lines) — **over 500-line limit**

The file acts as the implicit `mod.rs` for the `memory_chart` module (Rust 2018 naming convention). To add more submodules, convert it to an explicit `memory_chart/mod.rs`.

#### Extraction Plan

**`memory_chart/mod.rs`** (~280 lines) — keep:
- Module doc, imports, constants (lines 1-41)
- `MemoryChart` struct + `Widget` impl + `render()` dispatch (lines 42-145)
- `render_compact_summary()` (lines 147-191, ~45 lines)
- `render_chart_area()` orchestration (lines 193-257, ~65 lines) — this is the router that calls into submodules
- `mod` declarations for `chart`, `table`, `braille_canvas`, `tests`

**`memory_chart/chart.rs`** (~220 lines) — extract:
- `render_sample_chart()` (lines 259-411, ~153 lines) — the heaviest section, canvas-filling logic
- `render_history_chart()` (lines 413-481, ~69 lines) — fallback chart renderer
- Helper functions called only by the above: `render_legend()`, `render_y_axis_labels()`, `render_x_axis_labels()`

**`memory_chart/table.rs`** (~90 lines) — extract:
- `render_allocation_table()` (lines 617-705, ~89 lines) — standalone concern, self-contained

#### Module Visibility

The extracted functions are called from `mod.rs` methods on `MemoryChart`. Two approaches:

**Option A: Methods on MemoryChart** (recommended)
Keep all functions as `impl MemoryChart` methods across files using `pub(super)` visibility. The `mod.rs` file defines the struct, and submodule files add `impl MemoryChart` blocks:

```rust
// memory_chart/chart.rs
use super::*;

impl MemoryChart<'_> {
    pub(super) fn render_sample_chart(&self, area: Rect, buf: &mut Buffer) { ... }
    pub(super) fn render_history_chart(&self, area: Rect, buf: &mut Buffer) { ... }
    pub(super) fn render_legend(&self, area: Rect, buf: &mut Buffer) { ... }
    pub(super) fn render_y_axis_labels(&self, area: Rect, buf: &mut Buffer) { ... }
    pub(super) fn render_x_axis_labels(&self, area: Rect, buf: &mut Buffer) { ... }
}
```

**Option B: Free functions**
Convert methods to free functions that take `&MemoryChart` as the first parameter. Simpler but changes the calling convention.

Option A is preferred because it preserves the existing `self.method()` call sites in `mod.rs` without any changes.

#### File Conversion Steps

1. Rename `memory_chart.rs` to `memory_chart/mod.rs`
2. Move `render_sample_chart`, `render_history_chart`, `render_legend`, `render_y_axis_labels`, `render_x_axis_labels` into `memory_chart/chart.rs`
3. Move `render_allocation_table` into `memory_chart/table.rs`
4. Add `mod chart;` and `mod table;` to `mod.rs`
5. Ensure all types needed by submodules are accessible (add `use super::*;` or specific imports)
6. Verify `cargo check -p fdemon-tui` and `cargo test -p fdemon-tui`

### Acceptance Criteria

1. `memory_chart/mod.rs` is under 500 lines
2. Each new submodule (`chart.rs`, `table.rs`) is under 500 lines
3. Existing `braille_canvas.rs` and `tests.rs` continue to work
4. All existing rendering tests pass without modification
5. No public API changes (all new functions are `pub(super)`)
6. `cargo check --workspace && cargo test --workspace` passes

### Testing

No new tests needed — this is a pure refactor. All existing tests in `memory_chart/tests.rs` (489 lines, 30+ tests) should continue to pass.

Verify:
```bash
cargo test -p fdemon-tui -- memory_chart
```

### Notes

- The `use super::*;` pattern in submodule files imports all `pub` and `pub(crate)` items from `mod.rs`. This includes the ratatui types, style constants, and the `MemoryChart` struct.
- Keep the constants (`MIN_CHART_HEIGHT`, `MIN_TABLE_HEIGHT`, etc.) in `mod.rs` since they're referenced by both the dispatch logic and the submodules.
- The `CHART_PROPORTION` constant is only used in `render_chart_area()` which stays in `mod.rs` — but if `chart.rs` needs it, move constants to `mod.rs` with `pub(super)` visibility.
