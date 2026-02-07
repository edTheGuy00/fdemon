# Phase 1: Theme Foundation - Task Index

## Overview

Create the centralized theme module and migrate all color/style definitions to use it. No visual changes yet — just infrastructure. This phase establishes a single source of truth for all colors, styles, and icons across the TUI crate.

**Total Tasks:** 5
**Crate:** `fdemon-tui`

## Task Dependency Graph

```
┌────────────────────────────┐     ┌──────────────────────────────┐
│  01-create-theme-module    │     │  02-create-modal-overlay     │
│  (palette, styles, icons)  │     │  (dim, shadow, centering)    │
└────────────┬───────────────┘     └──────────────────────────────┘
             │
    ┌────────┴─────────┐
    ▼                  ▼
┌──────────────────┐  ┌──────────────────────────────┐
│  03-migrate-     │  │  04-consolidate-phase-       │
│  widget-styles   │  │  mapping                     │
└────────┬─────────┘  └──────────────┬───────────────┘
         │                           │
         └───────────┬───────────────┘
                     ▼
          ┌────────────────────────┐
          │  05-update-tests       │
          └────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-create-theme-module](tasks/01-create-theme-module.md) | Not Started | - | `theme/mod.rs`, `theme/palette.rs`, `theme/styles.rs`, `theme/icons.rs`, `lib.rs` |
| 2 | [02-create-modal-overlay](tasks/02-create-modal-overlay.md) | Not Started | - | `widgets/modal_overlay.rs`, `widgets/mod.rs` |
| 3 | [03-migrate-widget-styles](tasks/03-migrate-widget-styles.md) | Not Started | 1 | 15 widget files (see task details) |
| 4 | [04-consolidate-phase-mapping](tasks/04-consolidate-phase-mapping.md) | Not Started | 1 | `tabs.rs`, `status_bar/mod.rs`, `theme/styles.rs` |
| 5 | [05-update-tests](tasks/05-update-tests.md) | Not Started | 3, 4 | 4 test files + inline test modules |

## Execution Strategy

**Wave 1** (parallel): Tasks 01 and 02 are independent — can be implemented simultaneously.

**Wave 2** (parallel, after 01): Tasks 03 and 04 both depend on the theme module from Task 01 but are independent of each other.

**Wave 3** (after 03 and 04): Task 05 fixes any test breakage from the migration.

## Success Criteria

Phase 1 is complete when:

- [ ] `crates/fdemon-tui/src/theme/` module exists with `palette.rs`, `styles.rs`, `icons.rs`
- [ ] All 15+ widget files import colors from `theme::` instead of hardcoded values
- [ ] Phase-to-icon/color mapping consolidated to single location in `theme::styles`
- [ ] Shared modal overlay utilities exist (`dim_background`, `render_shadow`, `centered_rect`)
- [ ] `cargo test --workspace` passes with zero regressions
- [ ] `cargo clippy --workspace` passes with no warnings
- [ ] No hardcoded `Color::` references remain outside `theme/` module (except in tests)

## Notes

- **No visual changes**: Phase 1 maps existing named colors (e.g., `Color::Cyan`) to theme constants. The actual RGB color values from the design spec are introduced in Phase 2+.
- **Test strategy**: Most tests assert on buffer cell styles. Since we're mapping to the same colors initially, most tests should continue to pass. Task 05 handles any that break due to the `phase_indicator` consolidation changing exact style values.
- **Incremental migration**: Task 03 is the largest task. It can be done file-by-file, verifying `cargo check` after each file.
