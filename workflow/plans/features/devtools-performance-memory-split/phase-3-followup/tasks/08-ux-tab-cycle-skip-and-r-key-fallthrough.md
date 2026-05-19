# 08 — UX: Tab Cycle Skip + R-Key Fallthrough

**Wave:** 3
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1.5–2h
**Addresses:** M1, M4

## Context

Two related UX fixes:

- **M1.** When `rebuild_stats_enabled == false`, `PerfDetailsTab::next()` (`crates/fdemon-app/src/state.rs:198–204`) still cycles through `RebuildStats` in the state machine. The renderer's `effective_tab()` falls through to `TimelineEvents` for visual purposes, so press 1 of `]` from FrameAnalysis advances state to `RebuildStats` but renders TimelineEvents; press 2 advances state to `TimelineEvents` and renders the same content. The user experiences one apparent "dead" press. Per `TASKS.md:136`, cycling should be `FrameAnalysis → TimelineEvents → FrameAnalysis` when rebuild tracking is off.
- **M4.** `handler/keys.rs:544–555` unconditionally early-returns when `Char('R')` is pressed in DevTools mode + Performance + Details focused, returning `Some(ToggleRebuildStats)` only when on the RebuildStats tab and `None` otherwise. The current behavior is that `R` in Inspector / Memory / Network / Performance-with-FrameChart-focused / Performance-with-FrameAnalysis-or-TimelineEvents is a **silent no-op** — it does NOT fall through to the global `Char('R')` arm which maps to `HotRestart`. Surprising for muscle memory.

Per PLAN.md Design Decision §2, the fix for M4 is **option (i) — relax the early-return** so `R` outside the exact `(Performance + Details + RebuildStats)` context falls through to global `HotRestart`. The existing misleadingly-named test `test_capital_r_on_frame_analysis_tab_triggers_hot_restart` (which currently only asserts `!ToggleRebuildStats`) becomes accurate when its assertion is upgraded to assert `Some(Message::HotRestart)`.

## Acceptance Criteria

1. **M1 resolved.** `PerfDetailsTab` exposes a tab-cycle method that takes visibility into account. Two acceptable shapes:
   - **(a)** Extend `PerfDetailsTab::next` to take a `rebuild_stats_enabled: bool` parameter and skip `RebuildStats` when false.
   - **(b)** Add a parallel `next_visible(rebuild_stats_enabled: bool) -> Self` method alongside the existing `next`.
   - Choose (b) if `next` has other call sites that don't care about visibility; otherwise (a) is simpler.
2. **M1 handler wiring.** `handle_perf_cycle_details_tab` in `handler/devtools/performance/details.rs` reads `rebuild_stats_enabled` from the session's `PerformanceState` and uses the visibility-aware cycle method.
3. **M1 tests:**
   - `test_cycle_skips_rebuild_stats_when_disabled` — cycles `FrameAnalysis → TimelineEvents → FrameAnalysis` when off.
   - `test_cycle_includes_rebuild_stats_when_enabled` — cycles `FrameAnalysis → RebuildStats → TimelineEvents → FrameAnalysis` when on.
4. **M4 resolved.** In `handler/keys.rs`, the `Char('R')` early-return inside the DevTools-mode block:
   - Returns `Some(Message::ToggleRebuildStats)` ONLY when `focused_section == Details && details_tab == RebuildStats`.
   - In all other DevTools contexts (Performance/FrameChart, Performance/Details with FrameAnalysis or TimelineEvents focused, Inspector, Memory, Network), falls through to the existing `Char('R') if !is_busy => Some(Message::HotRestart)` arm in the main match.
5. **M4 test updates:**
   - Rename or rewrite `test_capital_r_on_frame_analysis_tab_triggers_hot_restart` so the assertion is `assert_eq!(msg, Some(Message::HotRestart))`. The name now accurately describes the behavior.
   - Add `test_capital_r_on_inspector_panel_triggers_hot_restart`, `test_capital_r_on_memory_panel_triggers_hot_restart`, `test_capital_r_on_network_panel_triggers_hot_restart`, `test_capital_r_on_frame_chart_focused_triggers_hot_restart`, `test_capital_r_on_timeline_events_tab_triggers_hot_restart` — each asserts `Some(Message::HotRestart)`.
   - Keep `test_capital_r_on_rebuild_stats_tab_emits_toggle_rebuild_stats` (or equivalent) unchanged — preserves the contextual toggle behavior.
6. `cargo fmt --all -- --check && cargo check -p fdemon-app && cargo test -p fdemon-app && cargo clippy -p fdemon-app --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-app/src/state.rs` — extend or add visibility-aware tab cycle on `PerfDetailsTab`.
- `crates/fdemon-app/src/handler/devtools/performance/details.rs` — `handle_perf_cycle_details_tab` uses the new method.
- `crates/fdemon-app/src/handler/keys.rs` — relax `Char('R')` early-return; update/add tests.

## Files Read (Dependencies)

- `crates/fdemon-app/src/session/performance.rs` — confirm `rebuild_stats_enabled` field name.
- `crates/fdemon-app/src/handler/keys.rs` — confirm the existing main-match `Char('R') if !is_busy` arm path.

## Approach Hints

- For M1: option (b) — `next_visible` — keeps `next` semantics intact and matches the project's pattern of adding parameterized variants rather than mutating signatures.
- The simplest M1 implementation: in `next_visible`, if `rebuild_stats_enabled == false`, route `FrameAnalysis → TimelineEvents` and `TimelineEvents → FrameAnalysis` directly, skipping the `RebuildStats` step.
- For M4: the relaxation is a structural change to the early-return guard. The existing nested-condition pattern should be inverted: only early-return `Some(ToggleRebuildStats)` when ALL conditions match; otherwise, fall through (not `return None`) so the main match can apply.
- Watch out: the current code may be doing `if in_performance && focused_section == Details { match ... }`. The fix may require restructuring the outer guard to NOT swallow the `Char('R')` case for non-RebuildStats sub-contexts.

## Out of Scope

- Any change to the prev-cycle direction (Shift+Tab / `[`) — if it has the same issue, it can be addressed similarly in a follow-up. For this task, focus on `]` (forward cycle).
- Footer hint text changes — if the existing hint mentions `R` ambiguously, leave it; the doc update task (T12) will revisit.
- Removing the `is_busy` check from the global `Char('R')` arm — out of scope; preserve existing busy-state guard semantics.
- Changing the test naming convention for the existing rebuild-stats tests.
- Refactoring `PerfDetailsTab` into a different enum shape.
