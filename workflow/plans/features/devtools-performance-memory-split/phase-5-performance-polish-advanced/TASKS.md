# Phase 5 — Performance Tab Advanced — Task Index

## Overview

Five tasks add interactive features to the Phase 4 Gantt timeline. See [`PLAN.md`](PLAN.md) for the rationale, design decisions, and the **Codebase Verification (2026-05-20)** drift table referenced from individual task files.

- **Wave 1 (sequential, foundational):** T01 viewport state machine + pan/zoom keys + auto-scroll toggle. **Includes pre-flight extraction of `gantt.rs` inline tests to `gantt_tests.rs`** (Drift #7) so T03/T04 overlay additions stay under the file-length ceiling.
- **Wave 2 (mixed):** T02 minimap (parallel-safe), T03 selection+popup and T04 search (sequential — both modify `gantt.rs` overlays and the timeline handler).
- **Wave 3 (sequential, doc_maintainer):** T05 doc updates (includes fixing the stale `timeline_tracks` doc-string `default 1000` → `default 10_000`, Drift #10).

**Total Tasks:** 5
**Estimated Hours:** 18–26 hours (T01 +1h for test extraction)

## Task Dependency Graph

```
Wave 1 (sequential)
┌──────────────────────────────────────────────────────────────────────────────┐
│ 01 timeline-viewport-pan-zoom                                                │
│   state.rs (viewport fields, follow_latest), handler/keys.rs (new arms),     │
│   handler/devtools/performance/timeline.rs (handlers), viewport.rs (zoom     │
│   math), gantt.rs (consume manual viewport vs auto)                          │
└──────────────────────────────┬───────────────────────────────────────────────┘
                               │
Wave 2 (T02 parallel; T03 + T04 sequential — gantt.rs + handler write overlap)
                               ▼
┌──────────────────────────────────────┐ ┌─────────────────────────────────────┐
│ 02 timeline-minimap-ribbon           │ │ 03 timeline-event-selection-and-    │
│   NEW minimap.rs, mod.rs composition │ │    details (cursor + popup overlay) │
│                                      │ │   state, NEW popup.rs, handler,     │
│                                      │ │   gantt.rs selection highlight      │
│                                      │ └──────────────────┬──────────────────┘
│                                      │                    │
│                                      │ ┌──────────────────▼──────────────────┐
│                                      │ │ 04 timeline-search-filter           │
│                                      │ │   state, NEW search.rs, handler,    │
│                                      │ │   gantt.rs match highlight          │
└──────────────────────────────────────┘ └─────────────────────────────────────┘
                               │
Wave 3 (doc_maintainer)
                               ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ 05 update-arch-and-review-focus-docs                                         │
│   docs/ARCHITECTURE.md + docs/REVIEW_FOCUS.md   [doc_maintainer]             │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [timeline-viewport-pan-zoom](tasks/01-timeline-viewport-pan-zoom.md) | Done ✅ | Phase 4 complete | 5–7h | implementor | 1 |
| 02 | [timeline-minimap-ribbon](tasks/02-timeline-minimap-ribbon.md) | Done ✅ | 01 | 3–4h | implementor | 2 |
| 03 | [timeline-event-selection-and-details](tasks/03-timeline-event-selection-and-details.md) | Done ⚠️ CONCERN | 01 | 5–7h | implementor | 2 |
| 04 | [timeline-search-filter](tasks/04-timeline-search-filter.md) | Done ✅ | 01, 03 | 3–5h | implementor | 2 |
| 05 | [update-arch-and-review-focus-docs](tasks/05-update-arch-and-review-focus-docs.md) | Done ✅ | 01,02,03,04 | 2h | doc_maintainer | 3 |

## File Overlap Analysis

> Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** timeline-viewport-pan-zoom | `crates/fdemon-app/src/session/performance.rs` (new fields ONLY: `timeline_viewport_start_micros: u64`, `timeline_viewport_width_micros: u64`, `timeline_follow_latest: bool` — **do NOT redeclare `committed_frame_anchor`, `frame_anchor_generation`, `frame_anchor_map`; Phase 4 landed them**, Drift #1; new constants for default viewport width and zoom factors), `crates/fdemon-app/src/handler/keys.rs` (add arms in DevTools/Performance/Details/TimelineEvents context for `+`/`-` zoom, `←`/`→` pan **with tab guard placed BEFORE the existing global `SelectPerformanceFrame` arm**, Drift #3; `g` (primary) and `End` (guarded alias placed BEFORE `PerfJumpToEnd`) for follow-latest reset, Drift #4), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (new handlers `handle_timeline_zoom_in`, `handle_timeline_zoom_out`, `handle_timeline_pan_left`, `handle_timeline_pan_right`, `handle_timeline_follow_latest`), `crates/fdemon-app/src/message.rs` (new `Message` variants), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/viewport.rs` (**add new `compute_active_viewport(perf: &PerformanceState) -> (u64, u64)` that composes 3 modes per PLAN D2: manual / frame-anchored via existing `compute_frame_anchored_viewport` / live-edge**, Drift #2; add `pan_viewport(start, width, direction, factor)` and `zoom_viewport(width, factor, anchor)` pure helpers; keep deprecated `compute_viewport(tracks)` for now), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` (call `compute_active_viewport` instead of any direct viewport math; render a small "PAUSED" indicator when `!follow_latest`), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` (**NEW — extract inline tests from `gantt.rs` to keep it under ~800 lines before T03/T04 add overlays**, Drift #7) | `crates/fdemon-app/src/state.rs` (verify Message dispatch table), Phase 4 outputs (all of `timeline_events/` subdirectory) |
| **02** timeline-minimap-ribbon | `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/minimap.rs` (NEW file: `render(area, buf, tracks, viewport_start, viewport_end, full_history_start, full_history_end)`; per-column dominant-thread color computation; viewport bracket overlay; tests for empty, single-thread, multi-thread cases; clip-and-truncate behavior at small widths), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (declare `pub(super) mod minimap;`; insert `Layout::vertical` constraint `Constraint::Length(MINIMAP_HEIGHT)` above the time axis; call `minimap::render(...)`), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/palette.rs` (read-only — reuse `bar_color` for minimap pixel coloring) | T01 outputs (viewport state — minimap reads to determine the bracket overlay position) |
| **03** timeline-event-selection-and-details | `crates/fdemon-app/src/session/performance.rs` (new fields: `timeline_selected_event: Option<TimelineEventCursor>`, `timeline_details_popup_open: bool`; new type `TimelineEventCursor { tid, depth, ts }`), `crates/fdemon-app/src/handler/keys.rs` (new arms — **all placed BEFORE the existing scroll arms (`j`/`k`/`Up`/`Down` → `PerfScrollUp/Down`) when `has_selection`**, Drift #6: `Enter` when on TimelineEvents tab opens selection / opens popup; `←`/`→` (already tab-guarded by T01) refine to move selection within row when active, pan when not; `↑`/`↓` traverse depth/threads when active, fall through to scroll otherwise; `Esc` closes popup then clears selection then exits DevTools per existing fallthrough chain), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (new handlers: `handle_timeline_select_first_visible`, `handle_timeline_move_selection`, `handle_timeline_open_popup`, `handle_timeline_close_popup`, `handle_timeline_clear_selection`; selection navigation traverses the per-thread tree by (depth, ts); **auto-pan via `compute_active_viewport`-aware helper, sets `follow_latest = false`**), `crates/fdemon-app/src/message.rs` (new Message variants for selection navigation, popup open/close), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/popup.rs` (NEW: modal overlay widget rendering full event name, category, ts, dur (μs + human-readable), thread label, parent chain breadcrumb, child count; uses `widgets/modal_overlay` helpers from `crates/fdemon-tui/src/widgets/modal_overlay.rs` — confirmed available per Drift #9; click-outside-to-close via mouse region), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` (selection-overlay render: highlight the selected bar with a distinct border/inverted color; ensure overlay does not double-render or bleed), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` (extend test suite extracted in T01 with selection-overlay tests), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (declare `pub(super) mod popup;`; conditionally render popup last so it overlays Gantt) | T01 outputs (viewport state machine + `compute_active_viewport` — selection nav must auto-pan to keep selection visible), Phase 4 outputs (`TimelineTrack`, `TimelineNode` for tree traversal) |
| **04** timeline-search-filter | `crates/fdemon-app/src/session/performance.rs` (new fields: `timeline_search_query: Option<String>`, `timeline_search_input_active: bool`, `timeline_search_match_cursor: usize`), `crates/fdemon-app/src/handler/keys.rs` (new arms — **`n`/`N` tab-guarded arm placed BEFORE the existing global `n` → `SwitchDevToolsPanel(Network)` arm**, Drift #5, with internal guard `if perf.timeline_search_query.is_some()` so non-search `n` still falls through to Network: `/` opens search input on TimelineEvents tab; `n`/`N` jump to next/prev match when search is non-empty; while `timeline_search_input_active`, char keys append to query, `Backspace` deletes, `Enter` confirms and closes input, `Esc` clears query and closes), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (new handlers: `handle_timeline_search_open`, `handle_timeline_search_input`, `handle_timeline_search_close`, `handle_timeline_search_jump_to_match`; match collection iterates all tracks/nodes filtering by `name.contains(query)`, sorts by ts, navigation cycles), `crates/fdemon-app/src/handler/devtools/mod.rs` (extend the existing Performance-leave clear list to also clear `timeline_search_query`, `timeline_search_input_active`, `timeline_search_match_cursor`), `crates/fdemon-app/src/message.rs` (new Message variants), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/search.rs` (NEW: input-mode bar at the top of the canvas showing `/<query>_`, match count `(3/12)`, hotkeys hint), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` (match-overlay render: brighten/border bars whose name contains the query case-insensitively), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` (extend test suite from T01 with match-overlay tests), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (declare `pub(super) mod search;`; insert search bar above filter strip when input is active OR a query is set) | T01 outputs (viewport state — `n`/`N` pans viewport to center on match), Phase 4 outputs, T03 outputs (selection cursor — `n`/`N` selects-and-pans in one action) |
| **05** update-arch-and-review-focus-docs | `docs/ARCHITECTURE.md` (DevTools Subsystem → Performance Panel section: document the **three-mode viewport composition** (manual / frame-anchored / live-edge) per PLAN D2, pan/zoom math, minimap rendering pipeline, selection cursor and details popup modal, search-and-jump UX; cross-reference Phase 4's Gantt baseline and the existing `compute_frame_anchored_viewport`), `docs/REVIEW_FOCUS.md` (new approved patterns: viewport state in PerformanceState (not widget-local), three-mode viewport priority order, selection cursor by `(tid, depth, ts)` not by index, search-as-highlight not filter, minimap dominant-thread coloring; document that CPU sampling is deferred to Phase 6), `crates/fdemon-app/src/session/performance.rs` (**doc-string fix only**: update `timeline_tracks` comment "default 1000" → "default 10_000" matching the actual `default_timeline_event_buffer_size()` returned by `config/types.rs`, Drift #10), `docs/CONFIGURATION.md` (if it mentions the 1000 default for `performance.timeline_event_buffer_size`, update to 10000) | T01–T04 completion summaries; `crates/fdemon-app/src/config/types.rs` (read-only — confirm `default_timeline_event_buffer_size` returns 10_000) |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | None | — | **Sequential by dependency** — T02 reads T01's viewport state. |
| 01 + 03 | None | — | **Sequential by dependency** — T03 reads T01's viewport state. |
| 01 + 04 | None | — | **Sequential by dependency** — T04 reads T01's viewport state. |
| 02 + 03 | None | 2 | **Parallel (worktree)** — T02 owns `minimap.rs`; T03 owns `popup.rs` and `gantt.rs` selection overlay. The `mod.rs` lines T02 adds (declare `pub(super) mod minimap;` + insert minimap row above time axis) are disjoint from T03's lines (declare `pub(super) mod popup;` + conditional popup overlay at end of render). Auto-merge expected to succeed. |
| 02 + 04 | None | 2 | **Parallel (worktree)** — same reasoning; T02 and T04 touch disjoint sections of `mod.rs`. |
| 03 + 04 | `crates/fdemon-tui/.../timeline_events/gantt.rs`, `crates/fdemon-tui/.../timeline_events/gantt_tests.rs`, `crates/fdemon-tui/.../timeline_events/mod.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/devtools/performance/timeline.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/session/performance.rs`, `crates/fdemon-app/src/message.rs` | 2 | **Sequential** — T03 adds selection overlay to `gantt.rs`; T04 adds match-highlight overlay to the same file. Both add tests to `gantt_tests.rs` (extracted by T01), Message variants, and handler arms. Run T03 first (selection is foundational for `n`/`N` "jump-to-match-and-select"), then T04 builds on it. |
| 04 + 05 | None | — | T05 is docs-only. |

## Success Criteria

Phase 5 is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Pan/zoom verified:** `+` zooms in; `-` zooms out; `←`/`→` pans (no selection); viewport range visible in a status indicator (e.g., footer).
- [ ] **Auto-scroll toggle verified:** After manual pan, "PAUSED" indicator shows; new events do not auto-scroll. Pressing `g` (or `End` on TimelineEvents tab) resets to live-follow mode (or to the frame-anchored viewport if a frame is selected).
- [ ] **Conflict guards verified:** `Left`/`Right` on FrameChart focus still selects frames (Drift #3); `End` on FrameChart focus still does `PerfJumpToEnd` (Drift #4); `n` outside TimelineEvents-with-query still opens Network panel (Drift #5); `Up`/`Down` without selection still scrolls (Drift #6).
- [ ] **Minimap verified:** A 1-row strip above the time axis shows compressed event history, with a `[...]` bracket on the current viewport. Bracket moves on pan/zoom.
- [ ] **Selection verified:** `Enter` on TimelineEvents tab selects the first visible event. `←`/`→` traverses siblings; `↑`/`↓` traverses depth. Selected bar has distinct border or inverted color.
- [ ] **Details popup verified:** `Enter` on selected event opens modal with name, ts, dur, thread, parent chain. `Esc` closes.
- [ ] **Search verified:** `/` opens input; query highlights matching bars; `n`/`N` jumps viewport to next/prev match; `Esc` clears query.
- [ ] **Filter still preserved:** `T` cycle still works; query persists across filter changes.
- [ ] **No regression on Phase 4 features:** thread rows + colored bars + depth stacking + thread filter all still work.
- [ ] **Mouse interaction (stretch):** clicking a bar selects it; clicking outside clears selection; clicking the minimap pans the viewport (or no-op if scope tight).
- [ ] **Doc updates verified:** ARCHITECTURE.md documents viewport state machine, selection model, search pipeline. REVIEW_FOCUS.md adds approved-pattern entries.

## Notes

- **T01 is foundational** — every other task reads `timeline_viewport_*` state and depends on T01's `compute_active_viewport` helper. Land T01 fully (validator PASS, merged) before starting T02–T04.
- **T01 includes a test extraction pre-step.** Before adding new viewport math + state + handlers, extract `gantt.rs`'s inline `#[cfg(test)] mod tests` to a sibling `gantt_tests.rs` module (using the workspace's `#[path]` convention, or `mod gantt_tests;` if the project uses external test modules). This is a refactor-only move with no behavior change. Without it, T03 (selection overlay) + T04 (match overlay) will push `gantt.rs` past 1300 lines. See Drift #7 in PLAN.
- **Wave 2 mixed strategy:** T02 (minimap) is genuinely parallel-safe. T03 (selection) and T04 (search) share write files in `gantt.rs`, `gantt_tests.rs`, `handler/keys.rs`, `handler/devtools/performance/timeline.rs`, `message.rs`, `session/performance.rs`. Run T03 first (selection cursor needed for `n`/`N` jump-to-match-and-select), then T04. Dispatch T02 in parallel with T03 in a worktree, then T04 sequentially after.
- **Keybinding conflicts verified 2026-05-20** (see PLAN's Codebase Verification table for details):
  - `Left`/`Right` are globally consumed by `SelectPerformanceFrame` in Performance mode (Drift #3) — T01 must insert its tab-guarded arms **before** the existing global arm.
  - `End` is `PerfJumpToEnd` in the `in_performance` block (Drift #4) — T01 uses `g` as primary follow-latest key; `End` as guarded alias only.
  - `n` is `SwitchDevToolsPanel(Network)` at DevTools scope (Drift #5) — T04 fallthrough guard `if perf.timeline_search_query.is_some()` only.
  - `j`/`k`/`Up`/`Down` are `PerfScrollUp`/`PerfScrollDown` (Drift #6) — T03 selection-nav arms gated by `has_selection` and ordered before scroll arms.
- **State drift caveat (Drift #1):** Phase 4 landed `committed_frame_anchor`, `frame_anchor_generation`, `frame_anchor_map` on `PerformanceState` ahead of plan. T01 must **not** redeclare these — they exist; T01 composes with them via the new `compute_active_viewport` helper (PLAN D2).
- **Phase 6 (deferred):** CPU sampling via `getCpuSamples`, cross-thread async connector lines, per-frame zoom-to-frame coupling, event annotation/pinning, trace export.
