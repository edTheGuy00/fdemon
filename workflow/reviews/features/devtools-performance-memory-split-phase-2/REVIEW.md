# Review: DevTools Performance Phase 2 — Details Pane + Frame Analysis

**Review Date:** 2026-05-19
**Branch:** `feat/devtools-inspector-parity`
**Diff Range:** `e69c649..HEAD` (7 merge commits across 3 waves)
**Plan:** `workflow/plans/features/devtools-performance-memory-split/phase-2/TASKS.md`
**Verdict:** ⚠️ **NEEDS WORK**

## Summary

Phase 2 transforms the Performance panel from a frame-only chart into a chart-plus-tabbed-details layout with three tabs (Frame Analysis populated; Rebuild Stats and Timeline Events stubbed for Phase 3). Implementation is structurally sound — the handler split, dual-pane layout, `frame_hints` core helper, and TEA-conformant state additions all follow project patterns. **However, one user-visible duplication bug, one dead-code smell with a false comment, and several documentation gaps need attention before this is shippable.**

**Reviewer verdicts:**

| Reviewer | Verdict |
|----------|---------|
| Architecture Enforcer | ⚠️ CONCERNS (2 warnings + 1 suggestion) |
| Code Quality Inspector | ⚠️ NEEDS WORK (1 major, 3 minor, 2 nitpicks) |
| Logic Reasoning Checker | ⚠️ WARNINGS (1 high, 5 medium, 4 notes) |
| Risks & Tradeoffs Analyzer | ⚠️ CONCERNS (1 high, 2 medium, 6 low) |
| Security Reviewer | ✅ PASS (0 critical, 0 high, 2 medium, 2 low) |

## Stats

- **Files Modified:** 20 (~2,354 insertions / 101 deletions)
- **New crates/modules:** `fdemon-core::frame_hints`, `fdemon-tui::widgets::devtools::performance::details/{mod, frame_analysis_tab, rebuild_stats_tab, timeline_events_tab}`
- **Refactor:** `handler::devtools::performance.rs` → `performance/{mod, frame, details}.rs`
- **New tests:** 49+ (15 in `frame_hints`, 13 in `frame_analysis_tab`, ~21 across details/mod, performance/tests, handler/details/keys)
- **All four CI gates green per implementor reports** (fmt, check, test, clippy -D warnings)

---

## Critical Findings

### 🔴 C1 — Frame-detail content rendered twice in dual-pane mode
**Sources:** logic_reasoning_checker (W1)
**Files:**
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs:142, 187` — calls `render_detail_panel` unconditionally
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs:21-24` — doc comment claims "Used only in the chart-only fallback" (contradicted by code)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — also renders the same data

**Problem:** The plan (TASKS.md note: "the per-frame summary that lives there today moves into `frame_analysis_tab.rs`. The no-selection FPS / Avg / Jank / Shader summary line **stays** in `frame_chart/detail.rs`") said only the no-selection summary stays in the chart. In the merged code, `FrameChart::render` and `FrameChart::render_with_regions` unconditionally consume 3 rows for a detail panel that, for a *selected frame*, renders `Frame #N  Total: X.Xms`, `UI: …`, `Raster: …` — the same data the Frame Analysis tab below it shows. At 200×30 the user sees the selected frame's stats twice on screen. The chart's no-selection summary line should stay; the per-frame detail should be suppressed in dual-pane mode.

**Severity:** Major (user-visible, contradicts plan intent, contradicts in-source doc comment)

---

## Major Findings

### 🟠 M1 — Dead `render_width` binding with actively false comment
**Sources:** code_quality_inspector (MAJOR), task_validator (noted at merge)
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:165-167`

```rust
let render_width = label.len().min(width as usize) as u16;
buf.set_string(label_x, label_y, label, Style::default().fg(color));
let _ = render_width; // used implicitly by ratatui set_string
```

`render_width` is computed and never used. The comment is factually false: `Buffer::set_string` does **not** take a width parameter. The rendering happens to be correct today only because the label-selection logic upstream guarantees `label.len() <= width as usize` — but the dead binding + false comment will mislead future maintainers into believing there's a safety mechanism that doesn't exist. Either truncate the label explicitly or delete both lines and add an invariant comment at the label-selection block.

### 🟠 M2 — Two-source-of-truth: `is_janky()` vs `display_refresh_rate`
**Sources:** risks_tradeoffs_analyzer (HIGH)
**Files:**
- `crates/fdemon-core/src/performance.rs:261-263` — `is_janky()` is hard-coded to `FRAME_BUDGET_60FPS_MICROS`
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs::render_verdict` — uses `display_refresh_rate`
- `crates/fdemon-app/src/session/performance.rs:228` (`PerformanceStats.jank_count`) — uses `is_janky()`

**Problem:** Today both produce identical output because `display_refresh_rate` defaults to 60.0. Phase 3 plans to populate `display_refresh_rate` from `Display.Refresh` events. The moment that lands, a 120 Hz device will see the Frame Analysis tab say "JANK +1ms" at 9ms total while the chart bars colour as OK (16ms budget) and the summary says "Jank: 0". The plan's "conservative default" rationale is partially false — `is_janky()` ignores the field entirely, so the conservatism only applies as long as `display_refresh_rate` is unused.

**Action:** Phase 3 must refactor `is_janky()` to consume `display_refresh_rate` (or take a budget argument) and migrate all 5+ call sites simultaneously. Track explicitly in Phase 3 task index.

### 🟠 M3 — `docs/ARCHITECTURE.md` documents `OverBudget` with the wrong field name
**Sources:** architecture_enforcer (WARN), logic_reasoning_checker
**File:** `docs/ARCHITECTURE.md:1091`

Doc says `OverBudget { budget_ms, actual_ms }`; code defines `OverBudget { excess_ms: f64, budget_ms: f64 }` (`crates/fdemon-core/src/frame_hints.rs:70-74`). These have different semantics — `actual_ms` would be total frame time, while `excess_ms` is the overage. A Phase 3 caller reading the docs would construct the wrong message string.

**Fix:** Change line 1091 to `| OverBudget { excess_ms, budget_ms }`.

---

## Minor Findings

### 🟡 m1 — New `Cell<usize>` field not registered in `REVIEW_FOCUS.md`
**Sources:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer (all flagged this)
**File:** `docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage" list

`docs/REVIEW_FOCUS.md` explicitly states: "New `Cell`-based render-hint fields require explicit review and documentation here." Phase 2 adds `PerformanceState::details_pane_visible_height: Cell<usize>` (written by render in `performance/mod.rs:265-267`, reserved for Phase 3 consumers). Not listed in the registry.

Additionally, the pre-existing `PerformanceState::frame_chart_visible_width: Cell<usize>` (added in an earlier phase) is also missing from the same registry — chronic doc gap, but should be patched while updating this section.

**Fix:** Add both fields to the "Current usage" list following the existing entry pattern.

### 🟡 m2 — `docs/ARCHITECTURE.md` uses stale variant name `DetailsTab`
**Sources:** architecture_enforcer, risks_tradeoffs_analyzer
**File:** `docs/ARCHITECTURE.md:1044`

Doc says `PerfSection` has variants `FrameChart` and `DetailsTab`. The variant is named `Details` (TASKS.md notes explicitly rejected the rename). Trivial fix.

### 🟡 m3 — Stale "Unreachable via Tab" comments in `frame.rs`
**Sources:** logic_reasoning_checker (W2)
**File:** `crates/fdemon-app/src/handler/devtools/performance/frame.rs:164, 187`

`// No-op in Phase 2. Unreachable via Tab; kept for exhaustiveness.` — after Phase 2 the `Details` arm IS reachable via Tab (T02 fixed the cycling). Update or remove these comments.

### 🟡 m4 — `&name[..1]` byte-slice on phase labels
**Sources:** code_quality_inspector, risks_tradeoffs_analyzer
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:151-152`

Byte-slices the first byte of "Build" / "Layout" / "Paint" / "Raster". Safe today (ASCII), but a future rename or i18n introducing a multibyte first character would panic at the UTF-8 boundary. Use `name.chars().next().unwrap_or(' ')` or document the ASCII-only invariant.

### 🟡 m5 — `MIN_PHASE_BAR_WIDTH` accessed via `const _: u16 = …` workaround
**Sources:** code_quality_inspector
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:358-361`

The `const _: u16 = MIN_PHASE_BAR_WIDTH;` trick suppresses unused-constant warnings because the only consumer is a sibling submodule. Cleaner: declare `pub(super) const MIN_PHASE_BAR_WIDTH` (the consumer is a descendant, so `pub(super)` from `mod.rs` is sufficient) or relocate the constant into `frame_analysis_tab.rs` where it's exclusively consumed.

### 🟡 m6 — Footer hint `[j/k] Scroll` is shown while Details is focused but does nothing
**Sources:** risks_tradeoffs_analyzer (MEDIUM)
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` / `widgets/devtools/mod.rs:374-376`

When `focused_section == Details`, Up/Down/j/k/PgUp/PgDn/Home/End emit `PerfScroll*` messages that the handlers explicitly no-op (`frame.rs:90-94`, `131-135`, `163-165`, `186-188`). The footer still advertises `[j/k] Scroll`, falsely promising functionality. Either make hints section-aware (drop `[j/k]` on Details) or implicitly move focus back to FrameChart on scroll keys.

### 🟡 m7 — `MIN_DUAL_PANE_HEIGHT = 18` derivation comment math is incorrect
**Sources:** code_quality_inspector, logic_reasoning_checker (W4)
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:53-58`

Comment claims `chart_inner ≥ 10 + details_inner ≥ 8 = 18`. With `MIN_CHART_HEIGHT(4) + DETAIL_PANEL_HEIGHT(3) = 7` (not 10), and `usable.height - footer - borders` arithmetic that doesn't resolve to 10. The threshold may still be empirically correct, but the derivation is unverifiable from the comment. Tighten the math or bump the value.

### 🟡 m8 — Footer hint `[]/[] Tabs` reads as empty brackets
**Sources:** risks_tradeoffs_analyzer (LOW)
**File:** `crates/fdemon-tui/src/widgets/devtools/mod.rs:375`

`"[]/[] Tabs"` looks like an unfilled template. Consider `]/[  Tabs` (no surrounding brackets — the keys themselves are the brackets) for clarity.

### 🟡 m9 — Proportional bar visual contract not test-locked
**Sources:** risks_tradeoffs_analyzer (LOW)
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` tests

`renders_proportional_bar_when_phases_and_wide_enough` only asserts the four phase labels appear somewhere. Doesn't verify (a) segment widths sum to `area.width`, (b) `█` (U+2588) appears on the bar row, (c) widths are proportional to phase micros. A regression that draws zero bars or swaps colors would slip through.

### 🟡 m10 — Bar remainder always allocated to Raster
**Sources:** logic_reasoning_checker (W5)
**File:** `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:129-131`

`raster_cells = area.width - (build + layout + paint)` — when raster_micros is zero, the rounding remainder still creates a green Raster cell labeled "0.0ms". Mathematically the remainder should go to the largest segment. Cosmetic in practice.

### 🟡 m11 — Non-saturating u16 arithmetic in render paths
**Sources:** security_reviewer (MEDIUM, x2)
**Files:**
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:129-131` — `build_cells + layout_cells + paint_cells` computed as u16 before `saturating_sub`
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:174, 329` — `area.y + 1`, `area.y + 1 + i as u16` use plain `+`

Realistic terminal widths (≤ 500) keep this well within u16 bounds; not exploitable. But the project pattern elsewhere uses `saturating_sub` consistently. Either cast intermediate sums to u32 or use `saturating_add` for symmetry with the rest of the codebase.

### 🟡 m12 — `PerfSection::next()` and `prev()` have identical bodies
**Sources:** risks_tradeoffs_analyzer (LOW)
**File:** `crates/fdemon-app/src/session/performance.rs:28-41`

Both methods return the opposite variant — correct only for a 2-variant enum. A future 3rd variant (e.g., Filters section) would silently make `next == prev`, breaking Shift+Tab semantics. Add a comment warning, or rewrite to be n-arity safe.

### 🟡 m13 — Fallback values inconsistent across handlers
**Sources:** risks_tradeoffs_analyzer (LOW)
**File:** `crates/fdemon-app/src/handler/devtools/performance/frame.rs:114-118, 155-160`

`handle_perf_page` falls back to `DEFAULT_PERF_PAGE_SIZE = 10` when `frame_chart_visible_width.get() == 0`; `handle_perf_jump_to_start` falls back to `visible.max(1)`. Pre-first-render keypresses produce different scroll outcomes from these two paths. Trivial alignment.

---

## What's Good

- **Layer compliance** — zero upward imports in `fdemon-core`; `tui` correctly reads from `app`; no daemon touched. Compile-time enforced.
- **TEA discipline** — `handle_perf_cycle_details_tab` is pure; the `details_pane_visible_height.set()` write is correctly annotated with the exception comment at the source site (even if the central registry was missed).
- **Defensive belt-and-suspenders** — keys.rs gates `]`/`[` on `focused_section == Details`, AND `handle_perf_cycle_details_tab` re-checks the same condition. Insulates future mouse-click routing.
- **`frame_hints` module** — 15 inline tests cover 120 Hz, ordering invariants, balanced frames, max-cap, message length. Pure helper, no side effects.
- **Handler split** — `performance.rs` cleanly decomposed into `mod.rs` (re-exports), `frame.rs` (existing handlers moved file-for-file), `details.rs` (new cycle/focus). One net behavior change as planned.
- **Dual-pane layout fallback gradation** — three thresholds (compact → chart-only → dual-pane) all tested at boundary heights.
- **Phase 3 anchors** — `PerfDetailsTab::RebuildStats`/`TimelineEvents` variants and stub modules avoid a second state-shape migration. Stubs are isolated under `details/` and can't leak into Phase 2 logic.
- **No production unwraps**, no `unsafe`, no new IPC/file I/O/credentials.

---

## Doc Freshness Check

| Doc | Status |
|-----|--------|
| `docs/ARCHITECTURE.md` | ⚠️ Updated, but with 2 errors (M3 OverBudget field, m2 DetailsTab variant) |
| `docs/KEYBINDINGS.md` | ✅ `]`/`[` documented correctly with focus-gating note |
| `docs/REVIEW_FOCUS.md` | ⚠️ Should be updated (m1 missing Cell registrations) |
| `docs/CODE_STANDARDS.md` | ✅ No relevant changes |
| `docs/DEVELOPMENT.md` | ✅ No relevant changes |

---

## Recommendation

Address C1 (duplicate detail rendering), M1 (dead variable + false comment), M3 (ARCHITECTURE doc field error), and m1+m2 (REVIEW_FOCUS / ARCHITECTURE doc gaps) before considering Phase 2 closed.

Defer M2 (jank source-of-truth) to a Phase 3 task — add an explicit "migrate `is_janky` to refresh-rate-aware budget" entry to the Phase 3 plan now so the dependency is visible.

The rest (m3–m13) can be batched into a Phase 2-followup doc/cleanup task.

See `ACTION_ITEMS.md` in this directory for the prioritized fix list.
