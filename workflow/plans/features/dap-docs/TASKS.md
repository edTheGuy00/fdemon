# DAP Documentation Update - Task Index

## Overview

Update all documentation and the website to reflect the completed DAP server feature (Phases 1–5), including auto-configuration (Phase 5).

**Total Tasks:** 4
**Estimated Hours:** 3-5 hours

## Task Dependency Graph

```
Wave 1 (all parallel)
├── 01-update-keybindings
├── 02-update-ide-setup
├── 03-create-debugging-page
└── 04-wire-debugging-page
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Files |
|---|------|--------|------------|------------|-------|
| 1 | [01-update-keybindings](tasks/01-update-keybindings.md) | Done | - | 0.5h | `docs/KEYBINDINGS.md` |
| 2 | [02-update-ide-setup](tasks/02-update-ide-setup.md) | Done | - | 1-1.5h | `docs/IDE_SETUP.md` |
| 3 | [03-create-debugging-page](tasks/03-create-debugging-page.md) | Done | - | 2-3h | `website/src/pages/docs/debugging.rs` |
| 4 | [04-wire-debugging-page](tasks/04-wire-debugging-page.md) | Done | - | 0.5h | `website/src/{lib.rs,pages/docs/mod.rs,components/icons.rs}` |

## Success Criteria

Documentation update is complete when:

- [x] `D` keybinding documented in KEYBINDINGS.md
- [x] Phase 5 auto-configuration documented in IDE_SETUP.md
- [x] Website `/docs/debugging` page exists with full DAP coverage
- [x] Website sidebar includes "Debugging" entry
- [x] All files compile cleanly
