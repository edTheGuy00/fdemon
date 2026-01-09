# Phase 2: Keybinding Changes - Task Index

## Overview

Replace the "n" keybinding with "+" for starting new sessions. The "n" key will only be used for next search match.

**Total Tasks:** 3

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-replace-n-with-plus             │
│  (Core keybinding change)           │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐     ┌─────────────────────────────────┐
│  02-update-keybinding-tests         │     │  03-update-related-handlers     │
│  (Unit test updates)                │     │  (Handler cleanup)              │
└─────────────────────────────────────┘     └─────────────────────────────────┘
         │                                           │
         └───────────────────┬───────────────────────┘
                             │
                    (Can run in parallel)
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-replace-n-with-plus](tasks/01-replace-n-with-plus.md) | Not Started | - | `app/handler/keys.rs` |
| 2 | [02-update-keybinding-tests](tasks/02-update-keybinding-tests.md) | Not Started | 1 | `app/handler/keys.rs`, `app/handler/tests.rs` |
| 3 | [03-update-related-handlers](tasks/03-update-related-handlers.md) | Not Started | 1 | `app/handler/keys.rs` |

## Success Criteria

Phase 2 is complete when:

- [ ] '+' key (Shift+=) shows StartupDialog when no sessions exist
- [ ] '+' key shows DeviceSelector when sessions are running
- [ ] 'n' key ONLY triggers NextSearchMatch (when search active)
- [ ] 'n' key does nothing when no search is active
- [ ] 'd' key continues to work as alternative to '+'
- [ ] All unit tests in `keys.rs` pass
- [ ] All tests in `handler/tests.rs` pass
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy -- -D warnings` passes

## Keyboard Shortcuts After Phase 2

| Key | Condition | Action |
|-----|-----------|--------|
| `+` | No sessions | Show StartupDialog |
| `+` | Sessions running | Show DeviceSelector |
| `d` | No sessions | Show StartupDialog (unchanged) |
| `d` | Sessions running | Show DeviceSelector (unchanged) |
| `n` | Search active | NextSearchMatch |
| `n` | No search | Nothing (returns None) |
| `N` | Any | PrevSearchMatch (unchanged) |

## Notes

- '+' requires Shift key on standard keyboards (Shift + =)
- The 'd' key remains as a more accessible alternative for device selection
- Comments in code should be updated to reflect new keybinding purpose
