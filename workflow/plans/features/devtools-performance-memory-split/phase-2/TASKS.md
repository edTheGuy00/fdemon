# Phase 2 — Performance Details Pane + Frame Analysis Populated — Task Index

## Overview

Phase 2 transforms the now-frame-only Performance tab into a **chart-plus-tabbed-details** layout that mirrors the official DevTools `tabbed_performance_view`. The Frame Chart stays in the top portion of the panel; a new tabbed Details pane is added below with three tabs:

- **Frame Analysis** — populated in Phase 2 using existing `FramePhases` data: phase percentage bar, total / budget verdict, refresh-rate-aware hints, no-data + no-selection fallbacks.
- **Rebuild Stats** — Phase 2 stub ("Coming soon — Phase 3 adds widget rebuild tracking.").
- **Timeline Events** — Phase 2 stub ("Coming soon — Phase 3 streams UI/Raster thread timeline events.").

Tab cycling reuses the **Inspector parity** convention but with new bindings to avoid colliding with `Tab`'s existing section-focus toggle: `]`/`[` cycle details tabs when `focused_section == Details`. `Tab`/`Shift+Tab` continue to cycle `PerfSection` between `FrameChart` ↔ `Details` (Phase 1 made these no-ops; Phase 2 makes them functional).

No new VM Service work — Phase 2 is data-complete with existing `FrameTiming.phases`. Phase 3 adds the Rebuild Stats + Timeline Events RPCs.

**Total Tasks:** 7
**Estimated Hours:** 16–22 hours

## Task Dependency Graph

```
        ┌──────────────────────────────────┐   ┌──────────────────────────────────┐
Wave 1  │ 01-add-frame-hints-core          │   │ 02-perf-details-state-foundation │
        │  (fdemon-core: frame_hints,      │   │  (PerfDetailsTab enum,           │
        │   FrameHint enum)                │   │   PerfCycleDetailsTab msg,       │
        │                                  │   │   PerfSection cycling fix,       │
        │                                  │   │   display_refresh_rate field)    │
        └──────────────────┬───────────────┘   └────────────────┬─────────────────┘
                           │                                    │
                           │                       ┌────────────┴────────────┐
                           │                       ▼                         ▼
        ┌──────────────────┴───────────────┐   ┌──────────────────────────────────┐
Wave 2  │ 04 perf-details-widget-shell     │   │ 03 perf-handler-split-and-keys   │
        │  (widgets/.../performance/       │   │  (handler/devtools/performance/  │
        │   details/{mod, stubs}.rs,       │   │   {mod, frame, details}.rs;      │
        │   mod.rs dual-pane restructure)  │   │   keys.rs ]/[; update.rs)        │
        └──────────────────┬───────────────┘   └────────────────┬─────────────────┘
                           │                                    │
       ┌───────────────────┴─────┐                              │
       ▼                         ▼                              ▼
┌──────────────────────────┐  ┌──────────────────────────┐  ┌──────────────────────────┐
│ 05 frame-analysis-content│  │ 06 keybindings-and-footer│  │ 07 update-architecture-doc│
│   (proportional bar,     │  │   (KEYBINDINGS.md +      │  │   (ARCHITECTURE.md,      │
│    hints rendering)      │  │    footer hint string)   │  │    doc_maintainer)        │
└──────────────────────────┘  └──────────────────────────┘  └──────────────────────────┘
        Wave 3 (parallel)             Wave 3 (parallel)           Wave 3 (parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [add-frame-hints-core](tasks/01-add-frame-hints-core.md) | Not Started | — | 2–3h | implementor | 1 |
| 02 | [perf-details-state-foundation](tasks/02-perf-details-state-foundation.md) | Not Started | — | 2–3h | implementor | 1 |
| 03 | [perf-handler-split-and-keys](tasks/03-perf-handler-split-and-keys.md) | Not Started | 02 | 3–5h | implementor | 2 |
| 04 | [perf-details-widget-shell](tasks/04-perf-details-widget-shell.md) | Not Started | 02 | 4–6h | implementor | 2 |
| 05 | [frame-analysis-content](tasks/05-frame-analysis-content.md) | Not Started | 01, 04 | 3–4h | implementor | 3 |
| 06 | [keybindings-and-footer](tasks/06-keybindings-and-footer.md) | Not Started | 03 | 0.5–1h | implementor | 3 |
| 07 | [update-architecture-doc](tasks/07-update-architecture-doc.md) | Not Started | 03, 04 | 1–1.5h | doc_maintainer | 3 |

## File Overlap Analysis

> The orchestrator uses this section to decide isolation strategy per wave. Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** add-frame-hints-core | `crates/fdemon-core/src/performance.rs` (or NEW `crates/fdemon-core/src/frame_hints.rs` + a `pub mod frame_hints;` line in `crates/fdemon-core/src/lib.rs`) | — |
| **02** perf-details-state-foundation | `crates/fdemon-app/src/state.rs` (add `PerfDetailsTab` enum), `crates/fdemon-app/src/session/performance.rs` (fix `PerfSection::next/prev` cycling; add `details_tab`, `details_pane_visible_height`, `display_refresh_rate` fields), `crates/fdemon-app/src/message.rs` (`PerfCycleDetailsTab`, `PerfFocusDetailsTab` variants), `crates/fdemon-app/src/session/mod.rs` (re-export `PerfDetailsTab` if exposed via session module) | — |
| **03** perf-handler-split-and-keys | DELETE `crates/fdemon-app/src/handler/devtools/performance.rs`; create `crates/fdemon-app/src/handler/devtools/performance/mod.rs` (re-exports + module decls), `crates/fdemon-app/src/handler/devtools/performance/frame.rs` (existing frame-selection / scroll / page / jump handlers moved here unchanged), `crates/fdemon-app/src/handler/devtools/performance/details.rs` (NEW — `handle_perf_cycle_details_tab`, `handle_perf_focus_details_tab`); `crates/fdemon-app/src/handler/devtools/mod.rs` (replace `pub mod performance;` no change in line text but verify); `crates/fdemon-app/src/handler/keys.rs` (route `]`/`[` when `in_performance && focused_section == Details`; verify `Tab/Shift+Tab` cycling now lands on `PerfSection::Details`); `crates/fdemon-app/src/handler/update.rs` (dispatch new `Perf*DetailsTab` messages) | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/session/performance.rs` (read T02 outputs) |
| **04** perf-details-widget-shell | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (dual-pane restructure; add `MIN_DUAL_PANE_HEIGHT`, `MIN_DETAILS_HEIGHT`, `MIN_PHASE_BAR_WIDTH` constants), `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` (NEW — tab strip + dispatch), `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` (NEW — minimal stub: frame-number header + total/budget line so the file exists for T05), `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` (NEW — "Coming soon" stub), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` (NEW — "Coming soon" stub), `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` (add dual-pane layout tests; mark obsolete chart-only-shape tests as still-passing or rewrite as needed) | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` (reference for tab strip rendering pattern), T02 outputs |
| **05** frame-analysis-content | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` (replace stub with proportional phase bar + hint list + no-data + no-selection fallbacks), `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` (trim: the per-frame summary moves to frame_analysis_tab; the no-selection summary line stays in the FrameChart's bottom strip when dual-pane is collapsed) | `crates/fdemon-core/src/{performance, frame_hints}.rs` (T01 output), `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` (reference for property-row rendering style) |
| **06** keybindings-and-footer | `docs/KEYBINDINGS.md` (document `]`/`[` Performance details tab cycling), `crates/fdemon-tui/src/widgets/devtools/mod.rs` (update Performance arm of `render_footer` to mention `]/[` Tabs hint) | T03 task spec |
| **07** update-architecture-doc | `docs/ARCHITECTURE.md` ("Performance Panel Interactivity" section: add `PerfDetailsTab` enum, dual-pane layout, three details tabs, three responsive thresholds, frame_hints helper) | T01–T05 task specs and completion summaries |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | **None** | 1 | **Parallel (worktree)** — T01 lives in `fdemon-core`; T02 lives in `fdemon-app/{state, session/performance, message, session/mod}.rs`. Different crates, zero overlap. |
| 03 + 04 | **None** | 2 | **Parallel (worktree)** — T03 lives entirely in `fdemon-app/handler/`; T04 lives entirely in `fdemon-tui/widgets/devtools/performance/` plus a `details/` subtree. Each task reads from T02 but they do not write the same files. |
| 03 + 02 | `crates/fdemon-app/src/handler/...` (T03 only), `state.rs`/`session/performance.rs`/`message.rs` (T02 only) | — | **Sequential** — T03 depends on T02 (must merge first). |
| 04 + 01 | None (T01 writes `fdemon-core`, T04 writes `fdemon-tui`) | — | **Sequential by dependency only** — T04 reads T01's `frame_hints` at compile time, no write overlap. T04 may import the function even though it only fully uses it in T05; the stub is allowed to do nothing with it. To keep T04 independent of T01, T04 does **not** import `frame_hints` — only T05 does. |
| 04 + 02 | None | — | **Sequential** — T04 depends on T02 (must merge first). |
| 05 + 04 | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` (T04: stub; T05: populated content) | — | **Sequential** — T05 runs after T04 merges. |
| 05 + 06 | **None** | 3 | **Parallel** — T05 writes `frame_analysis_tab.rs`; T06 writes `docs/KEYBINDINGS.md` + `widgets/devtools/mod.rs`. |
| 05 + 07 | **None** | 3 | **Parallel** — T07 writes `docs/ARCHITECTURE.md` only. |
| 06 + 07 | **None** | 3 | **Parallel** — different doc files. |
| 06 + 04 | `crates/fdemon-tui/src/widgets/devtools/mod.rs` is only edited by T06 in Wave 3 (T04 does **not** touch this file — its work stays within the `performance/` subtree). | — | **Sequential** — T06 runs after T04 merges only by dependency on T03's footer-string change, not by file-write overlap. |

## Success Criteria

Phase 2 is complete when:

- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.
- [ ] At terminal `200×30`: Performance panel shows the Frame Chart (top ~55%) **and** the tabbed Details pane (bottom ~45%) with the active tab underlined.
- [ ] At terminal `200×16`: Performance panel falls back to the **chart-only** layout — the Details pane collapses (matches existing < `MIN_DUAL_PANE_HEIGHT` Phase 1 behaviour).
- [ ] Selecting a frame (`←`/`→`) populates the **Frame Analysis** tab with: frame number, `Total: 18.2 ms  Budget @ 60 Hz: 16.7 ms — JANK +1.5 ms` (when janky), a proportional 4-segment phase bar (build / layout / paint / raster) when `phases` is `Some`, and a hint list of up to 5 entries.
- [ ] Frames with `phases == None` fall back to the aggregate build+raster split with no proportional bar.
- [ ] When `area.width < MIN_PHASE_BAR_WIDTH` the phase bar degrades to a single-line `B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms` summary.
- [ ] No selection → Frame Analysis tab shows "Select a frame above (`←`/`→`) to view analysis."
- [ ] Pressing `]`/`[` while `focused_section == Details` cycles through `FrameAnalysis → RebuildStats → TimelineEvents → FrameAnalysis`. The Rebuild Stats and Timeline Events tabs render the "Coming soon" stub.
- [ ] Pressing `Tab`/`Shift+Tab` cycles `focused_section` between `FrameChart` and `Details` (Phase 1 no-op is gone — verify the cycling round-trips through both values).
- [ ] All previous Performance tests still pass. New tests cover: hint generation (table-driven), phase-bar proportions, tab cycling, layout decision thresholds, no-data fallbacks.
- [ ] `docs/KEYBINDINGS.md` documents `]`/`[` details-tab cycling under the Performance panel section.
- [ ] `docs/ARCHITECTURE.md` "Performance Panel Interactivity" section is updated to describe the dual-pane layout, the three tabs, and the new state fields.

## Phase Acceptance Test Plan

After all 7 tasks merge, run the manual smoke test:

1. `cargo run -- ~/Dev/some-flutter-app` in a 200×30 iTerm split.
2. Press `d` → DevTools. Press `p` → Performance. Verify the Frame Chart fills the top ~55% and the Details pane fills the bottom ~45% with the **Frame Analysis** label underlined.
3. Press `←` to select the most-recent frame. Verify the Frame Analysis tab shows: header line (frame number), total/budget verdict line, proportional phase bar, hint list.
4. Press `]` → tab cycles to **Rebuild Stats**. Verify the "Coming soon — Phase 3" stub renders centered.
5. Press `]` again → tab cycles to **Timeline Events**. Verify the "Coming soon — Phase 3" stub renders centered.
6. Press `]` again → tab wraps back to **Frame Analysis**.
7. Press `[` → tab cycles backward to **Timeline Events**.
8. Press `Tab` → focus moves from `Details` to `FrameChart` (border highlight shifts to the top section).
9. Press `Esc` (with frame selected) → frame deselects, Frame Analysis tab shows the no-selection prompt.
10. Press `Esc` again → returns to Logs.
11. Resize terminal to 200×16. Verify dual-pane collapses — only the Frame Chart is visible (no tabs).
12. Resize terminal to 36×40 (narrow but tall). Verify the proportional phase bar degrades to the inline `B/L/P/R` summary line.

## Keyboard Shortcuts Added in Phase 2

| Key | Context | Action |
|-----|---------|--------|
| `]` | Performance, `focused_section == Details` | Cycle to next details tab (Frame Analysis → Rebuild Stats → Timeline Events) |
| `[` | Performance, `focused_section == Details` | Cycle to previous details tab |
| `Tab` / `Shift+Tab` | Performance | (Phase 1 no-op) → now cycles `FrameChart ↔ Details` |

`Tab/Shift+Tab` keymap is unchanged from Phase 1; only its **runtime behaviour** changes when `PerfSection::next/prev` are fixed to actually cycle.

## Notes

- **`PerfSection` rename rejected.** Code uses `PerfSection::Details` (Phase 1-followup T03 named it). The plan's "DetailsTab" wording in `PLAN.md` refers to the conceptual destination of the focus, not a rename. The variant stays `Details`. The new tab-within-section enum is named `PerfDetailsTab` (variants: `FrameAnalysis`, `RebuildStats`, `TimelineEvents`).
- **`display_refresh_rate` defaults to 60.0 Hz** in Phase 2 — `Flutter.Frame` events do not expose target refresh rate. Phase 3 may parse `Display.Refresh` Extension events to bump 90/120 Hz devices. Default is conservative (never wrong for `is_janky()`, slightly conservative for hint thresholds).
- **No new VM Service work.** Phase 2 is data-complete with existing `FrameTiming.phases`. No changes under `crates/fdemon-daemon/`.
- **Mouse clicks on tab labels** are NOT implemented in Phase 2 (Inspector parity work also deferred this — see `widgets/devtools/inspector/details/mod.rs` TODO comment). Keyboard cycling is sufficient for both panels' Phase 2 ship.
- **`frame_chart/detail.rs` trimming**: the per-frame summary that lives there today moves into `frame_analysis_tab.rs`. The no-selection FPS / Avg / Jank / Shader summary line **stays** in `frame_chart/detail.rs` because the chart-only fallback (small terminal) still uses it. T05 owns this split.
- **`details_pane_visible_height: Cell<usize>` render-hint** is added in T02 but only consumed in Phase 3 (for Rebuild Stats + Timeline Events scrolling). T02 sets it; T04/T05 do not need to read it. Adding the field now avoids a second state-shape migration when Phase 3 lands.
- **`widgets/devtools/mod.rs` `render_footer`**: T04 does **not** edit this file. T06 updates the Performance footer-hint string to mention `]/[`. This keeps the Wave-2 parallel boundary clean.
- **The handler split** (T03) is a refactor with a single net behaviour change (cycle-tab handler is new). The existing frame-selection / scroll / page / jump handlers move file-for-file into `performance/frame.rs` with zero logic change. T03's review focus should be on dispatch correctness, not on the moved code.
- **Phase 3 anchors already exist**: the `RebuildStats` and `TimelineEvents` variants on `PerfDetailsTab` are added in Phase 2 (T02) so Phase 3 does not need a second state migration. The Phase 2 stub tab modules (T04) are the integration points for Phase 3's populated content.
