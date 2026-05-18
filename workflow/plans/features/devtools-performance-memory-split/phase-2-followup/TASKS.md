# Phase 2-Followup — Review-Driven Fixes — Task Index

## Overview

Four tasks address the 1 Critical + 2 Major + 13 Minor findings from the Phase 2 code review ([`../../../../reviews/features/devtools-performance-memory-split-phase-2/REVIEW.md`](../../../../reviews/features/devtools-performance-memory-split-phase-2/REVIEW.md)). See [`PLAN.md`](PLAN.md) for the rationale, finding↔task mapping, and the M2 hand-off note to Phase 3.

- **Wave 1 (parallel):** Three independent fix tracks — C1 dual-pane detail dedup (T01), M1 + `frame_analysis_tab.rs` cleanup (T02), and the ARCHITECTURE.md doc errors (T03, `doc_maintainer`). Zero write-file overlap.
- **Wave 2 (sequential):** Consolidated minors cleanup (T04). Touches `performance/mod.rs`, which T01 also edits, so it must run after Wave 1 merges.

**Total Tasks:** 4
**Estimated Hours:** 4–6 hours

## Task Dependency Graph

```
            ┌────────────────────────────────┐ ┌──────────────────────────────┐ ┌────────────────────────────────┐
Wave 1      │ 01 fix-duplicate-detail-render │ │ 02 frame-analysis-tab-cleanup│ │ 03 fix-architecture-doc-errors │
(parallel)  │   C1                           │ │   M1 + m4 + m9 + m10 + m11   │ │   M3 + m2 (doc_maintainer)     │
            └────────────────┬───────────────┘ └───────────────────────────────┘ └────────────────────────────────┘
                             │
                             │ (sequential — same-file write overlap on performance/mod.rs)
                             ▼
            ┌────────────────────────────────┐
Wave 2      │ 04 consolidated-minor-cleanup  │
(sequential)│   m1 + m3 + m5 + m6 + m7 + m8  │
            │   + m12 + m13                  │
            └────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [fix-duplicate-detail-render](tasks/01-fix-duplicate-detail-render.md) | Not Started | — | 1.5–2h | implementor | 1 |
| 02 | [frame-analysis-tab-cleanup](tasks/02-frame-analysis-tab-cleanup.md) | Not Started | — | 1–1.5h | implementor | 1 |
| 03 | [fix-architecture-doc-errors](tasks/03-fix-architecture-doc-errors.md) | Not Started | — | 0.5h | doc_maintainer | 1 |
| 04 | [consolidated-minor-cleanup](tasks/04-consolidated-minor-cleanup.md) | Not Started | 01 | 1.5–2h | implementor | 2 |

## File Overlap Analysis

> The orchestrator uses this section to decide isolation strategy per wave. Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** fix-duplicate-detail-render | `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` (add `dual_pane: bool` parameter to `FrameChart::new` + thread it to render path), `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` (gate or refactor `render_detail_panel`), `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (pass `dual_pane` flag from the dual-pane vs chart-only call sites), `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` (add regression test asserting frame-detail line appears exactly once at 200×30 and present at 200×16) | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` (verify the Frame Analysis tab is the surviving renderer) |
| **02** frame-analysis-tab-cleanup | `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` (M1 dead binding removal; m4 `&name[..1]` → `chars().next()`; m10 remainder-to-largest-segment; m11 saturating/u32-widened arithmetic; m9 new regression test asserting `█` count per segment matches proportions ±1) | `crates/fdemon-core/src/performance.rs` (FramePhases struct for test fixtures) |
| **03** fix-architecture-doc-errors | `docs/ARCHITECTURE.md` (line 1091: `OverBudget { budget_ms, actual_ms }` → `OverBudget { excess_ms, budget_ms }`; line 1044: `DetailsTab` → `Details` in the `PerfSection` variant list) | `crates/fdemon-core/src/frame_hints.rs` (verify field names), `crates/fdemon-app/src/session/performance.rs` (verify `PerfSection` variant names) |
| **04** consolidated-minor-cleanup | `docs/REVIEW_FOCUS.md` (m1: add bullets for `PerformanceState::frame_chart_visible_width` and `PerformanceState::details_pane_visible_height`), `crates/fdemon-app/src/handler/devtools/performance/frame.rs` (m3: remove "Unreachable via Tab" comments at lines 164, 187; m13: align `handle_perf_jump_to_start` fallback with `handle_perf_page` to use `DEFAULT_PERF_PAGE_SIZE`), `crates/fdemon-app/src/session/performance.rs` (m12: add 2-variant warning comment on `PerfSection::next/prev`), `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (m5: replace `const _: u16 = MIN_PHASE_BAR_WIDTH` workaround with `pub(super)` visibility; m7: fix `MIN_DUAL_PANE_HEIGHT` derivation comment math), `crates/fdemon-tui/src/widgets/devtools/mod.rs` (m6: drop `[j/k] Scroll` from Performance footer when `focused_section == Details`; m8: disambiguate `[]/[] Tabs` glyph) | T01 completion summary (to confirm `performance/mod.rs` callsite shape before editing constants) |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | **None** | 1 | **Parallel (worktree)** — T01 lives in `frame_chart/{mod, detail}.rs` + `performance/mod.rs` + `performance/tests.rs`. T02 lives entirely in `details/frame_analysis_tab.rs`. Distinct file sets. |
| 01 + 03 | **None** | 1 | **Parallel (worktree)** — T03 only writes `docs/ARCHITECTURE.md`. |
| 02 + 03 | **None** | 1 | **Parallel (worktree)** — different file trees. |
| 04 + 01 | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (T01: dual-pane plumbing in render paths; T04: constant visibility + derivation comment) | — | **Sequential** — T04 must merge after T01 to avoid conflicting edits to the same file. The edits are line-disjoint but the orchestrator requires same-file writes to serialize. |
| 04 + 02 | **None** | — | **Sequential by dependency only** — T04 does not write `frame_analysis_tab.rs`. The Wave-2 placement is solely because of T04 + T01. |
| 04 + 03 | **None** | — | T03 is a Wave-1 task; T04 does not write ARCHITECTURE.md. No conflict — just dependency-ordering. |

## Success Criteria

Phase 2-followup is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **C1 verified:** At 200×30 with a frame selected, `Frame #N  Total: …` appears exactly once in the rendered buffer. Regression test in `widgets/devtools/performance/tests.rs` asserts this and the chart-only fallback at 200×16 still shows the frame detail strip.
- [ ] **M1 verified:** No dead `render_width` binding in `frame_analysis_tab.rs`. Either the label is explicitly clipped or the upstream fit invariant is documented inline.
- [ ] **M3 verified:** `docs/ARCHITECTURE.md` uses `OverBudget { excess_ms, budget_ms }` and `PerfSection::{FrameChart, Details}`.
- [ ] **m1 verified:** Both new and pre-existing `PerformanceState` `Cell<usize>` render-hint fields appear in `docs/REVIEW_FOCUS.md` "Current usage".
- [ ] **m3 verified:** No "Unreachable via Tab" comments remain in `frame.rs`.
- [ ] **m4 verified:** Phase-name first-character extraction uses `chars().next()`, not `&name[..1]`.
- [ ] **m5 verified:** `MIN_PHASE_BAR_WIDTH` is `pub(super)`; no `const _: …` workaround line remains in `performance/mod.rs`.
- [ ] **m6 verified:** When `focused_section == Details`, the rendered footer hint does NOT contain `[j/k] Scroll`.
- [ ] **m7 verified:** The `MIN_DUAL_PANE_HEIGHT` derivation comment arithmetic resolves to the stated value.
- [ ] **m8 verified:** The Performance footer hint string contains `]/[ Tabs` (or another unambiguous form), not `[]/[] Tabs`.
- [ ] **m9 verified:** A test asserts `█` count per phase segment equals `(phase_micros * width / total).round()` within ±1 column.
- [ ] **m10 verified:** The proportional bar allocates the rounding remainder to the largest phase segment, not unconditionally to raster.
- [ ] **m11 verified:** `frame_analysis_tab.rs` uses `saturating_add` or u32-widened intermediates in u16 coordinate arithmetic.
- [ ] **m12 verified:** A comment near `PerfSection::next/prev` warns that the bodies assume exactly 2 variants.
- [ ] **m13 verified:** `handle_perf_jump_to_start` and `handle_perf_page` use the same fallback (`DEFAULT_PERF_PAGE_SIZE`) when `frame_chart_visible_width.get() == 0`.

## Phase Acceptance Test Plan

After all 4 tasks merge, run the manual smoke sequence:

1. `cargo run -- ~/Dev/some-flutter-app` in a 200×30 iTerm split.
2. Press `d` → DevTools, `p` → Performance. Press `←` to select the newest frame.
3. **C1 check:** Verify `Frame #` text appears exactly once on screen (in the Frame Analysis tab's header line). The chart's bottom strip should show the no-selection FPS / Avg / Jank / Shader summary line (because the dual-pane detail moved to the tab).
4. Resize to 200×16. Verify the chart-only fallback re-shows the per-frame detail strip (`Frame #N  Total: …`).
5. Press `Tab` → focus moves to Details. **m6 check:** Verify the footer hint no longer mentions `[j/k] Scroll`. **m8 check:** Verify the tab-cycling hint reads unambiguously (e.g. `]/[ Tabs`).
6. Press `]` three times to cycle through all three tabs. Verify cycling still works (no regression from m5's `pub(super)` move).
7. Open `docs/ARCHITECTURE.md` Phase-2 section. Verify the `OverBudget` variant signature matches the source.

## Notes

- **No new keyboard shortcuts.** The `]`/`[` and `Tab`/`Shift+Tab` bindings from Phase 2 stay as shipped.
- **No layout-threshold value changes** unless m7's correction forces a bump.
- **The C1 fix changes a within-crate API surface** (`FrameChart::new`'s parameter list). All current callers live under `crates/fdemon-tui/src/widgets/devtools/performance/` — the blast radius is one parent file plus the chart's own tests module.
- **M2 (`is_janky` / `display_refresh_rate` migration) is deferred to Phase 3.** See PLAN.md "Open Decision Handed to Phase 3" — do not address it here.
