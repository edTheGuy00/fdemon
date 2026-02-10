# Phase 4 Fixes — Task Index

## Overview

Address critical bugs and quality issues found during Phase 4 code review. Two user-reported rendering bugs (empty info banners, misaligned empty states), plus cleanup of dead code annotations, IconMode hardcoding, accent bar style loss, and a pre-existing truncate_str bug.

**Total Tasks:** 6
**Review Reference:** `workflow/reviews/features/redesign-cyber-glass-phase-4/`

## Task Dependency Graph

```
Wave 1 (parallel — critical bugs)
├── 01-fix-info-banner-height
├── 02-fix-empty-state-alignment
└── 03-fix-truncate-str

Wave 2 (parallel — major/minor cleanup, after Wave 1)
├── 04-cleanup-dead-code-annotations ─── depends on: none (parallel-safe)
├── 05-wire-icon-mode-from-settings ──── depends on: none (parallel-safe)
└── 06-fix-accent-bar-background ─────── depends on: none (parallel-safe)
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-info-banner-height](tasks/01-fix-info-banner-height.md) | Done | - | `settings_panel/mod.rs`, `tests.rs` |
| 2 | [02-fix-empty-state-alignment](tasks/02-fix-empty-state-alignment.md) | Done | - | `settings_panel/mod.rs`, `tests.rs` |
| 3 | [03-fix-truncate-str](tasks/03-fix-truncate-str.md) | Done | - | `settings_panel/styles.rs`, `tests.rs` |
| 4 | [04-cleanup-dead-code-annotations](tasks/04-cleanup-dead-code-annotations.md) | Done | - | `settings_panel/styles.rs`, `settings_panel/mod.rs` |
| 5 | [05-wire-icon-mode-from-settings](tasks/05-wire-icon-mode-from-settings.md) | Done | - | `settings_panel/mod.rs`, `tests.rs` |
| 6 | [06-fix-accent-bar-background](tasks/06-fix-accent-bar-background.md) | Done | - | `settings_panel/mod.rs` |

## Wave Execution

### Wave 1: Critical Bug Fixes (Tasks 01, 02, 03 — parallel)
All three are independent: different functions, different line ranges, no overlapping edits.

### Wave 2: Cleanup & Minor Fixes (Tasks 04, 05, 06 — parallel)
Independent: different functions and different concerns. Task 05 touches `IconSet::new()` call sites but does not overlap with tasks 04 or 06.

## Success Criteria

Phase 4 Fixes complete when:

- [ ] USER tab info banner shows "Local Settings" text
- [ ] VSCODE tab info banner shows "VSCode Launch Configurations" text
- [ ] Launch/VSCode empty states are top-aligned and horizontally centered
- [ ] `truncate_str` output never exceeds `max_len` characters
- [ ] No `#[allow(dead_code)]` on actively-used style functions
- [ ] Settings panel respects user's IconMode setting
- [ ] Accent bar cell preserves `SELECTED_ROW_BG` background
- [ ] `cargo test --workspace --lib` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes clean
- [ ] `cargo fmt --all -- --check` passes
