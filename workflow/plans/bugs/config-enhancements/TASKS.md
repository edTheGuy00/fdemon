# Config Enhancements - Task Index

## Overview

Fix two configuration bugs: watcher paths from config.toml being silently ignored (Issue #17), and auto_start in launch.toml having no effect (Issue #18). Both are wiring issues where existing infrastructure isn't connected.

**Total Tasks:** 5

## Task Dependency Graph

```
┌─────────────────────────┐     ┌─────────────────────────┐
│  01-fix-watcher-paths   │     │  03-fix-auto-start      │
└────────────┬────────────┘     └────────────┬────────────┘
             │                               │
             ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│  02-watcher-tests       │     │  04-auto-start-tests    │
└────────────┬────────────┘     └────────────┬────────────┘
             │                               │
             └──────────┬────────────────────┘
                        ▼
             ┌─────────────────────────┐
             │  05-example-apps-testing│
             └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-watcher-paths](tasks/01-fix-watcher-paths.md) | Not Started | - | `engine.rs`, `watcher/mod.rs` |
| 2 | [02-watcher-tests](tasks/02-watcher-tests.md) | Not Started | 1 | `engine.rs`, `watcher/mod.rs` |
| 3 | [03-fix-auto-start](tasks/03-fix-auto-start.md) | Not Started | - | `startup.rs`, `runner.rs` |
| 4 | [04-auto-start-tests](tasks/04-auto-start-tests.md) | Not Started | 3 | `startup.rs`, `runner.rs` |
| 5 | [05-example-apps-testing](tasks/05-example-apps-testing.md) | Not Started | 1, 2, 3, 4 | `example/` |

## Success Criteria

Config enhancements are complete when:

- [ ] Custom watcher paths and extensions from `config.toml` are respected
- [ ] Relative paths (including `../../`) are canonicalized before watching
- [ ] `auto_start = true` in `launch.toml` triggers auto-launch on startup
- [ ] `behavior.auto_start = true` in `config.toml` triggers auto-launch
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality
- [ ] Example apps cover all edge cases for manual verification
- [ ] `cargo test --workspace` passes

## Notes

- Tasks 1+2 and 3+4 are independent and can be worked in parallel
- Task 5 depends on all preceding tasks
- The auto-launch infrastructure (`StartAutoLaunch`, `spawn_auto_launch`, `AutoLaunchResult`) already exists and is tested — only the trigger wiring is missing
- The watcher `with_paths()` and `with_extensions()` builder methods already exist — only the call site needs updating
