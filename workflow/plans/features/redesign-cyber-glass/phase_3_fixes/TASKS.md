# Phase 3 Fixes: Review Action Items - Task Index

## Overview

Address all critical and major issues identified in the Phase 3 code review. Two critical bugs (ghost DartDefines field in navigation, LaunchButton ignoring focus state), stale dead_code annotations, inconsistent overlay for Dart Defines modal, incorrect min_height() arithmetic, and commented-out test assertions.

**Total Tasks:** 6
**Crate:** `fdemon-tui` (rendering), `fdemon-app` (if DartDefines navigation adjusted)
**Depends on:** Phase 3 (all phase 3 tasks complete)
**Review Reference:** `workflow/reviews/features/redesign-cyber-glass-phase-3/REVIEW.md`

## Task Dependency Graph

```
Wave 1 (parallel — critical fixes):
┌───────────────────────────────┐  ┌───────────────────────────────┐
│ 01-add-dart-defines-field     │  │ 02-fix-launch-button-focus    │
│ (CRITICAL: ghost nav field)   │  │ (CRITICAL: no focus visual)   │
└───────────────┬───────────────┘  └───────────────┬───────────────┘
                │                                  │
                ▼                                  │
Wave 2 (parallel — cleanup, after wave 1):         │
┌───────────────────────────────┐                  │
│ 03-remove-stale-dead-code     │                  │
│ (dead_code annotations)       │                  │
└───────────────────────────────┘                  │
┌───────────────────────────────┐                  │
│ 04-fix-dart-defines-overlay   │                  │
│ (Clear vs dim_background)     │                  │
└───────────────────────────────┘                  │
┌───────────────────────────────┐                  │
│ 05-fix-min-height-arithmetic  │                  │
│ (21 → correct value)          │                  │
└───────────────┬───────────────┘                  │
                │                                  │
                ▼                                  ▼
Wave 3 (after all):
┌───────────────────────────────┐
│ 06-update-tests               │
│ (uncomment + add assertions)  │
└───────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-dart-defines-field](tasks/01-add-dart-defines-field.md) | Done | - | `launch_context.rs` |
| 2 | [02-fix-launch-button-focus](tasks/02-fix-launch-button-focus.md) | Done | - | `launch_context.rs` |
| 3 | [03-remove-stale-dead-code](tasks/03-remove-stale-dead-code.md) | Done | - | `theme/palette.rs` |
| 4 | [04-fix-dart-defines-overlay](tasks/04-fix-dart-defines-overlay.md) | Done | - | `new_session_dialog/mod.rs`, `dart_defines_modal.rs` |
| 5 | [05-fix-min-height-arithmetic](tasks/05-fix-min-height-arithmetic.md) | Done | 1 | `launch_context.rs` |
| 6 | [06-update-tests](tasks/06-update-tests.md) | Done | 1, 2, 3, 4, 5 | `launch_context.rs` tests |

## Execution Strategy

**Wave 1** (parallel): Tasks 01 and 02 are the two critical issues flagged by all 4 reviewer agents. They are independent — Task 01 adds the missing DartDefines field rendering, Task 02 adds focus visual feedback to LaunchButton.

**Wave 2** (parallel, after wave 1): Tasks 03, 04 are independent cleanup. Task 05 depends on Task 01 because adding DartDefines to the layout changes the minimum height calculation.

**Wave 3** (after all): Task 06 uncomments the DartDefines test assertions (now valid after Task 01), adds LaunchButton focus tests (after Task 02), and verifies all fixes.

## Success Criteria

Phase 3 Fixes are complete when:

- [ ] DartDefines field is visible in both horizontal and compact layouts
- [ ] Keyboard navigation through all fields has no ghost/invisible focus states
- [ ] LaunchButton shows visual focus feedback (distinct border) when selected via keyboard
- [ ] No stale `#[allow(dead_code)]` annotations on actively-used constants
- [ ] Dart Defines modal uses `dim_background()` consistently with other modals
- [ ] `min_height()` returns correct value matching actual layout arithmetic
- [ ] All commented-out test assertions are restored and passing
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace --lib && cargo clippy --workspace -- -D warnings` all pass

## Notes

- **No new features**: This phase is strictly bug fixes and cleanup from the Phase 3 review.
- **DartDefines field uses existing ActionField widget**: The `ActionField` widget was designed for DartDefines but never wired into the layout. Task 01 connects it.
- **LaunchButton follows existing patterns**: `DropdownField` and `ActionField` already handle focus correctly via `styles::border_active()`. Task 02 applies the same pattern.
- **min_height depends on Task 01**: Adding DartDefines to the layout increases the minimum height, so Task 05 must wait for Task 01.
