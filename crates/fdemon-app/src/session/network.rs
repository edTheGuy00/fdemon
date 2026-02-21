//! # Network Monitor State
//!
//! Per-session state for HTTP/WebSocket network profiling.
//! Stores the rolling request history, selected request detail,
//! and UI interaction state (filter, sort, recording toggle).

use fdemon_core::network::{
    HttpProfileEntry, HttpProfileEntryDetail, NetworkDetailTab, SocketEntry,
};

/// Maximum number of network entries to keep per session.
pub const DEFAULT_MAX_NETWORK_ENTRIES: usize = 500;

/// Per-session network monitoring state.
#[derive(Debug)]
pub struct NetworkState {
    /// Rolling history of HTTP requests (FIFO, bounded).
    pub entries: Vec<HttpProfileEntry>,
    /// Maximum entries to keep. Oldest are evicted when exceeded.
    pub max_entries: usize,
    /// Index of the currently selected request in `entries`. `None` if no selection.
    pub selected_index: Option<usize>,
    /// Full detail for the currently selected request (fetched on-demand).
    pub selected_detail: Option<Box<HttpProfileEntryDetail>>,
    /// Whether we are actively recording/polling for network data.
    pub recording: bool,
    /// Current filter text (empty = no filter).
    pub filter: String,
    /// Which detail sub-tab is active.
    pub detail_tab: NetworkDetailTab,
    /// Whether we are currently loading detail for the selected request.
    pub loading_detail: bool,
    /// Timestamp from the last `getHttpProfile` response, used for incremental polling.
    pub last_poll_timestamp: Option<i64>,
    /// Scroll offset for the request table.
    pub scroll_offset: usize,
    /// Socket entries (optional, refreshed periodically).
    pub socket_entries: Vec<SocketEntry>,
    /// Whether the `ext.dart.io.*` extensions are available (false in release mode).
    pub extensions_available: Option<bool>,
    /// Error message from the last failed network operation.
    pub last_error: Option<String>,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: DEFAULT_MAX_NETWORK_ENTRIES,
            selected_index: None,
            selected_detail: None,
            recording: true, // auto-start recording by default
            filter: String::new(),
            detail_tab: NetworkDetailTab::default(),
            loading_detail: false,
            last_poll_timestamp: None,
            scroll_offset: 0,
            socket_entries: Vec::new(),
            extensions_available: None,
            last_error: None,
        }
    }
}

impl NetworkState {
    /// Reset to initial state (used on session switch or disconnect).
    pub fn reset(&mut self) {
        *self = Self {
            max_entries: self.max_entries,
            ..Self::default()
        };
    }

    /// Merge new entries from an incremental poll into the existing list.
    ///
    /// Updates existing entries (matched by ID) and appends new ones.
    /// Evicts oldest entries if `max_entries` is exceeded.
    pub fn merge_entries(&mut self, new_entries: Vec<HttpProfileEntry>) {
        for new_entry in new_entries {
            if let Some(existing) = self.entries.iter_mut().find(|e| e.id == new_entry.id) {
                // Update existing entry (e.g., request completed, status code arrived)
                *existing = new_entry;
            } else {
                self.entries.push(new_entry);
            }
        }
        // Evict oldest entries if over capacity
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
            // Adjust selected_index and scroll_offset
            if let Some(ref mut idx) = self.selected_index {
                if *idx == 0 {
                    self.selected_index = None;
                    self.selected_detail = None;
                } else {
                    *idx -= 1;
                }
            }
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
            }
        }
    }

    /// Get entries filtered by the current filter text.
    pub fn filtered_entries(&self) -> Vec<&HttpProfileEntry> {
        if self.filter.is_empty() {
            return self.entries.iter().collect();
        }
        let filter_lower = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.method.to_lowercase().contains(&filter_lower)
                    || e.uri.to_lowercase().contains(&filter_lower)
                    || e.status_code
                        .is_some_and(|s| s.to_string().contains(&filter_lower))
                    || e.content_type
                        .as_deref()
                        .is_some_and(|ct| ct.to_lowercase().contains(&filter_lower))
            })
            .collect()
    }

    /// Number of entries visible after filtering.
    pub fn filtered_count(&self) -> usize {
        self.filtered_entries().len()
    }

    /// Clear all entries and reset poll timestamp.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.selected_index = None;
        self.selected_detail = None;
        self.last_poll_timestamp = None;
        self.scroll_offset = 0;
    }

    /// Navigate selection up.
    pub fn select_prev(&mut self) {
        let count = self.filtered_count();
        if count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(0) | None => 0,
            Some(i) => i - 1,
        });
        self.selected_detail = None; // invalidate cached detail
    }

    /// Navigate selection down.
    pub fn select_next(&mut self) {
        let count = self.filtered_count();
        if count == 0 {
            return;
        }
        let max = count.saturating_sub(1);
        self.selected_index = Some(match self.selected_index {
            None => 0,
            Some(i) => (i + 1).min(max),
        });
        self.selected_detail = None; // invalidate cached detail
    }

    /// Get the selected entry (if any).
    pub fn selected_entry(&self) -> Option<&HttpProfileEntry> {
        let filtered = self.filtered_entries();
        self.selected_index.and_then(|i| filtered.get(i).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::network::HttpProfileEntry;

    fn make_entry(id: &str, method: &str, status: Option<u16>) -> HttpProfileEntry {
        HttpProfileEntry {
            id: id.to_string(),
            method: method.to_string(),
            uri: format!("https://example.com/{}", id),
            status_code: status,
            content_type: Some("application/json".to_string()),
            start_time_us: 1_000_000,
            end_time_us: status.map(|_| 1_050_000),
            request_content_length: None,
            response_content_length: Some(128),
            error: None,
        }
    }

    #[test]
    fn test_default_state() {
        let state = NetworkState::default();
        assert!(state.entries.is_empty());
        assert!(state.recording);
        assert!(state.filter.is_empty());
        assert_eq!(state.detail_tab, NetworkDetailTab::General);
    }

    #[test]
    fn test_merge_entries_appends_new() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![make_entry("1", "GET", Some(200))]);
        assert_eq!(state.entries.len(), 1);
        state.merge_entries(vec![make_entry("2", "POST", Some(201))]);
        assert_eq!(state.entries.len(), 2);
    }

    #[test]
    fn test_merge_entries_updates_existing() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![make_entry("1", "GET", None)]); // pending
        assert!(state.entries[0].is_pending());
        state.merge_entries(vec![make_entry("1", "GET", Some(200))]); // completed
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].status_code, Some(200));
    }

    #[test]
    fn test_merge_entries_evicts_oldest() {
        let mut state = NetworkState::default();
        state.max_entries = 3;
        for i in 0..5 {
            state.merge_entries(vec![make_entry(&i.to_string(), "GET", Some(200))]);
        }
        assert_eq!(state.entries.len(), 3);
        assert_eq!(state.entries[0].id, "2"); // oldest remaining
    }

    #[test]
    fn test_filtered_entries_no_filter() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![
            make_entry("1", "GET", Some(200)),
            make_entry("2", "POST", Some(201)),
        ]);
        assert_eq!(state.filtered_entries().len(), 2);
    }

    #[test]
    fn test_filtered_entries_by_method() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![
            make_entry("1", "GET", Some(200)),
            make_entry("2", "POST", Some(201)),
        ]);
        state.filter = "POST".to_string();
        assert_eq!(state.filtered_entries().len(), 1);
        assert_eq!(state.filtered_entries()[0].method, "POST");
    }

    #[test]
    fn test_select_navigation() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![
            make_entry("1", "GET", Some(200)),
            make_entry("2", "POST", Some(201)),
            make_entry("3", "PUT", Some(204)),
        ]);
        state.select_next(); // 0
        assert_eq!(state.selected_index, Some(0));
        state.select_next(); // 1
        assert_eq!(state.selected_index, Some(1));
        state.select_prev(); // 0
        assert_eq!(state.selected_index, Some(0));
        state.select_prev(); // stays at 0 (boundary)
        assert_eq!(state.selected_index, Some(0));
    }

    #[test]
    fn test_select_empty_list() {
        let mut state = NetworkState::default();
        state.select_next();
        assert_eq!(state.selected_index, None);
    }

    #[test]
    fn test_clear_resets_state() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![make_entry("1", "GET", Some(200))]);
        state.selected_index = Some(0);
        state.last_poll_timestamp = Some(12345);
        state.clear();
        assert!(state.entries.is_empty());
        assert!(state.selected_index.is_none());
        assert!(state.last_poll_timestamp.is_none());
    }

    #[test]
    fn test_reset_preserves_max_entries() {
        let mut state = NetworkState::default();
        state.max_entries = 100;
        state.merge_entries(vec![make_entry("1", "GET", Some(200))]);
        state.reset();
        assert!(state.entries.is_empty());
        assert_eq!(state.max_entries, 100);
    }
}
