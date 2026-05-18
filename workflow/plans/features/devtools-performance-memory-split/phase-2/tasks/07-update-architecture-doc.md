## Task: Update Documentation for Performance Details Pane (Phase 2)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the new Performance dual-pane layout, the `PerfDetailsTab` enum and tab cycling, the three details tabs (Frame Analysis populated; Rebuild Stats / Timeline Events stubs), the responsive-layout thresholds, and the new `fdemon-core::frame_hints` helper module.

**Depends on**: 03 (handler split + key routing), 04 (widget shell), 05 (frame analysis content)

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — targeted edits to:
  1. **"Performance Panel Interactivity" section** (around line 1016): add subsections for the dual-pane layout, the `PerfDetailsTab` enum + tab cycling (`]`/`[` keys), the three details tabs, and the new state fields (`details_tab`, `details_pane_visible_height` `Cell`, `display_refresh_rate`).
  2. **Workspace layout / file tree** (around line 280–390): under `fdemon-tui/widgets/devtools/performance/`, add the new `details/` subtree (`mod.rs`, `frame_analysis_tab.rs`, `rebuild_stats_tab.rs`, `timeline_events_tab.rs`). Under `fdemon-app/handler/devtools/`, mark `performance.rs` as split into `performance/{mod, frame, details}.rs`.
  3. **DevTools subsystem ASCII diagram** (around line 855–895): if the diagram lists per-panel widget names, add `details/` under the Performance arm. The Frame Chart widget reference stays.
  4. **`fdemon-core` module listing** (around line 240–290): under `crates/fdemon-core/src/`, add `frame_hints.rs` as a new module with a one-line description: `"Refresh-rate-aware frame analysis hints (Phase 2 helper)."`.
  5. **List of approved TEA exception cells** (where `frame_chart_visible_width` and `alloc_table_visible_height` are enumerated) — add `details_pane_visible_height` to that list.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- T01–T05 task specs and completion summaries.
- Current `docs/ARCHITECTURE.md` "Performance Panel Interactivity" subsection (lines 1016–1038) — model new content on the existing prose style.

### Change Context

Summarize what implementation changes require doc updates:

1. **`PerfDetailsTab` enum (state.rs) + tab cycling (`]`/`[`)** — Phase 2 introduces a new per-session tab enumeration with three variants. ARCHITECTURE.md must explain:
   - All three tabs are visible unconditionally (unlike Inspector's `DetailsTab::visible_tabs()` which conditionally hides).
   - Cycling is keyboard-only in Phase 2 (`]`/`[`); mouse clicks on tab labels are Phase 3.
   - Default tab on first render is `FrameAnalysis`; cycling is round-trip across all three.

2. **`PerformanceState` new fields** — `details_tab`, `details_pane_visible_height: Cell<usize>` (TEA render-hint exception), `display_refresh_rate: f64` (hard-coded 60.0 in Phase 2). Document the TEA exception in the same paragraph that lists the other Cell-based render hints.

3. **Dual-pane responsive layout** — three new thresholds in `widgets/devtools/performance/mod.rs`:
   - `MIN_DUAL_PANE_HEIGHT (18)` — chart + details. Below this → chart-only fallback.
   - `MIN_DETAILS_HEIGHT (8)` — details pane minimum.
   - `MIN_PHASE_BAR_WIDTH (40)` — proportional phase bar minimum. Below this → inline `B/L/P/R` summary.
   - `FRAME_CHART_PCT (55)` — frame chart's share of the dual-pane usable area.

4. **`fdemon-core::frame_hints`** — new module providing `frame_hints(frame, refresh_rate_hz) -> Vec<FrameHint>` and the `FrameHint` enum (`OverBudget`, `ShaderCompilation`, `LongestUiPhase`, `RasterDominant`, `BuildDominant`). Pure helper, no I/O, fully unit-tested. The TUI consumes this directly from `frame_analysis_tab`.

5. **Handler split** — `crates/fdemon-app/src/handler/devtools/performance.rs` is replaced by a directory module `performance/{mod, frame, details}.rs`. The split mirrors the inspector's `handler/devtools/inspector/` pattern.

6. **No new VM Service work** — explicitly call out that Phase 2 is data-complete with existing `FrameTiming.phases`. Phase 3 will add Rebuild Stats + Timeline Events RPCs.

### Acceptance Criteria

1. The "Performance Panel Interactivity" section accurately describes the dual-pane layout, the three responsive thresholds with derivation comments, the `PerfDetailsTab` enum + cycling, and the new state fields.
2. `details_pane_visible_height: Cell<usize>` is listed alongside `frame_chart_visible_width` and `alloc_table_visible_height` as an approved TEA render-hint exception.
3. The workspace file-tree diagram (in the workspace structure section) shows `crates/fdemon-tui/src/widgets/devtools/performance/details/` with the four files, and `crates/fdemon-app/src/handler/devtools/performance/` with the three files.
4. The `fdemon-core` module listing includes `frame_hints.rs` with a one-line description.
5. The DevTools subsystem ASCII diagram (if it shows per-panel widget names) lists `details/` under the Performance arm.
6. No content boundary violations:
   - **Code conventions** stay in CODE_STANDARDS.md, not ARCHITECTURE.md.
   - **Build / test commands** stay in DEVELOPMENT.md.
   - **User-facing key bindings** stay in KEYBINDINGS.md (T06's job).
7. Edits are **targeted** — no whole-section rewrites. Each addition slots into the existing structure cleanly.
8. Cross-references valid: any new section or subsection link uses the same anchor format as existing headings.

### Notes

- **Read the schemas first.** `~/.claude/skills/doc-standards/schemas.md` defines the canonical content boundaries. If a sentence describes a coding standard (e.g. "fields > 500 lines must be split"), it goes in CODE_STANDARDS.md, not ARCHITECTURE.md.
- **Do NOT touch KEYBINDINGS.md** — T06 owns the user-facing key documentation. T07's `]/[` references in ARCHITECTURE.md should be implementation-context (e.g. "Pressing `]`/`[` emits `Message::PerfCycleDetailsTab { forward }`"), not user-facing instruction.
- **Do NOT touch CODE_STANDARDS.md** unless a genuinely new convention emerges in Phase 2. The TEA render-hint pattern is already documented (Principle 3) and `details_pane_visible_height` is just another instance, not a new pattern.
- **Phase 3 anchors**: where Phase 2 ships a stub (Rebuild Stats / Timeline Events), the architecture doc may briefly note "populated in Phase 3" without committing to RPC details. The full Phase 3 architecture update happens in Phase 3's doc task.
- **`display_refresh_rate = 60.0` default**: document the hard-coded value with the same rationale the plan gives — Phase 3 may parse `Display.Refresh` events for 90/120 Hz devices; conservative 60 Hz is never wrong for `is_janky`.
- **Cell exception list**: search ARCHITECTURE.md for `alloc_table_visible_height` to find the list of approved TEA render-hint Cells; insert `details_pane_visible_height` alongside it in the same table / paragraph.

---

## Completion Summary

(Filled in by doc_maintainer after work completes.)
