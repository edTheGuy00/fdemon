## Task: Integrate Filter/Search State into Session

**Objective**: Add FilterState and SearchState to the Session struct, add corresponding Message variants, and wire up the state management for log filtering and search.

**Depends on**: 01-add-filter-types, 02-add-search-types

**Estimated Time**: 3-4 hours

### Scope

- `src/app/session.rs`: Add filter and search state to Session
- `src/app/message.rs`: Add filter and search message variants
- `src/core/mod.rs`: Ensure new types are exported

### Details

#### 1. Update `src/core/mod.rs`

Ensure the new filter and search types are exported:

```rust
pub use types::{
    // Existing exports...
    AppPhase, LogEntry, LogLevel, LogSource,
    // New exports
    FilterState, LogLevelFilter, LogSourceFilter,
    SearchMatch, SearchState,
};
```

#### 2. Update `src/app/session.rs`

Add filter and search state fields to the `Session` struct:

```rust
use crate::core::{FilterState, SearchState};

pub struct Session {
    // ... existing fields ...
    
    /// Log filter state for this session
    pub filter_state: FilterState,
    
    /// Search state for this session
    pub search_state: SearchState,
}
```

Update the `Session::new()` constructor:

```rust
impl Session {
    pub fn new(...) -> Self {
        Self {
            // ... existing fields ...
            filter_state: FilterState::default(),
            search_state: SearchState::default(),
        }
    }
}
```

Add convenience methods to Session:

```rust
impl Session {
    /// Cycle the log level filter
    pub fn cycle_level_filter(&mut self) {
        self.filter_state.level_filter = self.filter_state.level_filter.cycle();
    }
    
    /// Cycle the log source filter
    pub fn cycle_source_filter(&mut self) {
        self.filter_state.source_filter = self.filter_state.source_filter.cycle();
    }
    
    /// Reset all filters to default
    pub fn reset_filters(&mut self) {
        self.filter_state.reset();
    }
    
    /// Get filtered logs (returns indices of matching entries)
    pub fn filtered_log_indices(&self) -> Vec<usize> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.filter_state.matches(entry))
            .map(|(i, _)| i)
            .collect()
    }
    
    /// Check if any filter is active
    pub fn has_active_filter(&self) -> bool {
        self.filter_state.is_active()
    }
    
    /// Start search mode
    pub fn start_search(&mut self) {
        self.search_state.activate();
    }
    
    /// Cancel search mode
    pub fn cancel_search(&mut self) {
        self.search_state.deactivate();
    }
    
    /// Clear search completely
    pub fn clear_search(&mut self) {
        self.search_state.clear();
    }
    
    /// Update search query
    pub fn set_search_query(&mut self, query: &str) {
        self.search_state.set_query(query);
    }
    
    /// Check if search mode is active
    pub fn is_searching(&self) -> bool {
        self.search_state.is_active
    }
}
```

Update `Session::clear_logs()` to also reset search matches:

```rust
pub fn clear_logs(&mut self) {
    self.logs.clear();
    self.log_view_state.offset = 0;
    // Clear search matches since logs are gone
    self.search_state.matches.clear();
    self.search_state.current_match = None;
}
```

#### 3. Update `src/app/message.rs`

Add new message variants for filtering and searching:

```rust
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // Log Filter Messages (Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Cycle to next log level filter
    CycleLevelFilter,
    /// Cycle to next log source filter
    CycleSourceFilter,
    /// Reset all filters to default
    ResetFilters,

    // ─────────────────────────────────────────────────────────
    // Log Search Messages (Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Enter search mode (show search prompt)
    StartSearch,
    /// Cancel search mode (hide prompt, keep query)
    CancelSearch,
    /// Clear search completely (remove query and matches)
    ClearSearch,
    /// Update search query text
    SearchInput { text: String },
    /// Navigate to next search match
    NextSearchMatch,
    /// Navigate to previous search match
    PrevSearchMatch,
    /// Search completed with matches (internal)
    SearchCompleted { matches: Vec<crate::core::SearchMatch> },
}
```

### Acceptance Criteria

1. `Session` struct has `filter_state: FilterState` and `search_state: SearchState` fields
2. `Session::new()` initializes both states with defaults
3. All convenience methods on Session work correctly
4. New Message variants are defined and compile
5. `core::mod.rs` exports all new types
6. Existing tests still pass
7. New tests cover the session filter/search methods

### Testing

Add tests to `src/app/session.rs`:

```rust
#[cfg(test)]
mod filter_search_tests {
    use super::*;
    use crate::core::{LogLevelFilter, LogSourceFilter};

    #[test]
    fn test_session_has_filter_state() {
        let session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::All);
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::All);
    }

    #[test]
    fn test_session_cycle_level_filter() {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Errors);
    }

    #[test]
    fn test_session_reset_filters() {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        session.cycle_level_filter();
        session.cycle_source_filter();
        session.reset_filters();
        assert!(!session.has_active_filter());
    }

    #[test]
    fn test_session_filtered_log_indices() {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        session.log_info(LogSource::App, "info message");
        session.log_error(LogSource::App, "error message");
        session.log_info(LogSource::Flutter, "flutter info");
        
        // No filter - all logs
        let indices = session.filtered_log_indices();
        assert_eq!(indices.len(), 3);
        
        // Errors only
        session.filter_state.level_filter = LogLevelFilter::Errors;
        let indices = session.filtered_log_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 1); // The error message
    }

    #[test]
    fn test_session_search_mode() {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        assert!(!session.is_searching());
        
        session.start_search();
        assert!(session.is_searching());
        
        session.cancel_search();
        assert!(!session.is_searching());
    }

    #[test]
    fn test_session_clear_logs_clears_search() {
        let mut session = Session::new(
            "device".into(),
            "Device".into(),
            "ios".into(),
            false,
        );
        session.log_info(LogSource::App, "test");
        session.search_state.update_matches(vec![
            crate::core::SearchMatch::new(0, 0, 4),
        ]);
        session.search_state.current_match = Some(0);
        
        session.clear_logs();
        
        assert!(session.search_state.matches.is_empty());
        assert!(session.search_state.current_match.is_none());
    }
}
```

### Notes

- Filter and search state are per-session, meaning each running Flutter instance has its own independent filters
- When switching sessions, the UI should reflect that session's filter/search state
- The `filtered_log_indices()` method returns indices into the original log buffer, which will be used by the LogView widget
- Search matches will be computed asynchronously in Task 6 to avoid blocking the UI for large log buffers
- Consider adding a `filter_changed` flag to Session for caching optimization (deferred to Task 4)