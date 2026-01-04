## Task: Implement Search Mode

**Objective**: Implement the search mode UI including the search prompt widget, keyboard handlers for entering/exiting search mode, and input handling for search queries.

**Depends on**: 03-integrate-filter-search-state

**Estimated Time**: 4-5 hours

### Scope

- `src/app/handler/keys.rs`: Add search keyboard handlers
- `src/app/handler/update.rs`: Handle search mode messages
- `src/app/state.rs`: Add search UI mode
- `src/tui/widgets/search_input.rs`: **NEW** Search input widget
- `src/tui/widgets/mod.rs`: Export new widget
- `src/tui/render.rs`: Render search prompt when active

### Details

#### 1. Update `src/app/state.rs`

Add a search input mode variant to `UiMode`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    #[default]
    Normal,
    DeviceSelector,
    EmulatorSelector,
    ConfirmDialog,
    Loading,
    /// Search input mode - capturing text for log search
    SearchInput,
}
```

#### 2. Update `src/app/handler/keys.rs`

##### 2.1 Add search mode entry in normal mode

```rust
fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // ... existing handlers ...
        
        // ─────────────────────────────────────────────────────────
        // Log Search (Phase 1)
        // ─────────────────────────────────────────────────────────
        // '/' - Enter search mode (vim-style)
        (KeyCode::Char('/'), KeyModifiers::NONE) => Some(Message::StartSearch),
        
        // 'n' - Next search match (only when search has matches)
        (KeyCode::Char('n'), KeyModifiers::NONE) => Some(Message::NextSearchMatch),
        
        // 'N' - Previous search match
        (KeyCode::Char('N'), KeyModifiers::NONE) => Some(Message::PrevSearchMatch),
        (KeyCode::Char('N'), m) if m.contains(KeyModifiers::SHIFT) => {
            Some(Message::PrevSearchMatch)
        }
        
        // Escape clears search if active (in addition to quit behavior)
        // Note: This is handled specially - if search is active, clear it;
        // otherwise, proceed to quit
        
        // ... rest of handlers ...
    }
}
```

##### 2.2 Add search input mode handler

```rust
/// Handle key events in search input mode
fn handle_key_search_input(state: &AppState, key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Cancel search input (return to normal mode)
        (KeyCode::Esc, _) => Some(Message::CancelSearch),
        
        // Submit search and return to normal mode
        (KeyCode::Enter, _) => Some(Message::CancelSearch), // Keep query, exit input mode
        
        // Delete character
        (KeyCode::Backspace, _) => {
            if let Some(session) = state.session_manager.current_session() {
                let mut query = session.search_state.query.clone();
                query.pop();
                Some(Message::SearchInput { text: query })
            } else {
                None
            }
        }
        
        // Clear all input
        (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
            Some(Message::SearchInput { text: String::new() })
        }
        
        // Type character
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            if let Some(session) = state.session_manager.current_session() {
                let mut query = session.search_state.query.clone();
                query.push(c);
                Some(Message::SearchInput { text: query })
            } else {
                None
            }
        }
        
        // Force quit even in search mode
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        
        _ => None,
    }
}
```

##### 2.3 Update main handler dispatch

```rust
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::SearchInput => handle_key_search_input(state, key),
        UiMode::DeviceSelector => handle_key_device_selector(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
    }
}
```

#### 3. Update `src/app/handler/update.rs`

Add message handlers for search mode:

```rust
pub fn update(state: &mut AppState, msg: Message) -> UpdateResult {
    match msg {
        // ... existing handlers ...
        
        // ─────────────────────────────────────────────────────────
        // Log Search Messages (Phase 1)
        // ─────────────────────────────────────────────────────────
        Message::StartSearch => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.start_search();
            }
            state.ui_mode = UiMode::SearchInput;
            UpdateResult::none()
        }
        
        Message::CancelSearch => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.cancel_search();
            }
            state.ui_mode = UiMode::Normal;
            UpdateResult::none()
        }
        
        Message::ClearSearch => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.clear_search();
            }
            state.ui_mode = UiMode::Normal;
            UpdateResult::none()
        }
        
        Message::SearchInput { text } => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.set_search_query(&text);
                // Trigger search execution (will be implemented in Task 6)
                // For now, just update the query
            }
            UpdateResult::none()
        }
        
        Message::NextSearchMatch => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.search_state.next_match();
                // Scroll to show current match (will be implemented in Task 6)
            }
            UpdateResult::none()
        }
        
        Message::PrevSearchMatch => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.search_state.prev_match();
                // Scroll to show current match (will be implemented in Task 6)
            }
            UpdateResult::none()
        }
        
        // ... rest of handlers ...
    }
}
```

#### 4. Create `src/tui/widgets/search_input.rs`

```rust
//! Search input prompt widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::core::SearchState;

/// Search input prompt widget
pub struct SearchInput<'a> {
    /// The search state containing query and status
    search_state: &'a SearchState,
    /// Whether to show as a popup or inline
    inline: bool,
}

impl<'a> SearchInput<'a> {
    pub fn new(search_state: &'a SearchState) -> Self {
        Self {
            search_state,
            inline: false,
        }
    }
    
    /// Render as inline prompt (at bottom of log view)
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }
}

impl Widget for SearchInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.inline {
            self.render_inline(area, buf);
        } else {
            self.render_popup(area, buf);
        }
    }
}

impl SearchInput<'_> {
    /// Render as inline search bar
    fn render_inline(self, area: Rect, buf: &mut Buffer) {
        // Format: "/query█" or "/query [3/10 matches]"
        let mut spans = vec![
            Span::styled(
                "/",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &self.search_state.query,
                Style::default().fg(Color::White),
            ),
        ];
        
        // Add cursor
        if self.search_state.is_active {
            spans.push(Span::styled(
                "█",
                Style::default().fg(Color::Yellow),
            ));
        }
        
        // Add match count if query is not empty
        if !self.search_state.query.is_empty() {
            let status = self.search_state.display_status();
            if !status.is_empty() {
                spans.push(Span::raw(" "));
                
                // Color based on whether matches were found
                let status_style = if self.search_state.has_matches() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };
                spans.push(Span::styled(status, status_style));
            }
            
            // Show error if regex is invalid
            if let Some(ref error) = self.search_state.error {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("({})", error),
                    Style::default().fg(Color::Red),
                ));
            }
        }
        
        let line = Line::from(spans);
        Paragraph::new(line).render(area, buf);
    }
    
    /// Render as centered popup
    fn render_popup(self, area: Rect, buf: &mut Buffer) {
        // Calculate popup dimensions
        let width = 50.min(area.width.saturating_sub(4));
        let height = 3;
        
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        
        let popup_area = Rect::new(x, y, width, height);
        
        // Clear the area behind the popup
        Clear.render(popup_area, buf);
        
        // Draw popup with border
        let block = Block::default()
            .title(" Search ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);
        
        // Render search content
        let mut spans = vec![
            Span::styled(
                "/",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &self.search_state.query,
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "█",
                Style::default().fg(Color::Yellow),
            ),
        ];
        
        // Add status on same line if room
        let status = self.search_state.display_status();
        if !status.is_empty() && inner.width > 30 {
            spans.push(Span::raw("  "));
            let status_style = if self.search_state.has_matches() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            spans.push(Span::styled(status, status_style));
        }
        
        let line = Line::from(spans);
        Paragraph::new(line).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_search_state(query: &str, active: bool) -> SearchState {
        let mut state = SearchState::default();
        state.set_query(query);
        state.is_active = active;
        state
    }

    #[test]
    fn test_search_input_new() {
        let state = make_search_state("test", true);
        let widget = SearchInput::new(&state);
        assert!(!widget.inline);
    }

    #[test]
    fn test_search_input_inline() {
        let state = make_search_state("test", true);
        let widget = SearchInput::new(&state).inline();
        assert!(widget.inline);
    }
}
```

#### 5. Update `src/tui/widgets/mod.rs`

Add the new widget to exports:

```rust
mod search_input;

pub use search_input::SearchInput;
```

#### 6. Update `src/tui/render.rs`

Add search prompt rendering when in search mode:

```rust
// In the main render function, after rendering log view:

// Render search input if in search mode
if state.ui_mode == UiMode::SearchInput {
    if let Some(session) = state.session_manager.current_session() {
        // Calculate position for inline search (bottom of log area)
        let search_area = Rect::new(
            log_area.x + 1,
            log_area.y + log_area.height.saturating_sub(2),
            log_area.width.saturating_sub(2),
            1,
        );
        
        // Clear the line and render search input
        Clear.render(search_area, buf);
        SearchInput::new(&session.search_state)
            .inline()
            .render(search_area, buf);
    }
}

// Also show search status in normal mode if search has results
if state.ui_mode == UiMode::Normal {
    if let Some(session) = state.session_manager.current_session() {
        if !session.search_state.query.is_empty() && session.search_state.has_matches() {
            // Show mini search status in corner or status bar
            // (Implementation depends on layout)
        }
    }
}
```

### Acceptance Criteria

1. Pressing `/` in normal mode enters search input mode
2. Search prompt appears at bottom of log view with `/` prefix
3. Typing characters appends to search query
4. Backspace removes last character
5. Ctrl+u clears the entire query
6. Enter exits search input mode but keeps query
7. Escape exits search input mode and keeps query
8. Cursor indicator (`█`) shows when in input mode
9. Invalid regex shows error message in red
10. Match count displays when query has results: `[3/10 matches]`
11. "No matches" displays in red when query finds nothing
12. `n` key navigates to next match (even in normal mode after search)
13. `N` (Shift+n) navigates to previous match
14. Ctrl+C still works to quit even in search mode
15. Search state persists per session

### Testing

Add tests to `src/app/handler/tests.rs`:

```rust
#[test]
fn test_start_search_changes_ui_mode() {
    let mut state = create_test_state_with_session();
    assert_eq!(state.ui_mode, UiMode::Normal);
    
    update(&mut state, Message::StartSearch);
    assert_eq!(state.ui_mode, UiMode::SearchInput);
}

#[test]
fn test_cancel_search_returns_to_normal() {
    let mut state = create_test_state_with_session();
    update(&mut state, Message::StartSearch);
    update(&mut state, Message::CancelSearch);
    
    assert_eq!(state.ui_mode, UiMode::Normal);
}

#[test]
fn test_search_input_updates_query() {
    let mut state = create_test_state_with_session();
    update(&mut state, Message::StartSearch);
    update(&mut state, Message::SearchInput { text: "error".to_string() });
    
    let query = &state.session_manager.current_session().unwrap()
        .search_state.query;
    assert_eq!(query, "error");
}

#[test]
fn test_search_key_handler_normal_mode() {
    let state = create_test_state_with_session();
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    
    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::StartSearch)));
}

#[test]
fn test_search_input_mode_escape() {
    let mut state = create_test_state_with_session();
    state.ui_mode = UiMode::SearchInput;
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    
    let msg = handle_key(&state, key);
    assert!(matches!(msg, Some(Message::CancelSearch)));
}
```

### Notes

- The search is vim-style: `/` enters search, type query, Enter/Escape to exit
- Search matches are computed in Task 6; this task only handles UI and state
- Consider adding search history (up/down arrows) as a future enhancement
- The inline search prompt overlays the last line of the log view
- Search query persists even when exiting search mode, allowing `n`/`N` navigation
- Consider adding `?` for reverse search as a future enhancement
- Regex errors are shown inline to help users understand invalid patterns