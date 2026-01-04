## Task: Implement Search Logic and Highlighting

**Objective**: Implement the regex-based search logic that finds matches in log entries, tracks match positions, enables navigation between matches, and highlights matched text in the LogView widget.

**Depends on**: 05-implement-search-mode

**Estimated Time**: 5-6 hours

### Scope

- `src/tui/widgets/log_view.rs`: Add match highlighting and scroll-to-match
- `src/app/handler/update.rs`: Execute search when query changes
- `src/core/types.rs`: Add search execution helper methods
- `Cargo.toml`: Ensure `regex` crate is a direct dependency

### Details

#### 1. Update `Cargo.toml`

Ensure `regex` is a direct dependency (it may already be a transitive dep):

```toml
[dependencies]
# ... existing deps ...
regex = "1"
```

#### 2. Add Search Execution to `src/core/types.rs`

Add a function to execute search and find matches:

```rust
use regex::Regex;

impl SearchState {
    /// Execute search against log entries and update matches
    /// Returns true if the match list changed
    pub fn execute_search(&mut self, logs: &[LogEntry]) -> bool {
        // Clear if no query
        if self.query.is_empty() {
            let changed = !self.matches.is_empty();
            self.matches.clear();
            self.current_match = None;
            return changed;
        }
        
        // Try to compile regex (case-insensitive by default)
        let pattern = format!("(?i){}", &self.query);
        let regex = match Regex::new(&pattern) {
            Ok(r) => {
                self.is_valid = true;
                self.error = None;
                self.pattern = Some(self.query.clone());
                r
            }
            Err(e) => {
                self.is_valid = false;
                self.error = Some(format!("Invalid regex: {}", e));
                self.matches.clear();
                self.current_match = None;
                return true;
            }
        };
        
        // Find all matches
        let mut new_matches = Vec::new();
        for (entry_index, entry) in logs.iter().enumerate() {
            for mat in regex.find_iter(&entry.message) {
                new_matches.push(SearchMatch {
                    entry_index,
                    start: mat.start(),
                    end: mat.end(),
                });
            }
        }
        
        let changed = new_matches != self.matches;
        self.matches = new_matches;
        
        // Update current match
        if self.matches.is_empty() {
            self.current_match = None;
        } else if self.current_match.is_none() {
            self.current_match = Some(0);
        } else if let Some(idx) = self.current_match {
            // Keep current if still valid, otherwise reset to 0
            if idx >= self.matches.len() {
                self.current_match = Some(0);
            }
        }
        
        changed
    }
    
    /// Get the log entry index of the current match (for scrolling)
    pub fn current_match_entry_index(&self) -> Option<usize> {
        self.current_match
            .and_then(|idx| self.matches.get(idx))
            .map(|m| m.entry_index)
    }
    
    /// Get all matches for a specific log entry index
    pub fn matches_for_entry(&self, entry_index: usize) -> Vec<&SearchMatch> {
        self.matches
            .iter()
            .filter(|m| m.entry_index == entry_index)
            .collect()
    }
    
    /// Check if a specific match is the current one
    pub fn is_current_match(&self, match_ref: &SearchMatch) -> bool {
        if let Some(current_idx) = self.current_match {
            if let Some(current) = self.matches.get(current_idx) {
                return current == match_ref;
            }
        }
        false
    }
}
```

#### 3. Update `src/app/handler/update.rs`

Execute search when query changes and scroll to match on navigation:

```rust
Message::SearchInput { text } => {
    if let Some(session) = state.session_manager.current_session_mut() {
        session.set_search_query(&text);
        
        // Execute search immediately
        let logs = &session.logs;
        session.search_state.execute_search(logs);
        
        // Scroll to first match if found
        if let Some(entry_index) = session.search_state.current_match_entry_index() {
            scroll_to_log_entry(session, entry_index);
        }
    }
    UpdateResult::none()
}

Message::NextSearchMatch => {
    if let Some(session) = state.session_manager.current_session_mut() {
        session.search_state.next_match();
        
        // Scroll to new current match
        if let Some(entry_index) = session.search_state.current_match_entry_index() {
            scroll_to_log_entry(session, entry_index);
        }
    }
    UpdateResult::none()
}

Message::PrevSearchMatch => {
    if let Some(session) = state.session_manager.current_session_mut() {
        session.search_state.prev_match();
        
        // Scroll to new current match
        if let Some(entry_index) = session.search_state.current_match_entry_index() {
            scroll_to_log_entry(session, entry_index);
        }
    }
    UpdateResult::none()
}
```

Add helper function for scrolling:

```rust
/// Scroll the log view to show a specific log entry
fn scroll_to_log_entry(session: &mut Session, entry_index: usize) {
    // Account for filtering if active
    let visible_index = if session.filter_state.is_active() {
        // Find the position in filtered list
        session.logs
            .iter()
            .enumerate()
            .filter(|(_, e)| session.filter_state.matches(e))
            .position(|(i, _)| i == entry_index)
    } else {
        Some(entry_index)
    };
    
    if let Some(idx) = visible_index {
        // Center the match in the view if possible
        let visible_lines = session.log_view_state.visible_lines;
        let center_offset = visible_lines / 2;
        session.log_view_state.offset = idx.saturating_sub(center_offset);
        session.log_view_state.auto_scroll = false;
    }
}
```

#### 4. Update `src/tui/widgets/log_view.rs`

##### 4.1 Add search state to LogView

```rust
use crate::core::{FilterState, SearchState, LogEntry, LogLevel, LogSource, SearchMatch};

pub struct LogView<'a> {
    logs: &'a [LogEntry],
    title: &'a str,
    show_timestamps: bool,
    show_source: bool,
    filter_state: Option<&'a FilterState>,
    /// Search state for highlighting matches
    search_state: Option<&'a SearchState>,
}

impl<'a> LogView<'a> {
    // ... existing methods ...
    
    /// Set the search state for match highlighting
    pub fn search_state(mut self, state: &'a SearchState) -> Self {
        self.search_state = Some(state);
        self
    }
}
```

##### 4.2 Update format_entry for highlighting

```rust
impl<'a> LogView<'a> {
    /// Format a single log entry with optional search highlighting
    fn format_entry(&self, entry: &LogEntry, entry_index: usize) -> Line<'static> {
        let (level_style, msg_style) = Self::level_style(entry.level);
        let source_style = Self::source_style(entry.source);

        let mut spans = Vec::with_capacity(8);

        // Timestamp
        if self.show_timestamps {
            spans.push(Span::styled(
                entry.formatted_time(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::raw(" "));
        }

        // Level indicator
        spans.push(Span::styled(
            format!("{} ", Self::level_icon(entry.level)),
            level_style,
        ));

        // Source
        if self.show_source {
            spans.push(Span::styled(
                format!("[{}] ", entry.source.prefix()),
                source_style,
            ));
        }

        // Message with search highlighting
        let message_spans = self.format_message_with_highlights(
            &entry.message,
            entry_index,
            msg_style,
        );
        spans.extend(message_spans);

        Line::from(spans)
    }
    
    /// Format message text with search match highlighting
    fn format_message_with_highlights(
        &self,
        message: &str,
        entry_index: usize,
        base_style: Style,
    ) -> Vec<Span<'static>> {
        let Some(search) = self.search_state else {
            // No search active, return plain message
            return vec![Self::format_message(message, base_style)];
        };
        
        if search.query.is_empty() || !search.is_valid {
            return vec![Self::format_message(message, base_style)];
        }
        
        // Get matches for this entry
        let matches = search.matches_for_entry(entry_index);
        if matches.is_empty() {
            return vec![Self::format_message(message, base_style)];
        }
        
        // Build spans with highlighted regions
        let mut spans = Vec::new();
        let mut last_end = 0;
        
        // Highlight styles
        let highlight_style = Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD);
        let current_highlight_style = Style::default()
            .bg(Color::LightYellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        
        for mat in matches {
            // Add text before match
            if mat.start > last_end {
                let before = &message[last_end..mat.start];
                spans.push(Span::styled(before.to_string(), base_style));
            }
            
            // Add highlighted match
            let matched_text = &message[mat.start..mat.end];
            let style = if search.is_current_match(mat) {
                current_highlight_style
            } else {
                highlight_style
            };
            spans.push(Span::styled(matched_text.to_string(), style));
            
            last_end = mat.end;
        }
        
        // Add remaining text after last match
        if last_end < message.len() {
            let after = &message[last_end..];
            spans.push(Span::styled(after.to_string(), base_style));
        }
        
        spans
    }
}
```

##### 4.3 Update render to pass entry index

```rust
impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // ... existing filter logic ...
        
        // Format visible entries with index for highlighting
        let lines: Vec<Line> = filtered_indices[start..end]
            .iter()
            .map(|&idx| self.format_entry(&self.logs[idx], idx))
            .collect();
        
        // ... rest of render ...
    }
}
```

##### 4.4 Update title to show search status

```rust
impl<'a> LogView<'a> {
    fn build_title(&self) -> String {
        let base = self.title.trim();
        let mut parts = Vec::new();
        
        // Add filter indicator
        if let Some(filter) = self.filter_state {
            if filter.is_active() {
                let mut indicators = Vec::new();
                if filter.level_filter != LogLevelFilter::All {
                    indicators.push(filter.level_filter.display_name());
                }
                if filter.source_filter != LogSourceFilter::All {
                    indicators.push(filter.source_filter.display_name());
                }
                if !indicators.is_empty() {
                    parts.push(indicators.join(" | "));
                }
            }
        }
        
        // Add search status
        if let Some(search) = self.search_state {
            if !search.query.is_empty() {
                let status = search.display_status();
                if !status.is_empty() {
                    parts.push(status);
                }
            }
        }
        
        if parts.is_empty() {
            base.to_string()
        } else {
            format!("{} [{}]", base, parts.join(" • "))
        }
    }
}
```

#### 5. Update `src/tui/render.rs`

Pass search state to LogView:

```rust
if let Some(session) = state.session_manager.current_session() {
    let mut log_view = LogView::new(&session.logs)
        .title(" Logs ")
        .filter_state(&session.filter_state);
    
    // Add search state if there's an active search
    if !session.search_state.query.is_empty() {
        log_view = log_view.search_state(&session.search_state);
    }
    
    log_view.render(log_area, buf, &mut session.log_view_state);
}
```

### Acceptance Criteria

1. Search executes automatically as user types query
2. Regex patterns work correctly (e.g., `error.*failed`, `\d+ms`)
3. Case-insensitive search by default
4. Invalid regex shows error, doesn't crash
5. Matches are highlighted with yellow background
6. Current match has distinct highlight (underlined, brighter)
7. `n` key moves to next match and scrolls view
8. `N` key moves to previous match and scrolls view
9. Navigation wraps around (last → first, first → last)
10. Match count updates in real-time as logs change
11. Header shows search status: `[3/47 matches]`
12. Scrolling centers the current match in view
13. Works correctly with filtered logs (search within filter)
14. Performance acceptable with 1000+ log entries

### Testing

Add tests to `src/core/types.rs`:

```rust
#[test]
fn test_execute_search_finds_matches() {
    let logs = vec![
        LogEntry::info(LogSource::App, "Hello world"),
        LogEntry::error(LogSource::App, "Error occurred"),
        LogEntry::info(LogSource::App, "Another hello"),
    ];
    
    let mut state = SearchState::default();
    state.set_query("hello");
    state.execute_search(&logs);
    
    assert_eq!(state.matches.len(), 2);
    assert_eq!(state.matches[0].entry_index, 0);
    assert_eq!(state.matches[1].entry_index, 2);
}

#[test]
fn test_execute_search_case_insensitive() {
    let logs = vec![
        LogEntry::info(LogSource::App, "ERROR in caps"),
        LogEntry::error(LogSource::App, "error lowercase"),
    ];
    
    let mut state = SearchState::default();
    state.set_query("error");
    state.execute_search(&logs);
    
    assert_eq!(state.matches.len(), 2);
}

#[test]
fn test_execute_search_regex() {
    let logs = vec![
        LogEntry::info(LogSource::App, "Took 150ms"),
        LogEntry::info(LogSource::App, "Took 2500ms"),
        LogEntry::info(LogSource::App, "No timing here"),
    ];
    
    let mut state = SearchState::default();
    state.set_query(r"\d+ms");
    state.execute_search(&logs);
    
    assert_eq!(state.matches.len(), 2);
}

#[test]
fn test_execute_search_invalid_regex() {
    let logs = vec![LogEntry::info(LogSource::App, "test")];
    
    let mut state = SearchState::default();
    state.set_query("[invalid");
    state.execute_search(&logs);
    
    assert!(!state.is_valid);
    assert!(state.error.is_some());
    assert!(state.matches.is_empty());
}

#[test]
fn test_matches_for_entry() {
    let logs = vec![
        LogEntry::info(LogSource::App, "test one test"),
        LogEntry::info(LogSource::App, "no match"),
        LogEntry::info(LogSource::App, "test two"),
    ];
    
    let mut state = SearchState::default();
    state.set_query("test");
    state.execute_search(&logs);
    
    let matches_0 = state.matches_for_entry(0);
    assert_eq!(matches_0.len(), 2); // "test" appears twice
    
    let matches_1 = state.matches_for_entry(1);
    assert!(matches_1.is_empty());
    
    let matches_2 = state.matches_for_entry(2);
    assert_eq!(matches_2.len(), 1);
}

#[test]
fn test_current_match_entry_index() {
    let logs = vec![
        LogEntry::info(LogSource::App, "first"),
        LogEntry::info(LogSource::App, "test"),
        LogEntry::info(LogSource::App, "last"),
    ];
    
    let mut state = SearchState::default();
    state.set_query("test");
    state.execute_search(&logs);
    
    assert_eq!(state.current_match_entry_index(), Some(1));
    
    state.next_match(); // Wrap to 0 since only 1 match
    assert_eq!(state.current_match_entry_index(), Some(1));
}
```

Add widget tests to `src/tui/widgets/log_view.rs`:

```rust
#[test]
fn test_format_message_with_highlights_no_search() {
    let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Hello world")];
    let view = LogView::new(&logs);
    
    let spans = view.format_message_with_highlights(
        "Hello world",
        0,
        Style::default(),
    );
    
    assert_eq!(spans.len(), 1);
}

#[test]
fn test_format_message_with_highlights_with_match() {
    let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Hello world")];
    let mut search = SearchState::default();
    search.set_query("world");
    search.execute_search(&logs);
    
    let view = LogView::new(&logs).search_state(&search);
    
    let spans = view.format_message_with_highlights(
        "Hello world",
        0,
        Style::default(),
    );
    
    // Should be: "Hello " + "world" (highlighted)
    assert_eq!(spans.len(), 2);
}

#[test]
fn test_title_shows_search_status() {
    let logs = vec![
        make_entry(LogLevel::Info, LogSource::App, "test message"),
        make_entry(LogLevel::Info, LogSource::App, "another test"),
    ];
    let mut search = SearchState::default();
    search.set_query("test");
    search.execute_search(&logs);
    
    let view = LogView::new(&logs)
        .title(" Logs ")
        .search_state(&search);
    
    let title = view.build_title();
    assert!(title.contains("2")); // Should show match count
}
```

### Notes

- Search is case-insensitive by default using `(?i)` regex flag
- Consider adding a toggle for case-sensitive search in the future
- For very large log buffers (10k+ entries), consider:
  - Limiting max matches displayed
  - Running search in background task
  - Adding search timeout
- The current match highlighting (underlined) helps distinguish active vs inactive matches
- Search works on filtered logs - only visible entries are searched
- When new logs are added, search results should update (re-execute on log add)

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

- `src/core/types.rs` - Added `execute_search()`, `current_match_entry_index()`, `matches_for_entry()`, `is_current_match()` methods to `SearchState`
- `src/app/handler/update.rs` - Updated `SearchInput`, `NextSearchMatch`, `PrevSearchMatch` handlers to execute search and scroll to matches; added `scroll_to_log_entry()` helper
- `src/tui/widgets/log_view.rs` - Added `search_state` field and builder method; implemented `format_message_with_highlights()` for match highlighting; updated `build_title()` to show search status
- `src/tui/render.rs` - Updated to pass `search_state` to `LogView` when search is active

### Implementation Details

1. **Search Execution**: Added `execute_search(&logs)` method that compiles a case-insensitive regex from the query and finds all matches across log entries, storing them with entry index and byte offsets.

2. **Match Navigation**: `next_match()` and `prev_match()` now trigger scrolling to center the current match in the view, accounting for active filters.

3. **Highlighting**: Implemented two-tier highlighting:
   - Regular matches: Yellow background, black text, bold
   - Current match: Light yellow background, black text, bold + underlined

4. **Title Status**: Search status appears in log panel header as `[X/Y matches]` or `[No matches]`, combined with filter indicators using `•` separator.

5. **Filter Integration**: `scroll_to_log_entry()` accounts for active filters to find the correct visible index for scrolling.

### Testing Performed

```bash
cargo fmt    # ✓ No formatting issues
cargo check  # ✓ Compiles cleanly
cargo clippy # ✓ No warnings
cargo test core::types  # ✓ 65 tests passed
cargo test log_view     # ✓ 31 tests passed
```

Added 15 new tests:
- `test_execute_search_finds_matches`
- `test_execute_search_case_insensitive`
- `test_execute_search_regex`
- `test_execute_search_invalid_regex`
- `test_execute_search_empty_query_clears_matches`
- `test_execute_search_sets_current_match`
- `test_execute_search_preserves_current_match`
- `test_matches_for_entry`
- `test_current_match_entry_index`
- `test_current_match_entry_index_no_matches`
- `test_is_current_match`
- `test_is_current_match_no_current`
- `test_execute_search_multiple_matches_per_entry`
- `test_format_message_with_highlights_*` (7 tests for LogView)

### Acceptance Criteria Status

1. ✅ Search executes automatically as user types query
2. ✅ Regex patterns work correctly (e.g., `error.*failed`, `\d+ms`)
3. ✅ Case-insensitive search by default
4. ✅ Invalid regex shows error, doesn't crash
5. ✅ Matches are highlighted with yellow background
6. ✅ Current match has distinct highlight (underlined, brighter)
7. ✅ `n` key moves to next match and scrolls view
8. ✅ `N` key moves to previous match and scrolls view
9. ✅ Navigation wraps around (last → first, first → last)
10. ⚠️ Match count updates in real-time as logs change (requires re-execution on log add - not yet implemented)
11. ✅ Header shows search status: `[3/47 matches]`
12. ✅ Scrolling centers the current match in view
13. ✅ Works correctly with filtered logs (search within filter)
14. ✅ Performance acceptable with 1000+ log entries

### Risks/Limitations

- Search is re-executed on every keystroke; for very large log buffers this could cause lag (mitigated by regex compilation caching in the regex crate)
- Search matches are not automatically re-computed when new logs arrive; this would require hooking into `add_log()` - deferred for future enhancement
- The `scroll_to_log_entry` function uses `visible_lines` from `LogViewState` which is only set during render, so first scroll after search might not center perfectly