# Phase 5 — Performance Tab Advanced (Timeline Interactive)

## Overview

Phase 5 builds on Phase 4's MVP Gantt timeline. This phase adds the **interactive** features explicitly deferred from Phase 4 — pan/zoom, minimap, event-level selection with a details popup, and search/filter. Together these bring the Timeline Events tab closer to feature parity with Flutter DevTools' legacy `FlameChart` UX.

**Total tasks:** 5 across 3 waves.
**Estimated effort:** 18–26 hours (T01 +1h for pre-flight `gantt.rs` test extraction).
**Prerequisite:** Phase 4 fully landed (the Gantt widget, `TimelineTrack`/`TimelineNode` state, `timeline_thread_name_map` wiring, plus the Phase-4-bonus frame-anchored viewport primitives — see Codebase Verification).

## Background

- **Phase 3** shipped rebuild stats + timeline events polling.
- **Phase 3-followup** tightened lifecycle and parser hygiene.
- **Phase 4** replaced the flat timeline-events list with a Gantt MVP (thread rows, colored bars, depth-stacked nesting, fixed 5s viewport, vertical scroll only). It also landed **frame-anchored viewport** primitives ahead of plan — see Codebase Verification below.
- **Phase 5 (this)** adds horizontal interactivity: pan, zoom, selection, search, and minimap.
- **Phase 6 (not yet scoped)** will add CPU sampling via `getCpuSamples`.

## Codebase Verification (2026-05-20)

Re-research of the post-Phase-4 codebase surfaced several drift points that update assumptions baked into the original Phase 5 plan. Each is folded into the relevant section below.

| # | Drift Point | Phase 5 Impact |
|---|-------------|----------------|
| 1 | `PerformanceState` already has `committed_frame_anchor: Option<u64>`, `frame_anchor_generation: u64`, `frame_anchor_map: BTreeMap<u64, (u64, u64)>` (Phase 4 landed frame-anchored viewport ahead of schedule, commit `2b65891`). | T01 must **not** re-declare these. T01's pan/zoom math composes with the frame anchor, not replaces it. |
| 2 | The active viewport function is `viewport::compute_frame_anchored_viewport(frame_anchor_map, frame_number) -> Option<(u64, u64)>` — **frame-anchored**, not live-edge. The Phase-4-planned `compute_viewport(tracks) -> (u64, u64)` exists but is `#[allow(dead_code)]` (deprecated). | T01's `compute_viewport` extension must compose three modes: (a) manual viewport when `!follow_latest`, (b) frame-anchored when `committed_frame_anchor.is_some()`, (c) live-edge fallback. |
| 3 | `Left` / `Right` are unconditionally consumed by `SelectPerformanceFrame` in the in-Performance branch — they fire regardless of focused section or details tab. | T01's pan and T03's selection-nav must add a guard `focused_section == Details && details_tab == TimelineEvents`. Frame Chart frame-cycling must still work elsewhere. |
| 4 | `End` is already bound to `PerfJumpToEnd` in the `in_performance` block. `Home` is bound to `PerfJumpToStart`. | T01's "follow latest" cannot use `End` without a tab-specific guard placed **before** the existing arm. Prefer `g` as the primary key, `End` as a guarded alias. |
| 5 | `n` is `SwitchDevToolsPanel(Network)` at top-level DevTools scope. | T04 already plans the fallthrough pattern; confirm guard `if perf.timeline_search_query.is_some()` fires **before** the global `n` arm. |
| 6 | `Up` / `Down` (and `j`/`k`) on Performance currently dispatch `PerfScrollUp` / `PerfScrollDown`. | T03's depth-navigation arms (`↑`/`↓` when selection active) must be ordered **before** the existing scroll arms and gated by `has_selection`. |
| 7 | `gantt.rs` is **1078 lines** with inline tests. Phase 5 adds selection overlay (T03) and search highlight (T04) directly to this file. | Plan a pre-flight test extraction (`gantt_tests.rs`) **inside T01** to keep `gantt.rs` under the ~1300-line ceiling after T03/T04. |
| 8 | The Phase 4 plan called the metadata side-channel type `MetadataEvent`; it landed as `ThreadMetadata` and `TimelinePhase` was **not** extended with a `Metadata` variant. | Cosmetic only — Phase 5 task files reference Phase 4 outputs accurately if they say `ThreadMetadata`. |
| 9 | `widgets/modal_overlay` helpers (`centered_rect`, `centered_rect_percent`, `dim_background`, `render_shadow`, `clear_area`) are all `pub` and ready for T03's popup. | No change — confirms T03's popup plan is viable as written. |
| 10 | Timeline event buffer default is now `10_000` (commit `26aba6a`), but the doc comment on `timeline_tracks` and `docs/CONFIGURATION.md` still say `1_000`. | T05 doc task picks up the stale doc-string fix. |

The numbered table above is referenced by the affected task files via the marker `[Drift #N]`.

## Why Phase 5

The Phase-4 Gantt is observation-only — users can see events fly by but can't:

- **Hold the viewport still** to study a specific 100 ms window (no pan/zoom)
- **See where they are** in the overall event history (no minimap)
- **Inspect an individual event's args, full name, parent chain** (no selection)
- **Find a specific event by name** in a busy timeline (no search)

Phase 5 closes each gap with a focused task per feature.

## Findings & Constraints

### Keybinding context (verified 2026-05-20 against `handler/keys.rs`)

The Performance Details panel currently uses these keys. **Conflict-resolution strategy** for each is listed in the right column.

| Key | Current Binding (verified) | Phase 5 Strategy |
|-----|----------------------------|------------------|
| `]` / `[` | `PerfCycleDetailsTab` (forward/back) when Details focused | Keep — Phase 5 does not touch tab cycling. |
| `Tab` / `BackTab` | `PerfFocusSection` (cycle FrameChart ↔ Details) | Keep. |
| `f` | `TimelineEventsCycleFilter` when on TimelineEvents tab; else falls through | Keep — `T` is **not** the filter key (the plan's earlier table was wrong); `f` is. Phase 5 preserves it. |
| `R` | `ToggleRebuildStats` (RebuildStats tab) / `HotRestart` otherwise | Keep. |
| `T` | `ShowTagFilter` in Normal mode; **not wired in DevTools** | Available — reserve for future use; Phase 5 doesn't claim it. |
| `j` / `Down` | `PerfScrollDown` (chart or details scroll) | **T03 must insert selection-nav arm before this** when `selected_event.is_some()`. |
| `k` / `Up` | `PerfScrollUp` | Same guard pattern as `j`/`Down`. |
| `←` / `Right` | **Global in `in_performance`:** `SelectPerformanceFrame` (frame-cycle prev/next) — fires regardless of focused section or details tab | **Drift #3 — Critical conflict.** T01 must add a tab guard `if focused_section == Details && details_tab == TimelineEvents` placed **before** the existing arm; on TimelineEvents tab Left/Right pan (no selection) or move-sibling (with selection). |
| `End` | `PerfJumpToEnd` in the `in_performance` block | **Drift #4.** T01's "follow latest" prefers `g`; `End` only as a guarded alias on the TimelineEvents tab. |
| `Home` | `PerfJumpToStart` | Reserved — Phase 5 does not claim. |
| `n` | `SwitchDevToolsPanel(Network)` at DevTools scope | **Drift #5.** T04 fallthrough: `n`/`N` on TimelineEvents tab only when `search_query.is_some()`; otherwise falls through. |
| `PageUp` / `PageDown` | `PerfPageUp` / `PerfPageDown` | Reserved. |

Phase 5 introduces these new keys on the Timeline Events tab (subject to the guards in the right column above):

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

**Already on the struct as of Phase 4** (Drift #1 — do not redeclare):

```rust
pub committed_frame_anchor: Option<u64>,           // selected frame anchor (FrameChart)
pub frame_anchor_generation: u64,                  // versioning for anchor change detection
pub frame_anchor_map: BTreeMap<u64, (u64, u64)>,   // frame_number → (vm_ts_start, vm_ts_end)
```

**New in Phase 5:**

```rust
pub timeline_viewport_start_micros: u64,   // pinned manually if !follow_latest
pub timeline_viewport_width_micros: u64,   // default = TIMELINE_VIEWPORT_MICROS (5s)
pub timeline_follow_latest: bool,          // true = auto-scroll forward (default)
pub timeline_selected_event: Option<TimelineEventCursor>,  // None = no selection
pub timeline_details_popup_open: bool,
pub timeline_search_query: Option<String>,
pub timeline_search_input_active: bool,    // distinguishes "typing" from "committed query"
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
├── gantt.rs        # Phase 4 (1078 lines — see Drift #7)
├── gantt_tests.rs  # NEW Phase 5 T01 — extracted inline tests; keeps gantt.rs under ceiling
├── palette.rs      # Phase 4
├── viewport.rs     # Phase 4, extended in Phase 5 (manual + frame-anchored + live-edge composition)
├── minimap.rs      # NEW Phase 5 (T02)
├── popup.rs        # NEW Phase 5 (T03)
├── search.rs       # NEW Phase 5 (T04)
└── (no tests.rs — Phase 4 absorbed it inline)
```

Each Phase 5 task adds its own file (avoiding write overlap on existing Phase 4 files where possible) and touches `mod.rs` to compose its output. **T01 includes a test-extraction pre-step** for `gantt.rs` (Drift #7) so that T03 and T04's overlay-rendering additions don't push it past the line-budget ceiling.

## Design Decisions

### D1 — Viewport state lives in `PerformanceState`, not in widget

Pan/zoom is **TEA-managed state**, not widget-local. Keybindings dispatch messages; the handler updates `timeline_viewport_*`; the renderer is a pure function of state. Follows the existing CODE_STANDARDS Principle 3 model (render-hint Cells for read-back, but state proper for user-driven mutation).

### D2 — Three viewport modes, resolved in priority order (Drift #2)

The renderer's `compute_viewport` composes three sources, top-priority first:

1. **Manual viewport** (`!follow_latest`) → `(viewport_start_micros, viewport_start_micros + viewport_width_micros)`. Engaged by pan/zoom; reset by `g`/`End`.
2. **Frame-anchored viewport** (`follow_latest && committed_frame_anchor.is_some()`) → `compute_frame_anchored_viewport(frame_anchor_map, frame_number)`. Engaged when the user selects a frame in the Frame Chart; survives across panel switches until the frame ages out of `frame_anchor_map`.
3. **Live-edge viewport** (`follow_latest && committed_frame_anchor.is_none()`) → latest `TIMELINE_VIEWPORT_MICROS` of events from `timeline_tracks`. Default cold-start state.

When the user pans/zooms while frame-anchored, the manual viewport takes precedence — the frame anchor is preserved (so `g`/`End` returns to the frame view rather than live-edge) but visually overridden. Pressing `g`/`End` clears manual override; if a frame anchor is set, the renderer returns to mode 2, otherwise mode 3.

### D2.1 — Auto-scroll-forward vs. manual pan: a single boolean

`timeline_follow_latest: bool`. While `true`, the renderer uses mode 2 or mode 3 per D2. When the user pans/zooms manually, set to `false`. Pressing `g` (primary) or `End` (guarded alias on TimelineEvents tab — Drift #4) resets to `true`.

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
  └─ T01 timeline-viewport-pan-zoom         (a) extract gantt.rs tests → gantt_tests.rs (Drift #7)
                                            (b) state machine + keybindings + viewport math
                                            (c) compute_active_viewport composer (3-mode, Drift #2)
                                            (d) conflict-guarded key arms (Drift #3, #4)
                                            files: state, handler/keys.rs, handler/devtools/perf,
                                                   viewport.rs, gantt.rs, gantt_tests.rs

Wave 2 (mixed; see overlap matrix in TASKS.md)
  ├─ T02 timeline-minimap-ribbon            NEW minimap.rs + mod.rs composition slot
  ├─ T03 timeline-event-selection-and-details  state, NEW popup.rs, handler, gantt.rs (selection
  │      (sequential before T04)               highlight), gantt_tests.rs, Drift #6 ordering
  └─ T04 timeline-search-filter             state, NEW search.rs, handler, gantt.rs (match highlight),
         (sequential after T03)             gantt_tests.rs, Drift #5 ordering

Wave 3 (sequential, doc_maintainer)
  └─ T05 update-arch-and-review-focus-docs  docs/ARCHITECTURE.md, docs/REVIEW_FOCUS.md,
                                            + Drift #10 doc-string fix in PerformanceState
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
