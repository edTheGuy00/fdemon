# Phase 3: Config File Lifecycle Tests - Task Index

## Status: BLOCKED

**Blocked By:** PTY/crossterm limitation - Enter/Space keys don't trigger actions in E2E tests

**See:** [E2E PTY Input Bug Investigation](../../../bugs/e2e-pty-input/TASKS.md)

---

## Overview

Test config file creation, modification, and persistence.

**Total Tasks:** 5
**Status:** All blocked pending PTY fix

## Blocking Issue

Phase 3 tests require keyboard interactions that don't work in PTY mode:

| Required Action | Key | PTY Status |
|----------------|-----|------------|
| Edit setting | Enter | Not working |
| Toggle boolean | Enter/Space | Not working |
| Save changes | Ctrl+S | Likely not working |
| Confirm dialogs | Enter | Not working |

Without these keys, E2E tests cannot:
- Trigger config file saves
- Modify settings values
- Test the full edit → save → persist workflow

## Planned Tasks (Blocked)

| # | Task | Status | Description |
|---|------|--------|-------------|
| 1 | Create test fixtures | Blocked | `no_config_app/` fixture |
| 2 | Config directory creation tests | Blocked | Test `.fdemon/` created on first save |
| 3 | Config file update tests | Blocked | Test settings persist to file |
| 4 | Config file integrity tests | Blocked | Test invalid/partial config handling |
| 5 | User preferences tests | Blocked | Test `settings.local.toml` behavior |

## Alternative Coverage

Config file functionality is covered by unit tests:

- `src/config/settings.rs` - Settings loading/saving
- `src/config/launch.rs` - Launch config management
- `src/config/vscode.rs` - VSCode config parsing

## Unblocking Criteria

This phase can proceed when:

- [ ] PTY Enter/Space key issue resolved, OR
- [ ] Alternative E2E testing approach implemented (headless mode, JSON events)

## Notes

- Test fixtures (`no_config_app/`, etc.) could be created independently if needed for other purposes
- Unit test coverage for config operations is adequate for now
- Phase 4 (Project Type Configuration Tests) is also blocked for the same reason
- Phase 5 (Settings Edit Mode Tests) is also blocked for the same reason
