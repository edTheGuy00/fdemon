# Phase 5b: Review Fixes — Task Index

## Overview

Address the 8 issues identified in the [Phase 5 Review](../../../../reviews/features/session-resilience-phase-5/REVIEW.md). Two Major issues (mutex unwrap, unused parameter) and six Minor issues (constants, visibility, imports, tests) carried over from the original monolithic `actions.rs`.

**Total Tasks:** 5

## Task Dependency Graph

```
Wave 1 — Independent (can be parallelized)
┌─────────────────────────────────┐  ┌─────────────────────────────────┐
│ 01-fix-mutex-unwrap             │  │ 02-remove-unused-msg-tx         │
│ (Major: Issue #1)               │  │ (Major: Issue #2)               │
└─────────────────────────────────┘  └─────────────────────────────────┘

┌─────────────────────────────────┐  ┌─────────────────────────────────┐
│ 03-named-constants-and-imports  │  │ 04-tighten-module-visibility    │
│ (Minor: Issues #3, #5, #6)     │  │ (Minor: Issue #4)               │
└────────────────┬────────────────┘  └─────────────────────────────────┘
                 │
Wave 2 — Depends on constant promotion from Task 03
                 │
┌────────────────┴────────────────┐
│ 05-add-test-modules             │
│ (Minor: Issues #7, #8)         │
└─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-mutex-unwrap](tasks/01-fix-mutex-unwrap.md) | Done | - | `actions/mod.rs` |
| 2 | [02-remove-unused-msg-tx](tasks/02-remove-unused-msg-tx.md) | Done | - | `actions/network.rs`, `actions/mod.rs` |
| 3 | [03-named-constants-and-imports](tasks/03-named-constants-and-imports.md) | Done | - | `actions/vm_service.rs`, `actions/network.rs`, `actions/inspector/mod.rs` |
| 4 | [04-tighten-module-visibility](tasks/04-tighten-module-visibility.md) | Done | - | `actions/mod.rs`, `actions/session.rs` |
| 5 | [05-add-test-modules](tasks/05-add-test-modules.md) | Done | 3 | `actions/vm_service.rs`, `actions/performance.rs`, `actions/network.rs`, `actions/inspector/mod.rs` |

## Success Criteria

Phase 5b is complete when:

- [x] Zero `unwrap()` calls on mutex locks in `actions/` directory
- [x] No unused parameters in any function signature
- [x] All timeout/duration values use named constants
- [x] All submodule declarations use `pub(super)` visibility
- [x] All `use` declarations at top-level (no inline `use` in async blocks)
- [x] Every file in `actions/` has a `#[cfg(test)]` module with at least one assertion
- [x] `cargo fmt --all` clean
- [x] `cargo check --workspace` passes
- [x] `cargo test --workspace` passes
- [x] `cargo clippy --workspace -- -D warnings` clean

## Notes

- Tasks 01-04 are all independent and can be dispatched in parallel (Wave 1).
- Task 05 depends on Task 03 because `LAYOUT_FETCH_TIMEOUT` must be promoted to module scope before `inspector/mod.rs` can have a constant verification test.
- All issues are pre-existing patterns carried over from the original monolithic `actions.rs` — none are regressions introduced by Phase 5.
