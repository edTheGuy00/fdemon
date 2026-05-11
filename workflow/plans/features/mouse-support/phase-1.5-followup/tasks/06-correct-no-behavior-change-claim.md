## Task: Correct the misleading "no behavior change" claim in Phase 1 plan docs

**Objective**: Fix the inaccurate "nothing changes" wording in Phase 1's TASKS.md overview (and the parallel claim in PLAN.md success criteria) so the plan accurately describes Phase 1's user-visible impact. The default `enable_mouse: true` is **kept** — only the wording changes.

**Depends on**: None

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md`: Reword the Phase 1 overview (line ~7) so it does not claim "nothing changes."
- `workflow/plans/features/mouse-support/PLAN.md`: Reword the Phase 1 milestone (line ~206) and Phase 1 success criteria (line ~447) similarly.

**Files Read (Dependencies):**
- `workflow/reviews/features/mouse-support-phase-1-foundation/REVIEW.md`: For the full reasoning behind why "no behavior change" is misleading (finding M3).

### Details

The Phase 1 review found that the claim "a user can scroll/click anywhere in fdemon and nothing changes" is false in user-perceived terms, because enabling mouse capture sends DECSET 1000/1002/1003/1015/1006 — sequences that intercept wheel scroll (so it no longer moves the host terminal's scrollback) and disable native text selection in many terminals (Shift+drag is needed to reach the selection layer beneath capture).

The intent — "no fdemon TEA-state change" — is correct. Only the wording is misleading.

**`phase-1-foundation/TASKS.md` line ~7** currently reads:

> When Phase 1 is done, a user can scroll/click anywhere in fdemon and nothing changes — but the terminal is never left in a broken state on crash, and `enable_mouse = false` truly disables capture (no escape sequences emitted). Phases 2+ rewrite `handle_mouse` to do real work.

Suggested rewording:

> When Phase 1 is done, mouse capture is on by default but no fdemon TEA-state changes in response to clicks or wheel events — `handle_mouse` is a no-op for every `UiMode`. Note that enabling capture itself **does** change the terminal's behavior visibly: wheel events that previously scrolled the host terminal's scrollback are now consumed by fdemon (and silently discarded for now), and many terminals require `Shift+drag` for native text selection while capture is on. Users who prefer the previous behavior can set `enable_mouse = false` to fully disable capture (no escape sequences emitted). The terminal is never left in a broken state on crash. Phases 2+ rewrite `handle_mouse` to do real work.

**`PLAN.md` line ~206** currently reads:

> **Milestone**: A user can scroll the wheel inside fdemon and nothing changes — no crashes, no terminal corruption on Ctrl-C, and `enable_mouse = false` truly disables the capture.

Suggested rewording:

> **Milestone**: Mouse events flow through the TEA bus and are consumed without any fdemon state change — `handle_mouse` is a no-op for every `UiMode`. Wheel events are intentionally captured (so they no longer move host-terminal scrollback when fdemon is focused); users who want native scrollback / native text selection without `Shift+drag` can set `enable_mouse = false`. Ctrl-C and panic paths leave the terminal usable.

**`PLAN.md` line ~447** currently reads:

> - [ ] Mouse events flow into the engine and are silently consumed (no behavior change)

Suggested rewording:

> - [ ] Mouse events flow into the engine and produce no fdemon TEA-state change (terminal-mode side effects of enabling capture are documented in `docs/CONFIGURATION.md`)

You may adjust phrasing while preserving accuracy. The minimum bar is: the plan no longer asserts "nothing changes" without qualification.

### Acceptance Criteria

1. `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md` no longer contains the unqualified phrase "nothing changes" (or equivalent) in the Phase 1 overview.
2. `workflow/plans/features/mouse-support/PLAN.md` Phase 1 milestone and Phase 1 success criterion are reworded to distinguish "no fdemon state change" from "no terminal-visible change."
3. The default `enable_mouse: true` is **not** changed in code or in PLAN.md — only the wording of the Phase 1 contract is corrected.
4. No other sections of TASKS.md or PLAN.md are altered.

### Testing

No code changes. Visual diff review only.

### Notes

- This is a documentation-only, workflow-only change. No code is touched.
- Why we are not flipping the default: see the orchestrator session decision (default `true` confirmed). The full discoverability story (e.g., a one-time first-launch hint) is out of scope for Phase 1.5; revisit in a later phase if user reports indicate confusion.
- The cross-reference to `docs/CONFIGURATION.md` in the suggested success-criteria rewording presumes Task 05 (document-enable-mouse-config) has landed or will land in the same wave. If Task 05 has not yet landed when you write this rewording, adjust the cross-reference accordingly.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md` | Rewrote Phase 1 overview paragraph (line 7) to accurately distinguish "no fdemon TEA-state change" from "no terminal-visible change"; explicitly documents that capture intercepts wheel events and requires Shift+drag for text selection |
| `workflow/plans/features/mouse-support/PLAN.md` | Rewrote Phase 1 milestone (line 206) to describe the TEA-bus no-op accurately while calling out the intentional capture side-effects; rewrote Phase 1 success criterion (line 446) to use "no fdemon TEA-state change" with a cross-reference to `docs/CONFIGURATION.md` |

### Notable Decisions/Tradeoffs

1. **Cross-reference to docs/CONFIGURATION.md retained**: Task 05 (document-enable-mouse-config) had not landed when this task ran, but the cross-reference is forward-looking and accurate — it describes where the documentation will live once Task 05 completes. Kept the reference rather than omitting it, as the task notes say "adjust accordingly" not "remove."
2. **Line 92 smoke-test entry left unchanged**: The acceptance criteria scopes the fix to the "Phase 1 overview" paragraph. Line 92 ("click anywhere → no behavior change, no crash") is in the manual smoke test checklist and refers to the absence of a crash, not the broad claim about user-visible impact. It was not changed per AC scope.

### Testing Performed

- Visual diff review of both modified files — confirms "nothing changes" (unqualified) is gone from PLAN.md and TASKS.md overview
- `grep -n "nothing changes"` across both files returned "NOT FOUND (expected)"
- No code changes; no build required

### Risks/Limitations

1. **No code impact**: This is a documentation-only change in workflow plan files. Zero risk of regression.
