## Task: Replace 'n' with '+' for New Session

**Objective**: Change the keybinding for starting a new session from 'n' to '+', making 'n' exclusively for search navigation.

**Depends on**: None (but should be done after Phase 1)

### Scope

- `src/app/handler/keys.rs`: Modify `handle_key_normal()` function (lines 233-248)

### Details

**Current 'n' key handling (lines 233-248):**
```rust
// 'n' - Next search match (only when search has query)
// Note: 'n' is overloaded - it's also used for "New session"
// If there's an active search query, use it for next match
// Otherwise: show StartupDialog if no sessions, DeviceSelector if sessions running
(KeyCode::Char('n'), KeyModifiers::NONE) => {
    if let Some(handle) = state.session_manager.selected() {
        if !handle.session.search_state.query.is_empty() {
            return Some(Message::NextSearchMatch);
        }
    }
    if state.has_running_sessions() {
        Some(Message::ShowDeviceSelector)
    } else {
        Some(Message::ShowStartupDialog)
    }
}
```

**Change to:**

```rust
// 'n' - Next search match (vim-style)
// Only works when there's an active search query
(KeyCode::Char('n'), KeyModifiers::NONE) => {
    if let Some(handle) = state.session_manager.selected() {
        if !handle.session.search_state.query.is_empty() {
            return Some(Message::NextSearchMatch);
        }
    }
    None // No action when no search query
}
```

**Add new '+' key handler (after 'd' handler around line 210):**

```rust
// '+' - Start new session
// If sessions are running: show quick device selector
// If no sessions: show full startup dialog
(KeyCode::Char('+'), KeyModifiers::NONE)
| (KeyCode::Char('+'), KeyModifiers::SHIFT) => {
    if state.has_running_sessions() {
        Some(Message::ShowDeviceSelector)
    } else {
        Some(Message::ShowStartupDialog)
    }
}
```

**Note on KeyModifiers**: The '+' character typically requires Shift on US keyboards (Shift + =). Test both `KeyModifiers::NONE` and `KeyModifiers::SHIFT` to ensure compatibility across different keyboard layouts and terminal emulators.

### Acceptance Criteria

1. '+' key shows StartupDialog when no sessions exist
2. '+' key shows DeviceSelector when sessions are running
3. 'n' key triggers NextSearchMatch when search query is active
4. 'n' key does nothing (returns None) when no search query is active
5. 'd' key behavior remains unchanged
6. Code comments updated to reflect new behavior

### Testing

Manual verification:
```bash
cargo run -- tests/fixtures/simple_app
# App starts in Normal mode (from Phase 1)
# Press 'n' - nothing should happen (no search active)
# Press '+' - StartupDialog should appear
# Press Escape to close
# Press '/' to start search, type something, press Enter
# Press 'n' - should jump to next match
```

Run unit tests:
```bash
cargo test keys -- --nocapture
```

### Notes

- The 'd' key handler at lines 201-210 remains unchanged as an alternative
- Some terminal emulators may report '+' differently; test with `KeyCode::Char('+')` first
- Consider adding a fallback for `KeyCode::Char('=')` if '+' doesn't work in some terminals

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending
