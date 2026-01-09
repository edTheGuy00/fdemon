## Task: Allow Settings Access from DeviceSelector Mode

**Objective**: Add comma key handling to DeviceSelector mode to allow opening settings without first dismissing the device selector.

**Depends on**: None
**Priority**: High (Required - UX enhancement for consistent settings access)

### Rationale

Users may want to access settings while viewing the device selector. Currently, they must first dismiss the selector (Escape), then press comma. Adding comma support to DeviceSelector mode improves UX and aligns with user expectations.

### Scope

- `src/app/handler/keys.rs`: Add comma handler to `handle_key_device_selector()`

### Implementation

1. Add comma key handling to `handle_key_device_selector()`:

```rust
fn handle_key_device_selector(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Message::DeviceSelectorUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::DeviceSelectorDown),

        // Selection
        KeyCode::Enter => {
            // ... existing code ...
        }

        // Refresh
        KeyCode::Char('r') => Some(Message::RefreshDevices),

        // Settings (NEW)
        KeyCode::Char(',') => Some(Message::ShowSettings),

        // Cancel/close
        KeyCode::Esc => Some(Message::HideDeviceSelector),

        // Quit
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Char('q') => Some(Message::Quit),

        _ => None,
    }
}
```

2. Add unit test for the new behavior in `keys.rs`:

```rust
#[test]
fn test_comma_opens_settings_from_device_selector() {
    let state = AppState {
        ui_mode: UiMode::DeviceSelector,
        ..AppState::new()
    };
    let msg = handle_key_device_selector(&state, key(KeyCode::Char(',')));
    assert!(matches!(msg, Some(Message::ShowSettings)));
}
```

### Acceptance Criteria

1. [ ] Comma key added to `handle_key_device_selector()` → returns `Message::ShowSettings`
2. [ ] Unit test added for new behavior
3. [ ] All existing tests pass: `cargo test --lib`
4. [ ] No clippy warnings: `cargo clippy -- -D warnings`
5. [ ] Settings can be opened from device selector in actual app

### Testing

```bash
# Unit tests
cargo test --lib handle_key

# Verify no regressions
cargo test

# Clippy
cargo clippy -- -D warnings
```

### Notes

- This is a required UX enhancement for consistent settings access
- Combined with Task 01 for complete fix
- Consider also adding comma to other modes (EmulatorSelector, Loading) for consistency

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/keys.rs` | Added comma key handler to `handle_key_device_selector()` that returns `Message::ShowSettings`, added unit test `test_comma_opens_settings_from_device_selector()` |

### Notable Decisions/Tradeoffs

1. **Placement in match arms**: Added the comma handler between "Refresh" and "Cancel/close" sections to maintain logical grouping of functionality (navigation, selection, actions, cancel/quit).
2. **Minimal change**: Only modified the device selector key handler; did not add comma support to EmulatorSelector or Loading modes as noted in task notes - those can be done separately if needed.

### Testing Performed

- `cargo test --lib device_selector_key_tests` - Passed (6 tests including new test)
- `cargo test --lib` - Passed (1321 passed, 0 failed, 3 ignored)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None identified**: This is a low-risk additive change that enhances UX without modifying existing behavior
