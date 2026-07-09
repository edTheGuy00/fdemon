# Tasks: wrap-scroll-bounds-drift (issue #73)

Plan: [BUG.md](BUG.md) · Research: [research/RESEARCH.md](research/RESEARCH.md)
PHASE_BASE: 7f66caeea19f3a67ffd8819f8a7029f8cbf3c65b (branch `fix/73-wrap-scroll-bounds`)

## File Overlap Analysis

| Task | Files Modified (Write) | Read-only deps |
|------|------------------------|----------------|
| 01-exact-frame-line-estimate | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` | `crates/fdemon-app/src/log_view_state.rs` (shared helpers), `crates/fdemon-core/src/stack_trace.rs` (StackFrame fields) |

### Overlap Matrix

Single task — no wave peers, no overlap. **Strategy: sequential (main loop), no worktree.**

## Wave 1

- [x] **01-exact-frame-line-estimate** — `tasks/01-exact-frame-line-estimate.md`
  - Complexity: medium
  - Agent: implementor (sequential, main loop)
  - Done: commit fe151802, validator PASS (all 8 criteria; gates green)

## Notes

- Deferred (from BUG.md Further Considerations): per-entry row-count cache → file as follow-up issue after merge.
- Pre-existing, out of scope: message-line link badges unmeasured by estimate (transient link-mode-only inaccuracy) — noted for the follow-up issue.
- Review round 0 concerns (APPROVED_WITH_CONCERNS, all Minor — see ../../../reviews/wrap-scroll-bounds-drift/REVIEW.md):
  - Wrapped collapsed-indicator render-loop accounting → **fixed inline** post-review (commit 517b5837, red/green-verified region-alignment test).
  - Deferred to follow-up issue: hardcoded INDENT/async-gap constants, 60-line function (optional collapse-helper extraction), message-line badge measurement, hot-path profiling (cache).

## Phase Review

| Round | Verdict | Review | Reviewed HEAD |
|-------|---------|--------|---------------|
| 0 | ✅ APPROVED_WITH_CONCERNS | ../../../reviews/wrap-scroll-bounds-drift/REVIEW.md | fe151802 (+ inline minor fix 517b5837) |
