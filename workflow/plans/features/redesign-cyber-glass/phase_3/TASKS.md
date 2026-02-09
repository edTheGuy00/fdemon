# Phase 3: New Session Modal Redesign - Task Index

## Overview

Transform the New Session modal to match the Cyber-Glass design with a glass overlay, shadow, refined two-pane layout with a tab toggle, categorized devices, configuration fields, and a prominent launch button. Also migrate the palette from named colors to RGB design tokens.

**Total Tasks:** 8
**Crate:** `fdemon-tui` (rendering), `fdemon-tui/src/theme` (palette)
**Depends on:** Phase 1 (theme module), Phase 2 (main screen redesign)

## Task Dependency Graph

```
┌───────────────────────────────┐
│  01-migrate-palette-to-rgb    │
│  (named colors → RGB tokens)  │
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐     ┌───────────────────────────────┐
│  02-redesign-modal-overlay    │     │  03-redesign-modal-frame      │
│  (dim bg, shadow, centering)  │     │  (glass frame, header, title) │
└───────────────┬───────────────┘     └───────────────┬───────────────┘
                │                                     │
                └─────────────┬───────────────────────┘
                              ▼
┌───────────────────────────────┐     ┌───────────────────────────────┐
│  04-redesign-target-selector  │     │  05-redesign-launch-context   │
│  (tab toggle, device list,    │     │  (fields, mode buttons,       │
│   category headers, icons)    │     │   launch button, dropdowns)   │
└───────────────┬───────────────┘     └───────────────┬───────────────┘
                │                                     │
                └─────────────┬───────────────────────┘
                              ▼
              ┌───────────────────────────────┐
              │  06-redesign-modal-footer     │
              │  (kbd-style shortcut hints)   │
              └───────────────┬───────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │  07-migrate-nested-modals     │
              │  (fuzzy + dart defines theme) │
              └───────────────┬───────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │  08-update-tests              │
              │  (fix broken, add new)        │
              └───────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-migrate-palette-to-rgb](tasks/01-migrate-palette-to-rgb.md) | Not Started | - | `theme/palette.rs` |
| 2 | [02-redesign-modal-overlay](tasks/02-redesign-modal-overlay.md) | Not Started | 1 | `widgets/modal_overlay.rs`, `render/mod.rs` |
| 3 | [03-redesign-modal-frame](tasks/03-redesign-modal-frame.md) | Not Started | 1 | `widgets/new_session_dialog/mod.rs` |
| 4 | [04-redesign-target-selector](tasks/04-redesign-target-selector.md) | Not Started | 2, 3 | `widgets/new_session_dialog/target_selector.rs`, `tab_bar.rs`, `device_list.rs` |
| 5 | [05-redesign-launch-context](tasks/05-redesign-launch-context.md) | Not Started | 2, 3 | `widgets/new_session_dialog/launch_context.rs` |
| 6 | [06-redesign-modal-footer](tasks/06-redesign-modal-footer.md) | Not Started | 4, 5 | `widgets/new_session_dialog/mod.rs` |
| 7 | [07-migrate-nested-modals](tasks/07-migrate-nested-modals.md) | Not Started | 1, 2 | `widgets/new_session_dialog/fuzzy_modal.rs`, `dart_defines_modal.rs` |
| 8 | [08-update-tests](tasks/08-update-tests.md) | Not Started | 3, 4, 5, 6, 7 | All test modules |

## Execution Strategy

**Wave 1**: Task 01 — RGB palette migration. This is the foundation: all subsequent tasks reference RGB design token values.

**Wave 2** (parallel, after 01): Tasks 02 and 03 — modal overlay system and modal frame redesign. These are independent of each other (overlay handles dim/shadow, frame handles the dialog container itself).

**Wave 3** (parallel, after 02+03): Tasks 04 and 05 — left pane (target selector) and right pane (launch context) redesign. These are independent and can be developed simultaneously. Task 07 (nested modals) can also run in parallel here since it only depends on 01 and 02.

**Wave 4** (after 04+05): Task 06 — modal footer redesign. Needs both panes to be in place since footer integrates with the overall layout.

**Wave 5** (after all): Task 08 — update all broken tests and add new ones.

## Success Criteria

Phase 3 is complete when:

- [ ] All palette colors use `Color::Rgb()` values matching the Cyber-Glass design tokens
- [ ] New Session modal renders with dimmed background overlay (all cells darkened)
- [ ] Modal has 1-cell shadow effect (dark offset to right+bottom)
- [ ] Modal frame uses glass container style (`POPUP_BG` bg, `BorderType::Rounded`, `BORDER_DIM` border)
- [ ] Modal header shows "New Session" title + subtitle with themed typography
- [ ] Left panel (40%): pill-style tab toggle (Connected/Bootable) with `ACCENT` active tab
- [ ] Left panel: categorized device list with uppercase headers in `ACCENT_DIM`
- [ ] Left panel: device rows with platform icons and themed selection highlighting
- [ ] Right panel: labeled dropdown fields with `SURFACE` bg and `BORDER_DIM` border
- [ ] Right panel: mode selector with 3 buttons (selected uses `ACCENT` bg + glow style)
- [ ] Right panel: full-width launch button with `GRADIENT_BLUE` background and play icon
- [ ] Footer shows keyboard hints in "kbd" style (styled key badges + muted labels)
- [ ] Fuzzy modal uses theme palette colors (no hardcoded RGB outside `theme/`)
- [ ] Dart defines modal uses theme palette colors (no hardcoded RGB outside `theme/`)
- [ ] Horizontal (>= 70x20) and Vertical (40-69x20) layouts both render correctly
- [ ] All existing functionality preserved (tab switching, device selection, fuzzy search, dart defines editing)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes with no warnings

## Notes

- **Palette migration first**: Unlike Phase 2 which deferred RGB migration, Phase 3 starts by migrating the entire palette to RGB values. This gives all subsequent tasks the correct design token colors from the start.
- **Functionality preservation is critical**: The new session dialog has complex features (responsive layouts, nested modals, fuzzy search, device grouping, config loading). None of these should regress.
- **Compact/vertical mode**: All redesigned widgets must work in both horizontal (2-pane) and vertical (stacked) layouts. Tasks should address both modes.
- **Existing modal_overlay.rs utilities**: `dim_background()`, `render_shadow()`, and `centered_rect()` already exist but are not currently used by the new session dialog. Task 02 wires these up.
- **Phase 2 palette comments**: The palette currently has comments like `// Phase 2: Rgb(10,12,16)` — Task 01 replaces the named colors with these RGB values.
