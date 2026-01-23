# Phase 5: Review Follow-Up Fixes - Task Index

## Overview

Address issues identified in the code review of Phases 1, 2, and 4 implementations. Includes 3 critical issues, 5 major issues, and selected minor improvements.

**Total Tasks:** 8
**Priority:** High (blocks merge)

## Task Dependency Graph

```
Wave 1 (Independent - can run in parallel):
┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
│ 01-duplicate-cache  │  │ 02-validation-bypass│  │ 03-unwrap-safety    │
└─────────────────────┘  └─────────────────────┘  └─────────────────────┘

┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
│ 04-error-clearing   │  │ 05-loop-bounds      │  │ 06-tool-timeout     │
└─────────────────────┘  └─────────────────────┘  └─────────────────────┘

Wave 2 (After Wave 1):
┌─────────────────────────────────────────────────────────────────────────┐
│                        07-vertical-space-validation                      │
│                           (requires manual testing)                      │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                        08-width-threshold-adjustment                     │
│                           (requires manual testing)                      │
└─────────────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Priority | Status | Depends On | Modules |
|---|------|----------|--------|------------|---------|
| 1 | [01-duplicate-cache](tasks/01-duplicate-cache.md) | Critical | Not Started | - | `state.rs` |
| 2 | [02-validation-bypass](tasks/02-validation-bypass.md) | Critical | Not Started | - | `launch_context.rs` (handler) |
| 3 | [03-unwrap-safety](tasks/03-unwrap-safety.md) | Major | Not Started | - | `launch_context.rs` (handler) |
| 4 | [04-error-clearing](tasks/04-error-clearing.md) | Major | Not Started | - | `target_selector.rs` |
| 5 | [05-loop-bounds](tasks/05-loop-bounds.md) | Major | Not Started | - | `state.rs` (new_session_dialog) |
| 6 | [06-tool-timeout](tasks/06-tool-timeout.md) | Major | Not Started | - | `spawn.rs`, `update.rs` |
| 7 | [07-vertical-space-validation](tasks/07-vertical-space-validation.md) | Critical | Not Started | 1-6 | Manual testing |
| 8 | [08-width-threshold-adjustment](tasks/08-width-threshold-adjustment.md) | Major | Not Started | 1-6 | `launch_context.rs` (widget) |

## Issue Mapping

| Review Issue | Task | Severity |
|--------------|------|----------|
| Critical #1: Duplicate Cache Checking | 01-duplicate-cache | Critical |
| Critical #2: Auto-Config Validation Bypass | 02-validation-bypass | Critical |
| Critical #3: Vertical Space Budget | 07-vertical-space-validation | Critical |
| Major #1: Unwrap Calls in Handler | 03-unwrap-safety | Major |
| Major #2: Error Not Cleared | 04-error-clearing | Major |
| Major #3: Width Threshold | 08-width-threshold-adjustment | Major |
| Major #4: Tool Availability Timeout | 06-tool-timeout | Major |
| Major #5: Unbounded Loop | 05-loop-bounds | Major |

## Success Criteria

Phase 5 is complete when:

- [ ] All critical issues resolved (3)
- [ ] All major issues resolved (5)
- [ ] `cargo fmt` passes
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Manual testing at minimum terminal dimensions (80x20) passes

## Notes

- Tasks 1-6 are code changes that can be implemented in parallel
- Tasks 7-8 require manual testing after code changes are complete
- All tasks should follow the project's no-panic policy (CODE_STANDARDS.md)
- Verification command: `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings`
