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
│  04-dialog-messages                 │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-dialog-state](tasks/01-dialog-state.md) | Not Started | Phase 5, 6, 6.1 | 25m | `new_session_dialog/state/dialog.rs` |
| 2 | [02-dialog-layout](tasks/02-dialog-layout.md) | Not Started | 1 | 30m | `new_session_dialog/mod.rs` |
| 3 | [03-modal-overlay](tasks/03-modal-overlay.md) | Not Started | 2 | 25m | `new_session_dialog/mod.rs` |
| 4 | [04-dialog-messages](tasks/04-dialog-messages.md) | Not Started | 3 | 20m | `app/message.rs`, `app/handler/new_session/` |

## Success Criteria

Phase 7 is complete when:

- [ ] `NewSessionDialogState` combines Target Selector and Launch Context state
- [ ] Two-pane layout renders correctly (50/50 split)
- [ ] Tab key switches pane focus
- [ ] Focused pane has highlighted border
- [ ] Fuzzy modal renders as overlay when open
- [ ] Dart Defines modal renders as full-screen overlay when open
- [ ] Footer shows context-sensitive keybindings
- [ ] Dialog respects terminal size constraints
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

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
