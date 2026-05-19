# Task 05 — Gantt-Style Timeline Events Widget (MVP)

**Status:** Not Started
**Wave:** 3
**Agent:** implementor
**Estimated Effort:** 5–8 hours
**Depends On:** T04 (consumes `timeline_tracks`, `timeline_thread_name_map`, `timeline_thread_scroll_offset`, `timeline_visible_row_count`)

## Problem

The current Timeline Events tab is a flat scrolling list of `(thread, name, duration, ts)` rows — nothing like DevTools' Gantt view (the user's reference screenshot shows colored thread-row bars across a time axis).

Replace `widgets/devtools/performance/details/timeline_events_tab.rs` with a Gantt-style renderer modeled on DevTools' legacy `FlameChart` primitives:

- **Thread rows** with labels on the left, time canvas on the right.
- **Colored event bars** spanning `[ts, ts+dur]` mapped to terminal columns.
- **Depth-stacked nesting** — child events render as bars stacked vertically within their parent bar's row band.
- **Color by event type** — UI thread events blue, raster darker blue, async teal, other purple.
- **Fixed viewport** — most recent `TIMELINE_VIEWPORT_MICROS` (default ~5 s), auto-scrolling as new events arrive. **No pan/zoom in MVP** (deferred to Phase 5).
- **Vertical scroll** — `↑/↓` scrolls between thread rows when more rows than visible.
- **Thread filter preserved** — `T` still cycles `All → UI → Raster → All`.

## Files (Write — NEW subdirectory structure)

Replace single file `widgets/devtools/performance/details/timeline_events_tab.rs` with a subdirectory:

- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` — public `render(area, buf, state)` entry, thread-filter strip orchestration
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` — Gantt renderer: thread-row layout, depth-stacked bars, time axis
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/palette.rs` — color constants per `TimelinePhase` / `TimelineThread`
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/viewport.rs` — pure helpers: `compute_viewport`, `micros_to_column`, `clip_bar_to_viewport`
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/tests.rs` — unit tests (or keep `#[cfg(test)] mod tests` inline per submodule)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` — replace `mod timeline_events_tab;` with `mod timeline_events;`; update `timeline_events_tab::render(...)` call site to `timeline_events::render(...)`

Delete the old `timeline_events_tab.rs` file.

## Files (Read)

- T04 outputs: `crates/fdemon-app/src/session/performance.rs` (`timeline_tracks`, `timeline_thread_name_map`, `timeline_thread_scroll_offset`, `timeline_visible_row_count`)
- `crates/fdemon-core/src/timeline.rs` — `TimelineTrack`, `TimelineNode`, `TimelinePhase`, `TimelineThread`
- `crates/fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` — `truncate_with_ellipsis` for bar labels and thread name truncation
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — verify `T`-key filter footer hint reference

## Approach Hints

### Module structure

```
timeline_events/
├── mod.rs          # 50–100 lines: entry, filter strip, dispatch to gantt
├── gantt.rs        # 200–400 lines: row layout, bar rendering, depth stacking
├── palette.rs      # 30–60 lines: color constants + per-event lookup
├── viewport.rs     # 50–100 lines: pure math helpers
└── tests.rs        # 200–400 lines: unit tests (or inline per-submodule)
```

Each file ≤ 500 lines per CODE_STANDARDS module-organization rule.

### Layout (`gantt.rs`)

```
┌─Filter strip─────────────────────────────────┐
│ [All] UI Raster                              │
├─Time axis────────────────────────────────────┤
│ -5s    -4s    -3s    -2s    -1s    0s        │
├─Thread row 1 (io.flutter.raster 45067)──────┤
│ ┃▓▓▓▓Raster::DoDraw▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ │
│ ┃  ▓▓▓Rasterizer::DrawToSurfaces▓▓▓▓▓▓▓▓▓  │
├─Thread row 2 (io.flutter.ui 45068)──────────┤
│ ┃▓▓▓UI::Frame▓▓▓▓▓▓                          │
├─Thread row 3 (DartWorker 36011)─────────────┤
│ ┃▓▓▓DartWorker task▓▓▓▓                      │
└──────────────────────────────────────────────┘
```

### Constants

```rust
/// Width of the thread-name label column on the left of each row.
pub(super) const THREAD_LABEL_WIDTH: u16 = 25;

/// Default viewport span — show the most recent N microseconds.
/// Equals 5 seconds. Phase 5 will make this configurable.
pub(super) const TIMELINE_VIEWPORT_MICROS: u64 = 5_000_000;

/// Maximum nesting depth rendered per thread row. Deeper children
/// are flattened to the deepest visible level.
pub(super) const MAX_DEPTH: u8 = 5;

/// Minimum bar width in columns. Bars narrower than this are drawn
/// as 1-col vertical marks. Prevents flicker for sub-pixel events.
pub(super) const MIN_BAR_WIDTH: u16 = 1;

/// Height of the time axis row (in terminal lines).
pub(super) const TIME_AXIS_HEIGHT: u16 = 1;

/// Height of a single thread row's content area (in terminal lines).
/// Allows up to MAX_DEPTH stacked child bars + 1 root row = 6 lines.
pub(super) const THREAD_ROW_HEIGHT: u16 = 6;
```

### Viewport math (`viewport.rs`)

```rust
/// Returns the (start_micros, end_micros) viewport bounds based on the
/// latest event timestamp across all tracks. If tracks are empty, returns
/// (0, TIMELINE_VIEWPORT_MICROS).
pub(super) fn compute_viewport(tracks: &BTreeMap<i64, TimelineTrack>) -> (u64, u64) { ... }

/// Maps a microsecond timestamp to a column offset within `time_canvas_width`.
/// Clamped to [0, time_canvas_width).
pub(super) fn micros_to_column(ts: u64, start: u64, end: u64, width: u16) -> u16 { ... }

/// Clips a (ts, dur) bar to the viewport. Returns (col_start, col_width)
/// or None if entirely outside the viewport.
pub(super) fn clip_bar(
    ts: u64, dur: u64, vp_start: u64, vp_end: u64, canvas_width: u16,
) -> Option<(u16, u16)> { ... }
```

### Color palette (`palette.rs`)

```rust
use ratatui::style::Color;
use fdemon_core::timeline::{TimelinePhase, TimelineThread};

pub(super) fn bar_color(thread: TimelineThread, depth: u8) -> Color {
    let palette = match thread {
        TimelineThread::Ui => &[Color::LightBlue, Color::Blue],
        TimelineThread::Raster => &[Color::Blue, Color::DarkGray],
        TimelineThread::Tester => &[Color::Yellow, Color::LightYellow],
        TimelineThread::Other => &[Color::Magenta, Color::LightMagenta],
    };
    palette[(depth as usize) % palette.len()]
}

pub(super) fn label_color(thread: TimelineThread) -> Color {
    match thread {
        TimelineThread::Ui => Color::LightBlue,
        TimelineThread::Raster => Color::Blue,
        TimelineThread::Tester => Color::Yellow,
        TimelineThread::Other => Color::Magenta,
    }
}
```

(DevTools alternates by row depth, not by phase — we mirror that.)

### Bar rendering (`gantt.rs`)

```rust
fn render_bar(
    area: Rect,
    buf: &mut Buffer,
    node: &TimelineNode,
    vp_start: u64,
    vp_end: u64,
    depth: u8,
    label_width: u16,
) {
    let canvas_width = area.width.saturating_sub(label_width);
    let Some((col_off, width)) = clip_bar(
        node.ts as u64,
        node.dur.unwrap_or(0) as u64,
        vp_start, vp_end, canvas_width,
    ) else { return; };
    let y = area.y + depth as u16;
    if y >= area.bottom() { return; }
    let color = bar_color(node.thread, depth);
    let x = area.x + label_width + col_off;
    // Fill the bar with background color
    for dx in 0..width.min(MIN_BAR_WIDTH.max(width)) {
        if let Some(cell) = buf.cell_mut((x + dx, y)) {
            cell.set_bg(color);
        }
    }
    // Render clipped label inside the bar if it fits
    if width >= 4 {
        let label = truncate_with_ellipsis(&node.name, width.saturating_sub(2) as usize);
        let label_rect = Rect { x: x + 1, y, width: width - 2, height: 1 };
        buf.set_string(label_rect.x, label_rect.y, label, Style::default().fg(Color::White));
    }
    // Recurse into children one row down
    if depth + 1 < MAX_DEPTH {
        for child in &node.children {
            render_bar(area, buf, child, vp_start, vp_end, depth + 1, label_width);
        }
    }
}
```

### Render-hint write-back

In `mod.rs::render`, after computing the visible thread-row count:

```rust
// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
state.timeline_visible_row_count.set(visible_row_count);
```

### Vertical scroll

When `tracks.len() > visible_row_count`, slice tracks by `timeline_thread_scroll_offset`. Clamp `scroll_offset` in render-time to prevent off-screen selection.

The existing `↑/↓` handler in `handler/devtools/performance/timeline.rs` (or wherever the perf scroll lives — verify in handler/keys.rs) needs to use `timeline_visible_row_count` to bound the scroll. **Note:** the scroll handler may already exist for `timeline_events_scroll_offset` — repoint it to `timeline_thread_scroll_offset`. If the handler is in `handler/devtools/performance/timeline.rs` (T04's territory), confirm T04 already repointed; if not, this task does the touch-up.

### Empty state

If `timeline_tracks.is_empty()`, render the existing centered placeholder ("Waiting for timeline events…") using `text_helpers::PLACEHOLDER_LINE_COUNT` and the `Layout::vertical` absorber pattern from Phase 3-followup.

## Acceptance Criteria

1. **Subdirectory created** — `timeline_events/` exists with `mod.rs`, `gantt.rs`, `palette.rs`, `viewport.rs`, and either `tests.rs` or per-submodule test blocks. Old `timeline_events_tab.rs` is deleted.
2. **Renders thread rows** — Given `timeline_tracks` with two `tid`s, two distinct thread-row regions appear with labels in the left column.
3. **Thread names displayed** — When `timeline_thread_name_map[tid] = "io.flutter.raster"`, the left label shows `"io.flutter.raster 45067"` (name + tid). Truncated to `THREAD_LABEL_WIDTH` via `truncate_with_ellipsis`.
4. **Fallback labels** — When `timeline_thread_name_map` lacks an entry for a `tid`, fall back to `format!("{:?} {}", track.thread, tid)` (e.g., `"Raster 45067"`).
5. **Bars within viewport rendered with correct color** — Root-level UI events render in light blue; raster in blue; other in magenta. Verified via buffer inspection test.
6. **Bars outside viewport not rendered** — Events with `ts + dur < viewport.start` produce no buffer modifications.
7. **Depth-stacked children** — A root event with 2 nested children renders as 3 stacked rows within the thread's row band. Verified by a layout test that inspects buffer rows at depth 0, 1, 2.
8. **Bar labels truncated** — Long event names are clipped with `truncate_with_ellipsis` to the bar's width minus padding. Bars narrower than 4 columns omit the label entirely.
9. **Empty state placeholder** — When `timeline_tracks.is_empty()`, the centered "Waiting for timeline events…" placeholder still renders correctly using the existing `text_helpers` pattern.
10. **Thread filter** — `timeline_events_filter = TimelineFilter::Ui` causes only UI thread rows to render. `Raster` shows only raster. `All` shows everything. New test `gantt_filter_ui_hides_raster_rows`.
11. **Vertical scroll** — `timeline_thread_scroll_offset = 1` skips the first thread row and starts rendering at the second. New test `gantt_thread_scroll_offset_skips_top_rows`.
12. **Render-hint write-back** — Each render updates `timeline_visible_row_count` to the actual row count drawn. New test `gantt_writes_visible_row_count_render_hint`.
13. **No panic on zero area** — `render(Rect::ZERO, ...)` does not panic. Test `gantt_no_panic_zero_area`.
14. **Filter strip preserved** — The existing thread-filter chips (`[All] UI Raster`) still render above the Gantt area, with the active filter highlighted.
15. **Time axis renders** — A `TIME_AXIS_HEIGHT`-row strip above the thread rows shows time tick labels at `-5s`, `-4s`, …, `0s` (relative to viewport end). New test `time_axis_labels_at_one_second_intervals`.
16. **No remaining references** to `timeline_events_tab` in any source file (`rg` clean).
17. **Quality gate** — `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- Mirroring DevTools: color is determined by **row depth within thread**, not by individual event name. Two-color palette per thread alternating with depth (palette[depth % 2]).
- The `TimelineThread` enum is the post-classification type (`Ui`, `Raster`, `Tester`, `Other`). Per-thread color decision keys off it.
- **No pan/zoom keys yet.** The viewport is fixed at the most recent 5 s and auto-scrolls forward as new events arrive. Phase 5 will add `+`/`-` for zoom and `[`/`]` for pan.
- **No event-level selection yet.** The user can't click a bar to see details — that's Phase 5 scope.
- **No minimap.** Single Gantt canvas only. Phase 5 will add a minimap ribbon above the time axis.
- If the existing `↑/↓` scroll-handler for the timeline tab is in a file T04 already touched, this task may not need to write any handler code. Confirm at implementation time.
- Use `Layout::vertical` with `Constraint::Length(TIME_AXIS_HEIGHT)`, `Constraint::Length(THREAD_ROW_HEIGHT)` per row, and `Constraint::Min(0)` absorber — per CODE_STANDARDS Principle 2.
