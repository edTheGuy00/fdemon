# Action Items: DevTools Performance Phase 2

**Review Date:** 2026-05-19
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 1 critical (C1), 3 major (M1, M2, M3)

---

## Critical Issues (Must Fix)

### 1. Suppress per-frame detail rendering in dual-pane mode
- **Source:** logic_reasoning_checker (W1)
- **Files:**
  - `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs:142, 187`
  - `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs:21-37`
- **Problem:** `FrameChart::render` and `FrameChart::render_with_regions` unconditionally call `render_detail_panel`. When a frame is selected, that panel renders `Frame #N  Total: X.Xms / UI: … / Raster: …` — the same data the Frame Analysis tab below shows in dual-pane mode. The doc comment claims "Used only in the chart-only fallback" but the code does not honor that contract. At 200×30 users see the selected frame's stats twice.
- **Required Action:** Either (a) pass a `dual_pane: bool` flag through `FrameChart::new` and skip `render_detail_panel` when true (the dual-pane caller in `performance/mod.rs::render_chart_only`/`render_chart_panel_dual` would set this), OR (b) when `selected_frame.is_some()` and the dual-pane path is taken, switch the detail panel to render only the no-selection summary line (FPS / Avg / Jank / Shader). Option (a) is cleaner — the caller already knows which mode it's in.
- **Acceptance:** Open Performance panel at 200×30, select a frame with `←`. Verify `Frame #N  Total:` appears exactly once on screen (in the Frame Analysis tab, not in the chart strip). Verify chart-only fallback at 200×16 still shows the frame detail strip.

---

## Major Issues (Should Fix)

### 1. Remove dead `render_width` binding and false comment
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:165-167`
- **Problem:** `let render_width = label.len().min(width as usize) as u16; ... let _ = render_width; // used implicitly by ratatui set_string` — the variable is genuinely unused, and the comment is wrong (`Buffer::set_string` takes no width parameter; it writes the entire string, clipping only at buffer edge).
- **Required Action:** Delete both `render_width` lines. If defensive clipping is desired, truncate `label` explicitly: `let clipped: String = label.chars().take(width as usize).collect(); buf.set_string(label_x, label_y, &clipped, …)`. If the label-selection invariant upstream is trusted (it currently guarantees `label.len() <= width`), just delete the lines and add a one-line invariant comment at the label-selection block.
- **Acceptance:** No dead bindings; comment matches code semantics; `cargo clippy --workspace --all-targets -- -D warnings` still green.

### 2. Fix `OverBudget` field name in `docs/ARCHITECTURE.md`
- **Source:** architecture_enforcer, logic_reasoning_checker
- **File:** `docs/ARCHITECTURE.md:1091`
- **Problem:** Doc says `OverBudget { budget_ms, actual_ms }`. Code defines `OverBudget { excess_ms: f64, budget_ms: f64 }` (`frame_hints.rs:70-74`). `actual_ms` would be the full frame time; `excess_ms` is the overage. Semantically different — a Phase 3 implementor reading only the doc would emit the wrong message string.
- **Required Action:** Change line 1091 from `OverBudget { budget_ms, actual_ms }` to `OverBudget { excess_ms, budget_ms }`.
- **Acceptance:** Doc field names match the source.

### 3. Track `is_janky()` migration to `display_refresh_rate` as Phase 3 task
- **Source:** risks_tradeoffs_analyzer (HIGH)
- **Files:**
  - `crates/fdemon-core/src/performance.rs:261-263` (`is_janky` hard-coded to `FRAME_BUDGET_60FPS_MICROS`)
  - `crates/fdemon-app/src/session/performance.rs:228` (`PerformanceStats.jank_count` uses `is_janky()`)
  - `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs:258`, `frame_chart/detail.rs:159` (jank bar coloring, status label use `is_janky()`)
- **Problem:** Frame Analysis tab uses `display_refresh_rate` for the budget verdict. All other call sites use the hard-coded 60fps `is_janky()`. Today both agree because `display_refresh_rate = 60.0`. Phase 3 populating `display_refresh_rate` will create a visible split-brain ("Frame Analysis says JANK, chart bars say OK, summary says Jank: 0").
- **Required Action:** Add a Phase 3 task: "migrate `is_janky()` to consume `display_refresh_rate` (or take a `budget_micros` argument); update all 5+ call sites including `PerformanceStats.jank_count` aggregation". Do this BEFORE Phase 3 lands a `display_refresh_rate` writer.
- **Acceptance:** A new entry exists in the Phase 3 plan referencing this migration.

---

## Minor Issues (Consider Fixing — batch into a Phase 2-followup)

### Doc hygiene
1. **m1** — Register `details_pane_visible_height` (new) AND `frame_chart_visible_width` (pre-existing gap) in `docs/REVIEW_FOCUS.md` "Current usage" list. The doc's own policy requires this for new Cell render-hint fields.
2. **m2** — Fix `docs/ARCHITECTURE.md:1044`: change `DetailsTab` to `Details` (PerfSection variant name).

### Code cleanup
3. **m3** — Update stale `// Unreachable via Tab` comments at `crates/fdemon-app/src/handler/devtools/performance/frame.rs:164, 187` — the Details arm IS reachable now.
4. **m4** — Replace `&name[..1]` byte-slice with `name.chars().next().unwrap_or(' ')` at `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:151-152` to avoid future UTF-8 panic.
5. **m5** — Replace `const _: u16 = MIN_PHASE_BAR_WIDTH` workaround in `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:358-361` with `pub(super) const MIN_PHASE_BAR_WIDTH` (or relocate the constant into `frame_analysis_tab.rs`).
6. **m7** — Either bump `MIN_DUAL_PANE_HEIGHT` to make the derivation comment in `performance/mod.rs:53-58` honest, or rewrite the comment so the arithmetic matches the value.
7. **m11** — Use `saturating_add`/u32 intermediates for u16 coordinate arithmetic in `frame_analysis_tab.rs:129-131, 174, 329` for symmetry with the rest of the codebase.
8. **m12** — Add a comment near `PerfSection::next/prev` (`session/performance.rs:28-41`) noting the implementation assumes exactly 2 variants, or rewrite for n-arity safety.
9. **m13** — Align fallback values in `handle_perf_page` and `handle_perf_jump_to_start` (both should use `DEFAULT_PERF_PAGE_SIZE`).

### UX polish
10. **m6** — Make footer hints section-aware: hide `[j/k] Scroll` when `focused_section == Details` (mirror the pattern Memory panel uses).
11. **m8** — Disambiguate footer `[]/[] Tabs` to `]/[  Tabs` (the bracket characters are the keys themselves).
12. **m10** — Allocate proportional-bar rounding remainder to the largest segment rather than always to Raster.

### Test coverage
13. **m9** — Add a regression test asserting the proportional bar's `█` count per segment matches `(phase_micros * width / total).round()` within ±1 column.

---

## Re-review Checklist

After addressing critical and major issues, the following must pass:

- [ ] **C1**: Selected-frame detail appears exactly once at 200×30 (Frame Analysis tab only, not in chart strip); chart-only fallback at 200×16 unchanged.
- [ ] **M1**: Dead `render_width` binding removed; comment matches code.
- [ ] **M2**: `docs/ARCHITECTURE.md:1091` field name corrected.
- [ ] **M3**: Phase 3 plan contains an entry for `is_janky` migration.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` green.
- [ ] Manual smoke test (Phase Acceptance Test Plan in TASKS.md steps 1-12) re-run.

After the followup task addresses minors:

- [ ] `docs/REVIEW_FOCUS.md` lists both `Cell<usize>` render-hint fields on `PerformanceState`.
- [ ] `docs/ARCHITECTURE.md` variant names match code.
- [ ] No `// Unreachable via Tab` comments remain in `frame.rs`.
