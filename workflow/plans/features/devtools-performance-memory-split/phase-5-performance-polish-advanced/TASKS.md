# Phase 5 — Performance Tab Advanced — Task Index

## Overview

Five tasks add interactive features to the Phase 4 Gantt timeline. See [`PLAN.md`](PLAN.md) for the rationale and design decisions.

- **Wave 1 (sequential, foundational):** T01 viewport state machine + pan/zoom keys + auto-scroll toggle.
- **Wave 2 (mixed):** T02 minimap (parallel-safe), T03 selection+popup and T04 search (sequential — both modify `gantt.rs` overlays and the timeline handler).
- **Wave 3 (sequential, doc_maintainer):** T05 doc updates.

**Total Tasks:** 5
**Estimated Hours:** 17–24 hours

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
| 01 | [timeline-viewport-pan-zoom](tasks/01-timeline-viewport-pan-zoom.md) | Not Started | Phase 4 complete | 4–6h | implementor | 1 |
| 02 | [timeline-minimap-ribbon](tasks/02-timeline-minimap-ribbon.md) | Not Started | 01 | 3–4h | implementor | 2 |
| 03 | [timeline-event-selection-and-details](tasks/03-timeline-event-selection-and-details.md) | Not Started | 01 | 5–7h | implementor | 2 |
| 04 | [timeline-search-filter](tasks/04-timeline-search-filter.md) | Not Started | 01 | 3–5h | implementor | 2 |
| 05 | [update-arch-and-review-focus-docs](tasks/05-update-arch-and-review-focus-docs.md) | Not Started | 01,02,03,04 | 2h | doc_maintainer | 3 |

## File Overlap Analysis

> Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** timeline-viewport-pan-zoom | `crates/fdemon-app/src/session/performance.rs` (new fields: `timeline_viewport_start_micros: u64`, `timeline_viewport_width_micros: u64`, `timeline_follow_latest: bool`; new constants for default viewport width and zoom factors), `crates/fdemon-app/src/handler/keys.rs` (add arms in DevTools/Performance/Details/TimelineEvents context for `+`/`-` zoom, `←`/`→` pan when no selection, `End`/`g` follow-latest reset; verify no conflict with existing tab/frame/network shortcuts), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (new handlers `handle_timeline_zoom_in`, `handle_timeline_zoom_out`, `handle_timeline_pan_left`, `handle_timeline_pan_right`, `handle_timeline_follow_latest`), `crates/fdemon-app/src/message.rs` (new `Message` variants), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/viewport.rs` (extend `compute_viewport` to honor manual viewport when `!follow_latest`; add `pan_viewport(start, width, direction, factor)` and `zoom_viewport(width, factor, anchor)` pure helpers), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` (read new state to compose viewport; render a small "PAUSED" indicator when `!follow_latest`) | `crates/fdemon-app/src/state.rs` (verify Message dispatch table), Phase 4 outputs (all of `timeline_events/` subdirectory) |
| **02** timeline-minimap-ribbon | `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/minimap.rs` (NEW file: `render(area, buf, tracks, viewport_start, viewport_end, full_history_start, full_history_end)`; per-column dominant-thread color computation; viewport bracket overlay; tests for empty, single-thread, multi-thread cases; clip-and-truncate behavior at small widths), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (declare `pub(super) mod minimap;`; insert `Layout::vertical` constraint `Constraint::Length(MINIMAP_HEIGHT)` above the time axis; call `minimap::render(...)`), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/palette.rs` (read-only — reuse `bar_color` for minimap pixel coloring) | T01 outputs (viewport state — minimap reads to determine the bracket overlay position) |
| **03** timeline-event-selection-and-details | `crates/fdemon-app/src/session/performance.rs` (new fields: `timeline_selected_event: Option<TimelineEventCursor>`, `timeline_details_popup_open: bool`; new type `TimelineEventCursor { tid, depth, ts }`), `crates/fdemon-app/src/handler/keys.rs` (new arms: `Enter` when on TimelineEvents tab opens selection / opens popup; `←`/`→` move selection within row when active; `↑`/`↓` traverse depth/threads when active; `Esc` closes popup then clears selection then exits DevTools per existing fallthrough chain), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (new handlers: `handle_timeline_select_first_visible`, `handle_timeline_move_selection`, `handle_timeline_open_popup`, `handle_timeline_close_popup`, `handle_timeline_clear_selection`; selection navigation traverses the per-thread tree by (depth, ts)), `crates/fdemon-app/src/message.rs` (new Message variants for selection navigation, popup open/close), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/popup.rs` (NEW: modal overlay widget rendering full event name, category, ts, dur (μs + human-readable), thread label, parent chain breadcrumb, child count; uses `widgets/modal_overlay` helpers; click-outside-to-close via mouse region), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` (selection-overlay render: highlight the selected bar with a distinct border/inverted color; ensure overlay does not double-render or bleed), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (declare `pub(super) mod popup;`; conditionally render popup last so it overlays Gantt) | T01 outputs (viewport state — selection nav must auto-pan to keep selection visible), Phase 4 outputs (`TimelineTrack`, `TimelineNode` for tree traversal) |
| **04** timeline-search-filter | `crates/fdemon-app/src/session/performance.rs` (new fields: `timeline_search_query: Option<String>`, `timeline_search_input_active: bool`, `timeline_search_match_cursor: usize`), `crates/fdemon-app/src/handler/keys.rs` (new arms: `/` opens search input on TimelineEvents tab; `n`/`N` jump to next/prev match when search is non-empty; while `timeline_search_input_active`, char keys append to query, `Backspace` deletes, `Enter` confirms and closes input, `Esc` clears query and closes), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (new handlers: `handle_timeline_search_open`, `handle_timeline_search_input`, `handle_timeline_search_close`, `handle_timeline_search_jump_to_match`; match collection iterates all tracks/nodes filtering by `name.contains(query)`, sorts by ts, navigation cycles), `crates/fdemon-app/src/message.rs` (new Message variants), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/search.rs` (NEW: input-mode bar at the top of the canvas showing `/<query>_`, match count `(3/12)`, hotkeys hint), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` (match-overlay render: brighten/border bars whose name contains the query case-insensitively), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` (declare `pub(super) mod search;`; insert search bar above filter strip when input is active OR a query is set) | T01 outputs (viewport state — `n`/`N` pans viewport to center on match), Phase 4 outputs, T03 outputs if running after (selection cursor — `n`/`N` may select-and-pan in one action) |
| **05** update-arch-and-review-focus-docs | `docs/ARCHITECTURE.md` (DevTools Subsystem → Performance Panel section: document the manual-viewport vs follow-latest state machine, pan/zoom math, minimap rendering pipeline, selection cursor and details popup modal, search-and-jump UX; cross-reference Phase 4's Gantt baseline), `docs/REVIEW_FOCUS.md` (new approved patterns: viewport state in PerformanceState (not widget-local), selection cursor by `(tid, depth, ts)` not by index, search-as-highlight not filter, minimap dominant-thread coloring; document that CPU sampling is deferred to Phase 6) | T01–T04 completion summaries |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | None | — | **Sequential by dependency** — T02 reads T01's viewport state. |
| 01 + 03 | None | — | **Sequential by dependency** — T03 reads T01's viewport state. |
| 01 + 04 | None | — | **Sequential by dependency** — T04 reads T01's viewport state. |
| 02 + 03 | None | 2 | **Parallel (worktree)** — T02 owns `minimap.rs`; T03 owns `popup.rs` and `gantt.rs` selection overlay. The `mod.rs` lines T02 adds (declare `pub(super) mod minimap;` + insert minimap row above time axis) are disjoint from T03's lines (declare `pub(super) mod popup;` + conditional popup overlay at end of render). Auto-merge expected to succeed. |
| 02 + 04 | None | 2 | **Parallel (worktree)** — same reasoning; T02 and T04 touch disjoint sections of `mod.rs`. |
| 03 + 04 | `crates/fdemon-tui/.../timeline_events/gantt.rs`, `crates/fdemon-tui/.../timeline_events/mod.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/devtools/performance/timeline.rs`, `crates/fdemon-app/src/session/performance.rs`, `crates/fdemon-app/src/message.rs` | 2 | **Sequential** — T03 adds selection overlay to `gantt.rs`; T04 adds match-highlight overlay to the same file. Both add Message variants and handler arms. Run T03 first (selection is foundational for `n`/`N` "jump-to-match-and-select"), then T04 builds on it. |
| 04 + 05 | None | — | T05 is docs-only. |

## Success Criteria

Phase 5 is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Pan/zoom verified:** `+` zooms in; `-` zooms out; `←`/`→` pans (no selection); viewport range visible in a status indicator (e.g., footer).
- [ ] **Auto-scroll toggle verified:** After manual pan, "PAUSED" indicator shows; new events do not auto-scroll. Pressing `End` resets to live-follow mode.
- [ ] **Minimap verified:** A 1-row strip above the time axis shows compressed event history, with a `[...]` bracket on the current viewport. Bracket moves on pan/zoom.
- [ ] **Selection verified:** `Enter` on TimelineEvents tab selects the first visible event. `←`/`→` traverses siblings; `↑`/`↓` traverses depth. Selected bar has distinct border or inverted color.
- [ ] **Details popup verified:** `Enter` on selected event opens modal with name, ts, dur, thread, parent chain. `Esc` closes.
- [ ] **Search verified:** `/` opens input; query highlights matching bars; `n`/`N` jumps viewport to next/prev match; `Esc` clears query.
- [ ] **Filter still preserved:** `T` cycle still works; query persists across filter changes.
- [ ] **No regression on Phase 4 features:** thread rows + colored bars + depth stacking + thread filter all still work.
- [ ] **Mouse interaction (stretch):** clicking a bar selects it; clicking outside clears selection; clicking the minimap pans the viewport (or no-op if scope tight).
- [ ] **Doc updates verified:** ARCHITECTURE.md documents viewport state machine, selection model, search pipeline. REVIEW_FOCUS.md adds approved-pattern entries.

## Notes

- **T01 is foundational** — every other task reads `timeline_viewport_*` state. Land T01 fully (validator PASS, merged) before starting T02–T04.
- **Wave 2 mixed strategy:** T02 (minimap) is genuinely parallel-safe. T03 (selection) and T04 (search) share write files in `gantt.rs`, `handler/keys.rs`, `handler/devtools/performance/timeline.rs`, `message.rs`, `session/performance.rs`. Run T03 first (selection cursor needed for `n`/`N` jump-to-match-and-select), then T04. Dispatch T02 in parallel with T03 in a worktree, then T04 sequentially after.
- **Keybinding conflicts to verify in T03/T04** — `n` is currently top-level "enter Network panel." On TimelineEvents tab, `n` must only mean "next match" when search is active; otherwise fall through. This is the same pattern as Phase 3-followup's `R`-key fallthrough resolution.
- **Phase 6 (deferred):** CPU sampling via `getCpuSamples`, cross-thread async connector lines, per-frame zoom-to-frame coupling, event annotation/pinning, trace export.
