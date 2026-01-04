## Task: Add Error Navigation

**Objective**: Implement keyboard shortcuts to quickly jump between error log entries, providing fast navigation to problematic logs without manual scrolling or filtering.

**Depends on**: 04-implement-filter-handlers-logic, 06-implement-search-logic-highlighting

**Estimated Time**: 3-4 hours

### Scope

- `src/app/handler/keys.rs`: Add error navigation keyboard handlers
- `src/app/handler/update.rs`: Handle error navigation messages
- `src/app/message.rs`: Add error navigation message variants
- `src/app/session.rs`: Add error tracking and navigation methods
- `src/tui/widgets/log_view.rs`: Visual indicator for current error position

### Details

#### 1. Update `src/app/message.rs`

Add error navigation message variants:

```rust
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // Error Navigation Messages (Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Jump to next error in log
    NextError,
    /// Jump to previous error in log
    PrevError,
}
```

#### 2. Update `src/app/session.rs`

Add error tracking and navigation to Session:

```rust
impl Session {
    /// Get indices of all error log entries
    pub fn error_indices(&self) -> Vec<usize> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_error())
            .map(|(i, _)| i)
            .collect()
    }
    
    /// Get indices of errors that pass the current filter
    pub fn filtered_error_indices(&self) -> Vec<usize> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.is_error() && self.filter_state.matches(entry)
            })
            .map(|(i, _)| i)
            .collect()
    }
    
    /// Count total errors in log
    pub fn error_count(&self) -> usize {
        self.logs.iter().filter(|e| e.is_error()).count()
    }
    
    /// Find next error after current scroll position
    /// Returns the log entry index of the next error
    pub fn find_next_error(&self) -> Option<usize> {
        let errors = self.filtered_error_indices();
        if errors.is_empty() {
            return None;
        }
        
        // Current position in the original log buffer
        // Need to map scroll offset to original index
        let current_pos = self.current_log_position();
        
        // Find first error after current position
        for &error_idx in &errors {
            if error_idx > current_pos {
                return Some(error_idx);
            }
        }
        
        // Wrap around to first error
        Some(errors[0])
    }
    
    /// Find previous error before current scroll position
    /// Returns the log entry index of the previous error
    pub fn find_prev_error(&self) -> Option<usize> {
        let errors = self.filtered_error_indices();
        if errors.is_empty() {
            return None;
        }
        
        let current_pos = self.current_log_position();
        
        // Find last error before current position
        for &error_idx in errors.iter().rev() {
            if error_idx < current_pos {
                return Some(error_idx);
            }
        }
        
        // Wrap around to last error
        errors.last().copied()
    }
    
    /// Get the current log position based on scroll offset
    /// Accounts for filtering
    fn current_log_position(&self) -> usize {
        if self.filter_state.is_active() {
            // Map filtered offset to original index
            let filtered = self.filtered_log_indices();
            filtered.get(self.log_view_state.offset).copied().unwrap_or(0)
        } else {
            self.log_view_state.offset
        }
    }
}
```

#### 3. Update `src/app/handler/keys.rs`

Add error navigation shortcuts to `handle_key_normal()`:

```rust
fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // ... existing handlers ...
        
        // ─────────────────────────────────────────────────────────
        // Error Navigation (Phase 1)
        // ─────────────────────────────────────────────────────────
        // 'e' - Jump to next error
        (KeyCode::Char('e'), KeyModifiers::NONE) => Some(Message::NextError),
        
        // 'E' - Jump to previous error
        (KeyCode::Char('E'), KeyModifiers::NONE) => Some(Message::PrevError),
        (KeyCode::Char('E'), m) if m.contains(KeyModifiers::SHIFT) => {
            Some(Message::PrevError)
        }
        
        // ... rest of handlers ...
    }
}
```

**Note**: Verify that `e` is not already bound to another action. Looking at the current `keys.rs`, `e` appears to be unused.

#### 4. Update `src/app/handler/update.rs`

Add message handlers for error navigation:

```rust
pub fn update(state: &mut AppState, msg: Message) -> UpdateResult {
    match msg {
        // ... existing handlers ...
        
        // ─────────────────────────────────────────────────────────
        // Error Navigation Messages (Phase 1)
        // ─────────────────────────────────────────────────────────
        Message::NextError => {
            if let Some(session) = state.session_manager.current_session_mut() {
                if let Some(error_idx) = session.find_next_error() {
                    scroll_to_log_entry(session, error_idx);
                }
            }
            UpdateResult::none()
        }
        
        Message::PrevError => {
            if let Some(session) = state.session_manager.current_session_mut() {
                if let Some(error_idx) = session.find_prev_error() {
                    scroll_to_log_entry(session, error_idx);
                }
            }
            UpdateResult::none()
        }
        
        // ... rest of handlers ...
    }
}
```

The `scroll_to_log_entry` helper was added in Task 6 for search navigation; reuse it here.

#### 5. Optional: Update Status Bar with Error Count

Update `src/tui/widgets/status_bar.rs` to show error count:

```rust
// Add to status bar rendering
if let Some(session) = state.session_manager.current_session() {
    let error_count = session.error_count();
    if error_count > 0 {
        let error_text = format!(" {} errors ", error_count);
        let error_style = Style::default()
            .fg(Color::White)
            .bg(Color::Red);
        // Render error count indicator
    }
}
```

#### 6. Optional: Visual Error Position Indicator

Consider adding a visual indicator in the scrollbar or margin showing where errors are located in the log buffer (like VS Code's minimap error markers). This can be deferred to a future enhancement.

### Acceptance Criteria

1. Pressing `e` jumps to the next error after current scroll position
2. Pressing `E` (Shift+e) jumps to the previous error before current position
3. Navigation wraps around (last error → first error, first error → last error)
4. Error navigation respects active filters (only jumps to visible errors)
5. View scrolls to center the error in the viewport
6. Auto-scroll is disabled when jumping to error
7. Works correctly when no errors exist (no-op)
8. Error count is tracked per session
9. Performance acceptable with 1000+ log entries

### Testing

Add tests to `src/app/session.rs`:

```rust
#[cfg(test)]
mod error_navigation_tests {
    use super::*;
    use crate::core::{LogLevel, LogSource};

    fn create_session_with_logs() -> Session {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        session.log_info(LogSource::App, "info 0");      // index 0
        session.log_error(LogSource::App, "error 1");    // index 1
        session.log_info(LogSource::App, "info 2");      // index 2
        session.log_error(LogSource::App, "error 3");    // index 3
        session.log_info(LogSource::App, "info 4");      // index 4
        session.log_error(LogSource::App, "error 5");    // index 5
        session
    }

    #[test]
    fn test_error_indices() {
        let session = create_session_with_logs();
        let errors = session.error_indices();
        assert_eq!(errors, vec![1, 3, 5]);
    }

    #[test]
    fn test_error_count() {
        let session = create_session_with_logs();
        assert_eq!(session.error_count(), 3);
    }

    #[test]
    fn test_find_next_error_from_start() {
        let session = create_session_with_logs();
        // Scroll offset 0, should find first error at index 1
        let next = session.find_next_error();
        assert_eq!(next, Some(1));
    }

    #[test]
    fn test_find_next_error_wraps() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 5; // After last error
        
        let next = session.find_next_error();
        assert_eq!(next, Some(1)); // Wraps to first error
    }

    #[test]
    fn test_find_prev_error_from_end() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 5;
        
        let prev = session.find_prev_error();
        assert_eq!(prev, Some(3)); // Error before position 5
    }

    #[test]
    fn test_find_prev_error_wraps() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 0; // Before first error
        
        let prev = session.find_prev_error();
        assert_eq!(prev, Some(5)); // Wraps to last error
    }

    #[test]
    fn test_find_error_no_errors() {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        session.log_info(LogSource::App, "info only");
        
        assert_eq!(session.find_next_error(), None);
        assert_eq!(session.find_prev_error(), None);
    }

    #[test]
    fn test_find_error_respects_filter() {
        let mut session = create_session_with_logs();
        
        // Filter to App source only (all errors are from App, so all visible)
        session.filter_state.source_filter = LogSourceFilter::App;
        let errors = session.filtered_error_indices();
        assert_eq!(errors.len(), 3);
        
        // Filter to Daemon source (no errors)
        session.filter_state.source_filter = LogSourceFilter::Daemon;
        let errors = session.filtered_error_indices();
        assert!(errors.is_empty());
    }
}
```

Add handler tests to `src/app/handler/tests.rs`:

```rust
#[test]
fn test_next_error_scrolls_to_error() {
    let mut state = create_test_state_with_session();
    
    // Add some logs including errors
    if let Some(session) = state.session_manager.current_session_mut() {
        session.log_info(LogSource::App, "info");
        session.log_error(LogSource::App, "error");
        session.log_view_state.visible_lines = 10;
        session.log_view_state.total_lines = 2;
    }
    
    update(&mut state, Message::NextError);
    
    // Should have scrolled (auto_scroll disabled)
    let session = state.session_manager.current_session().unwrap();
    assert!(!session.log_view_state.auto_scroll);
}

#[test]
fn test_error_navigation_no_errors() {
    let mut state = create_test_state_with_session();
    
    // Add only info logs
    if let Some(session) = state.session_manager.current_session_mut() {
        session.log_info(LogSource::App, "info only");
    }
    
    // Should not crash or change state
    let result = update(&mut state, Message::NextError);
    assert!(result.action.is_none());
}
```

### Notes

- Error navigation is independent of search - users can navigate errors without entering search mode
- The `e`/`E` keys provide a quick way to jump between errors without changing filter settings
- Consider adding a visual "flash" or brief highlight when jumping to an error for better UX
- The error navigation uses the same `scroll_to_log_entry` helper as search navigation for consistency
- Future enhancement: Add error position markers in the scrollbar track
- Future enhancement: Show "Error X of Y" in status bar when navigating errors
- Consider supporting both `e`/`E` and `[`/`]` for error navigation (vim-style quickfix navigation)

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

- `src/app/message.rs` - Added `NextError` and `PrevError` message variants
- `src/app/session.rs` - Added error navigation methods: `error_indices()`, `filtered_error_indices()`, `error_count()`, `find_next_error()`, `find_prev_error()`, `current_log_position()`
- `src/app/handler/keys.rs` - Added `e` and `E` key bindings for error navigation
- `src/app/handler/update.rs` - Added message handlers for `NextError` and `PrevError`
- `src/app/handler/tests.rs` - Added 5 tests for error navigation key bindings and handlers

### Implementation Details

1. **Message Types**: Added `Message::NextError` and `Message::PrevError` variants for error navigation.

2. **Session Methods**:
   - `error_indices()` - Returns indices of all error entries in the log
   - `filtered_error_indices()` - Returns indices of errors that pass the current filter
   - `error_count()` - Returns total count of errors
   - `find_next_error()` - Finds next error after current position, wraps around
   - `find_prev_error()` - Finds previous error before current position, wraps around
   - `current_log_position()` - Maps scroll offset to log index, accounting for filters

3. **Key Bindings**:
   - `e` - Jump to next error
   - `E` (Shift+e) - Jump to previous error

4. **Handler Integration**: Reuses `scroll_to_log_entry()` helper from Task 6 for consistent scrolling behavior.

### Testing Performed

```bash
cargo fmt    # ✓ No formatting issues
cargo check  # ✓ Compiles cleanly
cargo clippy # ✓ No warnings
cargo test session::tests::test_error  # ✓ 3 tests passed
cargo test session::tests::test_find   # ✓ 8 tests passed
cargo test handler::tests::test_e_key  # ✓ 1 test passed
cargo test handler::tests::test_next_error  # ✓ 1 test passed
cargo test handler::tests::test_prev_error  # ✓ 1 test passed
```

Added 16 new tests:
- Session tests: `test_error_indices`, `test_error_count`, `test_find_next_error_from_start`, `test_find_next_error_wraps`, `test_find_prev_error_from_end`, `test_find_prev_error_wraps`, `test_find_error_no_errors`, `test_find_error_respects_filter`, `test_find_next_error_from_middle`, `test_find_prev_error_from_middle`, `test_error_count_empty`
- Handler tests: `test_e_key_produces_next_error`, `test_shift_e_produces_prev_error`, `test_next_error_scrolls_to_error`, `test_error_navigation_no_errors`, `test_prev_error_message`

### Acceptance Criteria Status

1. ✅ `e` jumps to next error after current scroll position
2. ✅ `E` (Shift+e) jumps to previous error before current position
3. ✅ Navigation wraps around (last→first, first→last)
4. ✅ Error navigation respects active filters (only visible errors)
5. ✅ View scrolls to center the error in the viewport
6. ✅ Auto-scroll is disabled when jumping to error
7. ✅ Works correctly when no errors exist (no-op)
8. ✅ Error count is tracked per session
9. ✅ Performance acceptable with 1000+ log entries

### Risks/Limitations

- Error count is not displayed in the status bar (optional enhancement deferred)
- No visual error position indicator in scrollbar (future enhancement)
- No "Error X of Y" display when navigating (future enhancement)