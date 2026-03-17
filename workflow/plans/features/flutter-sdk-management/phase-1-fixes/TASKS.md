# Phase 1 Fixes: Review Remediation - Task Index

## Overview

Address all issues found during the Phase 1 code review. One critical bug (ToolAvailability overwrite), one major refactor (locator.rs), and a batch of minor code quality fixes.

**Total Tasks:** 3
**Review:** `workflow/reviews/features/flutter-sdk-management-phase-1/REVIEW.md`

## Task Dependency Graph

```
┌──────────────────────────┐     ┌──────────────────────────┐
│  01-fix-tool-availability│     │  02-refactor-locator     │
│  (CRITICAL bug fix)      │     │  (locator.rs refactor)   │
└──────────────────────────┘     └────────────┬─────────────┘
                                              │
                                              ▼
                                 ┌──────────────────────────┐
                                 │  03-minor-quality-fixes  │
                                 │  (code quality cleanup)  │
                                 └──────────────────────────┘
```

### Parallelism

| Wave | Tasks | Can Run In Parallel |
|------|-------|-------------------|
| 1 | 01, 02 | Yes |
| 2 | 03 | No (depends on 02 — shares locator.rs) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-tool-availability](tasks/01-fix-tool-availability.md) | Not Started | - | `fdemon-app: handler/update.rs, handler/tests.rs` |
| 2 | [02-refactor-locator](tasks/02-refactor-locator.md) | Not Started | - | `fdemon-daemon: flutter_sdk/locator.rs, flutter_sdk/types.rs` |
| 3 | [03-minor-quality-fixes](tasks/03-minor-quality-fixes.md) | Not Started | 02 | `fdemon-daemon: flutter_sdk/version_managers.rs, channel.rs, locator.rs` `fdemon-app: engine.rs, handler/tests.rs` |

## Success Criteria

Phase 1 fixes are complete when:

- [ ] `ToolAvailabilityChecked` handler preserves `flutter_sdk` and `flutter_sdk_source` fields
- [ ] Test verifies flutter_sdk fields survive ToolAvailabilityChecked processing
- [ ] `find_flutter_sdk` function body is under 100 lines (refactored with `try_resolve_sdk` helper)
- [ ] `read_version_file` failures fall through to next strategy instead of aborting the chain
- [ ] Bare PATH fallback is removed (or uses a distinct `SdkSource` variant)
- [ ] Tests exist for `SdkResolved` and `SdkResolutionFailed` handlers
- [ ] All version_managers.rs signatures use bare `Result<>` alias
- [ ] No duplicate info! logs on startup
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- Tasks 01 and 02 touch entirely different files and can be dispatched in parallel.
- Task 03 touches `locator.rs` (test fixes) so must wait for Task 02 to complete.
- All changes should be on the existing `feature/flutter-sdk-management` branch.
