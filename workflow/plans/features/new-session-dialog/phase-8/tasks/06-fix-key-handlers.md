## Task: Fix '+' and 'd' Key Handlers

**Objective**: Fix the '+' and 'd' key handlers that currently send deprecated messages when no sessions exist, causing silent failures.

**Depends on**: 05-documentation

**Estimated Time**: 15 minutes

**Priority**: 🔴 Critical

**Source**: Logic Reasoning Checker, Architecture Enforcer

### Scope

- `src/app/handler/keys.rs`: Update key handlers at lines 167-189

### Problem

Both keys fail silently when no sessions exist:

```rust
// Current '+' key (lines 167-175) - BOTH branches deprecated!
if state.has_running_sessions() {
    Some(Message::ShowDeviceSelector)  // Deprecated!
} else {
    Some(Message::ShowStartupDialog)   // Deprecated!
}

// Current 'd' key (lines 181-189) - else branch deprecated!
if state.has_running_sessions() {
    Some(Message::OpenNewSessionDialog)  // Works
} else {
    Some(Message::ShowStartupDialog)     // Deprecated - silently fails!
}
```

**User Experience**: User presses `+` or `d` without sessions → deprecated message sent → warning logged → nothing happens → user confused.

### Solution

Unify both handlers to always use `Message::OpenNewSessionDialog`:

```rust
// '+' - Start new session (unified handler)
(KeyCode::Char('+'), KeyModifiers::NONE) | (KeyCode::Char('+'), KeyModifiers::SHIFT) => {
    if state.ui_mode == UiMode::Loading {
        None
    } else {
        Some(Message::OpenNewSessionDialog)
    }
}

// 'd' for adding device/session (alternative to '+')
(KeyCode::Char('d'), KeyModifiers::NONE) => {
    if state.ui_mode == UiMode::Loading {
        None
    } else {
        Some(Message::OpenNewSessionDialog)
    }
}
```

### Acceptance Criteria

1. `+` key with no sessions → NewSessionDialog opens
2. `+` key with sessions → NewSessionDialog opens
3. `d` key with no sessions → NewSessionDialog opens
4. `d` key with sessions → NewSessionDialog opens
5. Both keys blocked during `UiMode::Loading`
6. No deprecated message warnings in logs
7. `cargo check` passes

### Testing

Manual testing required:
1. Start app with no sessions, press `+` → dialog opens
2. Start app with no sessions, press `d` → dialog opens
3. Start app with sessions, press `+` → dialog opens
4. Start app with sessions, press `d` → dialog opens

Unit tests in `keys.rs` will need updating in Task 09.

### Notes

- Also update line 44 (`KeyCode::Esc => Some(Message::ShowDeviceSelector)`) if it exists in the `NewSessionDialog` context
- The unified behavior simplifies the mental model: both keys always open the same dialog

---

## Completion Summary

**Status:** Not Started
