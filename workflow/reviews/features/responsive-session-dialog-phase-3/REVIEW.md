# Code Review: Phase 3 — Fix Scroll-to-Selected in Target Selector

**Review Date:** 2026-03-01
**Feature:** responsive-session-dialog / phase-3
**Branch:** feat/responsive-session-dialog
**Verdict:** APPROVED WITH CONCERNS

---

## Summary

Phase 3 closes the render-to-state feedback loop for scroll-to-selected behavior in the target selector. The implementation adds a `Cell<usize>` field to `TargetSelectorState` that the renderer writes each frame, the handler reads on key events, and a render-time scroll correction ensures the selected device is always visible regardless of terminal size.

The implementation is **logically sound** — the feedback loop cannot oscillate or diverge, edge cases are handled, and 11 new tests provide solid coverage. The core concern is that the render-to-state write via `Cell<usize>` deliberately violates the project's documented TEA purity rule without updating the project documentation to acknowledge the exception.

---

## Changes Reviewed

| File | Layer | Changes |
|------|-------|---------|
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | app | Added `pub last_known_visible_height: Cell<usize>` field + 4 unit tests |
| `crates/fdemon-app/src/handler/new_session/target_selector.rs` | app | Added `effective_visible_height()` helper, updated both handlers + 3 tests |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | tui | Renderer writes visible height, computes corrected scroll + 4 tests |
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | tui | Formatting only (cargo fmt) |

**Total:** +513 / -18 lines across 4 source files

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor |
|-------|---------|----------|-------|-------|
| Architecture Enforcer | CONCERNS | 0 | 0 | 2 warnings, 3 suggestions |
| Code Quality Inspector | CONCERNS | 0 | 2 | 4 minor, 2 nitpicks |
| Logic & Reasoning | PASS | 0 | 0 | 2 warnings (design-level), 3 notes |
| Risks & Tradeoffs | CONCERNS | 0 | 0 | 3 medium, 1 low |

---

## Consolidated Findings

### Theme 1: TEA Purity Violation Needs Documentation (all 4 agents)

**Severity:** Major (documentation gap, not a correctness bug)

Both `render_full()` and `render_compact()` call `self.state.last_known_visible_height.set(visible_height)` — a write inside what should be a pure render function. This directly contradicts `docs/REVIEW_FOCUS.md` line 15:

> "View function purity: `tui::render()` should only read state, never mutate"

The PLAN document endorses this as "a pragmatic concession common in TUI frameworks" and the design is sound for a single numeric render-hint. However, the project documentation does not acknowledge this exception, creating confusion for future contributors and reviewers.

**Recommendation:** Update `docs/REVIEW_FOCUS.md` to document the exception. Add inline comments at each `Cell::set()` call site referencing the rationale. Consider adding a note to the `TargetSelectorState` struct-level doc comment that the struct contains an interior-mutable render-hint field.

---

### Theme 2: Duplicate `calculate_scroll_offset` (3 agents)

**Severity:** Minor (deferred by design, but needs tracking)

Two byte-for-byte identical implementations exist:
- **Private:** `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:370`
- **Public:** `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs:453`

TASKS.md explicitly defers deduplication: "No deduplication in this phase." However, there is no TODO comment or tracking mechanism to ensure follow-up.

**Recommendation:** Add a `// TODO: deduplicate with device_list::calculate_scroll_offset — move to fdemon-core` comment at the app-layer copy. The function is pure arithmetic with no crate dependencies, making `fdemon-core` the natural home.

---

### Theme 3: Handler Borrow Ordering Relies on Two-Phase Borrows (1 agent)

**Severity:** Minor (compiles correctly but non-obvious to readers)

```rust
state.new_session_dialog_state.target_selector
    .adjust_scroll(effective_visible_height(state));
```

This compiles via Rust's two-phase borrow semantics — the mutable receiver borrow is "reserved" while `effective_visible_height(state)` completes its shared borrow. This is correct but non-obvious. The task completion summary's explanation of why this works is also technically inaccurate.

**Recommendation:** Extract to an explicit `let height = effective_visible_height(state);` binding before the `adjust_scroll` call:

```rust
let height = effective_visible_height(state);
state.new_session_dialog_state.target_selector.adjust_scroll(height);
```

---

### Theme 4: `corrected_scroll` Not Persisted to State (2 agents)

**Severity:** Minor (correct by design, edge case only)

The render-time `corrected_scroll` is used for display only — never written back to `state.scroll_offset`. This means after a terminal resize, the handler's next calculation uses a stale base offset. The logic reviewer confirmed this converges within one frame and cannot diverge, but it could produce a brief visual "bounce" during simultaneous resize + scroll.

**Recommendation:** Document this interaction in the code comments at the `corrected_scroll` computation sites. No code change needed — the design is intentional and the UX impact is minimal.

---

### Theme 5: Test Assertion Weakness (1 agent)

**Severity:** Minor

In `test_handle_device_up_uses_actual_height`, the comment claims "adjust_scroll sets scroll_offset = selected_index = 1" but the test only asserts `sel >= offset` (a weak invariant). The exact scroll_offset value is not tested.

**Recommendation:** Either add `assert_eq!(offset, 1)` to match the comment, or soften the comment to match the actual assertion.

---

## Logic Verification

The logic reviewer confirmed:

- The feedback loop is **stable** — `calculate_scroll_offset` is a pure idempotent function with a fixed point. No oscillation or divergence is possible.
- **First-frame fallback** is correct — `Cell::new(0)` triggers `DEFAULT_ESTIMATED_VISIBLE_HEIGHT = 10` on the first key press, then the render writes the actual height.
- **Terminal resize** is handled — the renderer always corrects stale scroll offsets, and `last_known_visible_height` is updated for the next handler call.
- **Edge cases** are covered — zero devices, single device, zero-height terminal, extremely large lists all behave correctly.
- **`Cell<usize>` safety** confirmed — single-threaded, no reentrancy, `!Sync` prevents accidental cross-thread sharing.
- **Clone semantics** are correct — `Cell<usize>` copies the inner value, no aliasing issues.

---

## Quality Metrics

| Metric | Score | Notes |
|--------|-------|-------|
| Architecture | 4/5 | Layer boundaries respected; TEA exception is pragmatic but undocumented |
| Code Quality | 4/5 | Clean Rust idioms; `Cell` used correctly; borrow ordering could be clearer |
| Logic & Correctness | 5/5 | Feedback loop is provably stable; all edge cases handled |
| Testing | 4/5 | 11 new tests with good coverage; one weak assertion |
| Documentation | 3/5 | Field-level docs are thorough; project-level TEA exception undocumented |
| Risk Management | 4/5 | Risks identified and mitigated; code duplication deferred but untracked |

---

## Action Items Summary

| # | Priority | Action | Files |
|---|----------|--------|-------|
| 1 | Should fix | Document TEA `Cell` exception in `docs/REVIEW_FOCUS.md` | `docs/REVIEW_FOCUS.md` |
| 2 | Should fix | Extract `effective_visible_height` call to explicit `let` binding | `handler/new_session/target_selector.rs` |
| 3 | Should fix | Add TODO comment for `calculate_scroll_offset` deduplication | `target_selector_state.rs` |
| 4 | Nice to have | Add struct-level doc note about interior mutability | `target_selector_state.rs` |
| 5 | Nice to have | Strengthen or fix comment in `test_handle_device_up_uses_actual_height` | `handler/new_session/target_selector.rs` |
| 6 | Nice to have | Add comment explaining `corrected_scroll` is intentionally ephemeral | `target_selector.rs` (TUI) |

---

## Verification

```
cargo fmt --all              — PASS
cargo check --workspace      — PASS
cargo test --workspace       — PASS (2525+ tests)
cargo clippy --workspace -- -D warnings — PASS
```
