# Phase 7: Main Dialog Assembly - Task Index

## Overview

Combine Target Selector and Launch Context into the main NewSessionDialog widget. Handle pane focus, modal rendering, and overall dialog layout.

**Total Tasks:** 4
**Estimated Time:** 2 hours

## Prerequisites

**Depends on:** Phase 6.1 (File Splitting Refactoring)

Phase 6.1 must be completed first because it restructures the files modified in this phase:
- `new_session_dialog/state.rs` → `new_session_dialog/state/dialog.rs`
- `app/handler/update.rs` → `app/handler/new_session/` module

## UI Design

```
┌── NewSessionDialog ─────────────────────────────────────────────────────┐
│                                                                         │
│  ┌── Target Selector ──────────────┐ ┌── Launch Context ─────────────┐  │
│  │         (50% width)             │ │        (50% width)            │  │
│  │                                 │ │                               │  │
│  │  ╭───────────╮ ╭───────────╮    │ │  Configuration:               │  │
│  │  │1 Connected│ │2 Bootable │    │ │  [ Development         ▼]    │  │
│  │  ╰───────────╯ ╰───────────╯    │ │                               │  │
│  │                                 │ │  Mode:                        │  │
│  │  iOS Devices                    │ │  (●) Debug (○) Profile        │  │
│  │  ▶ iPhone 15 Pro               │ │  (○) Release                  │  │
│  │                                 │ │                               │  │
│  │  Android Devices                │ │  Flavor:                      │  │
│  │    Pixel 8                      │ │  [ dev               ▼]      │  │
│  │                                 │ │                               │  │
│  │                                 │ │  Dart Defines:                │  │
│  │  [Enter] Select [r] Refresh     │ │  [ 0 items           ▶]      │  │
│  │                                 │ │                               │  │
│  └─────────────────────────────────┘ │  [    LAUNCH (Enter)    ]    │  │
│                                      └───────────────────────────────┘  │
│                                                                         │
│  [1/2] Tab  [Tab] Pane  [↑↓] Navigate  [Enter] Select  [Esc] Close     │
└─────────────────────────────────────────────────────────────────────────┘
```

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-dialog-state                    │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-dialog-layout                   │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-modal-overlay                   │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-dialog-messages  ✅ DONE        │
└────────────────┬────────────────────┘
                 │
    ┌────────────┼────────────┬───────────────┬───────────────┐
    │            │            │               │               │
    ▼            ▼            ▼               ▼               ▼
┌────────┐ ┌────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐
│ 05-fix │ │ 06-fix │ │ 07-key     │ │ 08-unwrap  │ │ 09-modal   │
│ tests  │ │ layers │ │ routing    │ │            │ │ exclusive  │
│CRITICAL│ │CRITICAL│ │ CRITICAL   │ │ CRITICAL   │ │ Major      │
└────────┘ └────────┘ └────────────┘ └────────────┘ └────────────┘
                 │                                        │
    ┌────────────┼────────────────────────────────────────┤
    │            │                                        │
    ▼            ▼                                        ▼
┌────────────┐ ┌────────────┐                   ┌────────────────┐
│ 10-config  │ │ 11-doc     │                   │ 12-footer      │
│ errors     │ │ comments   │                   │ constants      │
│ Major      │ │ Minor      │                   │ Minor          │
└────────────┘ └────────────┘                   └────────────────┘
```

**Review Follow-up Execution Order:**
- Wave 1 (parallel): 05, 06, 07, 08 - All CRITICAL, can run in parallel
- Wave 2 (parallel): 09, 10, 11, 12 - Major/Minor, depends on Wave 1

## Tasks

### Initial Implementation (Complete)

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-dialog-state](tasks/01-dialog-state.md) | Done | Phase 5, 6, 6.1 | 25m | `new_session_dialog/state/dialog.rs` |
| 2 | [02-dialog-layout](tasks/02-dialog-layout.md) | Done | 1 | 30m | `new_session_dialog/mod.rs` |
| 3 | [03-modal-overlay](tasks/03-modal-overlay.md) | Done | 2 | 25m | `new_session_dialog/mod.rs` |
| 4 | [04-dialog-messages](tasks/04-dialog-messages.md) | Done | 3 | 20m | `app/message.rs`, `app/handler/new_session/` |

### Review Follow-up Tasks

| # | Task | Status | Depends On | Est. | Modules | Priority |
|---|------|--------|------------|------|---------|----------|
| 5 | [05-fix-test-suite](tasks/05-fix-test-suite.md) | Done | 4 | 4-6h | `app/handler/tests.rs`, `new_session_dialog/state/tests/` | CRITICAL |
| 6 | [06-fix-layer-boundaries](tasks/06-fix-layer-boundaries.md) | Done | 4 | 2-3h | `app/new_session_dialog/`, `tui/widgets/new_session_dialog/` | CRITICAL |
| 7 | [07-complete-key-routing](tasks/07-complete-key-routing.md) | Done | 4 | 1-2h | `app/handler/keys.rs` | CRITICAL |
| 8 | [08-remove-unsafe-unwrap](tasks/08-remove-unsafe-unwrap.md) | Done | 4 | 15m | `app/handler/new_session/launch_context.rs` | CRITICAL |
| 9 | [09-modal-exclusivity](tasks/09-modal-exclusivity.md) | Done | 4 | 20m | `new_session_dialog/state/dialog.rs` | Major |
| 10 | [10-config-error-handling](tasks/10-config-error-handling.md) | Done | 4 | 20m | `app/handler/new_session/navigation.rs` | Major |
| 11 | [11-add-doc-comments](tasks/11-add-doc-comments.md) | Done | 4 | 30m | `app/handler/new_session/*.rs` | Minor |
| 12 | [12-extract-footer-constants](tasks/12-extract-footer-constants.md) | Done | 4 | 15m | `tui/widgets/new_session_dialog/mod.rs` | Minor |

## Success Criteria

### Initial Implementation (Complete)

- [x] `NewSessionDialogState` combines Target Selector and Launch Context state
- [x] Two-pane layout renders correctly (50/50 split)
- [x] Tab key switches pane focus
- [x] Focused pane has highlighted border
- [x] Fuzzy modal renders as overlay when open
- [x] Dart Defines modal renders as full-screen overlay when open
- [x] Footer shows context-sensitive keybindings
- [x] Dialog respects terminal size constraints

### Review Follow-up (Blocking Merge)

Phase 7 is complete when:

- [x] `cargo test --lib` compiles without errors (Task 05)
- [x] No TUI imports in App layer files (Task 06)
- [x] All keys routed correctly - dialog fully navigable (Task 07)
- [x] No `unwrap()` calls in handler code (Task 08)
- [x] Modal exclusivity assertions in place (Task 09)
- [x] Config loading errors handled gracefully (Task 10)
- [x] `cargo fmt && cargo check && cargo test --lib && cargo clippy -- -D warnings` passes

## Pane Focus

- **Tab key:** Switches focus between Target Selector and Launch Context
- **Visual indicator:** Active pane has cyan border, inactive has gray border
- **Keyboard routing:** Keys are routed to the focused pane

## Modal Layering

1. **Base layer:** Main dialog (Target Selector + Launch Context)
2. **Fuzzy modal:** Overlay at bottom 40%, background dimmed
3. **Dart Defines modal:** Full-screen overlay (replaces main dialog)

When modal is open:
- Main dialog keys are blocked
- Modal handles its own key events
- Esc closes modal, returns to main dialog

## Notes

- Dialog is centered in terminal (80% width, 70% height)
- Minimum terminal size: 80x24
- Use `ratatui::symbols::border::ROUNDED` for borders
- Footer updates based on context (focused pane, modal state)
