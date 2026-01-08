# Phase 3.6: Review Followup & TEA Compliance - Task Index

## Overview

Address code quality issues from Phase 3.5 Wave 4-6 review: fix assertion logic, deduplicate test helpers, reorganize large files, and update documentation to accurately reflect TEA pattern.

**Total Tasks:** 9
**Waves:** 4 (can be parallelized within waves)

## Task Dependency Graph

```
┌─────────────────────────┐  ┌─────────────────────────────┐
│ 01-fix-or-assertions    │  │ 02-document-terminal-field  │
└───────────┬─────────────┘  └───────────────┬─────────────┘
            │                                │
            └───────────────┬────────────────┘
                           Wave 1 complete
                            │
            ┌───────────────┴────────────────┐
            ▼                                ▼
┌────────────────────────────┐  ┌────────────────────────────┐
│ 03-extract-test-device     │  │                            │
└───────────┬────────────────┘  │                            │
            │                   │                            │
            ▼                   │                            │
┌────────────────────────────┐  │                            │
│ 04-migrate-widget-tests    │  │                            │
└───────────┬────────────────┘  │                            │
            │                   │                            │
            └───────────────┬───┘                            │
                           Wave 2 complete
                            │
            ┌───────────────┴────────────────┐
            ▼                                ▼
┌────────────────────────────┐  ┌────────────────────────────┐
│ 05-refactor-status-bar     │  │ 06-improve-testterminal    │
└───────────┬────────────────┘  └───────────┬────────────────┘
            │                                │
            └───────────────┬────────────────┘
                           Wave 3 complete
                            │
            ┌───────────────┴────────────────┐
            ▼                                ▼
┌────────────────────────────┐  ┌────────────────────────────┐
│ 07-update-architecture     │  │ 08-strengthen-search-test  │
└───────────┬────────────────┘  └───────────┬────────────────┘
            │                                │
            └───────────────┬────────────────┘
                            ▼
              ┌────────────────────────────┐
              │ 09-final-validation        │
              └────────────────────────────┘
                           Wave 4 complete
```

## Tasks

### Wave 1: Critical Fixes (Required Before Merge)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-or-assertions](tasks/01-fix-or-assertions.md) | Not Started | - | `render/tests.rs` |
| 2 | [02-document-terminal-field](tasks/02-document-terminal-field.md) | Not Started | - | `test_utils.rs` |

### Wave 2: Code Deduplication

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 3 | [03-extract-test-device](tasks/03-extract-test-device.md) | Not Started | Wave 1 | `test_utils.rs` |
| 4 | [04-migrate-widget-tests](tasks/04-migrate-widget-tests.md) | Not Started | 3 | `widgets/*.rs` |

### Wave 3: File Organization

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 5 | [05-refactor-status-bar](tasks/05-refactor-status-bar.md) | Not Started | Wave 2 | `widgets/status_bar/` |
| 6 | [06-improve-testterminal](tasks/06-improve-testterminal.md) | Not Started | Wave 2 | `test_utils.rs` |

### Wave 4: Documentation & Cleanup

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 7 | [07-update-architecture](tasks/07-update-architecture.md) | Not Started | Wave 3 | `docs/ARCHITECTURE.md` |
| 8 | [08-strengthen-search-test](tasks/08-strengthen-search-test.md) | Not Started | 6 | `render/tests.rs` |
| 9 | [09-final-validation](tasks/09-final-validation.md) | Not Started | 7, 8 | validation |

> **Note:** Task 09 incorporates Phase 3.5 Task 13, running validation after all fixes are applied.

## Success Criteria

Phase 3.6 is complete when:

- [ ] All transition tests use AND logic for assertions
- [ ] `test_device()` deduplicated to single location in `test_utils.rs`
- [ ] `status_bar.rs` refactored to directory module (widget < 500 lines)
- [ ] `TestTerminal::draw_with()` method added
- [ ] ARCHITECTURE.md accurately documents TUI→App dependency for TEA
- [ ] SearchInput test validates actual rendered content
- [ ] All tests pass: `cargo test`
- [ ] No flaky tests (5/5 runs pass)
- [ ] Clippy clean: `cargo clippy -- -D warnings`
- [ ] Final validation documented

## Notes

- Wave 1 contains **required fixes** before merging Phase 3.5
- Waves 2-4 are **recommended followup** that can be done incrementally
- Tasks within each wave can be parallelized
- TEA pattern compliance is about documentation, not code changes
