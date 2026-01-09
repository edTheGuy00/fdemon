# Phase 2: Boolean Toggle Bug Verification - Task Index

## Overview

Create tests that expose the boolean toggle bug and document it properly. The goal is **bug detection and documentation**, not test passage—tests should expose the issue so it can be tracked and fixed.

**Total Tasks:** 4
**Parent Plan:** [PLAN.md](../PLAN.md)

## Task Dependency Graph

```
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  01-e2e-toggle-test         │     │  02-unit-toggle-test        │
│  (E2E test skeleton)        │     │  (Unit test for handler)    │
└─────────────┬───────────────┘     └──────────────┬──────────────┘
              │                                    │
              └──────────────┬─────────────────────┘
                             ▼
              ┌─────────────────────────────┐
              │  03-bug-report              │
              │  (Create BUG.md)            │
              └─────────────┬───────────────┘
                            │
                            ▼
              ┌─────────────────────────────┐
              │  04-boolean-setting-tests   │
              │  (Individual setting tests) │
              └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-e2e-toggle-test](tasks/01-e2e-toggle-test.md) | Done | - | `tests/e2e/settings_page.rs` |
| 2 | [02-unit-toggle-test](tasks/02-unit-toggle-test.md) | Done | - | `src/app/handler/tests.rs` |
| 3 | [03-bug-report](tasks/03-bug-report.md) | Done | 1, 2 | `workflow/plans/bugs/boolean-toggle/BUG.md` |
| 4 | [04-boolean-setting-tests](tasks/04-boolean-setting-tests.md) | Done | 3 | `tests/e2e/settings_page.rs` |

## Success Criteria

Phase 2 is complete when:

- [x] Boolean toggle bug documented with failing E2E test
- [x] Unit tests added for `SettingsToggleBool` handler demonstrating the bug
- [x] Bug report created in `workflow/plans/bugs/boolean-toggle/BUG.md`
- [x] All bug-related tests marked `#[ignore]` with clear reason linking to bug report
- [x] Tests for individual boolean settings (auto_start, auto_reload, etc.) created

## Notes

- Tasks 1 and 2 can be worked on **in parallel** since they have no dependencies
- Task 3 (bug report) should synthesize findings from both E2E and unit test creation
- Task 4 adds comprehensive coverage after the bug is documented
- Testing philosophy: catch bugs, not make tests pass—mark failing tests with `#[ignore]`
- **Startup flow rework complete**: App now starts directly in `UiMode::Normal` with "Not Connected" state—no need to escape `StartupDialog` in tests
