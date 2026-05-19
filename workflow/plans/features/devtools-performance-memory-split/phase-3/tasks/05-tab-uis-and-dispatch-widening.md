## Task: Tab UIs and Dispatch Widening

**Objective**: Replace both Phase-2 stub tabs (`rebuild_stats_tab.rs`, `timeline_events_tab.rs`) with populated content driven by the new `PerformanceState` fields from T04. Widen the tab dispatcher in `details/mod.rs` to pass `&PerformanceState` to all three tabs and implement the conditional `RebuildStats` tab visibility (hidden when `rebuild_stats_enabled == false`). After this task, the user can see live rebuild data and timeline events in the TUI.

**Depends on**: T04 (`PerformanceState` fields, `TimelineFilter` enum, `RebuildStatsSnapshot` shape).

**Agent:** implementor

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**

| File | Change |
|---|---|
| `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` | (1) Widen `render` dispatch signatures to pass `&PerformanceState` to all 3 tab `render` functions. (2) Conditional `RebuildStats` visibility — when `state.rebuild_stats_enabled == false`, drop the `[Rebuild Stats]` chip from the tab strip and skip it in the underline. (3) `Tab` / `]` / `[` cycling continues to use `PerfDetailsTab::next()`/`prev()` — those methods return all 3 variants; the visibility filter is applied AFTER selection. If the selected tab is hidden, fall through to the next visible tab. (4) Update existing tests for new signature + visibility logic. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` | Replace stub. Signature becomes `pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState)`. Layout: 1-line header (`"Rebuild tracking: ON — R to disable"` or `"Rebuild tracking: OFF — R to enable"`), separator, 3-column sortable table (`Widget`, `file:line`, `Count`). Source: `state.rebuild_stats_frames.back()` for the latest-frame view, OR aggregated `state.rebuild_stats_totals` if implementor chooses "all-time" as the default view. Recommend latest-frame view (matches DevTools default). Scrolling driven by `state.rebuild_stats_scroll_offset`; selection driven by `state.rebuild_stats_selected_row`. Render-hint write-back: `state.details_pane_visible_height.set(table_inner_height)`. Empty-state placeholder ("No rebuilds in the most recent frame. Interact with the app to trigger widget builds."). |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` | Replace stub. Signature `pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState)`. Layout: 1-line filter strip `[All*] [UI] [Raster]` (active filter chip reversed), 1-line column headers `Thread  Event Name  Duration  ts(rel)`, scrollable list of events filtered by `state.timeline_events_filter`. Each row: thread-color badge (`Cyan` for UI, `Magenta` for Raster, `DarkGray` for Other), then event name (truncated), duration (`12.3 ms`), and `ts` relative to the newest event (e.g. `-150ms`). Render-hint write-back. Empty-state placeholder ("Waiting for timeline events…"). |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/tests.rs` (NEW or extended) | Snapshot/structural tests for both tabs + visibility gating in dispatch. |

**Files Read (Dependencies):**
- T04 outputs (`PerformanceState` field names, `TimelineFilter`, `RebuildStatsSnapshot`).
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — Phase-2 reference for state-aware tab signature + render-hint pattern.
- `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` — existing tab strip rendering (Phase 2) for the conditional-chip pattern.
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` — reference for table column layout + selection highlight.
- `crates/fdemon-tui/src/style.rs` (or equivalent) — color constants (`TEXT_MUTED`, etc.).

### Details

#### `details/mod.rs` — dispatch widening + visibility

Today (Phase 2), the dispatch matches `state.details_tab` and calls the per-tab render. Phase 3 changes:

1. **Pass state into every tab.** Even `frame_analysis_tab` already takes state, but verify the signature uniformly is `(area, buf, state: &PerformanceState)`. If `frame_analysis_tab` signature is `(area, buf, perf: &PerformanceState, selected_frame: Option<&FrameTiming>, ...)`, leave it — don't break Phase-2 API. Only widen the two stubs.

2. **Build the visible tab list dynamically:**

```rust
let visible_tabs: Vec<PerfDetailsTab> = {
    let mut v = vec![PerfDetailsTab::FrameAnalysis];
    if state.rebuild_stats_enabled {
        v.push(PerfDetailsTab::RebuildStats);
    }
    v.push(PerfDetailsTab::TimelineEvents);
    v
};
```

3. **Tab strip render** iterates `visible_tabs` (not `PerfDetailsTab::ALL` if such a constant exists today — replace with the dynamic list).

4. **Selection fall-through:** If `state.details_tab == RebuildStats` but `!state.rebuild_stats_enabled`, render the dispatch as `TimelineEvents` (the snap-to-next on disable is handled by T04 at state-update time, but defend against the transient frame between disable and re-render).

#### `rebuild_stats_tab.rs` — table layout

Suggested layout (200×30 terminal, details pane ~13 rows):

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Rebuild tracking: ON — R to disable    Frame: 142    Locations: 47       │  <- header
├──────────────────────────────────────────────────────────────────────────┤
│ Widget                  Location                              Count       │  <- column headers
├──────────────────────────────────────────────────────────────────────────┤
│ Container               package:foo/main.dart:23                    18    │  <- selected row (reverse video)
│ Padding                 package:foo/main.dart:45                    12    │
│ Text                    package:foo/widgets/title.dart:12            8    │
│ ...                                                                      │
└──────────────────────────────────────────────────────────────────────────┘
```

Implementation skeleton:

```rust
//! Phase 3 Rebuild Stats tab: per-frame widget rebuild counts.
use ratatui::{buffer::Buffer, layout::Rect};
use crate::widgets::devtools::performance::PerformanceState;

const MIN_TABLE_HEIGHT: u16 = 4;  // header + col header + 2 data rows

pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if !state.rebuild_stats_enabled {
        render_disabled_placeholder(area, buf);
        return;
    }
    if state.rebuild_stats_frames.is_empty() {
        render_empty_placeholder(area, buf, "Rebuild tracking is ON — waiting for first frame…");
        return;
    }
    // 1. Header line
    // 2. Column headers
    // 3. Sort rows by Count desc (default sort)
    // 4. Scroll-clamp using state.rebuild_stats_scroll_offset against
    //    visible_rows = area.height - 2 (header + col header).
    //    EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
    //    state.details_pane_visible_height.set(visible_rows);
    // 5. Render rows; selected row gets reverse-video.
}

fn render_disabled_placeholder(area: Rect, buf: &mut Buffer) {
    // Centered TEXT_MUTED:
    //   "Rebuild tracking is OFF."
    //   "Press R to enable."
    //   "(Tab will be hidden when toggle settles.)"
}
```

#### `timeline_events_tab.rs` — list layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [All*] [UI] [Raster]                                Events: 847 / 1000   │  <- filter strip
├──────────────────────────────────────────────────────────────────────────┤
│ Thread  Event                              Duration       ts (rel)        │  <- col header
├──────────────────────────────────────────────────────────────────────────┤
│   UI    Frame                              16.2 ms           0ms          │
│ Raster  GPURasterizer::Draw                10.5 ms          -8ms          │
│   UI    Build                               5.3 ms         -16ms          │
│ ...                                                                       │
└───────────────────────────────────────────────────────────────────────────┘
```

```rust
const COLOR_UI: ratatui::style::Color = ratatui::style::Color::Cyan;
const COLOR_RASTER: ratatui::style::Color = ratatui::style::Color::Magenta;
const COLOR_OTHER: ratatui::style::Color = ratatui::style::Color::DarkGray;

pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    // 1. Filter strip: render 3 chips; active filter is the one matching
    //    state.timeline_events_filter (reverse video).
    // 2. Build filtered slice:
    //    let visible: Vec<&TimelineEvent> = state.timeline_events.iter()
    //        .filter(|e| matches_filter(e.thread, state.timeline_events_filter))
    //        .collect();
    // 3. Newest-first display: iter in reverse so newest event is at top.
    // 4. ts (rel) = ts - newest_ts (so newest = "0ms", older = negative).
    // 5. Render-hint write-back: state.details_pane_visible_height.set(list_inner_height).
    // 6. Empty state if visible.is_empty().
}
```

#### `details/tests.rs` test coverage

- `rebuild_stats_tab_renders_disabled_state` — `rebuild_stats_enabled == false` → output contains `"Press R to enable"`.
- `rebuild_stats_tab_renders_empty_frames_state` — enabled but no snapshots → `"waiting for first frame"`.
- `rebuild_stats_tab_renders_table_with_selection` — fixture with 5 rebuilds, `selected_row == Some(2)` → row 2 has reverse-video style.
- `rebuild_stats_tab_writes_render_hint_height` — verify `details_pane_visible_height.get()` matches the inner height after render.
- `timeline_events_tab_renders_empty_state`.
- `timeline_events_tab_filters_by_ui_thread` — fixture with 2 UI + 2 Raster events, filter `Ui` → output mentions only UI events.
- `timeline_events_tab_filters_by_raster_thread`.
- `timeline_events_tab_filter_strip_highlights_active` — visual assertion on chip styling.
- `timeline_events_tab_writes_render_hint_height`.
- `details_mod_hides_rebuild_stats_tab_when_disabled` — tab strip output contains `[Frame Analysis]` and `[Timeline Events]` but NOT `[Rebuild Stats]`.
- `details_mod_shows_rebuild_stats_tab_when_enabled` — tab strip contains all three chips.
- `details_mod_dispatch_falls_through_when_selected_tab_hidden` — `details_tab == RebuildStats` + `rebuild_stats_enabled == false` → dispatcher renders `TimelineEvents` content.

### Acceptance Criteria

1. `cargo check -p fdemon-tui` passes.
2. `cargo test -p fdemon-tui` passes including new tests.
3. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` is clean.
4. Both tab `render` functions take `&PerformanceState` and use it to drive content.
5. `rebuild_stats_tab` renders a 3-column table when enabled with frames present; a disabled placeholder otherwise; an empty placeholder when enabled-but-no-frames.
6. `timeline_events_tab` renders the filter strip + scrollable list; respects `timeline_events_filter`; shows empty-state placeholder when no events match.
7. `details/mod.rs` dispatcher dynamically hides the `RebuildStats` tab chip when `rebuild_stats_enabled == false`, and gracefully falls through to a visible tab if the currently-selected tab is hidden.
8. Render-hint write-back: both tabs set `state.details_pane_visible_height` to the visible inner height (the EXCEPTION-annotated `Cell` write per CODE_STANDARDS Principle 3).
9. Existing Phase-2 frame-analysis-tab tests and dispatcher tests still pass (no regression).

### Notes

- **Use the Phase-2 render-hint pattern verbatim.** Annotate every `Cell::set` with the `// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md` comment. T04's `consolidated-minor-cleanup` (phase-2-followup) added the `details_pane_visible_height` field; Phase 3 is the first consumer.
- **Sort default: by count descending.** Both tabs need to display "most active" entries near the top. T05 does NOT implement an interactive sort toggle for Rebuild Stats (deferred — PLAN.md §5.3 mentions `s` but it's a nice-to-have; M2 from phase-2-followup recommends keeping `s` slot free for memory-tab use). If a sort toggle is desired here, it conflicts with the memory `s`. Leave sort fixed at "By Count desc" for Phase 3.
- **No mouse click on filter chips or rows** — Phase 2 deferred click-handlers for Inspector + Performance tabs. T05 follows the same deferral; keyboard-only navigation. (If you DO add a click region, gate it carefully to avoid drift with the keyboard-only convention.)
- **`PerfDetailsTab::ALL` constant** — if Phase 2 introduced such a constant, T05 replaces references with the dynamic `visible_tabs` Vec. If no such constant exists, T05 simply constructs the Vec inline.
- **Truncation policy:** Event names and widget names may exceed column width. Truncate with `…` suffix at column boundary. Use `ratatui::text::Line::from(...).style(...)` for inline styling.
- **`Frame: 142` header on Rebuild Stats** — pulls `frame_number` from the latest snapshot. If `state.rebuild_stats_frames` is empty, omit the field from the header.
- **Selection fall-through is defensive** — T04 handles the snap-to-next on the state-update side; T05's fall-through is for the 1-frame window between disable and re-render. Without it, the dispatcher would render the (now-empty) `RebuildStats` tab for one frame.
- **Color choices** (`Cyan` UI, `Magenta` Raster, `DarkGray` Other) follow Flutter DevTools' color conventions. If the project has theme constants in `crates/fdemon-tui/src/style.rs`, use those instead of literal `Color::*` — discover during implementation.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` | Widened dispatch to pass `&PerformanceState` to all 3 tabs; added `visible_tabs()` for conditional RebuildStats chip; added `effective_tab()` fall-through; updated `render_tab_strip()` to take `&[PerfDetailsTab]`. 12 new tests. Updated 2 Phase-2 stub dispatch tests to use new function signature. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` | Replaced Phase-2 stub. Full Phase-3 implementation: 3-column table (Widget/Location/Count), sorted by count desc, scroll offset clamping, selected-row reverse-video, render-hint write-back, disabled/empty-frame placeholders. 14 unit tests. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` | Replaced Phase-2 stub. Full Phase-3 implementation: filter strip [All/UI/Raster] with REVERSED active chip, column headers, scrollable event list with thread badge (Cyan/Magenta/DarkGray), duration, and relative ts. Render-hint write-back. 9 unit tests. |
| `crates/fdemon-app/src/session/mod.rs` | Added `TimelineFilter` to `pub use performance::` re-exports so fdemon-tui can import it. |
| `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | Updated 3 Phase-2 tests that expected stub "Coming soon" messages — now expect Phase-3 content (empty placeholders, conditional tab visibility). |

### Notable Decisions/Tradeoffs

1. **TimelineFilter re-export**: `TimelineFilter` was defined in `fdemon_app::session::performance` but not re-exported at the `session` module level. Added it to the `pub use` line rather than having the TUI use the `pub(crate)` module path directly.

2. **Color choices**: Used `Color::Cyan/Magenta/DarkGray` literal constants for thread badge colors (per task specification). The `crates/fdemon-tui/src/theme/palette.rs` doesn't have matching semantic colors for thread types, so literals match the Flutter DevTools convention directly.

3. **Rebuild Stats sort**: Fixed at "count descending" per task note — no interactive sort toggle (`s` key is reserved for memory tab, Phase-2-followup).

4. **Phase-2 test updates**: Three tests in `performance/tests.rs` expected Phase-2 stub text ("Coming soon"). Updated them to expect Phase-3 content. The `dual_pane` test now sets `rebuild_stats_enabled: true` to make the Rebuild Stats tab chip visible.

5. **Fall-through is defensive**: `effective_tab()` maps `RebuildStats + disabled → TimelineEvents` covering the single frame between a disable event and the handler snap-to-next. The handler (T04) already snaps the selection on the state-update path; this is a TUI safety net only.

### Testing Performed

- `cargo check -p fdemon-tui` — Passed
- `cargo test -p fdemon-tui` — Passed (1178 tests, 0 failed)
- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` — Passed (clean)
- `cargo fmt --all -- --check` — Passed
- `cargo test --workspace` — Passed (all crates, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No interactive sort toggle for Rebuild Stats**: Fixed at "count descending" per task notes. `s` key is reserved for memory tab.
2. **No mouse click on filter chips/rows**: Keyboard-only, matching Phase-2 deferral policy.
3. **Timeline events newest-first display**: Uses `iter().rev()` on the filtered vec. This is O(n) per frame, acceptable for ring buffer sizes ≤1000.
