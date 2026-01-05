# Log View Refactoring - Task Index

## Overview

This document tracks all tasks for refactoring the `log_view.rs` widget (2262 lines) into a modular directory structure. The refactoring follows Rust idioms and the project's existing patterns (e.g., `src/app/handler/tests.rs`).

**Total Tasks:** 7
**Estimated Effort:** 4-6 hours
**Priority:** High (Phase 1), Medium (Phase 2)

## Task Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Phase 1                                    │
│                    (Log View Module Refactoring)                        │
│                                                                         │
│  ┌──────────────────┐                                                   │
│  │ 01-create-module │                                                   │
│  │    -directory    │                                                   │
│  └────────┬─────────┘                                                   │
│           │                                                             │
│     ┌─────┴─────┐                                                       │
│     ▼           ▼                                                       │
│  ┌──────────┐  ┌──────────┐                                             │
│  │02-extract│  │03-extract│                                             │
│  │ -styles  │  │ -state   │                                             │
│  └────┬─────┘  └────┬─────┘                                             │
│       │             │                                                   │
│       └──────┬──────┘                                                   │
│              ▼                                                          │
│       ┌─────────────┐                                                   │
│       │ 04-migrate  │                                                   │
│       │   -widget   │                                                   │
│       └──────┬──────┘                                                   │
│              ▼                                                          │
│       ┌─────────────┐                                                   │
│       │ 05-extract  │                                                   │
│       │   -tests    │                                                   │
│       └──────┬──────┘                                                   │
│              ▼                                                          │
│       ┌─────────────┐                                                   │
│       │ 06-cleanup  │                                                   │
│       │  -verify    │                                                   │
│       └─────────────┘                                                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                              Phase 2                                    │
│                    (Documentation - Parallel)                           │
│                                                                         │
│       ┌──────────────────┐                                              │
│       │ 07-document-test │  (Can run in parallel with Phase 1)          │
│       │   -organization  │                                              │
│       └──────────────────┘                                              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules | Est. Time |
|---|------|--------|------------|---------|-----------|
| 1 | [01-create-module-directory](tasks/01-create-module-directory.md) | Done | - | `tui/widgets/` | 15 min |
| 2 | [02-extract-styles-module](tasks/02-extract-styles-module.md) | Done | 01 | `log_view/styles.rs` | 20 min |
| 3 | [03-extract-state-module](tasks/03-extract-state-module.md) | Done | 01 | `log_view/state.rs` | 45 min |
| 4 | [04-migrate-widget-implementation](tasks/04-migrate-widget-implementation.md) | Done | 01, 02, 03 | `log_view/mod.rs` | 1.5 hr |
| 5 | [05-extract-tests-module](tasks/05-extract-tests-module.md) | Done | 01-04 | `log_view/tests.rs` | 1 hr |
| 6 | [06-cleanup-and-verify](tasks/06-cleanup-and-verify.md) | Done | 01-05 | All | 30 min |
| 7 | [07-document-test-organization](tasks/07-document-test-organization.md) | Done | - | `docs/ARCHITECTURE.md` | 30 min |

## Success Metrics

### Code Organization
- [x] No single file exceeds 1100 lines (max: tests.rs at 1050)
- [x] Test code is isolated in dedicated `tests.rs` file
- [x] Module structure is clear and navigable

### Functionality
- [x] All 77 tests pass (originally estimated 68+)
- [x] `cargo check` passes with no new warnings
- [x] `cargo build --release` succeeds
- [x] Public API unchanged (`LogView`, `LogViewState` accessible)

### Documentation
- [x] ARCHITECTURE.md contains testing guidelines
- [x] Test organization is documented for contributors

## File Structure (Before → After)

**Before:**
```
src/tui/widgets/
├── log_view.rs          # 2262 lines (monolithic)
├── mod.rs
└── ...other widgets
```

**After:**
```
src/tui/widgets/
├── log_view/            # Module directory
│   ├── mod.rs           # ~980 lines (widget implementation)
│   ├── state.rs         # ~175 lines (LogViewState, FocusInfo)
│   ├── styles.rs        # ~40 lines (stack trace styling)
│   └── tests.rs         # ~1050 lines (all unit tests)
├── mod.rs               # Updated module declaration
└── ...other widgets
```

## Quick Start

```bash
# Start with task 01
cat workflow/plans/features/log-view-refactor/tasks/01-create-module-directory.md

# After completing each task, verify:
cargo check
cargo test log_view

# Final verification after task 06:
cargo test
cargo build --release
```

## Notes

- Tasks 01-06 must be completed in order (dependencies)
- Task 07 can be completed in parallel at any time
- Each task includes detailed acceptance criteria and testing steps
- If issues arise, see rollback plan in task 06