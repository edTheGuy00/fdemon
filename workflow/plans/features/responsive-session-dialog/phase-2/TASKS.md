# Phase 2: Fix Launch Button Overflow - Task Index

## Overview

Ensure the Launch button never renders outside the dialog bounds by including it in Ratatui's layout system instead of computing its `Rect` manually. Currently `render_full()` constructs the button position with raw arithmetic (`chunks[9].y + chunks[9].height + 1`) which can overshoot `area.bottom()` when the content area is shorter than 29 rows.

**Total Tasks:** 3
**Estimated Hours:** 4-6 hours

## Task Dependency Graph

```
┌────────────────────────────────────┐
│  01-extend-layout-with-button-slot │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│  02-render-full-use-layout-button  │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│  03-unit-tests                     │
└────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-extend-layout-with-button-slot](tasks/01-extend-layout-with-button-slot.md) | Done | - | 1-2h | `launch_context.rs` |
| 2 | [02-render-full-use-layout-button](tasks/02-render-full-use-layout-button.md) | Done | 1 | 1-2h | `launch_context.rs` |
| 3 | [03-unit-tests](tasks/03-unit-tests.md) | Done | 2 | 2h | `launch_context.rs` |

## Success Criteria

Phase 2 is complete when:

- [ ] Launch button never renders outside dialog bounds at any terminal size
- [ ] Button is included in the layout system (not manually positioned)
- [ ] Both `LaunchContext::render()` and `LaunchContextWithDevice::render_full()` use the layout slot
- [ ] `render_common_fields` signature updated for new array size
- [ ] `min_height()` remains correct and is verified by updated arithmetic test
- [ ] All existing tests pass (`cargo test -p fdemon-tui`)
- [ ] New unit tests cover button overflow scenarios (heights 20, 25, 29, 40)
- [ ] `cargo clippy --workspace -- -D warnings` passes

## Notes

- All changes in this phase are within `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`
- Phase 1 already implemented the auto-switch to compact mode when `area.height < MIN_EXPANDED_LAUNCH_HEIGHT` (29). This means `render_full()` should only be called when height >= 29. However, the layout fix is still needed as a defense-in-depth measure — the button should never overflow even if the compact guard is bypassed or the threshold is adjusted.
- The `render_compact()` path is already safe — it uses `Layout::vertical` with `Constraint::Length(3)` for the button slot. No changes needed there.
- The `LaunchContext` widget (without device awareness) has the same manual button placement bug as `LaunchContextWithDevice` and needs the same fix.
