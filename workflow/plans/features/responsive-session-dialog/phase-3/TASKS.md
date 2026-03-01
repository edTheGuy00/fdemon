# Phase 3: Fix Scroll-to-Selected in Target Selector - Task Index

## Overview

Ensure the selected device is always visible when scrolling through the device list, regardless of actual terminal height. Currently `handle_device_up/down` calls `adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT)` with a hardcoded value of `10`, but the actual device list area can be anywhere from 4 to 30+ rows depending on terminal size, compact/full mode, and layout orientation. This phase closes the render-to-state feedback loop so the handler uses the real visible height, and adds a render-time safety net so the display is always correct even if the handler's estimate is slightly stale.

**Total Tasks:** 4
**Estimated Hours:** 6-9 hours

## Task Dependency Graph

```
┌──────────────────────────────────────┐
│  01-add-visible-height-field         │
└───────────┬──────────────────────────┘
            │
       ┌────┴────┐
       ▼         ▼
┌──────────────────┐  ┌──────────────────────────────────┐
│ 02-renderer-     │  │ 03-handler-use-actual-height     │
│ write-and-clamp  │  └──────────────────────────────────┘
└──────────────────┘
       │                       │
       └───────────┬───────────┘
                   ▼
       ┌──────────────────────┐
       │  04-unit-tests        │
       └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-add-visible-height-field](tasks/01-add-visible-height-field.md) | Not Started | - | 1h | `target_selector_state.rs` |
| 2 | [02-renderer-write-and-clamp](tasks/02-renderer-write-and-clamp.md) | Not Started | 1 | 2-3h | `target_selector.rs` (TUI) |
| 3 | [03-handler-use-actual-height](tasks/03-handler-use-actual-height.md) | Not Started | 1 | 1-2h | `target_selector.rs` (app handler) |
| 4 | [04-unit-tests](tasks/04-unit-tests.md) | Not Started | 2, 3 | 2-3h | `target_selector_state.rs`, `target_selector.rs` (TUI + app handler), `device_list.rs` |

## Design Decision: `Cell<usize>` for Interior Mutability

The core challenge is that Ratatui's `Widget::render(self, area, buf)` consumes the widget by value, and the `TargetSelector` widget holds `state: &'a TargetSelectorState` — an immutable reference. To write back the computed `visible_height` from the render path, we use `Cell<usize>` for interior mutability.

**Why `Cell<usize>` (not `StatefulWidget` refactor):**
- `Cell<usize>` implements `Debug` and `Clone`, matching `TargetSelectorState`'s existing derives
- Changes are scoped to TargetSelector only — no cascading refactor through `NewSessionDialog`, `render_panes`, etc.
- The PLAN's "Edge Cases & Risks" section explicitly endorses this approach: "Use `Cell<usize>` interior mutability for a single numeric hint value. This is a pragmatic concession common in TUI frameworks."
- The alternative (`StatefulWidget` promotion of `NewSessionDialog`) would require threading `&mut NewSessionDialogState` through ~8 internal render methods across `mod.rs` — a large blast radius for a single feedback value

**Why not a `Message`-based approach:**
- Would introduce one-frame lag (height only known after render, but needed during handler which runs before render)
- Would require duplicating layout math outside the widget to pre-compute heights
- The handler still needs a fallback for the first frame anyway

## Success Criteria

Phase 3 is complete when:

- [ ] Selected device is always visible when scrolling with arrow keys
- [ ] Works correctly at all terminal heights (tested at 20, 25, 40, 80 rows)
- [ ] First frame uses `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` (10), subsequent frames use actual height
- [ ] Render-time scroll correction prevents off-screen selection even when handler estimate is stale
- [ ] Scroll indicators (arrows) still display correctly
- [ ] All existing target selector tests pass + new scroll visibility tests
- [ ] `cargo clippy --workspace -- -D warnings` passes

## Notes

- All TUI changes are within `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`
- All app-layer state changes are within `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`
- All handler changes are within `crates/fdemon-app/src/handler/new_session/target_selector.rs`
- The `calculate_scroll_offset` function is duplicated in both `target_selector_state.rs` (private) and `device_list.rs` (public). Task 02 reuses the public TUI copy for render-time correction. No deduplication in this phase.
- `Cell<usize>` is zero-cost at runtime (no runtime checks, same as a plain `usize` in generated code)
