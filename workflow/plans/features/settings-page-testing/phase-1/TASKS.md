# Phase 1: Settings Page E2E Tests - Task Index

## Overview

Create end-to-end tests for settings page navigation and visual output verification using PTY-based testing infrastructure.

**Total Tasks:** 5
**Modules:** `tests/e2e/settings_page.rs`, `tests/e2e/mod.rs`

## Task Dependency Graph

```
┌─────────────────────────────────┐
│  01-test-file-structure         │
│  (Create test file and module)  │
└───────────────┬─────────────────┘
                │
                ▼
┌───────────────────────────────────────────────────────────────┐
│                    Can run in parallel                         │
├─────────────────────┬─────────────────────┬───────────────────┤
│ 02-navigation-tests │ 03-tab-tests        │ 04-item-nav-tests │
│ (Open/close)        │ (Tab switching)     │ (Arrow/jk keys)   │
└─────────────────────┴─────────────────────┴───────────────────┘
                │                 │                   │
                └─────────────────┼───────────────────┘
                                  ▼
                    ┌─────────────────────────────┐
                    │  05-visual-output-tests     │
                    │  (Indicators & highlighting)│
                    └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-test-file-structure](tasks/01-test-file-structure.md) | Not Started | - | `tests/e2e/mod.rs`, `tests/e2e/settings_page.rs` |
| 2 | [02-navigation-tests](tasks/02-navigation-tests.md) | Not Started | 1 | `tests/e2e/settings_page.rs` |
| 3 | [03-tab-tests](tasks/03-tab-tests.md) | Not Started | 1 | `tests/e2e/settings_page.rs` |
| 4 | [04-item-nav-tests](tasks/04-item-nav-tests.md) | Not Started | 1 | `tests/e2e/settings_page.rs` |
| 5 | [05-visual-output-tests](tasks/05-visual-output-tests.md) | Not Started | 2, 3, 4 | `tests/e2e/settings_page.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] Test file `tests/e2e/settings_page.rs` created and integrated
- [ ] Settings page opens on `,` key and closes on `Escape`/`q`
- [ ] All four tabs navigable via number keys and Tab key
- [ ] Item navigation works with arrow keys and j/k
- [ ] Visual indicators (selection, dirty, readonly) verified
- [ ] All tests pass with `cargo nextest run --test e2e`
- [ ] No regressions in existing E2E tests

## Keyboard Shortcuts Under Test

| Key | Expected Action |
|-----|-----------------|
| `,` | Open settings page |
| `Escape` | Close settings page |
| `q` | Close settings page |
| `1-4` | Switch to tab by number |
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `j` / `↓` | Select next item |
| `k` / `↑` | Select previous item |

## Notes

- All PTY tests must use `#[serial]` attribute
- Use timing constants from `pty_utils.rs` for consistency
- Focus on catching bugs, not making tests pass
- If a test exposes a bug, document it and mark with `#[ignore]`
