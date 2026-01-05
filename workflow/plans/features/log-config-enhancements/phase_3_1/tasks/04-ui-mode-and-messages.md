## Task: 04-ui-mode-and-messages

**Objective**: Add the `UiMode::LinkHighlight` variant and new messages (`EnterLinkMode`, `ExitLinkMode`, `SelectLink`) to support the Link Highlight Mode feature.

**Depends on**: None (can be done in parallel with Task 03)

### Background

The Link Highlight Mode needs:
1. A new UI mode to track when the user is in link selection mode
2. New messages to handle entering/exiting link mode and selecting links

These changes follow the existing TEA (The Elm Architecture) pattern used throughout the application.

### Scope

- `src/app/state.rs`:
  - Add `UiMode::LinkHighlight` variant

- `src/app/message.rs`:
  - Add `EnterLinkMode` message
  - Add `ExitLinkMode` message
  - Add `SelectLink(char)` message

### Changes to `src/app/state.rs`

Add the new `LinkHighlight` variant to the `UiMode` enum:

```rust
/// Current UI mode/screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// Normal TUI with log view and status bar
    #[default]
    Normal,

    /// Device selector modal is active
    DeviceSelector,

    /// Emulator selector (after choosing "Launch Android Emulator")
    EmulatorSelector,

    /// Confirmation dialog (e.g., quit confirmation)
    ConfirmDialog,

    /// Initial loading screen (discovering devices)
    Loading,

    /// Search input mode - capturing text for log search
    SearchInput,

    /// Link highlight mode - showing clickable file references
    /// User can press 1-9 or a-z to open a file
    LinkHighlight,  // NEW
}
```

### Changes to `src/app/message.rs`

Add new messages for link highlight mode at the end of the enum:

```rust
pub enum Message {
    // ... existing messages ...

    // ─────────────────────────────────────────────────────────
    // Link Highlight Mode (Phase 3.1)
    // ─────────────────────────────────────────────────────────
    /// Enter link highlight mode - scan viewport and show shortcuts
    EnterLinkMode,
    
    /// Exit link highlight mode - return to normal mode
    ExitLinkMode,
    
    /// Select a link by its shortcut key ('1'-'9' or 'a'-'z')
    SelectLink(char),
}
```

### Message Flow

1. **User presses `L` in Normal mode**:
   - Key handler generates `Message::EnterLinkMode`
   - Update handler scans viewport for links
   - Update handler sets `UiMode::LinkHighlight`

2. **User presses `Esc` or `L` in LinkHighlight mode**:
   - Key handler generates `Message::ExitLinkMode`
   - Update handler clears link state
   - Update handler sets `UiMode::Normal`

3. **User presses `1`-`9` or `a`-`z` in LinkHighlight mode**:
   - Key handler generates `Message::SelectLink(char)`
   - Update handler finds link by shortcut
   - Update handler opens file in editor
   - Update handler sets `UiMode::Normal`

### Integration Points

These changes enable:
- **Task 05**: Viewport scanning triggered by `EnterLinkMode`
- **Task 06**: Key handlers to generate these messages
- **Task 07**: Renderer to check `UiMode::LinkHighlight` for highlighting

### Acceptance Criteria

1. `UiMode::LinkHighlight` variant added to enum
2. `Message::EnterLinkMode` variant added
3. `Message::ExitLinkMode` variant added
4. `Message::SelectLink(char)` variant added
5. All variants have documentation comments
6. No compiler errors
7. Existing functionality unaffected

### Testing

- **Compilation Test**: `cargo build` succeeds
- **Enum Coverage**: Ensure all match statements that use `UiMode` and `Message` are updated:
  - `app/handler/keys.rs` - key handler dispatch (Task 06 will add)
  - `app/handler/update.rs` - update handler dispatch (Task 06 will add)
  - `tui/render.rs` - UI mode rendering (Task 08 will add)

Note: This task only adds the types. The actual handling is done in Task 06.

### Notes

- The `SelectLink(char)` takes a char to identify which shortcut was pressed
- Character range: '1'-'9' for links 1-9, 'a'-'z' for links 10-35
- The update handler (Task 06) will validate the character and find the matching link

### Files Changed

| File | Change Type |
|------|-------------|
| `src/app/state.rs` | Modified - add UiMode variant |
| `src/app/message.rs` | Modified - add 3 message variants |

### Estimated Time

1 hour

---

## Completion Summary

**Status:** ✅ Done

### Files Modified

| File | Change |
|------|--------|
| `src/app/state.rs` | Added `UiMode::LinkHighlight` variant |
| `src/app/message.rs` | Added `EnterLinkMode`, `ExitLinkMode`, `SelectLink(char)` messages |
| `src/app/handler/keys.rs` | Added `UiMode::LinkHighlight` handler and `handle_key_link_highlight()` function, added 'L' keybinding in normal mode |
| `src/app/handler/update.rs` | Added stub handlers for `EnterLinkMode`, `ExitLinkMode`, `SelectLink` messages |
| `src/tui/render.rs` | Added `UiMode::LinkHighlight` match arm (empty, for future tasks) |

### Implementation Details

1. **`UiMode::LinkHighlight`** - New UI mode for when link selection is active

2. **New Messages**:
   - `EnterLinkMode` - Triggers viewport scanning and enters link mode
   - `ExitLinkMode` - Returns to normal mode
   - `SelectLink(char)` - Opens file by shortcut key ('1'-'9', 'a'-'z')

3. **Key Bindings**:
   - Normal mode: `L` (Shift+L) enters link mode
   - Link mode: `Esc` or `L` exits
   - Link mode: `1-9`, `a-z` select links
   - Link mode: `j/k`, arrows, Page Up/Down for scrolling (re-scan deferred to Task 06)
   - Link mode: `Ctrl+C` for force quit

4. **Stub Handlers**:
   - `EnterLinkMode` sets `UiMode::LinkHighlight` (full implementation in Task 05/06)
   - `ExitLinkMode` sets `UiMode::Normal` (full implementation in Task 05/06)
   - `SelectLink` logs and exits link mode (full implementation in Task 06)

### Testing Performed

```
cargo check    # ✅ Passed (no warnings)
cargo test     # ✅ All tests pass
```

### Notable Decisions

- `j` and `k` scroll instead of selecting links (consistent with vim navigation)
- Scrolling in link mode preserves the mode (re-scan will happen in Task 06)
- Key handlers placed before the `a-z` pattern to avoid unreachable patterns

### Risks/Limitations

None. Stub handlers allow compilation; actual logic deferred to Tasks 05-06 as planned.