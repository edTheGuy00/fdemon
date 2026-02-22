# Phase 5: Polish, Documentation & Integration Testing - Task Index

## Overview

Refine UX edge cases, wire up deferred interactions (allocation sort, network filter input), add network configuration options, update all documentation to reflect the post-Phase-4 state, and verify everything works together across all three DevTools panels.

**Total Tasks:** 8
**Waves:** 3 (01–04 parallel, then 05–07 parallel docs, then 08 solo)

## Task Dependency Graph

```
Wave 1 (parallel — different crates/files)
┌─────────────────────────────────┐  ┌─────────────────────────────────┐
│ 01-add-network-config-options   │  │ 02-wire-allocation-sort         │
│ (fdemon-app config + session)   │  │ (fdemon-app handler + tui)      │
└────────────────┬────────────────┘  └────────────────┬────────────────┘
                 │                                    │
┌────────────────┴────────────────┐  ┌────────────────┴────────────────┐
│ 03-add-network-filter-input     │  │ 04-polish-small-terminal        │
│ (fdemon-app handler/keys)       │  │ (fdemon-tui widgets)            │
└────────────────┬────────────────┘  └────────────────┬────────────────┘
                 │                                    │
Wave 2 (parallel — documentation, after code changes)
                 │                                    │
        ┌────────┴────────────────────────────────────┘
        │
        ├──────────────────────────────────────────────────────────────┐
        │                                   │                          │
        ▼                                   ▼                          ▼
┌───────────────────────────┐ ┌───────────────────────────┐ ┌─────────────────────┐
│ 05-update-keybindings-doc │ │ 06-update-architecture-doc│ │ 07-update-claude-md  │
│ (docs/KEYBINDINGS.md)     │ │ (docs/ARCHITECTURE.md)    │ │ (CLAUDE.md)          │
│ depends: 01, 02, 03      │ │ depends: 01               │ │ depends: 01          │
└────────────┬──────────────┘ └────────────┬──────────────┘ └──────────┬──────────┘
             │                             │                           │
Wave 3       └─────────────┬───────────────┴───────────────────────────┘
                           ▼
            ┌─────────────────────────────────┐
            │ 08-final-test-and-cleanup       │
            │ (workspace-wide)                │
            │ depends: 01–07                  │
            └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Modules |
|---|------|--------|------------|-------|---------|
| 1 | [01-add-network-config-options](tasks/01-add-network-config-options.md) | Not Started | - | `fdemon-app` | `config/types.rs`, `config/settings.rs`, `session/network.rs` |
| 2 | [02-wire-allocation-sort-interaction](tasks/02-wire-allocation-sort-interaction.md) | Not Started | - | `fdemon-app`, `fdemon-tui` | `session/performance.rs`, `handler/devtools/performance.rs`, `handler/keys.rs`, `widgets/devtools/performance/memory_chart/table.rs` |
| 3 | [03-add-network-filter-input](tasks/03-add-network-filter-input.md) | Not Started | - | `fdemon-app` | `state.rs`, `message.rs`, `handler/keys.rs`, `handler/devtools/network.rs` |
| 4 | [04-polish-small-terminal-rendering](tasks/04-polish-small-terminal-rendering.md) | Not Started | - | `fdemon-tui` | `widgets/devtools/network/mod.rs`, `widgets/devtools/inspector/mod.rs`, `widgets/devtools/mod.rs` |
| 5 | [05-update-keybindings-docs](tasks/05-update-keybindings-docs.md) | Not Started | 1, 2, 3 | docs | `docs/KEYBINDINGS.md` |
| 6 | [06-update-architecture-docs](tasks/06-update-architecture-docs.md) | Not Started | 1 | docs | `docs/ARCHITECTURE.md` |
| 7 | [07-update-project-metadata](tasks/07-update-project-metadata.md) | Not Started | 1 | docs | `CLAUDE.md` |
| 8 | [08-final-test-and-cleanup](tasks/08-final-test-and-cleanup.md) | Not Started | 1–7 | workspace | All devtools modules |

## Dispatch Plan

**Wave 1** (parallel — different crates/files, no conflicts):
- Task 01: Add network config options (fdemon-app config + session wiring)
- Task 02: Wire allocation sort interaction (fdemon-app handler/performance + fdemon-tui table)
- Task 03: Add network filter input mode (fdemon-app handler/keys + state)
- Task 04: Polish small terminal rendering (fdemon-tui widgets only)

**Wave 2** (parallel — documentation only, after code changes stabilize):
- Task 05: Update KEYBINDINGS.md (needs final key bindings from 01, 02, 03)
- Task 06: Update ARCHITECTURE.md (needs final module structure from 01)
- Task 07: Update CLAUDE.md (needs final config and module state from 01)

**Wave 3** (solo — final verification):
- Task 08: Full test and cleanup pass

## Success Criteria

Phase 5 is complete when:

- [ ] Network config options (`max_network_entries`, `network_auto_record`, `network_poll_interval_ms`) in `config.toml` and wired to runtime
- [ ] Allocation table sort toggleable via `s` key in Performance panel
- [ ] Network filter input accessible via `/` key with text entry
- [ ] All panels render gracefully at very small terminal sizes (< 60 cols, < 15 rows)
- [ ] `docs/KEYBINDINGS.md` reflects all DevTools key bindings (no stale `l` Layout, includes Network panel, Performance panel sections)
- [ ] `docs/ARCHITECTURE.md` includes DevTools subsystem section
- [ ] `CLAUDE.md` includes DevTools modules, updated test count, config fields
- [ ] Generated default `config.toml` documents all `[devtools]` fields
- [ ] All `#[allow(dead_code)]` TODOs for allocation sort resolved
- [ ] All panels handle VM service disconnection gracefully
- [ ] No regressions in existing functionality
- [ ] Full quality gate passes: `cargo fmt && cargo check && cargo test && cargo clippy`

## Notes

- **Phases 1–4 assumed complete**: All tasks reference the post-Phase-4 codebase where inspector, performance, and network panels are fully functional.
- **Network filter**: The `NetworkFilterChanged` message and `NetworkState::set_filter()` are already implemented and wired. What's missing is the key binding (`/`) and a text-input interaction mode to emit the message.
- **Allocation sort**: The `AllocationSortColumn` enum, `PerformanceState.allocation_sort` field, and two existing tests are present but marked `#[allow(dead_code)]`. The table widget currently sorts by `new_size` descending unconditionally.
- **Default config template**: The generated `config.toml` in `settings.rs:543` only documents `auto_open` and `browser` for `[devtools]`. All newer fields are missing from the template.
- **KEYBINDINGS.md staleness**: Still references `l` for "Layout Panel" (removed in Phase 2), has no Network or Performance panel sections, and describes DevTools as "Inspector/Layout/Performance panels".
