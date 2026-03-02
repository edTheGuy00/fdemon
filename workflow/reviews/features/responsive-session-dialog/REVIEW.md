# Code Review: Responsive Session Dialog — Phase 1

**Date:** 2026-02-28
**Feature:** Space-Aware Compact/Expanded Decision
**Branch:** `feat/responsive-session-dialog`
**Verdict:** :warning: **APPROVED WITH CONCERNS**

---

## Change Summary

Decoupled the compact/expanded rendering decision from layout orientation (horizontal vs vertical). Instead, the dialog now checks actual available height against threshold constants to choose the right rendering mode for both `LaunchContext` and `TargetSelector`.

**Files Modified:**

| File | Lines Changed | Description |
|------|--------------|-------------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | +334/-8 | Constants, render logic, 9 unit tests |
| `workflow/plans/features/responsive-session-dialog/phase-1/TASKS.md` | status updates | Task tracking |

**Quality Gate:**
- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-tui` — 782 passed, 0 failed
- `cargo clippy --workspace -- -D warnings` — Clean

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor | Notes |
|-------|---------|----------|-------|-------|-------|
| Architecture Enforcer | :white_check_mark: PASS | 0 | 0 | 3 suggestions | All changes within TUI layer; TEA compliance verified |
| Code Quality Inspector | :white_check_mark: APPROVED (minor issues) | 0 | 0 | 3 minor, 3 nitpicks | Clean Rust idioms; test helper uses `unwrap()` instead of `expect()` |
| Logic Reasoning Checker | :warning: CONCERNS | 0 | 3 warnings | 2 notes | Threshold 28 vs min_height() 29 off-by-one; hysteresis deferred |
| Risks & Tradeoffs Analyzer | :warning: CONCERNS | 0 | 0 | 4 risks | Button clipping at threshold boundary; decoupled constants |

---

## Consolidated Findings

### Warnings (Should Address)

#### W1: `MIN_EXPANDED_LAUNCH_HEIGHT` = 28 may clip launch button at boundary

**Source:** Logic Reasoning Checker, Risks & Tradeoffs Analyzer
**File:** `mod.rs:136`

`LaunchContext::min_height()` returns 29 (25 fields + 1 spacer + 3 button). The threshold constant is set at 28, meaning expanded mode activates when the content area is exactly 28 rows — but the manually-positioned button extends to row 29, overflowing by 1 row. Ratatui silently clips the bottom border of the button.

The doc comment claims the `Min(0)` absorber makes 28 sufficient, but the button is positioned _outside_ the layout system via manual `Rect` calculation, so the absorber is irrelevant.

**Impact:** Cosmetic — button loses its bottom border at exactly 28 rows of content height (terminal heights ~51-53 in horizontal layout). Not a crash.

**Recommendation:** Either raise `MIN_EXPANDED_LAUNCH_HEIGHT` to 29, or add an explicit code comment documenting the clipping and that Phase 2 resolves it.

#### W2: `MIN_EXPANDED_LAUNCH_HEIGHT` and `LaunchContext::min_height()` are decoupled

**Source:** Risks & Tradeoffs Analyzer
**File:** `mod.rs:136` vs `launch_context.rs:847`

These represent the same concept (minimum height for expanded rendering) but are defined independently with different values (28 vs 29). If `calculate_fields_layout()` ever changes, one constant could become stale without a compile-time or test-time check.

**Recommendation:** Add a test asserting the relationship:
```rust
#[test]
fn test_expanded_threshold_matches_min_height() {
    assert!(MIN_EXPANDED_LAUNCH_HEIGHT >= LaunchContext::min_height() - 1);
}
```

#### W3: Hysteresis constants defined but unused (`#[allow(dead_code)]`)

**Source:** Architecture Enforcer, Risks & Tradeoffs Analyzer
**File:** `mod.rs:141-152`

`COMPACT_LAUNCH_HEIGHT_THRESHOLD` (24) and `COMPACT_TARGET_HEIGHT_THRESHOLD` (7) suppress dead-code warnings. Without hysteresis, rapid terminal resizing near the threshold will cause frame-by-frame flickering between compact and expanded modes.

**Recommendation:** Either remove constants until hysteresis is implemented (re-introduce in the task that wires them), or add a `// TODO(phase-2): wire into stateful hysteresis` comment with a task reference.

### Minor Issues

#### M1: Misleading doc comment on `test_standard_120x40_renders_without_panic`

**Source:** Code Quality Inspector
**File:** `mod.rs:1205`

Comment says "likely expanded" but the math proves it's compact (content height 20 < 28).

**Fix:** Change to `"horizontal layout, compact mode due to height constraint"`.

#### M2: Test helper `render_dialog()` uses `unwrap()` instead of `expect()`

**Source:** Code Quality Inspector
**File:** `mod.rs:716, 723`

The project's `TestTerminal::with_size()` uses `.expect()` for better failure messages. The new `render_dialog()` helper uses raw `.unwrap()`.

**Fix:** Replace with `.expect("render_dialog: terminal setup failed")` or refactor to use `TestTerminal`.

#### M3: Test comment overhead breakdown is opaque

**Source:** Code Quality Inspector
**File:** `mod.rs:1037`

`layout overhead = 2+1+1+1+1 = 6` — five addends are not explained.

**Fix:** Annotate as `header(2) + sep(1) + mid-sep(1) + footer-sep(1) + footer(1) = 6`.

### Suggestions (Non-blocking)

1. **Extract threshold helper function** — Replace repeated `chunks[n].height < CONSTANT` with `fn needs_compact_launch(height: u16) -> bool` to centralize the policy (Architecture Enforcer)
2. **Add `render_panes()` doc comment** noting TargetSelector always renders full in horizontal mode (Architecture Enforcer)
3. **Use `//` comments instead of `///` on private test helpers** — `test_dialog_state()` and `render_dialog()` are `#[cfg(test)]` private functions that won't appear in docs (Code Quality Inspector)
4. **Complete Phase 0** — Responsive Layout Guidelines for `docs/CODE_STANDARDS.md` were planned but not yet added (Risks & Tradeoffs Analyzer)

---

## Architectural Compliance

| Check | Status |
|-------|--------|
| Layer boundaries respected | :white_check_mark: All changes within `fdemon-tui` |
| TEA pattern compliance | :white_check_mark: Render functions are pure (read-only) |
| No new cross-layer dependencies | :white_check_mark: No new imports |
| Module organization | :white_check_mark: Changes scoped to existing widget module |
| No blocking in render loop | :white_check_mark: Only `u16` integer comparison |
| Widget builder pattern | :white_check_mark: `.compact(bool)` used correctly |

---

## Test Assessment

| Aspect | Assessment |
|--------|-----------|
| Coverage | 9 new tests across 4 groups (horizontal, vertical, boundary, regression) |
| Edge cases | Boundary conditions tested with margin; exact threshold tested at 50 vs 55 |
| Assertions | Content-based (`buffer_contains`) — no position-based fragility |
| Naming | Descriptive scenario-based names following project conventions |
| Gap | No test at exact content height = 28 to verify button clipping behavior |

---

## Risk Summary

| Risk | Severity | Mitigation |
|------|----------|------------|
| Button overflow at height = 28 | Medium | Phase 2 integrates button into layout system |
| Flickering during resize (no hysteresis) | Low | Cosmetic only; hysteresis constants ready for Phase 2 |
| TargetSelector never compact in horizontal | Low | Full mode degrades gracefully at small heights |
| Threshold constant drift | Low | Add coupling test (see W2) |

---

## Verdict Rationale

**APPROVED WITH CONCERNS** because:
- Core functionality is correct — height-based compact/expanded decisions work as intended
- All existing tests pass (773 original + 9 new = 782)
- Architecture is clean with no layer violations
- The off-by-one threshold issue (W1) is cosmetic, affects a narrow range of terminal heights, and is explicitly scoped for Phase 2
- No blocking issues identified by any reviewer agent

**Before merging**, address:
- [ ] W1: Fix or document the 28 vs 29 threshold discrepancy
- [ ] M1: Fix misleading test doc comment

**Tracked for follow-up:**
- [ ] W2: Add coupling test for threshold constants
- [ ] W3: Wire hysteresis or remove dead-code constants
- [ ] Phase 0: Add responsive layout guidelines to CODE_STANDARDS.md
