# Phase 1.1: Review Fixes - Task Index

## Overview

Address issues identified during Phase 1 code review. This phase fixes a critical error handler bug, adds missing test coverage, and improves error messaging for better debugging.

**Total Tasks:** 3
**Estimated Hours:** 2-3 hours

## Background

Phase 1 completed the infrastructure for the startup flow consistency feature but code review identified:
- 1 critical issue (session creation error handler leaves invalid state)
- 1 major issue (missing test coverage for `AutoLaunchResult` handler)
- 3 minor issues (error message quality improvements)

**Review Reference:** `workflow/reviews/features/startup-flow-consistency-phase-1/`

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-fix-session-error-handler       │  Critical - Blocking
└───────────────┬─────────────────────┘
                │
                ▼
┌─────────────────────────────────────┐
│  02-add-auto-launch-result-tests    │  Major - Tests critical fix
└───────────────┬─────────────────────┘
                │
                ▼
┌─────────────────────────────────────┐
│  03-improve-error-messages          │  Minor - Polish
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-session-error-handler](tasks/01-fix-session-error-handler.md) | Done | - | 0.5h | `handler/update.rs` |
| 2 | [02-add-auto-launch-result-tests](tasks/02-add-auto-launch-result-tests.md) | Done | 1 | 1-1.5h | `handler/tests.rs` |
| 3 | [03-improve-error-messages](tasks/03-improve-error-messages.md) | Done | 1 | 0.5-1h | `tui/spawn.rs` |

## Success Criteria

Phase 1.1 is complete when:

- [x] Session creation error shows `StartupDialog` with error message (not silent)
- [x] `AutoLaunchResult` handler has unit tests for success and error paths
- [x] All `unwrap()` calls have descriptive `expect()` messages
- [x] Silent fallbacks log warning messages
- [x] `cargo fmt && cargo check && cargo clippy -- -D warnings` passes
- [x] `cargo test --lib` passes (including new tests)

## Re-review Checklist

After completing Phase 1.1:

- [x] Error path shows `StartupDialog` with error message
- [ ] Manual test: simulate session creation failure
- [x] No new warnings from clippy
- [x] Test coverage for all `AutoLaunchResult` branches

## Notes

- This phase must complete before Phase 2 (runner integration) can proceed
- The critical fix follows the same pattern as the device-discovery-failure handler (lines 1714-1721 in `update.rs`)
- E2E test failures mentioned in Phase 1 are pre-existing and unrelated
