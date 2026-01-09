# Phase 3: Documentation & Tests - Task Index

## Overview

Update all documentation, snapshot tests, and E2E tests to reflect the new startup flow and keybindings. Also includes visual output tests for settings page (moved from settings-page-testing/phase-1).

**Total Tasks:** 5

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-update-keybindings-doc          │
│  (KEYBINDINGS.md update)            │
└─────────────────────────────────────┘
         │
         └───────────── Can run in parallel ─────────────┐
                                                          │
┌─────────────────────────────────────┐     ┌─────────────────────────────────┐
│  02-update-snapshot-tests           │     │  03-update-e2e-tests            │
│  (Render snapshots)                 │     │  (PTY test utilities)           │
└─────────────────────────────────────┘     └─────────────────────────────────┘
         │                                           │
         └───────────────────┬───────────────────────┘
                             │
                             ▼
                ┌─────────────────────────────────┐
                │  04-verify-all-tests            │
                │  (Full verification)            │
                └─────────────────────────────────┘
                             │
                             ▼
                ┌─────────────────────────────────┐
                │  05-visual-output-tests         │
                │  (Settings page E2E tests)      │
                │  (Moved from settings-page-     │
                │   testing/phase-1)              │
                └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-update-keybindings-doc](tasks/01-update-keybindings-doc.md) | Done | - | `docs/KEYBINDINGS.md` |
| 2 | [02-update-snapshot-tests](tasks/02-update-snapshot-tests.md) | Done | - | `tui/render/tests.rs`, snapshots |
| 3 | [03-update-e2e-tests](tasks/03-update-e2e-tests.md) | Done | - | `tests/e2e/` |
| 4 | [04-verify-all-tests](tasks/04-verify-all-tests.md) | Done | 1, 2, 3 | All |
| 5 | [05-visual-output-tests](tasks/05-visual-output-tests.md) | Done | 3 | `tests/e2e/settings_page.rs` |

## Success Criteria

Phase 3 is complete when:

- [ ] KEYBINDINGS.md updated with '+' key documentation
- [ ] All snapshot tests pass with updated content
- [ ] E2E tests pass without startup dialog workarounds
- [ ] Settings page tests are unblocked and pass
- [ ] No documentation mentions 'n' for new session
- [ ] Visual output tests for settings page pass (task 05)
- [ ] Full verification passes:
  - `cargo fmt --check`
  - `cargo check`
  - `cargo test`
  - `cargo clippy -- -D warnings`
  - `cargo test --test e2e` (if applicable)

## Notes

- Snapshot regeneration: Use `UPDATE_EXPECT=1 cargo test` to update snapshots
- E2E tests may need `cargo nextest` for retry capability
- Some E2E tests may have been ignored due to startup dialog issues - check if they can be re-enabled
