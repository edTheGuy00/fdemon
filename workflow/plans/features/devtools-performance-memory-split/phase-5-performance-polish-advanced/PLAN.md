# Phase 5 — Performance Tab Advanced (Timeline Interactive)

## Overview

Phase 5 builds on Phase 4's MVP Gantt timeline. This phase adds the **interactive** features explicitly deferred from Phase 4 — pan/zoom, minimap, event-level selection with a details popup, and search/filter. Together these bring the Timeline Events tab closer to feature parity with Flutter DevTools' legacy `FlameChart` UX.

**Total tasks:** 5 across 4 waves.
**Estimated effort:** 17–24 hours.
**Prerequisite:** Phase 4 fully landed (the Gantt widget, `TimelineTrack`/`TimelineNode` state, `timeline_thread_name_map` wiring).

## Background

- **Phase 3** shipped rebuild stats + timeline events polling.
- **Phase 3-followup** tightened lifecycle and parser hygiene.
- **Phase 4** replaced the flat timeline-events list with a Gantt MVP (thread rows, colored bars, depth-stacked nesting, fixed 5s viewport, vertical scroll only).
- **Phase 5 (this)** adds horizontal interactivity: pan, zoom, selection, search, and minimap.
- **Phase 6 (not yet scoped)** will add CPU sampling via `getCpuSamples`.

## Why Phase 5

The Phase-4 Gantt is observation-only — users can see events fly by but can't:

- **Hold the viewport still** to study a specific 100 ms window (no pan/zoom)
- **See where they are** in the overall event history (no minimap)
- **Inspect an individual event's args, full name, parent chain** (no selection)
- **Find a specific event by name** in a busy timeline (no search)

Phase 5 closes each gap with a focused task per feature.

## Findings & Constraints

### Keybinding context (to verify per task)

The Performance Details panel currently uses these keys (Phase 4 baseline — confirm at implementation):

| Key | Action | Conflict with Phase 5? |
|-----|--------|------------------------|
| `]` / `[` | Cycle Performance Details tabs | No |
| `f` | Toggle chart/details focus | Reserved — Phase 5 uses `End`/`g` for "follow latest" instead |
| `R` | HotRestart (or RebuildStats toggle in that tab) | No |
| `T` | Cycle thread filter (`All`/`UI`/`Raster`) | No — preserved |
| `←` / `→` | FrameAnalysis tab: select frame; TimelineEvents tab (Phase 4): unused | **Reused** by Phase 5 for pan/selection (context-dependent) |
| `↑` / `↓` | TimelineEvents tab (Phase 4): scroll thread rows | **Reused** by Phase 5 for selection nav when an event is selected |

Phase 5 introduces these new keys on the Timeline Events tab:

| Key | Action |
|-----|--------|
| `+` / `-` | Zoom in / out (viewport_width_micros *= 0.5 / *= 2.0) |
| `←` / `→` (no selection) | Pan viewport left / right |
| `←` / `→` (selection active) | Move selection to previous / next sibling in same depth |
| `↑` / `↓` (selection active) | Move selection to parent / first child (or up/down thread rows if at root) |
| `Enter` | Open details popup for selected event |
| `Esc` | Close popup OR clear selection (or fall through to existing Esc-exits-DevTools behavior) |
| `End` (or `g`) | Re-enable auto-scroll-forward (clears manual pan) |
| `/` | Open search input |
| `n` / `N` | Next / previous search match (cycle through highlighted events) |
| `Esc` (search input) | Clear query and close input |

**Conflict resolution to verify:** `n` is currently the Network panel shortcut at the top-level. On the Timeline Events tab, `n` should only mean "next match" when search input is open OR a query is active. Otherwise it falls through to the existing `n` → Network handler. Implementor verifies at T04 time.

### State additions (to `PerformanceState`)

```rust
// New in Phase 5
pub timeline_viewport_start_micros: u64,   // pinned manually if non-zero
pub timeline_viewport_width_micros: u64,   // default = TIMELINE_VIEWPORT_MICROS (5s)
pub timeline_follow_latest: bool,          // true = auto-scroll forward (default)
pub timeline_selected_event: Option<TimelineEventCursor>,  // None = no selection
pub timeline_details_popup_open: bool,
pub timeline_search_query: Option<String>,
pub timeline_search_match_cursor: usize,   // current match index when navigating n/N
```

Where `TimelineEventCursor` uniquely identifies a node:

```rust
pub struct TimelineEventCursor {
    pub tid: i64,
    pub depth: u8,
    pub ts: i64,
    // OR a synthetic path: pub path: Vec<usize>,  (root_index, child_index, child_index, ...)
}
```

T03 picks the cursor representation. Path-based is more robust to event re-pairing across batches; (tid, depth, ts) is simpler.

### Module additions (under `widgets/devtools/performance/details/timeline_events/`)

```
timeline_events/
├── mod.rs          # composes minimap, gantt, popup, search (Phase 4 + 5)
├── gantt.rs        # Phase 4
├── palette.rs      # Phase 4
├── viewport.rs     # Phase 4, extended in Phase 5 (pan/zoom math)
├── minimap.rs      # NEW Phase 5 (T02)
├── popup.rs        # NEW Phase 5 (T03)
├── search.rs       # NEW Phase 5 (T04)
└── tests.rs        # Phase 4 + extensions per Phase 5 task
```

Each Phase 5 task adds its own file (avoiding write overlap on existing Phase 4 files where possible) and touches `mod.rs` to compose its output.

## Design Decisions

### D1 — Viewport state lives in `PerformanceState`, not in widget

Pan/zoom is **TEA-managed state**, not widget-local. Keybindings dispatch messages; the handler updates `timeline_viewport_*`; the renderer is a pure function of state. Follows the existing CODE_STANDARDS Principle 3 model (render-hint Cells for read-back, but state proper for user-driven mutation).

### D2 — Auto-scroll-forward vs. manual pan: a single boolean

`timeline_follow_latest: bool`. While `true`, the renderer computes the viewport from the latest event timestamp (Phase 4 behavior). When the user pans/zooms manually, set to `false`. Pressing `End` (or `g`) resets to `true` and snaps the viewport to the latest event window.

### D3 — Search is a query, not a filter

A query **highlights** matching event bars (e.g., a brighter shade or border) but does not hide non-matching events. Pressing `n` advances the **viewport** to center on the next match (pauses auto-scroll). Mirrors DevTools' search-and-jump UX rather than filter-by-name.

### D4 — Selection cursor: by `(tid, depth, ts)` not by index

Tracks mutate as events arrive (new roots appended, oldest evicted). An index-based cursor would drift. `(tid, depth, ts)` is stable as long as the event itself survives in the buffer; if the event ages out, the cursor clears with a warning log.

### D5 — Details popup is a modal overlay, not a side panel

Modal precedence rules from `docs/ARCHITECTURE.md` apply: when the popup is open, base-UI widgets pass `None` as `MouseCtx`, and `Esc` closes the popup before falling through. This avoids splitting the Gantt canvas.

### D6 — Minimap renders one row only

`MINIMAP_HEIGHT: u16 = 1`. Each pixel-column maps to a `(full_history_micros / canvas_width)` slice; pixels are colored by the dominant thread in that slice. A bracket overlay `[...]` shows the current viewport position. Click-to-jump is a stretch goal — leave a TODO if scope tight.

## Out of Scope (Deferred to Phase 6)

- **CPU sampling** (`getCpuSamples` VM Service extension) — substantial new data source requiring a separate plan.
- **Cross-thread async event lines** — DevTools draws connector lines between async start/end events on different threads. Defer.
- **Per-frame zoom-to-frame** — clicking a frame bar in the Frame Chart auto-zooms the Timeline to that frame's time window. Cross-tab coupling; defer.
- **Event annotation / pinning** — letting users mark important events with a label. Out of scope.
- **Trace export** — saving a JSON trace to disk for later analysis. Out of scope.

## Wave Strategy

```
Wave 1 (sequential, foundational)
  └─ T01 timeline-viewport-pan-zoom         (state machine + keybindings + viewport math)
                                            state, handler/keys.rs, handler/devtools/perf,
                                            viewport.rs, gantt.rs

Wave 2 (parallel × 3 worktrees — new files + mod.rs additions are line-disjoint)
  ├─ T02 timeline-minimap-ribbon            NEW minimap.rs + mod.rs composition slot
  ├─ T03 timeline-event-selection-and-details  state, NEW popup.rs, handler, gantt.rs (selection highlight)
  └─ T04 timeline-search-filter             state, NEW search.rs, handler, gantt.rs (match highlight)

Wave 3 (sequential, doc_maintainer)
  └─ T05 update-arch-and-review-focus-docs  docs/ARCHITECTURE.md, docs/REVIEW_FOCUS.md
```

**Note on Wave 2 parallelism:** T03 and T04 both modify `gantt.rs` to overlay rendering (selection highlight, search match highlight) and `handler/devtools/performance/timeline.rs` for key dispatch. This is **write overlap** — orchestrator will run them sequentially within Wave 2, not in worktrees. T02 (minimap) is genuinely disjoint from the others; it can run parallel with whichever of T03/T04 goes first.

## Phase Acceptance Test Plan

After all 5 tasks merge:

1. **Pan/zoom.** Open Timeline Events. Press `+` four times — viewport zooms from 5s to ~600ms. Press `-` to zoom back out. Press `←`/`→` (no selection active) — viewport pans by ~10% per keypress.
2. **Auto-scroll resume.** After manual pan, observe that new events stop scrolling onto the right edge automatically. Press `End` — viewport jumps to the latest events and auto-scroll resumes.
3. **Minimap.** Above the time axis, a 1-row minimap shows all events compressed; a `[...]` bracket indicates current viewport. Panning moves the bracket; minimap content updates as new events arrive.
4. **Event selection.** Press `Enter` on an event-rich row — first visible event highlights. Press `→` — selection moves to next sibling. Press `↓` — selection moves into first child. Press `↑` — back to parent.
5. **Details popup.** With an event selected, press `Enter` again (or some dedicated key — T03 confirms) — modal popup shows full event name, `category`, `ts`, `dur`, `thread`, parent chain. Press `Esc` — popup closes; selection remains. Press `Esc` again — selection clears; viewport remains.
6. **Search.** Press `/` — input opens at top. Type `Raster`. Bars whose name contains "Raster" are visibly highlighted (different border or brighter color). Press `n` — viewport jumps to center on next match. Press `N` — previous match. Press `Esc` — query clears.
7. **Filter still works.** Press `T` — cycles `All`/`UI`/`Raster`/`All`. Search query persists across filter changes (matches re-evaluated against visible threads).
8. **Mouse.** Click a bar — equivalent to selecting it. Click an empty area — clears selection. Click the minimap bracket — pans viewport (if implemented; otherwise no-op).
9. **Quality gate.** `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all green.

## References

- Phase 4 PLAN: `workflow/plans/features/devtools-performance-memory-split/phase-4-performance-polish/PLAN.md`
- DevTools FlameChart pan/zoom: `tmp/devtools/packages/devtools_app/lib/src/shared/charts/flame_chart.dart` (search `FlameChartViewportState`)
- DevTools search widget: `tmp/devtools/packages/devtools_app/lib/src/shared/ui/search.dart`
- DevTools selection model: `tmp/devtools/packages/devtools_app/lib/src/shared/ui/selection_model.dart`
- DevTools cpu profiler (FlameChart consumer): `tmp/devtools/packages/devtools_app/lib/src/screens/profiler/`
