# Phase 4 — Performance Tab Polish

## Overview

This phase addresses four user-reported issues in the Performance tab discovered after Phase 3-followup shipped. Three are usability bugs in the Frame Chart widget; the fourth replaces the flat Timeline Events list with a Gantt-style thread-row visualization modeled on Flutter DevTools' legacy `FlameChart` primitives.

**Total tasks:** 6 across 4 waves.
**Estimated effort:** 16–24 hours.

## Background

- **Phase 3** shipped rebuild stats + timeline events polling with green CI.
- **Phase 3-followup** tightened lifecycle (timeline pause/clear on Performance-leave) and parser hygiene.
- **This phase (4)** is the first user-driven polish round. Phase 5 (deferred) will add pan/zoom, minimap, event selection, and search to the Timeline view.

## User Complaints → Findings

### Complaint 1 — Frame-chart bars not proportional, some disappear at small heights

`ms_to_half_blocks` (`crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs:271`) rounds short frames to **0** half-blocks when `(ms / y_range_ms) * total_half_blocks < 0.5`. With a 20 ms `y_range_ms` floor and `area.height = 4` rows (= 8 half-blocks), any frame shorter than **~1.25 ms rounds to invisible**. There is no `max(1, …)` floor for nonzero durations.

### Complaint 2 — Selected-bar highlight invisible

`bars.rs:133-147` paints a single `▔` (U+2594, upper-eighth block) at `area.y` only — the very top row of the chart. The character sits **above** the bar's visible top whenever the bar is shorter than full height, and it's a single sub-pixel sliver of a character. Easy to miss.

### Complaint 3 — Selection pinned to right edge instead of moving through visible bars

`frame.rs:37-39` unconditionally resets `frame_chart_scroll_offset = 0` whenever `SelectPerformanceFrame { index: Some(_) }` is dispatched. Render-side `compute_visible_range` in `bars.rs:227-250` then anchors the visible window at `(sel+1, min(sel+1, count))` — so the selected bar is always painted at the **right-hand** edge. Pressing Left/Right just moves the global `selected_frame` and the viewport slides to follow, giving the appearance that "the chart scrolls and the selection stays pinned."

### Complaint 4 — Timeline Events tab shows only "Waiting for timeline events…"

Two contributing issues:

1. **Cold start delay.** Polling is 1 Hz; `spawn_timeline_polling` (`actions/performance.rs:556`) waits a full tick before the first fetch, so on every Performance-panel-enter the user sees the empty-state placeholder for ~1 s before events arrive. There is no immediate-fetch-on-unpause path (allocation polling has one; timeline doesn't).
2. **Visualization gap.** Even once events arrive, the current renderer is a flat scrolling list of `(thread, name, duration, ts)` rows — nothing like DevTools' colored thread-row bars across a time axis. We want a Gantt-style view: rows per thread, colored bars per event, depth-stacked nesting for parent/child events, fixed viewport on the most recent N seconds.

## Reference: Flutter DevTools

**Critical finding from research:** DevTools' current Timeline Events screen is **Perfetto-only** — it embeds the Perfetto web viewer in an iFrame with zero custom DevTools rendering code. The reusable design source is DevTools' **legacy `FlameChart` widget** at `tmp/devtools/packages/devtools_app/lib/src/shared/charts/flame_chart.dart`, still used by the CPU profiler.

Reusable primitives:

- **Data model:** `FlutterTimelineEvent` — tree node from B/E pairs with `name`, `time: TimeRange{start, end}`, `type: ui|raster|other`, `children: List<FlutterTimelineEvent>`.
- **Tree builder:** stack-based per-`trackId` (`tid`) B/E processor; `sliceBegin` pushes, `sliceEnd` pops and finalizes `time.end`.
- **Renderer:** thread rows down the left, colored bars across time, row depth = child nesting. Color by event type: `ui=blue`, `raster=darker blue`, `async=teal`, `other=purple`.
- **Time grid:** vertical tick overlay every N microseconds.

Our existing parser already emits B, E, X, i events with `tid`, `ts`, `dur`, `name`, `phase`. We need to: (a) pair B/E into durations, (b) stack-nest by overlap within the same `tid`, (c) group by thread, (d) render as a Gantt.

## Design Decisions

### D1 — Bundle frame-chart fixes (T01)

All three frame-chart fixes touch `frame_chart/bars.rs` and `handler/devtools/performance/frame.rs`. Bundling them keeps the diff coherent and avoids three sequential merges into the same files. Three sub-acceptance criteria.

### D2 — Pan/zoom deferred to Phase 5

The Phase-4 Gantt MVP shows a **fixed-window viewport** of the most recent `TIMELINE_VIEWPORT_MICROS` (default ~5 s) of events with **auto-scroll** as new events arrive. The user can scroll **vertically** between thread rows with ↑/↓ but cannot pan or zoom horizontally. The existing `T`-key thread filter (`UI`/`Raster`/`All`) is preserved.

Phase 5 will add: horizontal pan/zoom, minimap ribbon, event selection + details popup, search/filter by name.

### D3 — State migration is breaking, but contained

`PerformanceState::timeline_events: VecDeque<TimelineEvent>` is consumed only by `widgets/devtools/performance/details/timeline_events_tab.rs` — verified via codebase audit. No MCP, headless, or service code reads it. T04 replaces the flat buffer with a thread-grouped tree structure (see T04 task file for the proposed shape). All consumer tests in `session/performance.rs`, `handler/devtools/performance/timeline.rs`, and `timeline_events_tab.rs` will be updated in lockstep.

### D4 — Wire up the dead `timeline_thread_name_map`

`PerformanceState::timeline_thread_name_map: HashMap<i64, String>` is declared but never written (the polling task uses a local map; metadata `M` events are filtered out of `parse_vm_timeline` before they reach the handler). T04 fixes this by:

1. Including thread-name metadata in batches sent from the daemon side, OR
2. Building thread names in the handler from track-descriptor events parsed by the polling task.

The Gantt widget needs human-readable thread labels (`"io.flutter.raster 45067"` instead of `45067`), so this is required scope.

### D5 — Immediate fetch on Performance-unpause (T03)

Mirror the allocation-polling pattern at `actions/performance.rs:328-385`: when `timeline_pause_rx.changed()` fires with `false`, run one fetch immediately before waiting for the next 1-Hz tick. Eliminates the cold-start placeholder window.

## Out of Scope

- **No pan/zoom, no minimap, no event-selection-with-details.** All deferred to Phase 5.
- **No CPU sampling.** The "Include CPU samples" toggle in DevTools requires the `getCpuSamples` VM Service extension, which is a separate feature.
- **No new keybindings.** `T` (thread filter cycle) stays. `↑/↓` reuses existing scroll semantics applied to thread rows in the Gantt view.
- **No protobuf migration.** Continue using JSON `getVMTimeline` per Phase 3 §7.5.
- **No layout-threshold value changes** beyond named-constant introductions.

## Wave Strategy

```
Wave 1 (parallel × 3 worktrees — disjoint crates)
  ├─ T01 frame-chart-fixes         (complaints 1+2+3)  fdemon-tui + frame handler
  ├─ T02 timeline-parser-be-pairing (B/E → durations)  fdemon-core/src/timeline.rs
  └─ T03 immediate-timeline-fetch-on-unpause            fdemon-app/src/actions/performance.rs

Wave 2 (sequential after T02 — depends on new event tree type)
  └─ T04 timeline-state-thread-grouped-tree            session/performance.rs +
                                                       handler/devtools/performance/timeline.rs

Wave 3 (sequential after T04 — depends on new state shape)
  └─ T05 gantt-timeline-widget                         widgets/devtools/performance/details/
                                                       timeline_events/ (new subdirectory)

Wave 4 (sequential after all impl — doc_maintainer)
  └─ T06 update-arch-and-review-focus-docs             docs/ARCHITECTURE.md +
                                                       docs/REVIEW_FOCUS.md
```

## Phase Acceptance Test Plan

After all tasks merge:

1. **Frame Chart bar height.** Launch fdemon in a 80×20 terminal. Generate fast frames (~1–3 ms each). All bars visible — no missing columns. Resize to 80×8 — bars still visible, proportions still meaningful.
2. **Frame Chart selection highlight.** Press `]` to enter Details, then press `[` to return to chart focus. Press Left/Right — the selected bar shows a clear full-column highlight (not just a single character at the top). Visually impossible to lose track of which bar is selected.
3. **Frame Chart scroll behavior.** With the chart scrolled to show frames 100–130 (selection initially at frame 130), press Left. Selection moves to frame 129; viewport unchanged. Keep pressing Left until selection reaches frame 100 (leftmost visible). Press Left once more — viewport scrolls left to reveal frame 99 with selection on it.
4. **Timeline cold-start.** Press `p` to enter Performance, then `]` to enter Details, then `]` again to reach Timeline Events. First events visible within ~150 ms (not 1 s).
5. **Timeline Gantt rendering.** Tail `flutter run` on a non-trivial app. The Timeline Events tab shows thread rows labeled (`io.flutter.raster …`, `io.flutter.ui …`, `DartWorker …`). Each row has colored event bars across the most recent ~5 s; raster bars are darker blue than UI bars. Nested child events stack visually within their parent bar.
6. **Thread filter.** Press `T` to cycle through `All → UI → Raster → All`. Only the matching thread rows render in `UI` and `Raster` modes.
7. **Vertical scroll.** With more thread rows than fit on screen, press `↑/↓` to scroll between rows. Scroll offset wraps gracefully at top/bottom.
8. **Quality gate.** `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all green.

## References

- Phase 3 PLAN: `workflow/plans/features/devtools-performance-memory-split/phase-3/PLAN.md`
- Phase 3-followup TASKS: `workflow/plans/features/devtools-performance-memory-split/phase-3-followup/TASKS.md`
- DevTools FlameChart: `tmp/devtools/packages/devtools_app/lib/src/shared/charts/flame_chart.dart`
- DevTools color palettes: `tmp/devtools/packages/devtools_app/lib/src/shared/ui/colors.dart`
- DevTools timeline event processor: `tmp/devtools/packages/devtools_app/lib/src/screens/performance/panes/timeline_events/timeline_event_processor.dart`
