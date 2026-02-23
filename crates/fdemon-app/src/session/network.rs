//! # Network Monitor State
//!
//! Per-session state for HTTP/WebSocket network profiling.
//! Stores the rolling request history, selected request detail,
//! and UI interaction state (filter, sort, recording toggle).

use std::collections::VecDeque;

use fdemon_core::network::{HttpProfileEntry, HttpProfileEntryDetail, SocketEntry};

// ── NetworkDetailTab ──────────────────────────────────────────────────────────

/// Sub-tab selection for the network request detail panel.
///
/// This is a UI concern (which detail panel is active) and belongs in
/// `fdemon-app` alongside `NetworkState`, not in the zero-dependency
/// `fdemon-core` domain crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkDetailTab {
    #[default]
    General,
    Headers,
    RequestBody,
    ResponseBody,
    Timing,
}

/// Maximum number of network entries to keep per session.
pub const DEFAULT_MAX_NETWORK_ENTRIES: usize = 500;

/// Per-session network monitoring state.
#[derive(Debug)]
pub struct NetworkState {
    /// Rolling history of HTTP requests (FIFO, bounded). Uses a `VecDeque` so
    /// that front-eviction (`pop_front`) is O(1) instead of the O(n) shift
    /// required by `Vec::remove(0)`.
    pub entries: VecDeque<HttpProfileEntry>,
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
    /// Whether the filter text input is currently active.
    pub filter_input_active: bool,
    /// Buffer for the filter text being typed (committed on Enter).
    pub filter_input_buffer: String,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
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
            filter_input_active: false,
            filter_input_buffer: String::new(),
        }
    }
}

impl NetworkState {
    /// Create a new `NetworkState` with configurable settings.
    ///
    /// `max_entries` caps the rolling request history (FIFO eviction).
    /// `auto_record` sets whether recording starts automatically.
    pub fn with_config(max_entries: usize, auto_record: bool) -> Self {
        Self {
            max_entries,
            recording: auto_record,
            ..Self::default()
        }
    }

    /// Reset to initial state (used on session switch or disconnect).
    ///
    /// Preserves config-derived fields (`max_entries`, `recording`) so that
    /// settings from `.fdemon/config.toml` (e.g. `network_auto_record = false`)
    /// survive a session reset. All other fields revert to their defaults.
    pub fn reset(&mut self) {
        *self = Self {
            max_entries: self.max_entries,
            recording: self.recording,
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
                self.entries.push_back(new_entry);
            }
        }
        // Evict oldest entries if over capacity. `pop_front` is O(1) on VecDeque
        // whereas the previous `Vec::remove(0)` was O(n) due to element shifting.
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
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

    /// Returns `true` if `entry` matches the given lowercase filter string.
    ///
    /// Centralises the filter predicate used by both [`filtered_entries`] and
    /// [`filtered_count`] so they cannot diverge.
    fn entry_matches(entry: &HttpProfileEntry, filter_lower: &str) -> bool {
        entry.method.to_lowercase().contains(filter_lower)
            || entry.uri.to_lowercase().contains(filter_lower)
            || entry
                .status_code
                .is_some_and(|s| s.to_string().contains(filter_lower))
            || entry
                .content_type
                .as_deref()
                .is_some_and(|ct| ct.to_lowercase().contains(filter_lower))
    }

    /// Get entries filtered by the current filter text.
    pub fn filtered_entries(&self) -> Vec<&HttpProfileEntry> {
        if self.filter.is_empty() {
            return self.entries.iter().collect();
        }
        let filter_lower = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|e| Self::entry_matches(e, &filter_lower))
            .collect()
    }

    /// Number of entries visible after filtering.
    ///
    /// Uses an iterator count to avoid allocating a full `Vec` just to get a length.
    pub fn filtered_count(&self) -> usize {
        if self.filter.is_empty() {
            return self.entries.len();
        }
        let filter_lower = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|e| Self::entry_matches(e, &filter_lower))
            .count()
    }

    /// Update the filter text and clear any active selection.
    ///
    /// Clearing the selection on filter change avoids the index domain mismatch
    /// between the filtered list (used by `select_prev`/`select_next`/`selected_entry`)
    /// and the raw list (used by the eviction loop in `merge_entries`). When the
    /// filter changes the old `selected_index` would point to the wrong entry in
    /// the new filtered view, so we reset it here as the single authoritative
    /// location for this invariant.
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected_index = None;
        self.selected_detail = None;
        self.scroll_offset = 0;
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
    use fdemon_core::network::{HttpProfileEntry, HttpProfileEntryDetail};

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
    fn test_with_config_sets_max_entries() {
        let state = NetworkState::with_config(100, true);
        assert_eq!(state.max_entries, 100);
        assert!(state.recording);
        assert!(state.entries.is_empty());
    }

    #[test]
    fn test_with_config_sets_auto_record_false() {
        let state = NetworkState::with_config(500, false);
        assert_eq!(state.max_entries, 500);
        assert!(!state.recording);
    }

    #[test]
    fn test_with_config_preserves_other_defaults() {
        let state = NetworkState::with_config(200, true);
        assert!(state.filter.is_empty());
        assert!(state.selected_index.is_none());
        assert_eq!(state.detail_tab, NetworkDetailTab::General);
        assert!(!state.loading_detail);
        assert!(state.last_poll_timestamp.is_none());
        assert_eq!(state.scroll_offset, 0);
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

    #[test]
    fn test_reset_preserves_recording() {
        let mut state = NetworkState::default();
        // Simulate network_auto_record = false set from config.
        state.recording = false;
        state.merge_entries(vec![make_entry("1", "GET", Some(200))]);
        state.selected_index = Some(3);

        state.reset();

        assert!(
            !state.recording,
            "recording should be preserved across reset"
        );
        assert!(state.entries.is_empty(), "entries should be cleared");
        assert_eq!(state.selected_index, None, "selected_index should be reset");
    }

    // ── set_filter / selected_index semantics ─────────────────────────────────

    #[test]
    fn test_set_filter_clears_selected_index() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![
            make_entry("1", "GET", Some(200)),
            make_entry("2", "POST", Some(201)),
        ]);
        state.select_next(); // selected_index = Some(0)
        assert_eq!(state.selected_index, Some(0));

        // Changing the filter must clear the selection to avoid domain mismatch.
        state.set_filter("POST".to_string());
        assert_eq!(
            state.selected_index, None,
            "set_filter must clear selected_index to avoid filtered vs raw index mismatch"
        );
    }

    #[test]
    fn test_set_filter_clears_scroll_offset() {
        let mut state = NetworkState::default();
        state.scroll_offset = 5;
        state.set_filter("api".to_string());
        assert_eq!(
            state.scroll_offset, 0,
            "set_filter must reset scroll_offset"
        );
    }

    #[test]
    fn test_set_filter_clears_selected_detail() {
        let mut state = NetworkState::default();
        state.selected_index = Some(0);
        state.selected_detail = Some(Box::new(HttpProfileEntryDetail {
            entry: make_entry("1", "GET", Some(200)),
            request_headers: vec![],
            response_headers: vec![],
            request_body: vec![],
            response_body: vec![],
            events: vec![],
            connection_info: None,
        }));
        state.set_filter("something".to_string());
        assert!(
            state.selected_detail.is_none(),
            "set_filter must clear selected_detail"
        );
    }

    #[test]
    fn test_set_filter_to_empty_string_resets() {
        let mut state = NetworkState::default();
        state.set_filter("GET".to_string());
        assert_eq!(state.filter, "GET");
        // Clearing filter should also clear selection.
        state.set_filter(String::new());
        assert!(state.filter.is_empty());
        assert!(state.selected_index.is_none());
    }

    // ── eviction regression tests ─────────────────────────────────────────────

    #[test]
    fn test_eviction_without_filter_adjusts_selection() {
        // With no active filter, eviction must decrement selected_index correctly.
        let mut state = NetworkState::default();
        state.max_entries = 3;
        // Add 3 entries: raw index 0=a, 1=b, 2=c
        state.merge_entries(vec![
            make_entry("a", "GET", Some(200)),
            make_entry("b", "GET", Some(200)),
            make_entry("c", "GET", Some(200)),
        ]);
        // Select raw index 2 (entry "c")
        state.selected_index = Some(2);

        // Add a 4th entry, triggering eviction of entry "a" (raw 0).
        // selected_index should decrement from 2 to 1.
        state.merge_entries(vec![make_entry("d", "GET", Some(200))]);
        assert_eq!(
            state.selected_index,
            Some(1),
            "Eviction should decrement selected_index when no filter active"
        );
        // The selected entry should now be "c" (now at raw index 1)
        assert_eq!(
            state.entries[1].id, "c",
            "Entry 'c' should now be at raw index 1"
        );
    }

    #[test]
    fn test_eviction_clears_selection_when_selected_entry_is_evicted() {
        let mut state = NetworkState::default();
        state.max_entries = 2;
        state.merge_entries(vec![
            make_entry("a", "GET", Some(200)),
            make_entry("b", "GET", Some(200)),
        ]);
        // Select the oldest entry (raw index 0).
        state.selected_index = Some(0);

        // Adding a 3rd entry evicts "a" (raw index 0). Selected entry is gone.
        state.merge_entries(vec![make_entry("c", "GET", Some(200))]);
        assert_eq!(
            state.selected_index, None,
            "selected_index must be cleared when the selected entry is evicted"
        );
    }

    // ── filtered_count consistency tests ─────────────────────────────────────

    #[test]
    fn test_filtered_count_matches_filtered_entries_len_no_filter() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![
            make_entry("1", "GET", Some(200)),
            make_entry("2", "POST", Some(201)),
            make_entry("3", "PUT", Some(204)),
        ]);
        assert_eq!(
            state.filtered_count(),
            state.filtered_entries().len(),
            "filtered_count() must equal filtered_entries().len() with no filter"
        );
    }

    #[test]
    fn test_filtered_count_matches_filtered_entries_len_with_filter() {
        let mut state = NetworkState::default();
        state.merge_entries(vec![
            make_entry("1", "GET", Some(200)),
            make_entry("2", "POST", Some(201)),
            make_entry("3", "GET", Some(404)),
        ]);
        state.filter = "GET".to_string();
        assert_eq!(
            state.filtered_count(),
            state.filtered_entries().len(),
            "filtered_count() must equal filtered_entries().len() when filter is active"
        );
    }

    #[test]
    fn test_filtered_count_empty_state() {
        let state = NetworkState::default();
        assert_eq!(
            state.filtered_count(),
            0,
            "filtered_count() must be 0 for empty state"
        );
        assert_eq!(
            state.filtered_count(),
            state.filtered_entries().len(),
            "filtered_count() must equal filtered_entries().len() for empty state"
        );
    }

    // ── NetworkDetailTab moved-location tests ─────────────────────────────────

    #[test]
    fn test_network_detail_tab_default_is_general() {
        assert_eq!(
            NetworkDetailTab::default(),
            NetworkDetailTab::General,
            "NetworkDetailTab default must be General"
        );
    }

    #[test]
    fn test_network_detail_tab_all_variants() {
        // Ensure all variants are constructible and distinct.
        let tabs = [
            NetworkDetailTab::General,
            NetworkDetailTab::Headers,
            NetworkDetailTab::RequestBody,
            NetworkDetailTab::ResponseBody,
            NetworkDetailTab::Timing,
        ];
        for (i, a) in tabs.iter().enumerate() {
            for (j, b) in tabs.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }
}
