# Task: Complete Key Routing

## Summary

Implement complete keyboard handling for NewSessionDialog. Currently only Tab, Escape, and tab shortcuts (1/2) are handled - all other keys are ignored.

**Priority:** CRITICAL (Blocking merge)

## Files

| File | Action |
|------|--------|
| `src/app/handler/keys.rs` | Modify (implement full key routing) |

## Problem

Current implementation at `src/app/handler/keys.rs:679-703`:

```rust
// Line 700-702
// All other keys - delegate to modal-aware handler
_ => None,  // Should actually route to modal/pane handlers
```

This means users cannot:
- Navigate device lists (Up/Down)
- Select devices (Enter)
- Interact with modals
- Navigate launch context fields

## Implementation

Replace the current `handle_key_new_session_dialog()` function with complete routing:

```rust
fn handle_key_new_session_dialog(key: KeyEvent, state: &AppState) -> Option<Message> {
    use crate::tui::widgets::TargetTab;

    let dialog = &state.new_session_dialog_state;

    match (key.code, key.modifiers) {
        // Ctrl+C to quit (highest priority)
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Check if modal is open first
        _ if dialog.is_fuzzy_modal_open() => handle_fuzzy_modal_key(key),
        _ if dialog.is_dart_defines_modal_open() => handle_dart_defines_modal_key(key),

        // Main dialog keys
        (KeyCode::Esc, _) => Some(Message::NewSessionDialogEscape),
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchPane),
        (KeyCode::Char('1'), KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
        (KeyCode::Char('2'), KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),

        // Route based on focused pane
        _ => match dialog.focused_pane {
            DialogPane::TargetSelector => handle_target_selector_key(key),
            DialogPane::LaunchContext => handle_launch_context_key(key, dialog),
        },
    }
}

fn handle_fuzzy_modal_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFuzzyUp),
        KeyCode::Down => Some(Message::NewSessionDialogFuzzyDown),
        KeyCode::Enter => Some(Message::NewSessionDialogFuzzyConfirm),
        KeyCode::Esc => Some(Message::NewSessionDialogCloseFuzzyModal),
        KeyCode::Backspace => Some(Message::NewSessionDialogFuzzyBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogFuzzyInput { c }),
        _ => None,
    }
}

fn handle_dart_defines_modal_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Tab => Some(Message::NewSessionDialogDartDefinesSwitchPane),
        KeyCode::Up => Some(Message::NewSessionDialogDartDefinesUp),
        KeyCode::Down => Some(Message::NewSessionDialogDartDefinesDown),
        KeyCode::Enter => Some(Message::NewSessionDialogDartDefinesActivate),
        KeyCode::Esc => Some(Message::NewSessionDialogCloseDartDefinesModal),
        KeyCode::Backspace => Some(Message::NewSessionDialogDartDefinesBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogDartDefinesInput { c }),
        _ => None,
    }
}

fn handle_target_selector_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogDeviceUp),
        KeyCode::Down => Some(Message::NewSessionDialogDeviceDown),
        KeyCode::Enter => Some(Message::NewSessionDialogDeviceSelect),
        KeyCode::Char('r') => Some(Message::NewSessionDialogRefreshDevices),
        _ => None,
    }
}

fn handle_launch_context_key(key: KeyEvent, dialog: &NewSessionDialogState) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFieldPrev),
        KeyCode::Down => Some(Message::NewSessionDialogFieldNext),
        KeyCode::Enter => Some(Message::NewSessionDialogFieldActivate),
        KeyCode::Left if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModePrev)
        }
        KeyCode::Right if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModeNext)
        }
        _ => None,
    }
}
```

## Acceptance Criteria

1. All keys routed appropriately based on context
2. Dialog is fully navigable with keyboard
3. Modal keys work when modals are open
4. Main dialog keys work when no modal is open
5. Pane-specific keys work based on focused pane

## Manual Testing

After implementation, verify:
1. Open dialog with 'd' key
2. Up/Down navigates device list
3. Tab switches between panes
4. 1/2 switches between Connected/Bootable tabs
5. Enter on device selects it
6. 'r' refreshes devices
7. In Launch Context: Up/Down navigates fields
8. Enter on Configuration opens fuzzy modal
9. In fuzzy modal: Up/Down/Enter/Esc work
10. Ctrl+C quits from anywhere

## Testing

```bash
cargo fmt && cargo check && cargo clippy -- -D warnings
```

## Notes

- Key routing priority: Ctrl+C > Modals > Main dialog > Focused pane
- Modal state must be checked before routing to pane handlers
- `handle_launch_context_key` needs dialog reference to check focused field for Left/Right mode cycling

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/keys.rs` | Implemented complete keyboard routing for NewSessionDialog with modal-aware handling and pane-specific key routing |

### Notable Decisions/Tradeoffs

1. **Import Scoping**: Moved `LaunchContextField` import to the `handle_launch_context_key` function to avoid scoping issues. Each helper function now imports only the types it needs.
2. **State Parameter**: Updated `handle_key_new_session_dialog` signature to accept `&AppState` to access `new_session_dialog_state` for routing decisions.
3. **Match Guard Pattern**: Used `_ if dialog.is_fuzzy_modal_open()` pattern to check modal state before other key handling, ensuring modals have highest priority after Ctrl+C.
4. **Helper Functions**: Created four helper functions (`handle_fuzzy_modal_key`, `handle_dart_defines_modal_key`, `handle_target_selector_key`, `handle_launch_context_key`) for clean separation of concerns and maintainability.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (no compilation errors)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- Unit tests compilation: Some existing tests fail due to outdated dialog structure from previous refactoring (not related to this task)

### Risks/Limitations

1. **Existing test failures**: Pre-existing tests in `src/app/handler/tests.rs` reference old dialog state structure (e.g., direct access to `connected_devices` instead of `target_selector.connected_devices`). These tests need updating but are outside the scope of this task which focuses only on key routing implementation.
2. **Manual testing required**: Full keyboard navigation should be manually tested once the dialog UI is rendered to verify all key combinations work as expected.
