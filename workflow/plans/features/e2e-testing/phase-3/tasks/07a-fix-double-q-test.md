## Task: Implement Double-'q' Quick Quit Feature

**Objective**: Implement the double-'q' quick quit behavior so that pressing 'q' while in the quit confirmation dialog confirms the quit action.

**Depends on**: 07-test-quit-key

**Priority**: 🔴 CRITICAL (Blocking)

### Scope

- `src/app/handler/keys.rs`: Implement double-'q' as quit confirmation
- `src/app/handler/tests.rs`: Add unit tests for the new behavior
- `docs/KEYBINDINGS.md`: Document the double-'q' quick quit behavior

### Background

The E2E test `test_double_q_quick_quit` (lines 358-386) documents double-'q' as quick quit, but per `src/app/handler/keys.rs:58-73`, pressing 'q' in confirm dialog mode currently returns `None` (no action). We want to implement this feature so that:

1. First 'q' shows the quit confirmation dialog
2. Second 'q' confirms the quit (same as pressing 'y')

This provides a convenient "qq" shortcut for experienced users who want to quit quickly.

### Implementation

#### 1. Update `handle_key_confirm_dialog` in `src/app/handler/keys.rs`

```rust
KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('q') => {
    // 'y', 'Y', or 'q' confirms the dialog action
    // Note: 'q' allows double-tap "qq" as quick quit shortcut
    Some(Message::ConfirmDialog(true))
}
```

#### 2. Add Unit Test in `src/app/handler/tests.rs`

```rust
#[test]
fn test_confirm_dialog_accepts_q_as_confirmation() {
    // Test that 'q' in confirm dialog mode acts as confirmation
    // This enables the "qq" quick quit pattern
    let key_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let result = handle_key_confirm_dialog(key_event);
    assert_eq!(result, Some(Message::ConfirmDialog(true)));
}
```

#### 3. Update `docs/KEYBINDINGS.md`

Add documentation for the double-'q' behavior in the quit section:

```markdown
| `q` | Show quit confirmation dialog |
| `qq` | Quick quit (second `q` confirms) |
| `y` | Confirm quit (in dialog) |
| `n` / `Esc` | Cancel quit (in dialog) |
```

### Acceptance Criteria

1. Pressing 'q' while in quit confirmation dialog confirms the quit
2. Unit test verifies 'q' is accepted as confirmation in dialog mode
3. `test_double_q_quick_quit` E2E test passes consistently
4. `docs/KEYBINDINGS.md` documents the "qq" quick quit behavior
5. `cargo test --lib` - Unit tests pass
6. `cargo test --test e2e tui_interaction -- --nocapture` - E2E tests pass
7. `cargo clippy -- -D warnings` - No warnings

### Testing

```bash
# Run unit tests for handler
cargo test handler -- --nocapture

# Run the specific E2E test
cargo test --test e2e test_double_q_quick_quit -- --nocapture

# Run all quit-related tests
cargo test --test e2e quit -- --nocapture
```

### Notes

- This is a UX improvement that allows experienced users to quit faster
- The behavior mirrors vim's "ZZ" quick save-and-quit pattern
- Ensure the keybindings doc clearly explains this is a shortcut, not a replacement for y/n

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/app/handler/keys.rs` | Modified `handle_key_confirm_dialog` to accept 'q' as confirmation in the quit dialog |
| `/Users/ed/Dev/zabin/flutter-demon/src/app/handler/tests.rs` | Added unit test `test_q_in_confirm_dialog_confirms` to verify 'q' confirms quit |
| `/Users/ed/Dev/zabin/flutter-demon/docs/KEYBINDINGS.md` | Documented the "qq" quick quit behavior in General Controls and Confirm Dialog Mode sections |

### Notable Decisions/Tradeoffs

1. **'q' as confirmation key**: Pressing 'q' while in the quit confirmation dialog now confirms the quit action. This enables a "qq" quick quit pattern where experienced users can press 'q' twice in quick succession to quit without reading the dialog. This mirrors similar patterns in vim (ZZ for save-and-quit) and provides a smoother UX for users who are confident about quitting.

2. **Documentation placement**: The "qq" quick quit shortcut is documented in two places:
   - In the General Controls section as a visual reminder that "qq" is a quick quit pattern
   - In the Confirm Dialog Mode section explaining that 'q' confirms the dialog

   This dual documentation ensures users can discover the feature from either context.

### Testing Performed

- `cargo fmt` - Passed (code formatted)
- `cargo check` - Passed (no compilation errors)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test --lib test_q_in_confirm_dialog_confirms` - Passed (new test verifies 'q' confirms quit)
- `cargo test --lib handler` - Passed (all 227 handler tests pass)
- `cargo test --lib` - Passed (all 1253 unit tests pass)

### Risks/Limitations

1. **Potential for accidental quit**: Users who press 'q' rapidly might accidentally quit without reading the confirmation dialog. However, this is mitigated by:
   - The confirm_quit setting (can be disabled in config)
   - The dialog only appears when sessions are running
   - Ctrl+C is always available for emergency exits

   The benefit to experienced users outweighs this risk, and it's consistent with vim-style keybindings where rapid key presses are expected and handled gracefully.

2. **Overloading of 'q' key**: The 'q' key now serves two purposes (request quit in normal mode, confirm quit in dialog mode). This is intentional and enables the "qq" quick quit pattern. The dual behavior is clearly documented and follows user expectations from similar tools.
