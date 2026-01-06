# Task: Section Jump Keys (1-5)

**Objective**: Implement number keys 1-5 to jump directly to startup dialog sections.

**Depends on**: Task 04 (Dialog Key Handler)

## Problem

In `src/app/handler/keys.rs` (lines 536-539), the section jump handler returns `None`:

```rust
(KeyCode::Char('1'..='5'), KeyModifiers::NONE) => {
    // Quick section jump - not implemented yet (needs new message type)
    None
}
```

## Scope

- `src/app/message.rs` - Add new message type
- `src/app/handler/keys.rs` - Implement key handler
- `src/app/handler/update.rs` - Add message handler
- `src/app/state.rs` - Add `jump_to_section()` method

## Implementation

### 1. Add Message Type (`src/app/message.rs`)

```rust
// In StartupDialog Messages section:

/// Jump directly to a dialog section (1=Configs, 2=Mode, 3=Flavor, 4=DartDefines, 5=Devices)
StartupDialogJumpToSection(DialogSection),
```

### 2. Update Key Handler (`src/app/handler/keys.rs`)

```rust
// Replace the placeholder in handle_key_startup_dialog:

(KeyCode::Char('1'), KeyModifiers::NONE) => {
    Some(Message::StartupDialogJumpToSection(DialogSection::Configs))
}
(KeyCode::Char('2'), KeyModifiers::NONE) => {
    Some(Message::StartupDialogJumpToSection(DialogSection::Mode))
}
(KeyCode::Char('3'), KeyModifiers::NONE) => {
    Some(Message::StartupDialogJumpToSection(DialogSection::Flavor))
}
(KeyCode::Char('4'), KeyModifiers::NONE) => {
    Some(Message::StartupDialogJumpToSection(DialogSection::DartDefines))
}
(KeyCode::Char('5'), KeyModifiers::NONE) => {
    Some(Message::StartupDialogJumpToSection(DialogSection::Devices))
}
```

### 3. Add State Method (`src/app/state.rs`)

```rust
impl StartupDialogState {
    /// Jump directly to a section
    pub fn jump_to_section(&mut self, section: DialogSection) {
        self.editing = false;  // Exit any edit mode
        self.active_section = section;
    }
}
```

### 4. Add Message Handler (`src/app/handler/update.rs`)

```rust
Message::StartupDialogJumpToSection(section) => {
    state.startup_dialog_state.jump_to_section(section);
    UpdateResult::none()
}
```

## Acceptance Criteria

1. Pressing 1-5 in startup dialog jumps to corresponding section
2. Section mapping: 1=Configs, 2=Mode, 3=Flavor, 4=DartDefines, 5=Devices
3. Jumping to section exits any active edit mode
4. Unit tests for key handler and state method

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_keys_jump_to_section() {
        let state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;

        let msg = handle_key_startup_dialog(&state, key('1'));
        assert!(matches!(msg, Some(Message::StartupDialogJumpToSection(DialogSection::Configs))));

        let msg = handle_key_startup_dialog(&state, key('2'));
        assert!(matches!(msg, Some(Message::StartupDialogJumpToSection(DialogSection::Mode))));

        let msg = handle_key_startup_dialog(&state, key('3'));
        assert!(matches!(msg, Some(Message::StartupDialogJumpToSection(DialogSection::Flavor))));

        let msg = handle_key_startup_dialog(&state, key('4'));
        assert!(matches!(msg, Some(Message::StartupDialogJumpToSection(DialogSection::DartDefines))));

        let msg = handle_key_startup_dialog(&state, key('5'));
        assert!(matches!(msg, Some(Message::StartupDialogJumpToSection(DialogSection::Devices))));
    }

    #[test]
    fn test_jump_to_section_clears_editing() {
        let mut state = StartupDialogState::new();
        state.editing = true;
        state.active_section = DialogSection::Flavor;

        state.jump_to_section(DialogSection::Devices);

        assert!(!state.editing);
        assert_eq!(state.active_section, DialogSection::Devices);
    }
}
```

## Notes

- This matches the UX pattern from settings panel (1-4 for tabs)
- Visual hint should be added to footer in a future update

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/message.rs` | Added `StartupDialogJumpToSection(DialogSection)` message variant |
| `src/app/state.rs` | Added `jump_to_section(&mut self, section: DialogSection)` method to `StartupDialogState` |
| `src/app/handler/keys.rs` | Implemented key handlers for 1-5 keys to generate jump messages |
| `src/app/handler/update.rs` | Added message handler for `StartupDialogJumpToSection` |
| `src/app/handler/tests.rs` | Added 4 unit tests covering key handlers and state transitions |

### Notable Decisions/Tradeoffs

1. **Direct Section Mapping**: Keys 1-5 map directly to sections in order (Configs, Mode, Flavor, DartDefines, Devices) for consistency with settings panel UX pattern (1-4 for tabs)
2. **Exit Edit Mode on Jump**: Jumping to a section always exits edit mode to provide predictable state transitions and avoid confusion about which field is being edited
3. **Comprehensive Testing**: Added tests for both key handler generation and state mutation to ensure end-to-end functionality

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1153 tests total, including 4 new tests for this feature)
- `cargo clippy` - Passed (no warnings)

### Tests Added

1. `test_number_keys_jump_to_section` - Verifies all 5 number keys generate correct messages
2. `test_jump_to_section_clears_editing` - Ensures edit mode is cleared when jumping
3. `test_jump_to_section_message_handler` - Tests full message handling flow
4. `test_jump_to_section_changes_section` - Verifies section changes work correctly

### Risks/Limitations

None identified. Feature is self-contained and follows existing patterns in the codebase.
