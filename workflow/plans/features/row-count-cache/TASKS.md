# Tasks: row-count-cache (issue #75)

Plan: [PLAN.md](PLAN.md) · Research: [research/RESEARCH.md](research/RESEARCH.md)
PHASE_BASE: eeb87633 (branch `feat/75-row-count-cache`, stacked on PR #74 head)

## File Overlap Analysis

| Task | Files Modified (Write) | Read-only deps |
|------|------------------------|----------------|
| 01-row-count-cache | `crates/fdemon-app/src/log_view_state.rs`, `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` | `crates/fdemon-app/src/hyperlinks.rs`, `crates/fdemon-app/src/session/{session,collapse}.rs` |
| 02-review-focus-registry | `docs/REVIEW_FOCUS.md` | task 01's diff |

### Overlap Matrix

01 vs 02: no shared write files, but 02 documents 01's output → **dependency chain, sequential**. Strategy: sequential (main loop), no worktrees.

## Wave 1

- [x] **01-row-count-cache** — `tasks/01-row-count-cache.md`
  - Complexity: medium
  - Agent: implementor (sequential, main loop)
  - Done: commit 59f76f3c, validator PASS (sentinel probe + load-bearing badge test verified; both-toolchain gates green)

## Wave 2 (after 01)

- [x] **02-review-focus-registry** — `tasks/02-review-focus-registry.md`
  - Complexity: low
  - Agent: doc_maintainer
  - Done: commit 671ddb37, validated by inspection (single entry, all content requirements present, correct placement)

## Notes (additional)

- doc_maintainer flag: docs/REVIEW_FOCUS.md is at ~294 lines vs its 200-line cap (pre-existing bloat, mostly "Approved Optimizations") — schedule a docs-sync/compaction pass separately.

## Notes

- Branch stacked on #74 — rebase onto main after #74 merges, before opening the PR.
- Out of scope (file as new issue after landing): link rescan-on-filter-change gap + raw-index eviction shift (pre-existing link-mode staleness; formatter and estimate affected identically).

## Phase Review

| Round | Verdict | Review | Reviewed HEAD |
|-------|---------|--------|---------------|
| 0 | ✅ APPROVED_WITH_CONCERNS | ../../../reviews/row-count-cache/REVIEW.md | 671ddb37 (+ inline polish 261609fd, doc addendum d14be711) |
