# Nerd Font Detection — Phase 1 Task Index

## Overview

Add configuration-driven icon mode to fdemon, allowing users to opt-in to Nerd Font icons via `config.toml` or `FDEMON_ICONS` env var, while defaulting to safe Unicode.

**Total Tasks:** 6

## Task Dependency Graph

```
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  01-add-icon-mode-config    │     │  02-create-icon-set         │
│  (fdemon-app config types)  │     │  (fdemon-tui icons.rs)      │
└─────────────┬───────────────┘     └──────────────┬──────────────┘
              │                                    │
              └──────────────┬─────────────────────┘
                             ▼
              ┌─────────────────────────────┐
              │  03-wire-icon-set-to-tui    │
              │  (header, log_view, styles) │
              └──────────────┬──────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼                             ▼
┌─────────────────────────┐   ┌─────────────────────────────┐
│  04-settings-panel      │   │  05-update-tests            │
│  (settings_items.rs)    │   │  (all affected crates)      │
└────────────┬────────────┘   └─────────────────────────────┘
             │
             ▼
┌──────────────────────────────┐
│  06-update-configuration-docs│
│  (docs/CONFIGURATION.md)     │
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-icon-mode-config](tasks/01-add-icon-mode-config.md) | Done | - | `fdemon-app/config/types.rs`, `fdemon-app/config/settings.rs` |
| 2 | [02-create-icon-set](tasks/02-create-icon-set.md) | Done | - | `fdemon-tui/theme/icons.rs`, `fdemon-tui/theme/mod.rs` |
| 3 | [03-wire-icon-set-to-tui](tasks/03-wire-icon-set-to-tui.md) | Done | 1, 2 | `fdemon-tui/widgets/header.rs`, `fdemon-tui/widgets/log_view/mod.rs`, `fdemon-tui/theme/styles.rs` |
| 4 | [04-settings-panel](tasks/04-settings-panel.md) | Done | 1 | `fdemon-app/settings_items.rs` |
| 5 | [05-update-tests](tasks/05-update-tests.md) | Done | 3, 4 | All affected crates |
| 6 | [06-update-configuration-docs](tasks/06-update-configuration-docs.md) | Done | 1, 4 | `docs/CONFIGURATION.md` |

## Success Criteria

Phase 1 is complete when:

- [x] `IconMode` enum exists in `fdemon-app/config/types.rs` with serde support
- [x] `IconSet` struct in `fdemon-tui/theme/icons.rs` replaces dual static constants
- [x] `icons = "nerd_fonts"` in `config.toml` activates Nerd Font glyphs
- [x] `FDEMON_ICONS` env var overrides config setting
- [x] Default behavior (no config) renders safe Unicode (unchanged)
- [x] Settings panel shows icon mode as editable enum
- [x] Phase indicators in `styles.rs` use `IconSet` (no inline literals)
- [x] All existing tests pass, new tests cover both modes
- [x] `cargo check --workspace` passes
- [x] `cargo clippy --workspace -- -D warnings` passes

## Notes

- Tasks 1 and 2 are independent and can be worked on in parallel
- Task 3 depends on both 1 and 2 (needs `IconMode` from config and `IconSet` from TUI)
- Tasks 4 and 5 depend on task 3 being complete
- The `fdemon-app` crate owns `IconMode` (config type), the `fdemon-tui` crate owns `IconSet` (rendering)
