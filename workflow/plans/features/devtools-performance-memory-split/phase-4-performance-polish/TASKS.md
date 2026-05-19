# Phase 4 — Performance Tab Polish — Task Index

## Overview

Six tasks address three frame-chart bugs (bundled) and the Timeline Events Gantt rewrite. See [`PLAN.md`](PLAN.md) for the rationale.

- **Wave 1 (parallel × 3 worktrees):** T01 frame-chart fixes (fdemon-tui), T02 parser B/E pairing (fdemon-core), T03 immediate fetch-on-unpause (fdemon-app actions). Disjoint crates → no write overlap.
- **Wave 2 (sequential after T02):** T04 state migration to thread-grouped tree.
- **Wave 3 (sequential after T04):** T05 Gantt timeline widget (new subdirectory).
- **Wave 4 (sequential after all impl):** T06 doc updates via `doc_maintainer`.

**Total Tasks:** 6
**Estimated Hours:** 16–24 hours

## Task Dependency Graph

```
Wave 1 (parallel)
┌──────────────────────────────────────┐ ┌──────────────────────────────────┐ ┌──────────────────────────────────┐
│ 01 frame-chart-fixes                 │ │ 02 timeline-parser-be-pairing    │ │ 03 immediate-timeline-fetch-     │
│   (complaints 1+2+3)                 │ │   (B/E pairs → durations,        │ │    on-unpause                    │
│   bars.rs + frame.rs                 │ │    tree per tid)                 │ │   actions/performance.rs         │
└──────────────────────────────────────┘ │   fdemon-core/timeline.rs        │ │                                  │
                                         └────────────────┬─────────────────┘ └──────────────────────────────────┘
                                                          │
Wave 2 (sequential after T02)                             ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 04 timeline-state-thread-grouped-tree                                       │
│   session/performance.rs + handler/devtools/performance/timeline.rs +       │
│   wire up timeline_thread_name_map                                          │
└────────────────────────────────┬────────────────────────────────────────────┘
                                 │
Wave 3 (sequential after T04)    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 05 gantt-timeline-widget                                                    │
│   widgets/devtools/performance/details/timeline_events/ (split file into    │
│   subdirectory: mod.rs, gantt.rs, palette.rs, viewport.rs, tests.rs)        │
└────────────────────────────────┬────────────────────────────────────────────┘
                                 │
Wave 4 (sequential, doc_maintainer)
                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 06 update-arch-and-review-focus-docs                                        │
│   docs/ARCHITECTURE.md + docs/REVIEW_FOCUS.md   [doc_maintainer]            │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [frame-chart-fixes](tasks/01-frame-chart-fixes.md) | Not Started | — | 3–4h | implementor | 1 |
| 02 | [timeline-parser-be-pairing](tasks/02-timeline-parser-be-pairing.md) | Not Started | — | 2–3h | implementor | 1 |
| 03 | [immediate-timeline-fetch-on-unpause](tasks/03-immediate-timeline-fetch-on-unpause.md) | Not Started | — | 1–2h | implementor | 1 |
| 04 | [timeline-state-thread-grouped-tree](tasks/04-timeline-state-thread-grouped-tree.md) | Not Started | 02 | 3–5h | implementor | 2 |
| 05 | [gantt-timeline-widget](tasks/05-gantt-timeline-widget.md) | Not Started | 04 | 5–8h | implementor | 3 |
| 06 | [update-arch-and-review-focus-docs](tasks/06-update-arch-and-review-focus-docs.md) | Not Started | 01,02,04,05 | 2h | doc_maintainer | 4 |

## File Overlap Analysis

> The orchestrator uses this section to decide isolation strategy per wave. Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** frame-chart-fixes | `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` (Bug 1: clamp `ms_to_half_blocks` to min-1 for nonzero ms; Bug 2: replace single-char `▔` overlay with full-column selection highlight; Bug 3: change `compute_visible_range` to use `scroll_offset` as authoritative viewport anchor rather than selection-anchored), `crates/fdemon-app/src/handler/devtools/performance/frame.rs` (Bug 3: stop resetting `frame_chart_scroll_offset = 0` in `handle_select_performance_frame`; add viewport-edge-aware scroll adjustment so selection stays visible without snapping to right edge) | `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` (read-only — orchestration), `crates/fdemon-app/src/session/performance.rs` (verify field names: `frame_chart_scroll_offset`, `frame_chart_visible_width`, `selected_frame`) |
| **02** timeline-parser-be-pairing | `crates/fdemon-core/src/timeline.rs` (NEW types: `TimelineTrack { tid: i64, name: Option<String>, root_events: Vec<TimelineNode> }`, `TimelineNode { name: String, ts: i64, dur: Option<i64>, phase: TimelinePhase, children: Vec<TimelineNode>, category: Option<String> }`; NEW function `pair_be_events(events: &[TimelineEvent]) -> Vec<TimelineNode>` that stack-builds a tree per `tid`; STOP filtering `ph:"M"` metadata events — surface them as a separate `metadata: Vec<MetadataEvent>` return from `parse_vm_timeline` OR add `TimelinePhase::Metadata` variant; existing `parse_vm_timeline` signature stays compatible — add a sibling `parse_vm_timeline_with_metadata` that returns both events + metadata) | `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (read-only — verify consumers will absorb the additive types) |
| **03** immediate-timeline-fetch-on-unpause | `crates/fdemon-app/src/actions/performance.rs` (in `spawn_timeline_polling`, restructure the main loop using `tokio::select!` similar to `spawn_performance_polling` at lines 328–385 — branches: pause-rx.changed, shutdown-rx.changed, tick.next. On `pause_rx.changed -> false`, immediately run one `fetch_timeline_chunk` cycle before entering the tick loop; update existing pause/resume tests to cover the immediate-fetch behavior using the `VmRequestApi` mock from T11) | `crates/fdemon-daemon/src/vm_service/timeline.rs` (read-only — `fetch_timeline_chunk` unchanged), `crates/fdemon-daemon/src/vm_service/request_api.rs` (read-only — `VmRequestApi` trait) |
| **04** timeline-state-thread-grouped-tree | `crates/fdemon-app/src/session/performance.rs` (REPLACE `timeline_events: VecDeque<TimelineEvent>` with `timeline_tracks: BTreeMap<i64, TimelineTrack>`; KEEP `timeline_thread_name_map: HashMap<i64, String>` and **start writing to it** from metadata events; REMOVE `timeline_events_scroll_offset: usize` since the Gantt has thread-row scrolling, not event-line scrolling — replace with `timeline_thread_scroll_offset: usize`; KEEP `timeline_events_filter: TimelineFilter`; update `Default::default()` + all unit tests), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (rewrite `handle_batch` to: call `pair_be_events` per `tid`, merge into existing `TimelineTrack::root_events` honoring buffer cap; populate `timeline_thread_name_map` from batch metadata; update existing tests; preserve `T`-key filter logic), `crates/fdemon-app/src/handler/devtools/mod.rs` (the `timeline_events.clear()` call in `handle_exit_devtools_mode` and `handle_switch_panel` Performance-leave branch becomes `timeline_tracks.clear()` + `timeline_thread_name_map.clear()` + `timeline_thread_scroll_offset = 0`), `crates/fdemon-app/src/message.rs` (extend `TimelineEventsBatchReceived { session_id, events }` to also carry `metadata: Vec<MetadataEvent>` — additive on the variant) | T02 outputs (`TimelineTrack`, `TimelineNode`, `pair_be_events`, `MetadataEvent` types), `crates/fdemon-app/src/actions/performance.rs` (read-only — verify the polling task forwards metadata) |
| **05** gantt-timeline-widget | `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/` (NEW SUBDIRECTORY replacing the single `timeline_events_tab.rs` file): `mod.rs` (public `render(area, buf, state)` entry point + thread-filter strip preserved from existing), `gantt.rs` (the core renderer: thread-row layout, depth-stacked colored bars, time axis), `palette.rs` (color constants per `TimelinePhase` / `TimelineThread`: UI=blue, Raster=darker blue, Other=purple; uses ratatui `Color::Indexed` for 256-color terminals with fallback to basic 16), `viewport.rs` (helpers: `compute_viewport(now_micros, viewport_micros) -> (start, end)`, `micros_to_column(ts, start, end, width) -> u16`, `clip_bar_to_viewport(node, start, end, width)`), `tests.rs` (unit tests for layout, color mapping, viewport clipping, thread filter, vertical scroll, render-hint write-back, zero-area no-panic), `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` (replace `mod timeline_events_tab;` with `mod timeline_events;`; update dispatch from `timeline_events_tab::render(...)` to `timeline_events::render(...)`), `crates/fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` (read-only — reuse `truncate_with_ellipsis` for bar labels and thread name truncation) | T04 outputs (`TimelineTrack`, `TimelineNode`, `timeline_thread_name_map`, `timeline_thread_scroll_offset`), `crates/fdemon-tui/src/widgets/devtools/mod.rs` (read-only — `T`-key filter footer hint) |
| **06** update-arch-and-review-focus-docs | `docs/ARCHITECTURE.md` ("DevTools Subsystem → Performance Panel" section: document the new `TimelineTrack`/`TimelineNode` tree model, the B/E pairing algorithm, the Gantt widget's thread-row + depth-stacked rendering, the immediate-fetch-on-unpause path, the frame-chart selection-within-viewport behavior, the `timeline_thread_name_map` wiring), `docs/REVIEW_FOCUS.md` (new "Approved Patterns" entries: Gantt depth-stack rendering, thread-row scroll, full-column selection overlay; document that pan/zoom + minimap are deferred to Phase 5) | T01–T05 completion summaries |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | None | 1 | **Parallel (worktree)** — different crates (fdemon-tui vs fdemon-core). |
| 01 + 03 | None | 1 | **Parallel (worktree)** — different files (frame_chart vs actions/performance). |
| 02 + 03 | None | 1 | **Parallel (worktree)** — different crates (fdemon-core vs fdemon-app). |
| 02 + 04 | None | — | **Sequential by dependency** — T04 consumes T02's `TimelineTrack`/`TimelineNode`/`pair_be_events` types. |
| 04 + 05 | None | — | **Sequential by dependency** — T05 consumes T04's `timeline_tracks` state shape and `timeline_thread_name_map` wiring. |
| 05 + 06 | None | — | T06 is docs-only, runs after all impl tasks. |

## Success Criteria

Phase 4 is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Complaint 1 verified:** Run fdemon in a 80×8 terminal with fast frames (~1–3 ms). All bars visible — no missing columns. Resize to 80×4 — bars degrade gracefully but no zeros.
- [ ] **Complaint 2 verified:** Selected bar shows a clear full-column highlight (e.g., distinct `▏`/`▕` side markers across all rows, or a different background color spanning the column). User reports it is "impossible to miss."
- [ ] **Complaint 3 verified:** With frames 100–130 in viewport and selection at 130, pressing Left moves selection to 129 without scrolling the chart. Pressing Left until selection reaches frame 100 (leftmost visible), then one more Left scrolls the viewport left to reveal frame 99 with selection on it. Test: `test_left_within_viewport_does_not_scroll` and `test_left_at_left_edge_scrolls`.
- [ ] **Complaint 4a verified:** On Performance-panel-enter, timeline events appear within ~150 ms (well under the previous 1 s placeholder window). Tail fdemon log: `timeline immediate fetch on unpause` debug entry visible.
- [ ] **Complaint 4b verified:** Tail `flutter run` on a non-trivial app. Timeline Events tab shows thread rows with labeled names (`io.flutter.raster …`, `io.flutter.ui …`, `DartWorker …`). Each row has colored event bars across a ~5 s viewport. Raster bars distinguishable from UI bars by color. Nested child events stack visually.
- [ ] **Thread filter preserved:** `T` cycles through `All → UI → Raster → All` and the visible thread rows change accordingly. No regression vs Phase 3.
- [ ] **Vertical scroll:** With more thread rows than fit on screen, `↑/↓` scrolls between rows. Render-hint write-back keeps selection visible.
- [ ] **doc updates verified:** `docs/ARCHITECTURE.md` documents the Gantt model, `TimelineTrack`/`TimelineNode`, the panel-leave clear-on-tracks update, and the frame-chart viewport behavior. `docs/REVIEW_FOCUS.md` reflects new patterns and explicitly notes pan/zoom + minimap deferred to Phase 5.

## Phase Acceptance Test Plan

After all 6 tasks merge, run the manual smoke sequence:

1. `cargo run -- ~/Dev/some-flutter-app` in a 200×30 iTerm split. Wait for attach.
2. **Frame chart bar height (Complaint 1):** Resize the iTerm window to ~80×8. Inspect Performance frame chart — every frame bar is visible (no missing columns). Resize to 80×4 — bars degrade gracefully.
3. **Frame chart selection (Complaint 2):** Press `p` (Performance) → wait for frames → press `]` then `[` to focus the chart. Press Left/Right repeatedly — the selected bar shows a clear full-column highlight.
4. **Frame chart scroll (Complaint 3):** Scroll chart to frames 100–130 (selection at 130). Press Left repeatedly. Selection moves through visible bars; chart does not scroll until selection reaches frame 100. One more Left scrolls viewport left.
5. **Timeline cold start (Complaint 4a):** Press Esc back to Logs, then re-enter Performance (`p` → `]` → `]`). First timeline events visible within one redraw — no extended "Waiting for timeline events…" placeholder.
6. **Timeline Gantt (Complaint 4b):** Verify thread rows are labeled (`io.flutter.raster 45067`, `io.flutter.ui …`). Colored event bars span the recent ~5 s. Nested events render as stacked depth bars within their parent.
7. **Thread filter:** Press `T` — cycles `All → UI → Raster → All`. Only matching thread rows visible in `UI`/`Raster` modes.
8. **Vertical scroll:** If thread count exceeds visible rows, `↑/↓` scrolls. Selection (if implemented in T05) stays visible.

## Notes

- **T01 bundles three sub-bugs** because all three live in `frame_chart/bars.rs` + `frame.rs`; splitting would force three sequential merges into the same file and a more complex orchestration. Three sub-acceptance criteria within T01 keep the diff coherent and reviewable.
- **T04 is a breaking state-shape change** but completely contained — only the polling-task → handler → widget pair touches `timeline_events`. No MCP, headless, or service code consumes it.
- **T04 wires up `timeline_thread_name_map`** which is currently dead. The map gets populated from metadata events the polling task observes; the daemon side may need to forward `TrackDescriptor` events through. See T04 task file for the implementation choice.
- **T05 splits `timeline_events_tab.rs` into a subdirectory** because the Gantt rendering crosses several concerns (layout, palette, viewport math, rendering loop). Following the same module-decomposition pattern Phase 3-followup used for `text_helpers.rs`.
- **Phase 5 (deferred) will add:** pan/zoom keys (`+`/`-` or `[`/`]` within Gantt context), a minimap ribbon, event-level selection with a details popup, and search/filter by event name.
