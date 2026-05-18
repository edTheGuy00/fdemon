# Plan: Phase 2 Follow-up Fixes

**Status:** Draft — awaiting approval
**Driver:** [`workflow/reviews/features/devtools-performance-memory-split-phase-2/REVIEW.md`](../../../../reviews/features/devtools-performance-memory-split-phase-2/REVIEW.md) and [`ACTION_ITEMS.md`](../../../../reviews/features/devtools-performance-memory-split-phase-2/ACTION_ITEMS.md)
**Parent feature:** `devtools-performance-memory-split` (see [`../PLAN.md`](../PLAN.md))
**Predecessor phase:** [`../phase-2/TASKS.md`](../phase-2/TASKS.md) — all 7 tasks merged 2026-05-19

---

## TL;DR

Phase 2 of the Performance details work merged across 3 waves but post-merge review surfaced 1 user-visible Critical bug (duplicate frame-detail rendering in dual-pane mode), 2 Major correctness/hygiene issues (dead binding with false comment; ARCHITECTURE.md documents a wrong field name), and 13 Minor cleanups (doc registry gaps, stale comments, footer hint polish, derivation comment math, defensive arithmetic, byte-slice idiom). This phase bundles those into 4 tasks across 2 waves so Phase 2 reaches a shippable state.

One additional High-severity risk — `is_janky()` is hard-coded to 60 fps while `display_refresh_rate` is now a state field — is **explicitly deferred to Phase 3** as a prerequisite for any `Display.Refresh` writer. See *Open Decision Handed to Phase 3* below.

## Problem

The shipped Phase 2 has three concrete defects:

1. **`frame_chart/mod.rs:142, 187`** — `FrameChart::render` and `FrameChart::render_with_regions` unconditionally call `render_detail_panel`. At 200×30 in dual-pane mode, the chart's 3-row detail strip renders `Frame #N  Total: X.Xms / UI: … / Raster: …` for the selected frame — the same data the Frame Analysis tab below now renders. **User sees the same data twice on screen.** The doc comment at `frame_chart/detail.rs:21-24` already claims the panel is "Used only in the chart-only fallback", but the calling code does not honour that contract.
2. **`details/frame_analysis_tab.rs:165-167`** — A `render_width` binding is computed, not used, and silenced with `let _ = render_width; // used implicitly by ratatui set_string`. `Buffer::set_string` does not take a width parameter — the comment is actively false. Rendering happens to be correct only because the upstream label-selection logic guarantees the fit. Future maintainers will assume a safety mechanism exists where there isn't one.
3. **`docs/ARCHITECTURE.md:1091`** — Documents the `OverBudget` hint variant as `OverBudget { budget_ms, actual_ms }`. The implementation in `crates/fdemon-core/src/frame_hints.rs:70-74` defines `OverBudget { excess_ms, budget_ms }`. `actual_ms` and `excess_ms` have different semantics (full frame time vs overage). A Phase 3 caller reading only the doc would emit the wrong message string.

Plus a long tail of Minor doc/hygiene/UX items, summarised in the task-mapping table below.

## Goals

1. **C1 — Eliminate duplicate per-frame detail rendering in dual-pane mode** so the selected frame's `Total / UI / Raster` line appears exactly once on screen. The chart-only fallback (small terminal) keeps rendering its detail strip; the no-selection FPS / Avg / Jank / Shader summary line stays in `frame_chart/detail.rs` per the original Phase 2 plan.
2. **M1 — Remove dead binding + false comment in `frame_analysis_tab.rs`**; restate the actual fit invariant. Bundle related `frame_analysis_tab.rs` Minor cleanups (`&name[..1]` byte-slice, proportional-bar test coverage, raster-remainder allocation, non-saturating u16 arithmetic) into the same task since they live in the same file and have no inter-dependency.
3. **M3 — Fix `docs/ARCHITECTURE.md` `OverBudget` field name** and the stale `PerfSection::DetailsTab` variant name (m2). Both edits live in the same ARCHITECTURE.md section; bundle into a single `doc_maintainer` task.
4. **Consolidate remaining Minors** into one cleanup task: register the missing `Cell<usize>` render-hint fields in `docs/REVIEW_FOCUS.md` (m1), drop stale "Unreachable via Tab" comments in `frame.rs` (m3), surface `MIN_PHASE_BAR_WIDTH` via `pub(super)` to retire the `const _:u16 = ...` workaround (m5), make the Performance footer section-aware to stop falsely advertising `[j/k] Scroll` on Details focus (m6), disambiguate the footer `[]/[]` glyph (m8), fix or bump the `MIN_DUAL_PANE_HEIGHT` derivation comment (m7), add a 2-variant warning to `PerfSection::next/prev` (m12), align the `handle_perf_jump_to_start` fallback with `handle_perf_page` (m13).

## Non-Goals

- **M2 (`is_janky` ↔ `display_refresh_rate` migration) is deferred to Phase 3.** It is the right hand-off for Phase 3 because (a) the field has zero non-default writer in Phase 2, so today both producers agree on jank verdicts; (b) the migration must update 5+ call sites including `PerformanceStats.jank_count` in lock-step with the new `Display.Refresh` writer. Doing it here would land code with no consumer and risk drift before Phase 3 ships. Tracked under *Open Decision Handed to Phase 3* below.
- **No new VM Service work.** No changes under `crates/fdemon-daemon/`.
- **No new keyboard shortcuts.** The `]`/`[` and `Tab`/`Shift+Tab` bindings from Phase 2 stay as shipped.
- **No layout-threshold changes** beyond the m7 derivation-comment correction. `MIN_DUAL_PANE_HEIGHT`, `MIN_DETAILS_HEIGHT`, `MIN_PHASE_BAR_WIDTH`, `COMPACT_THRESHOLD` keep their current values unless the comment-math correction forces a bump.
- **No proportional-bar visual redesign.** m9 only adds a regression test for the existing behaviour; m10 only changes which segment gets the rounding remainder.

## Approach

Four tasks across two waves. Wave 1 runs three independent fixes in parallel worktrees (zero write-file overlap). Wave 2 runs the consolidated cleanup task on the working branch since it touches `performance/mod.rs`, which T01 also edits.

```
                ┌────────────────────────────────┐ ┌──────────────────────────────┐ ┌────────────────────────────────┐
    Wave 1      │ 01 fix-duplicate-detail-render │ │ 02 frame-analysis-tab-cleanup│ │ 03 fix-architecture-doc-errors │
    (parallel)  │   C1 — dual-pane gating        │ │   M1 + m4 + m9 + m10 + m11   │ │   M3 + m2 (doc_maintainer)     │
                └────────────────┬───────────────┘ └───────────────────────────────┘ └────────────────────────────────┘
                                 │
                                 │ (sequential — T04 writes perf/mod.rs that T01 also writes)
                                 ▼
                ┌────────────────────────────────┐
    Wave 2      │ 04 consolidated-minor-cleanup  │
    (sequential)│   m1 + m3 + m5 + m6 + m7 + m8  │
                │   + m12 + m13                  │
                └────────────────────────────────┘
```

## Background References

| Concern | Path | Notes |
|---|---|---|
| C1 — unconditional detail panel | `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs:142, 187` | Both `Widget::render` and `render_with_regions` call `render_detail_panel` without a dual-pane gate |
| C1 — contradicted doc comment | `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs:21-24` | "Used only in the chart-only fallback" — not enforced by caller |
| C1 — dual-pane callsite | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs::render_chart_only` (called from the dual-pane branch in `render_impl`) | The caller already knows whether it is in dual-pane mode but does not communicate that to `FrameChart` |
| M1 — dead binding | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:165-167` | `let render_width = …; buf.set_string(…); let _ = render_width;` |
| M3 — wrong field name | `docs/ARCHITECTURE.md:1091` | `OverBudget { budget_ms, actual_ms }` should be `OverBudget { excess_ms, budget_ms }` |
| m2 — stale variant name | `docs/ARCHITECTURE.md:1044` | `DetailsTab` should be `Details` (matches `session/performance.rs::PerfSection`) |
| m1 — missing Cell registry entries | `docs/REVIEW_FOCUS.md:29-36` ("Current usage") | Needs `frame_chart_visible_width` (pre-existing gap) and `details_pane_visible_height` (Phase 2) |
| m3 — stale "Unreachable" comments | `crates/fdemon-app/src/handler/devtools/performance/frame.rs:164, 187` | Phase 2 made `Details` reachable via Tab; comments now lie |
| m4 — byte-slice on phase name | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:151-152` | `&name[..1]` is safe today (ASCII labels) but fragile |
| m5 — `const _` workaround | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:358-361` | Replace with `pub(super)` visibility on `MIN_PHASE_BAR_WIDTH` |
| m6 — false footer advertising | `crates/fdemon-tui/src/widgets/devtools/mod.rs::render_footer` Performance arm | `[j/k] Scroll` shown while `focused_section == Details` even though scroll is no-op there |
| m7 — derivation comment math | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:53-58` | `MIN_DUAL_PANE_HEIGHT` derivation arithmetic does not resolve cleanly |
| m8 — ambiguous footer glyph | `crates/fdemon-tui/src/widgets/devtools/mod.rs:375` | `[]/[] Tabs` reads as empty brackets |
| m9 — bar contract not test-locked | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` tests | No test verifies `█` count per segment matches proportions |
| m10 — remainder allocated to raster | `frame_analysis_tab.rs:129-131` | Should go to the largest segment, not unconditionally to raster |
| m11 — non-saturating u16 add | `frame_analysis_tab.rs:129-131, 174, 329` | Out of step with `saturating_sub` use elsewhere |
| m12 — `next`/`prev` assume 2 variants | `crates/fdemon-app/src/session/performance.rs:28-41` | Same body — silently wrong if a 3rd variant is ever added |
| m13 — fallback mismatch | `crates/fdemon-app/src/handler/devtools/performance/frame.rs:114-118, 155-160` | `handle_perf_page` falls back to `DEFAULT_PERF_PAGE_SIZE`; `handle_perf_jump_to_start` uses `.max(1)` |

## Open Decision Handed to Phase 3

**M2 — `is_janky()` ↔ `display_refresh_rate` migration.** Phase 2 introduced `PerformanceState::display_refresh_rate: f64 = 60.0` as a Phase-3 anchor, and `frame_analysis_tab::render_verdict` already consumes it. But `is_janky()` in `fdemon-core/src/performance.rs:261-263` is hard-coded to `FRAME_BUDGET_60FPS_MICROS`, and `PerformanceStats.jank_count` aggregates over `is_janky()`. Today both producers agree because no writer for `display_refresh_rate` exists yet. The moment Phase 3 parses `Display.Refresh` Extension events, a 120 Hz device will show inconsistent jank verdicts across the chart bars, the summary line, and the Frame Analysis tab.

**Phase 3 must:**
1. Refactor `is_janky` to take a budget parameter (or consume `display_refresh_rate`).
2. Update all `is_janky()` call sites in lock-step: `frame_chart/bars.rs:258`, `frame_chart/detail.rs:159`, `session/performance.rs:228` (`PerformanceStats.jank_count` aggregation), and anywhere else `cargo grep` surfaces.
3. Ship the migration BEFORE (or in the same PR as) the `Display.Refresh` parser, never after.

This phase's `docs/ARCHITECTURE.md` fix (T03) is the only Phase-2 cross-reference and does not require touching this area.

## Success Criteria

Phase 2-followup is complete when:

- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.
- [ ] **C1 verified:** At 200×30 with a frame selected, `Frame #N  Total: …` appears exactly once on screen (in the Frame Analysis tab, not in the chart strip). At 200×16 (chart-only fallback) the chart detail strip still renders the per-frame detail. A new render test in `widgets/devtools/performance/tests.rs` enforces both.
- [ ] **M1 verified:** `frame_analysis_tab.rs:165-167` has no dead bindings. Either the label is explicitly clipped before `buf.set_string`, or the upstream invariant is documented inline and the lines are removed.
- [ ] **M3 verified:** `docs/ARCHITECTURE.md` Phase 2 section uses the `OverBudget { excess_ms, budget_ms }` field signature, and `PerfSection` variant names match source code (`Details`, not `DetailsTab`).
- [ ] **m1 verified:** `docs/REVIEW_FOCUS.md` "Current usage" lists both `PerformanceState::frame_chart_visible_width` and `PerformanceState::details_pane_visible_height`.
- [ ] **m3 verified:** No "Unreachable via Tab" comments remain in `handler/devtools/performance/frame.rs`.
- [ ] **m5 verified:** No `const _: u16 = MIN_PHASE_BAR_WIDTH` line in `performance/mod.rs`. The constant is `pub(super)` and imported by `details/frame_analysis_tab.rs`.
- [ ] **m6 verified:** Footer hint string omits `[j/k] Scroll` when `focused_section == Details`.
- [ ] **m9 verified:** A new test asserts `█` count per phase segment equals `(phase_micros * width / total).round()` ± 1 column.

## Notes

- This phase does NOT block Phase 3 mechanically — Phase 3 may begin after T03's `OverBudget` doc fix lands (the doc would otherwise mislead a Phase 3 implementor of additional hint message strings). The other items are quality improvements.
- All four tasks operate on existing data plumbing — no changes under `crates/fdemon-daemon/` or `crates/fdemon-core/`.
- The C1 fix in T01 changes a public-within-crate API on `FrameChart::new` (adding a new parameter). All current callers live under `crates/fdemon-tui/src/widgets/devtools/performance/` — the blast radius is one file.
