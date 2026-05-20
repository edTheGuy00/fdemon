# Task 02 — Timeline Minimap Ribbon

**Status:** Not Started
**Wave:** 2
**Agent:** implementor
**Estimated Effort:** 3–4 hours
**Depends On:** T01 (viewport state)

## Problem

The Phase 4 Gantt renders only the events visible in the current viewport. After T01 introduces pan/zoom, users have no visual cue for **where they are** in the overall event history — there could be 30s of buffered events with the viewport showing 500ms in the middle, and the user has no way to tell.

Phase 5 adds a 1-row **minimap ribbon** above the time axis: a horizontally-compressed view of the entire event buffer with a `[...]` bracket overlay showing the current viewport position.

## Files (Write)

- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/minimap.rs` (NEW)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (compose minimap above time axis)

## Files (Read)

- T01 outputs: viewport state in `PerformanceState` + `compute_active_viewport` (use this to source the bracket overlay's start/end — do **not** read `timeline_viewport_*` fields directly so frame-anchored viewport is respected)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/palette.rs` — reuse `bar_color`
- Phase 4 outputs: `TimelineTrack`, `TimelineNode`

## Approach Hints

### Layout integration (`mod.rs`)

Insert a new `Constraint::Length(MINIMAP_HEIGHT)` slot above the time axis row:

```rust
let chunks = Layout::vertical([
    Constraint::Length(FILTER_STRIP_HEIGHT),
    Constraint::Length(MINIMAP_HEIGHT),       // NEW (Phase 5 T02)
    Constraint::Length(TIME_AXIS_HEIGHT),
    Constraint::Min(0),                       // thread rows + Gantt canvas
]);
minimap::render(chunks[1], buf, ...);
```

### Constants

```rust
pub(super) const MINIMAP_HEIGHT: u16 = 1;

/// Default history span covered by the minimap, in microseconds.
/// Auto-extends to encompass all buffered events when they exceed this span.
pub(super) const MINIMAP_DEFAULT_HISTORY_MICROS: u64 = 30_000_000; // 30 s
```

### Renderer signature

The caller (in `mod.rs`) computes `(viewport_start, viewport_end) = compute_active_viewport(perf)` and passes them in. The minimap itself stays pure — no `PerformanceState` access.

```rust
pub(super) fn render(
    area: Rect,
    buf: &mut Buffer,
    tracks: &BTreeMap<i64, TimelineTrack>,
    viewport_start: u64,
    viewport_end: u64,
    filter: TimelineFilter,
) {
    if area.width == 0 || area.height == 0 || tracks.is_empty() {
        return;
    }
    // 1. Compute full history bounds (min ts, max ts+dur across all visible tracks).
    let (history_start, history_end) = compute_history_bounds(tracks, filter);
    // 2. For each column x in 0..area.width, determine the dominant thread color.
    for x in 0..area.width {
        let col_start_micros = history_start + (x as u64 * (history_end - history_start) / area.width as u64);
        let col_end_micros   = history_start + ((x + 1) as u64 * (history_end - history_start) / area.width as u64);
        let dominant = dominant_thread_in_range(tracks, col_start_micros, col_end_micros, filter);
        if let Some(thread) = dominant {
            let color = bar_color(thread, 0);
            if let Some(cell) = buf.cell_mut((area.x + x, area.y)) {
                cell.set_bg(color);
            }
        }
    }
    // 3. Overlay viewport bracket [...] at the current viewport's column range.
    let vp_start_col = micros_to_column(viewport_start, history_start, history_end, area.width);
    let vp_end_col   = micros_to_column(viewport_end,   history_start, history_end, area.width);
    let bracket_x_start = area.x + vp_start_col;
    let bracket_x_end   = area.x + vp_end_col.saturating_sub(1).max(vp_start_col);
    if let Some(cell) = buf.cell_mut((bracket_x_start, area.y)) {
        cell.set_char('[').set_fg(Color::White).add_modifier(Modifier::BOLD);
    }
    if let Some(cell) = buf.cell_mut((bracket_x_end, area.y)) {
        cell.set_char(']').set_fg(Color::White).add_modifier(Modifier::BOLD);
    }
}
```

### Dominant-thread computation

For each minimap column, count event-microseconds per thread:

```rust
fn dominant_thread_in_range(
    tracks: &BTreeMap<i64, TimelineTrack>,
    col_start: u64,
    col_end: u64,
    filter: TimelineFilter,
) -> Option<TimelineThread> {
    // Sum dur per thread for events overlapping [col_start, col_end].
    // Return the thread with the largest total. None if no events in range.
    // Respect `filter` — skip threads excluded by it.
}
```

Walk only `root_events` (depth 0) for the minimap — child events don't influence the macro-view materially and would dominate the cost.

## Acceptance Criteria

1. **NEW `minimap.rs` file** in `timeline_events/` with `render` function and tests.
2. **Layout slot added** — `mod.rs` inserts a `Constraint::Length(MINIMAP_HEIGHT)` row between the filter strip and time axis.
3. **Empty state** — When `tracks.is_empty()`, `render` produces no buffer changes. Test `minimap_empty_state_no_panic_no_paint`.
4. **Single thread** — A track with 5 root events spanning `[0, 5s]` produces a solid-color row across the full width. Test `minimap_single_thread_paints_solid_row`.
5. **Multi-thread dominance** — Two tracks (UI on left half, Raster on right half) produce a two-color row. Test `minimap_multi_thread_shows_dominant_per_column`.
6. **Viewport bracket** — Viewport `[1s, 2s]` on a `[0, 5s]` history produces `[` at column 20% and `]` at column 40% of width. Test `minimap_bracket_at_correct_columns`.
7. **Bracket clipped** — When viewport spans the full history, `[` at column 0 and `]` at column width-1. When viewport extends beyond history (after live-follow with no buffered events), bracket clips to canvas. Test `minimap_bracket_clipped_to_canvas`.
8. **Filter respected** — `TimelineFilter::Ui` causes minimap to ignore raster events for dominance computation. Test `minimap_filter_ui_excludes_raster_threads`.
9. **No panic on small width** — Width 1 still produces a 1-column minimap with bracket compressed to a single cell. Test `minimap_width_one_no_panic`.
10. **Mouse stretch goal** — Clicking on the minimap pans the viewport to center on the clicked column. **Skip if scope tight**, leave a TODO. Document the choice in Completion Summary.
11. **Quality gate** — `cargo fmt --all -- --check`, `cargo check -p fdemon-tui --all-targets`, `cargo test -p fdemon-tui`, `cargo clippy -p fdemon-tui --all-targets -- -D warnings` all pass.

## Notes

- Minimap is **read-only state consumer**. No state additions in this task.
- Bracket coloring: use `Color::White` + `Modifier::BOLD` to ensure visibility over any background palette.
- The 30s default history span is a soft target — if the buffer has older events, minimap auto-extends to include them; if newer, minimap compresses. The viewport bracket always reflects T01's `(viewport_start, viewport_end)`.
- Dominant-thread cost is `O(columns × root_events)`. With 100 columns and 1000 events, that's 100k iterations per frame at most — acceptable for TUI. If profiling shows it as a hot path, switch to a pre-binned histogram updated on batch-receive.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a050a56ae30ae0b26

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/minimap.rs` | NEW — pure minimap renderer with `render`, `compute_history_bounds`, `dominant_thread_in_range`, `column_to_micros` helpers and 16 unit tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` | Added `mod minimap` declaration, `MIN_HEIGHT_FOR_MINIMAP` constant, updated `render()` to insert `Constraint::Length(MINIMAP_HEIGHT)` layout slot and call `minimap::render` with resolved viewport bounds |

### Notable Decisions/Tradeoffs

1. **Viewport resolved once in `mod.rs`**: `compute_active_viewport` is called in the top-level `render` function and the resulting `(vp_start, vp_end)` is passed to both minimap and Gantt. This avoids calling the function twice per frame while keeping minimap.rs stateless (no `PerformanceState` import).

2. **Graceful fallback when too short**: When `area.height <= MIN_HEIGHT_FOR_MINIMAP` (2 rows), the minimap slot is dropped and only the filter strip + Gantt are rendered. This prevents layout corruption on very small terminal windows.

3. **AC10 (mouse stretch goal) skipped**: Adding click-to-pan on the minimap requires `MouseCtx` plumbing from T03/T04 which hasn't landed yet. A `TODO` comment with the exact implementation path is left in `minimap.rs`.

4. **`set_char` + `set_style` dual call for brackets**: The `set_fg` call on `cell` before `set_style` is intentionally redundant — `set_style` already sets `fg(Color::White)`. This matches the task's pseudocode pattern and is harmless.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check -p fdemon-tui --all-targets` - Passed
- `cargo test -p fdemon-tui` - Passed (1277 tests + 7 doc-tests; 16 new minimap tests)
- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **No mouse interaction**: AC10 skipped per task guidance ("skip if scope tight"). The TODO comment in minimap.rs describes the full implementation path for when T03/T04 mouse plumbing lands.
2. **Bracket overlap on width=1**: When `area.width == 1`, both `[` and `]` land on column 0 and `]` overwrites `[`. This is the expected behavior as noted in the test `minimap_width_one_no_panic`.
