## Task: Handle `Ctrl+C` in Tag Filter Overlay

**Objective**: Allow `Ctrl+C` to quit the application while the tag filter overlay is open. Currently the `_ => None` catch-all silently swallows `Ctrl+C`, making the user unable to force-quit without first closing the overlay.

**Depends on**: None

**Review Issue:** #5 (Major)

### Scope

- `crates/fdemon-app/src/handler/keys.rs`: Add `CharCtrl('c')` arm to the tag filter overlay match (lines 104-123)

### Details

#### Problem

The tag filter overlay key handler at lines 104-123:

```rust
if state.tag_filter_visible {
    return match key {
        InputKey::Esc | InputKey::Char('T') | InputKey::Char('t') => {
            Some(Message::HideTagFilter)
        }
        InputKey::Up | InputKey::Char('k') => Some(Message::TagFilterMoveUp),
        InputKey::Down | InputKey::Char('j') => Some(Message::TagFilterMoveDown),
        InputKey::Char(' ') | InputKey::Enter => Some(Message::TagFilterToggleSelected),
        InputKey::Char('a') => Some(Message::ShowAllNativeTags),
        InputKey::Char('n') => Some(Message::HideAllNativeTags),
        _ => None,   // ← CharCtrl('c') falls here — Ctrl+C swallowed
    };
}
```

Every other overlay in the same file explicitly handles `CharCtrl('c') => Some(Message::Quit)`:
- `ConfirmDialog` (line 33)
- `EmulatorSelector` (line 42)
- `Loading` (line 51)
- `SearchInput` (line 93)

#### Fix

Add `InputKey::CharCtrl('c') => Some(Message::Quit)` before the catch-all:

```rust
InputKey::Char('n') => Some(Message::HideAllNativeTags),
InputKey::CharCtrl('c') => Some(Message::Quit),
_ => None,
```

### Acceptance Criteria

1. Pressing `Ctrl+C` while the tag filter overlay is open produces `Message::Quit`
2. All other tag filter keys still work (Esc, T, j/k, Space, Enter, a, n)
3. Consistent with every other overlay in `keys.rs`
4. `cargo test -p fdemon-app -- keys` passes

### Testing

Add a test in the handler test suite:

```rust
#[test]
fn test_tag_filter_ctrl_c_quits() {
    let mut state = test_state();
    state.tag_filter_visible = true;
    let result = map_key_to_message(&state, InputKey::CharCtrl('c'));
    assert_eq!(result, Some(Message::Quit));
}
```

### Notes

- Single-line fix. Low risk.
- The catch-all `_ => None` is still correct for other unhandled keys (it prevents keypresses from leaking through the overlay to the main view).

---

## Completion Summary

**Status:** Not Started
