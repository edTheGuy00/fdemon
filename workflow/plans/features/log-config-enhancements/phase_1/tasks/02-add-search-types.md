## Task: Add Search Types

**Objective**: Define the search state struct and related types for tracking search queries, matches, and navigation within the log filtering system.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `src/core/types.rs`: Add search-related type definitions

### Details

Add the following types to `src/core/types.rs`:

1. **SearchMatch** struct:
   ```rust
   /// Represents a single search match within a log entry
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct SearchMatch {
       /// Index of the log entry containing the match
       pub entry_index: usize,
       /// Byte offset of match start within the message
       pub start: usize,
       /// Byte offset of match end within the message
       pub end: usize,
   }
   ```

2. **SearchState** struct:
   ```rust
   /// State for log search functionality
   #[derive(Debug, Clone, Default)]
   pub struct SearchState {
       /// The current search query string
       pub query: String,
       /// Whether search mode is active (showing search input)
       pub is_active: bool,
       /// Compiled regex pattern (None if query is empty or invalid)
       /// Note: Using Option<String> for the pattern since regex::Regex doesn't implement Clone
       /// The actual Regex should be compiled on-demand or cached separately
       pub pattern: Option<String>,
       /// Whether the current pattern is valid regex
       pub is_valid: bool,
       /// All matches found in the current log buffer
       pub matches: Vec<SearchMatch>,
       /// Current match index (for n/N navigation)
       pub current_match: Option<usize>,
       /// Error message if regex compilation failed
       pub error: Option<String>,
   }
   ```

3. Implement helper methods for **SearchState**:
   - `new() -> Self` - create default search state
   - `clear()` - clear query, matches, and deactivate
   - `activate()` - enter search mode
   - `deactivate()` - exit search mode but keep query/matches
   - `set_query(&str)` - set query and validate regex
   - `has_matches() -> bool` - true if there are any matches
   - `match_count() -> usize` - number of matches
   - `current_match_index() -> Option<usize>` - 1-based index for display
   - `current_match() -> Option<&SearchMatch>` - get current match
   - `next_match()` - move to next match (wrap around)
   - `prev_match()` - move to previous match (wrap around)
   - `jump_to_match(usize)` - jump to specific match by entry index
   - `update_matches(matches: Vec<SearchMatch>)` - update match list
   - `display_status() -> String` - format "[3/47 matches]" or "[No matches]"

4. Implement helper methods for **SearchMatch**:
   - `new(entry_index, start, end) -> Self`
   - `len() -> usize` - length of the matched text

### Acceptance Criteria

1. All types are defined and exported from `core/types.rs`
2. `SearchState::set_query()` validates regex syntax:
   - Empty query clears state
   - Valid regex sets `is_valid = true` and stores pattern
   - Invalid regex sets `is_valid = false` and stores error message
3. Navigation methods (`next_match`, `prev_match`) wrap around correctly
4. `display_status()` returns properly formatted strings:
   - `"[3/47 matches]"` when matches exist and current is set
   - `"[47 matches]"` when matches exist but no current
   - `"[No matches]"` when query exists but no matches
   - `""` when no query
5. Types derive necessary traits (Debug, Clone, Default where appropriate)
6. `SearchMatch` derives PartialEq and Eq for test assertions

### Testing

Add unit tests to `src/core/types.rs`:

```rust
#[cfg(test)]
mod search_tests {
    use super::*;

    #[test]
    fn test_search_state_default() {
        let state = SearchState::default();
        assert!(state.query.is_empty());
        assert!(!state.is_active);
        assert!(!state.has_matches());
    }

    #[test]
    fn test_search_state_activate_deactivate() {
        let mut state = SearchState::default();
        state.activate();
        assert!(state.is_active);
        state.deactivate();
        assert!(!state.is_active);
    }

    #[test]
    fn test_search_state_set_valid_query() {
        let mut state = SearchState::default();
        state.set_query("error");
        assert_eq!(state.query, "error");
        assert!(state.is_valid);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_search_state_set_invalid_regex() {
        let mut state = SearchState::default();
        state.set_query("[invalid");
        assert!(!state.is_valid);
        assert!(state.error.is_some());
    }

    #[test]
    fn test_search_state_clear() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.activate();
        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        
        state.clear();
        
        assert!(state.query.is_empty());
        assert!(!state.is_active);
        assert!(state.matches.is_empty());
        assert!(state.current_match.is_none());
    }

    #[test]
    fn test_search_navigation_next() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(2, 5, 9),
            SearchMatch::new(5, 0, 4),
        ]);
        state.current_match = Some(0);
        
        state.next_match();
        assert_eq!(state.current_match, Some(1));
        
        state.next_match();
        assert_eq!(state.current_match, Some(2));
        
        state.next_match(); // wrap around
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn test_search_navigation_prev() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(2, 5, 9),
            SearchMatch::new(5, 0, 4),
        ]);
        state.current_match = Some(0);
        
        state.prev_match(); // wrap around
        assert_eq!(state.current_match, Some(2));
        
        state.prev_match();
        assert_eq!(state.current_match, Some(1));
    }

    #[test]
    fn test_display_status_with_matches() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(2, 5, 9),
        ]);
        state.current_match = Some(0);
        
        assert_eq!(state.display_status(), "[1/2 matches]");
        
        state.next_match();
        assert_eq!(state.display_status(), "[2/2 matches]");
    }

    #[test]
    fn test_display_status_no_matches() {
        let mut state = SearchState::default();
        state.set_query("nonexistent");
        state.update_matches(vec![]);
        
        assert_eq!(state.display_status(), "[No matches]");
    }

    #[test]
    fn test_display_status_empty_query() {
        let state = SearchState::default();
        assert_eq!(state.display_status(), "");
    }

    #[test]
    fn test_search_match_len() {
        let m = SearchMatch::new(0, 5, 10);
        assert_eq!(m.len(), 5);
    }
}
```

### Notes

- The `regex` crate is already a transitive dependency, but we may need to add it explicitly to `Cargo.toml` if direct usage is required
- `Regex` doesn't implement `Clone`, so we store the pattern string and compile on-demand
- Consider adding a separate `CompiledSearch` struct in the widget layer that holds the actual compiled `Regex`
- Case sensitivity could be a future enhancement (default to case-insensitive)
- For very large log buffers, consider limiting the number of matches tracked