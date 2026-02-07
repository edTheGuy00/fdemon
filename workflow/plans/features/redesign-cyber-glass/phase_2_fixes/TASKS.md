# Phase 2 Fixes: Review Action Items - Task Index

## Overview

Address all blocking, major, and minor issues identified in the Phase 1+2 code review. Three critical bugs (broken multi-session tabs, broken Nerd Font icons, footer height desync), palette migration gaps (~46 hardcoded `Color::` references), dead code accumulation, and minor quality issues.

**Total Tasks:** 8
**Crate:** `fdemon-tui`
**Depends on:** Phase 2 (all phase 2 tasks complete)

## Task Dependency Graph

```
Wave 1 (parallel — independent fixes):
┌───────────────────────────────┐  ┌───────────────────────────────┐  ┌──────────────────────────────┐
│ 01-fix-multi-session-header   │  │ 02-fix-nerd-font-icons        │  │ 03-fix-footer-height-desync  │
│ (CRITICAL: tabs + layout)     │  │ (CRITICAL: safe Unicode)      │  │ (CRITICAL: edge case)        │
└───────────────────────────────┘  └───────────────────────────────┘  └──────────────────────────────┘

Wave 2 (parallel — after wave 1):
┌───────────────────────────────┐  ┌───────────────────────────────┐
│ 04-remove-dead-code           │  │ 05-complete-palette-migration │
│ (status_bar, tabs legacy,     │  │ (~46 Color:: refs remaining)  │
│  build_title, layout fns)     │  │                               │
└───────────────────────────────┘  └───────────────────────────────┘

Wave 3 (after 02, 04, 05):
┌───────────────────────────────┐  ┌───────────────────────────────┐
│ 06-fix-theme-module-hygiene   │  │ 07-fix-width-calculations     │
│ (allow(dead_code), SOURCE_*,  │  │ (consistent char counting)    │
│  docstrings, dedup)           │  │                               │
└───────────────────────────────┘  └───────────────────────────────┘

Wave 4 (after all):
┌───────────────────────────────┐
│ 08-update-tests               │
│ (snapshots, new unit tests)   │
└───────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-multi-session-header](tasks/01-fix-multi-session-header.md) | Done | - | `layout.rs`, `header.rs` |
| 2 | [02-fix-nerd-font-icons](tasks/02-fix-nerd-font-icons.md) | Done | - | `theme/icons.rs`, `header.rs`, `log_view/mod.rs` |
| 3 | [03-fix-footer-height-desync](tasks/03-fix-footer-height-desync.md) | Done | - | `log_view/mod.rs` |
| 4 | [04-remove-dead-code](tasks/04-remove-dead-code.md) | Done | - | `status_bar/`, `tabs.rs`, `log_view/mod.rs`, `layout.rs`, `widgets/mod.rs` |
| 5 | [05-complete-palette-migration](tasks/05-complete-palette-migration.md) | Done | - | `modal_overlay.rs`, `log_view/mod.rs`, `settings_panel/mod.rs`, `new_session_dialog/`, `confirm_dialog.rs` |
| 6 | [06-fix-theme-module-hygiene](tasks/06-fix-theme-module-hygiene.md) | Done | 2, 4, 5 | `theme/palette.rs`, `theme/icons.rs`, `theme/styles.rs` |
| 7 | [07-fix-width-calculations](tasks/07-fix-width-calculations.md) | Done | 2 | `log_view/mod.rs` |
| 8 | [08-update-tests](tasks/08-update-tests.md) | Done | 1-7 | All test files |

## Execution Strategy

**Wave 1** (parallel): Tasks 01, 02, 03 are the three critical/blocking issues. They are independent and can be worked simultaneously.

**Wave 2** (parallel, after wave 1): Tasks 04 and 05 are independent cleanup work. Task 04 removes dead code (status_bar module, legacy tabs, dead layout functions). Task 05 migrates remaining hardcoded `Color::` references to palette constants.

**Wave 3** (after 02, 04, 05): Tasks 06 and 07 depend on earlier work. Task 06 removes `#![allow(dead_code)]` from theme files (requires dead code to be removed first and icons to be stabilized). Task 07 standardizes width calculations (requires icon changes to be finalized).

**Wave 4** (after all): Task 08 updates snapshot tests and adds new unit tests for the fixes.

## Success Criteria

Phase 2 Fixes are complete when:

- [ ] Multi-session tabs are visible and functional with 2+ sessions
- [ ] Icons render correctly in both Ghostty and Zed integrated terminal (no `?` or tofu)
- [ ] Footer height is correct even in very small terminal areas
- [ ] Zero hardcoded `Color::` references in production code outside `theme/` (except tests)
- [ ] Zero `#![allow(dead_code)]` in theme module files
- [ ] Dead code removed: `StatusBar`/`StatusBarCompact`, `HeaderWithTabs`, `build_title()`, 7 layout functions
- [ ] Width calculations use consistent `.chars().count()` in metadata bars
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace --lib && cargo clippy --workspace -- -D warnings` all pass

## Notes

- **No new features**: This phase is strictly bug fixes and cleanup from the Phase 1+2 review.
- **Test updates are critical**: Removing the status_bar module and legacy tabs code will break their respective test suites. Snapshot tests will need updating after header height changes.
- **Icon strategy**: We replace Nerd Font glyphs with universally-supported Unicode (matching the safe chars already used by `phase_indicator()`). Keep Nerd Font constants for future opt-in via config.
