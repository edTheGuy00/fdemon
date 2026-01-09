# Boolean Toggle Bug Fix - Task Index

## Overview

Fix the boolean toggle bug in the settings page where pressing Enter marks the setting dirty but doesn't flip the boolean value. The handler stub needs to be completed with actual toggle logic.

**Total Tasks:** 3
**Parent Plan:** [BUG.md](BUG.md)

## Task Dependency Graph

```
┌─────────────────────────────────────────┐
│  01-implement-toggle-handler            │
│  (Complete SettingsToggleBool handler)  │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  02-fix-toggle-edit-dispatch            │
│  (Make Enter key dispatch toggle)       │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  03-enable-e2e-tests                    │
│  (Remove #[ignore] from toggle tests)   │
└─────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-implement-toggle-handler](tasks/01-implement-toggle-handler.md) | Done | - | `src/app/handler/update.rs` |
| 2 | [02-fix-toggle-edit-dispatch](tasks/02-fix-toggle-edit-dispatch.md) | Done | 1 | `src/app/handler/update.rs` |
| 3 | [03-enable-e2e-tests](tasks/03-enable-e2e-tests.md) | Done | 2 | `tests/e2e/settings_page.rs`, `src/app/handler/tests.rs` |

## Success Criteria

Bug fix is complete when:

- [ ] Pressing Enter on boolean setting flips the value (true↔false)
- [ ] Displayed value updates immediately after toggle
- [ ] Dirty indicator appears after toggle
- [ ] Changes persist to config file after save
- [ ] All boolean settings work (auto_start, auto_reload, devtools_auto_open, etc.)
- [ ] Unit tests for toggle behavior pass
- [ ] E2E tests (previously `#[ignore]`) pass
- [ ] No regressions in other setting types

## Notes

- Task 1 is the critical fix - it completes the stub handler
- Task 2 improves UX by making Enter key work directly (currently no-ops for booleans)
- Task 3 enables previously ignored tests to verify the fix
- All apply functions (`apply_project_setting`, etc.) already handle booleans correctly
- `get_selected_item()` already available on SettingsPanel
