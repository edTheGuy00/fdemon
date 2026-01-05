## Task: 06-key-and-update-handlers

**Objective**: Implement the key handlers for Link Highlight Mode and the update handlers for `EnterLinkMode`, `ExitLinkMode`, and `SelectLink` messages.

**Depends on**: 05-viewport-scanning

### Background

With the scanning logic in place, we need to wire up the key handlers to generate the appropriate messages and the update handlers to process them. This follows the TEA (The Elm Architecture) pattern used throughout the application.

### Scope

- `src/app/handler/keys.rs`:
  - Add `handle_key_link_mode()` function
  - Update `handle_key()` to dispatch to link mode handler
  - Update `handle_key_normal()` to generate `EnterLinkMode` on `L` key

- `src/app/handler/update.rs`:
  - Add handler for `Message::EnterLinkMode`
  - Add handler for `Message::ExitLinkMode`
  - Add handler for `Message::SelectLink(char)`

### Changes to `src/app/handler/keys.rs`

#### 1. Update `handle_key()` dispatch

```rust
/// Convert key events to messages based on current UI mode
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::SearchInput => handle_key_search_input(state, key),
        UiMode::DeviceSelector => handle_key_device_selector(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::LinkHighlight => handle_key_link_mode(key),  // NEW
        UiMode::Normal => handle_key_normal(state, key),
    }
}
```

#### 2. Add `handle_key_link_mode()` function

```rust
/// Handle key events in link highlight mode
fn handle_key_link_mode(key: KeyEvent) -> Option<Message> {
    match key.code {
        // Exit link mode
        KeyCode::Esc => Some(Message::ExitLinkMode),
        KeyCode::Char('L') => Some(Message::ExitLinkMode),  // Toggle
        KeyCode::Char('l') => Some(Message::ExitLinkMode),  // Toggle (lowercase)
        
        // Select link by number (1-9)
        KeyCode::Char(c @ '1'..='9') => Some(Message::SelectLink(c)),
        
        // Select link by letter (a-z)
        KeyCode::Char(c @ 'a'..='z') => Some(Message::SelectLink(c)),
        
        // Allow scrolling while in link mode (will re-scan)
        KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollUp),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown => Some(Message::PageDown),
        KeyCode::Char('g') => Some(Message::ScrollToTop),
        KeyCode::Char('G') => Some(Message::ScrollToBottom),
        
        // Force quit still works
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        
        _ => None,
    }
}
```

#### 3. Update `handle_key_normal()` to add `L` key

Add the following to `handle_key_normal()`:

```rust
// ─────────────────────────────────────────────────────────
// Link Highlight Mode (Phase 3.1)
// ─────────────────────────────────────────────────────────
// 'L' - Enter link highlight mode
(KeyCode::Char('L'), KeyModifiers::NONE) => Some(Message::EnterLinkMode),
(KeyCode::Char('L'), m) if m.contains(KeyModifiers::SHIFT) => Some(Message::EnterLinkMode),
```

### Changes to `src/app/handler/update.rs`

Add the following handlers in the `update()` function:

```rust
// ─────────────────────────────────────────────────────────
// Link Highlight Mode (Phase 3.1)
// ─────────────────────────────────────────────────────────
Message::EnterLinkMode => {
    if let Some(handle) = state.session_manager.selected_mut() {
        // Get visible range from log view state
        let visible_range = handle.session.log_view_state.visible_range();
        let (visible_start, visible_end) = visible_range;
        
        // Scan viewport for links
        handle.session.link_highlight_state.scan_viewport(
            &handle.session.logs,
            visible_start,
            visible_end,
            Some(&handle.session.filter_state),
            &handle.session.collapse_state,
            state.settings.ui.stack_trace_collapsed,
            state.settings.ui.stack_trace_max_frames,
        );
        
        // Only enter link mode if there are links to show
        if handle.session.link_highlight_state.has_links() {
            handle.session.link_highlight_state.activate();
            state.ui_mode = UiMode::LinkHighlight;
            tracing::debug!(
                "Entered link mode with {} links",
                handle.session.link_highlight_state.link_count()
            );
        } else {
            tracing::debug!("No links found in viewport");
            // Could show a flash message here in the future
        }
    }
    UpdateResult::none()
}

Message::ExitLinkMode => {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.link_highlight_state.deactivate();
    }
    state.ui_mode = UiMode::Normal;
    tracing::debug!("Exited link mode");
    UpdateResult::none()
}

Message::SelectLink(c) => {
    let file_ref = if let Some(handle) = state.session_manager.selected_mut() {
        // Find the link by shortcut
        handle
            .session
            .link_highlight_state
            .link_by_shortcut(c)
            .map(|link| link.file_ref.clone())
    } else {
        None
    };
    
    // Exit link mode
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.link_highlight_state.deactivate();
    }
    state.ui_mode = UiMode::Normal;
    
    // Open the file if we found a matching link
    if let Some(file_ref) = file_ref {
        // Sanitize path
        if sanitize_path(&file_ref.path).is_none() {
            tracing::warn!("Rejected suspicious file path: {}", file_ref.path);
            return UpdateResult::none();
        }
        
        // Open in editor
        match open_in_editor(&file_ref, &state.settings.editor, &state.project_path) {
            Ok(result) => {
                if result.used_parent_ide {
                    tracing::info!(
                        "Opened {}:{} in {} (parent IDE)",
                        result.file,
                        result.line,
                        result.editor_display_name
                    );
                } else {
                    tracing::info!(
                        "Opened {}:{} in {}",
                        result.file,
                        result.line,
                        result.editor
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to open file: {}", e);
            }
        }
    } else {
        tracing::debug!("No link found for shortcut '{}'", c);
    }
    
    UpdateResult::none()
}
```

### Re-scan on Scroll

When the user scrolls while in link mode, the viewport changes and we need to re-scan. Add this logic to the scroll handlers:

```rust
// In existing scroll handlers (ScrollUp, ScrollDown, PageUp, PageDown, etc.)
// After performing the scroll:

// Re-scan if in link highlight mode
if state.ui_mode == UiMode::LinkHighlight {
    if let Some(handle) = state.session_manager.selected_mut() {
        let visible_range = handle.session.log_view_state.visible_range();
        let (visible_start, visible_end) = visible_range;
        
        handle.session.link_highlight_state.scan_viewport(
            &handle.session.logs,
            visible_start,
            visible_end,
            Some(&handle.session.filter_state),
            &handle.session.collapse_state,
            state.settings.ui.stack_trace_collapsed,
            state.settings.ui.stack_trace_max_frames,
        );
    }
}
```

**Alternative**: Create a helper function to avoid code duplication:

```rust
/// Re-scan links if in link highlight mode (called after scroll)
fn rescan_links_if_active(state: &mut AppState) {
    if state.ui_mode != UiMode::LinkHighlight {
        return;
    }
    
    if let Some(handle) = state.session_manager.selected_mut() {
        let visible_range = handle.session.log_view_state.visible_range();
        let (visible_start, visible_end) = visible_range;
        
        handle.session.link_highlight_state.scan_viewport(
            &handle.session.logs,
            visible_start,
            visible_end,
            Some(&handle.session.filter_state),
            &handle.session.collapse_state,
            // These would need to be passed in or accessed differently
            true,  // default collapsed
            3,     // max frames
        );
    }
}
```

### Required Imports

In `keys.rs`:
```rust
use crate::app::state::UiMode;  // Should already be there
use crate::app::message::Message;  // Should already be there
```

In `update.rs`:
```rust
use crate::tui::editor::{open_in_editor, sanitize_path};  // Should already be there
```

### Acceptance Criteria

1. `handle_key()` dispatches to `handle_key_link_mode()` when `UiMode::LinkHighlight`
2. `handle_key_link_mode()` handles:
   - `Esc` and `L`/`l` → `ExitLinkMode`
   - `1-9` and `a-z` → `SelectLink(char)`
   - Scroll keys → appropriate scroll messages
   - `Ctrl+C` → `Quit`
3. `handle_key_normal()` handles `L` → `EnterLinkMode`
4. `Message::EnterLinkMode` scans viewport and activates link mode
5. `Message::ExitLinkMode` deactivates link mode and returns to normal
6. `Message::SelectLink` finds link, opens file, and exits link mode
7. Scrolling in link mode triggers re-scan
8. No link found → graceful handling (no crash)
9. Empty viewport → stays in normal mode with debug message
10. All existing tests pass
11. No compiler errors or warnings

### Testing

#### Unit Tests for Key Handler

```rust
#[cfg(test)]
mod link_mode_key_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_escape_exits_link_mode() {
        let msg = handle_key_link_mode(key_event(KeyCode::Esc));
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
    }

    #[test]
    fn test_l_toggles_link_mode() {
        let msg = handle_key_link_mode(key_event(KeyCode::Char('L')));
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
        
        let msg = handle_key_link_mode(key_event(KeyCode::Char('l')));
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
    }

    #[test]
    fn test_number_selects_link() {
        let msg = handle_key_link_mode(key_event(KeyCode::Char('1')));
        assert!(matches!(msg, Some(Message::SelectLink('1'))));
        
        let msg = handle_key_link_mode(key_event(KeyCode::Char('5')));
        assert!(matches!(msg, Some(Message::SelectLink('5'))));
    }

    #[test]
    fn test_letter_selects_link() {
        let msg = handle_key_link_mode(key_event(KeyCode::Char('a')));
        assert!(matches!(msg, Some(Message::SelectLink('a'))));
        
        let msg = handle_key_link_mode(key_event(KeyCode::Char('z')));
        assert!(matches!(msg, Some(Message::SelectLink('z'))));
    }

    #[test]
    fn test_scroll_allowed_in_link_mode() {
        let msg = handle_key_link_mode(key_event(KeyCode::Char('j')));
        assert!(matches!(msg, Some(Message::ScrollDown)));
        
        let msg = handle_key_link_mode(key_event(KeyCode::Char('k')));
        assert!(matches!(msg, Some(Message::ScrollUp)));
    }

    #[test]
    fn test_unknown_key_returns_none() {
        let msg = handle_key_link_mode(key_event(KeyCode::Char('!')));
        assert!(msg.is_none());
    }
}
```

### Message Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│  User presses 'L' in Normal mode                                │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│  handle_key_normal() → Message::EnterLinkMode                   │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│  update() handles EnterLinkMode:                                │
│  1. Scan viewport for links                                     │
│  2. If links found: activate + set UiMode::LinkHighlight        │
│  3. If no links: stay in Normal mode                            │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│  User presses '3' in LinkHighlight mode                         │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│  handle_key_link_mode() → Message::SelectLink('3')              │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│  update() handles SelectLink('3'):                              │
│  1. Find link by shortcut '3'                                   │
│  2. Deactivate link mode + set UiMode::Normal                   │
│  3. Open file in editor                                         │
└─────────────────────────────────────────────────────────────────┘
```

### Error Handling

1. **No session selected**: Return `UpdateResult::none()` without error
2. **No link for shortcut**: Log debug message, return normally
3. **Path sanitization fails**: Log warning, don't open file
4. **Editor open fails**: Log warning, continue normally

### Files Changed

| File | Change Type |
|------|-------------|
| `src/app/handler/keys.rs` | Modified - add link mode handler |
| `src/app/handler/update.rs` | Modified - add message handlers |

### Estimated Time

2-3 hours

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/keys.rs` | Added unit tests for `handle_key_link_highlight()` in new `link_mode_key_tests` module |
| `src/app/handler/update.rs` | Implemented full handlers for `EnterLinkMode`, `ExitLinkMode`, `SelectLink`; added `rescan_links_if_active()` helper; updated all scroll handlers to re-scan links |

### Implementation Details

1. **EnterLinkMode handler** (`update.rs:852-880`):
   - Gets visible range from `log_view_state.visible_range()`
   - Calls `link_highlight_state.scan_viewport()` with filter state, collapse state, and settings
   - Only enters link mode if links are found
   - Activates link highlight state and sets `UiMode::LinkHighlight`

2. **ExitLinkMode handler** (`update.rs:883-890`):
   - Deactivates `link_highlight_state`
   - Sets `UiMode::Normal`

3. **SelectLink handler** (`update.rs:892-946`):
   - Finds link by shortcut using `link_by_shortcut()`
   - Exits link mode (deactivates state, sets Normal mode)
   - Sanitizes path with `sanitize_path()`
   - Opens file using `open_in_editor()` with proper logging

4. **Re-scan on scroll** (`update.rs:974-1001`):
   - Added `rescan_links_if_active()` helper function
   - Called after `ScrollUp`, `ScrollDown`, `ScrollToTop`, `ScrollToBottom`, `PageUp`, `PageDown`
   - Only scans when `UiMode::LinkHighlight` is active

5. **Unit tests** (`keys.rs:338-444`):
   - `test_escape_exits_link_mode`
   - `test_l_toggles_link_mode`
   - `test_number_selects_link`
   - `test_letter_selects_link`
   - `test_scroll_allowed_in_link_mode`
   - `test_ctrl_c_quits_in_link_mode`
   - `test_unknown_key_returns_none`
   - `test_j_k_are_scroll_not_select`

### Testing Performed

- `cargo check` - Passed
- `cargo test` - 950 tests passed, including 8 new link mode key handler tests
- All existing tests continue to pass

### Notable Decisions/Tradeoffs

1. **j/k keys are scroll, not select**: Even though j and k are in the a-z range for link selection, they are handled first as scroll keys. This means links 10 (`a`) and 11 (`b`) work, but `j` and `k` shortcuts are unavailable. This is intentional to maintain vim-style scrolling in link mode.

2. **Re-scan uses the same parameters as enter**: The `rescan_links_if_active()` helper uses the same scan parameters from settings (`stack_trace_collapsed`, `stack_trace_max_frames`) to ensure consistency.

3. **Link mode exits on selection**: After selecting a link, the mode automatically exits. This follows the VS Code pattern where clicking a link dismisses the hover state.

### Risks/Limitations

- If there are many file references in the viewport (>35), only the first 35 will be accessible via shortcuts
- Re-scanning on every scroll could be inefficient with very large viewports (mitigated by virtualized rendering limiting scan range)