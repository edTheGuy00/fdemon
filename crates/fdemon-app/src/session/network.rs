//! # Network Monitor State
//!
//! Per-session state for HTTP/WebSocket network profiling.
//! Stores the rolling request history, selected request detail,
//! and UI interaction state (filter, sort, recording toggle).

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

use fdemon_core::network::{HttpProfileEntry, HttpProfileEntryDetail, SocketEntry};

// ── BodyWrapCache ─────────────────────────────────────────────────────────────

/// Cache key for the pre-wrapped body lines.
///
/// Invalidated when any of selection, detail tab, content width, body
/// byte-length, or body content changes.
///
/// `body_hash` is a lightweight `DefaultHasher` fingerprint of the formatted
/// body string. This makes the key collision-safe even for two different bodies
/// of equal formatted length: equal-length-but-different-content bodies hash
/// to different values and therefore never share a cache entry. This closes the
/// stale-render bug where a body update at a fixed `selected_index` with the
/// same formatted length would return the previous wrapped lines.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BodyWrapCacheKey {
    /// Raw index of the selected entry, or `None` if nothing is selected.
    pub selected_index: Option<usize>,
    /// Active detail sub-tab (determines which body field is rendered).
    pub detail_tab: NetworkDetailTab,
    /// Render content width used when the lines were wrapped.
    pub content_width: usize,
    /// Byte-length of the formatted body string at wrap time.
    pub body_len: usize,
    /// `DefaultHasher` fingerprint of the formatted body string.
    ///
    /// Ensures two bodies of equal formatted length but different content
    /// produce distinct keys and are never served from the same cache entry.
    pub body_hash: u64,
}

/// Memoized pre-wrapped body lines for the details pane.
///
/// Re-wrapping a 100–500 KB JSON body on every ~50 ms tick is visible as
/// stutter. This cache avoids re-wrapping when the inputs have not changed.
/// It lives on `NetworkState` (behind `RefCell` for interior mutability so
/// the immutable `&NetworkState` render path can update the cache) and is
/// naturally cleared by `reset()` and `clear()` (those methods replace the
/// entire struct). Explicit invalidation via `invalidate_wrap_cache()` handles
/// input changes (selection, tab, filter).
///
/// # Interior mutability
///
/// `RefCell` is used here for the same reason `Cell<usize>` is used for
/// render-hint fields in `PerformanceState`: the TUI render path receives
/// `&NetworkState` (not `&mut`), so `RefCell` provides the safe interior
/// mutability required to populate the cache during `render`. `NetworkState`
/// is owned by a single engine task (never shared across threads), so
/// `RefCell` (not `Mutex`) is correct.
#[derive(Debug, Default)]
pub struct BodyWrapCache {
    /// Cache discriminant. `None` means the cache is empty / invalid.
    pub key: Option<BodyWrapCacheKey>,
    /// The wrapped lines, valid when `key` matches the current render inputs.
    pub lines: Vec<String>,
}

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
    /// Scroll offset for the request details pane (body text viewport).
    pub details_scroll_offset: usize,
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
    /// Memoized pre-wrapped body lines for the details pane.
    ///
    /// Keyed by (selected_index, detail_tab, content_width, body_len, body_hash)
    /// to avoid re-wrapping large formatted JSON bodies on every ~50 ms render
    /// tick. The `body_hash` discriminator ensures different body content at the
    /// same index/tab/width/len never collide. Invalidated automatically by
    /// selection/tab/width changes, explicit `invalidate_wrap_cache()` calls,
    /// and by `reset()`/`clear()`. Behind `RefCell` so the immutable render
    /// path can populate the cache.
    pub body_wrap_cache: RefCell<BodyWrapCache>,
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
            details_scroll_offset: 0,
            socket_entries: Vec::new(),
            extensions_available: None,
            last_error: None,
            filter_input_active: false,
            filter_input_buffer: String::new(),
            body_wrap_cache: RefCell::new(BodyWrapCache::default()),
        }
    }
}

/// Compute a lightweight `DefaultHasher` fingerprint of a formatted body string.
///
/// Used to populate `BodyWrapCacheKey::body_hash` so that two bodies of equal
/// formatted length but different content always produce distinct cache keys.
pub fn hash_body(body: &str) -> u64 {
    let mut h = DefaultHasher::new();
    body.hash(&mut h);
    h.finish()
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

    /// Invalidate the body wrap cache.
    ///
    /// Called whenever the cache key inputs change: selection, detail tab switch,
    /// or filter change. `reset()` and `clear()` invalidate implicitly by
    /// replacing the entire struct (and thus the `RefCell`) with defaults.
    pub fn invalidate_wrap_cache(&mut self) {
        let mut cache = self.body_wrap_cache.borrow_mut();
        cache.key = None;
        cache.lines.clear();
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
        self.details_scroll_offset = 0;
        self.invalidate_wrap_cache();
    }

    /// Clear all entries and reset poll timestamp.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.selected_index = None;
        self.selected_detail = None;
        self.last_poll_timestamp = None;
        self.scroll_offset = 0;
        self.details_scroll_offset = 0;
        self.invalidate_wrap_cache();
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
        self.details_scroll_offset = 0; // reset detail viewport on selection change
        self.invalidate_wrap_cache();
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
        self.details_scroll_offset = 0; // reset detail viewport on selection change
        self.invalidate_wrap_cache();
    }

    /// Get the selected entry (if any).
    pub fn selected_entry(&self) -> Option<&HttpProfileEntry> {
        let filtered = self.filtered_entries();
        self.selected_index.and_then(|i| filtered.get(i).copied())
    }

    /// Return the cached pre-wrapped body lines if the cache is valid for
    /// the given key; otherwise compute them from `formatted_body`, store them,
    /// and return a reference-counted clone of the stored lines.
    ///
    /// The returned `Vec<String>` is a clone of the stored cache; this avoids
    /// holding the `RefCell` borrow across the caller's render loop.
    pub fn get_or_compute_wrapped_lines(
        &self,
        key: BodyWrapCacheKey,
        formatted_body: &str,
        max_width: usize,
    ) -> Vec<String> {
        {
            let cache = self.body_wrap_cache.borrow();
            if cache.key.as_ref() == Some(&key) {
                return cache.lines.clone();
            }
        }
        // Cache miss — compute and store.
        let lines = if max_width == 0 || formatted_body.is_empty() {
            Vec::new()
        } else {
            textwrap::wrap(formatted_body, max_width)
                .into_iter()
                .map(|cow| cow.into_owned())
                .collect()
        };
        {
            let mut cache = self.body_wrap_cache.borrow_mut();
            cache.key = Some(key);
            cache.lines = lines.clone();
        }
        lines
    }

    /// Scroll the details pane up by one line, clamped to 0.
    pub fn scroll_details_up(&mut self) {
        self.details_scroll_offset = self.details_scroll_offset.saturating_sub(1);
    }

    /// Scroll the details pane down by one line.
    ///
    /// The TUI renderer clamps `details_scroll_offset` to the valid range
    /// (`total_lines - viewport_height`) during rendering, so callers can
    /// increment without computing the line count at the handler site.
    pub fn scroll_details_down(&mut self) {
        self.details_scroll_offset = self.details_scroll_offset.saturating_add(1);
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
        let mut state = NetworkState {
            max_entries: 3,
            ..Default::default()
        };
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
        let mut state = NetworkState {
            max_entries: 100,
            ..Default::default()
        };
        state.merge_entries(vec![make_entry("1", "GET", Some(200))]);
        state.reset();
        assert!(state.entries.is_empty());
        assert_eq!(state.max_entries, 100);
    }

    #[test]
    fn test_reset_preserves_recording() {
        let mut state = NetworkState {
            // Simulate network_auto_record = false set from config.
            recording: false,
            ..Default::default()
        };
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
        let mut state = NetworkState {
            scroll_offset: 5,
            ..Default::default()
        };
        state.set_filter("api".to_string());
        assert_eq!(
            state.scroll_offset, 0,
            "set_filter must reset scroll_offset"
        );
    }

    #[test]
    fn test_set_filter_clears_selected_detail() {
        let mut state = NetworkState {
            selected_index: Some(0),
            selected_detail: Some(Box::new(HttpProfileEntryDetail {
                entry: make_entry("1", "GET", Some(200)),
                request_headers: vec![],
                response_headers: vec![],
                request_body: vec![],
                response_body: vec![],
                events: vec![],
                connection_info: None,
            })),
            ..Default::default()
        };
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
        let mut state = NetworkState {
            max_entries: 3,
            ..Default::default()
        };
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
        let mut state = NetworkState {
            max_entries: 2,
            ..Default::default()
        };
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

    // ── details_scroll_offset tests ───────────────────────────────────────────

    #[test]
    fn test_details_scroll_offset_default_zero() {
        let state = NetworkState::default();
        assert_eq!(state.details_scroll_offset, 0);
    }

    #[test]
    fn test_scroll_details_up_clamps_at_zero() {
        let mut state = NetworkState::default();
        // Already at 0 — should stay at 0 (no underflow).
        state.scroll_details_up();
        assert_eq!(state.details_scroll_offset, 0);
    }

    #[test]
    fn test_scroll_details_down_increments() {
        let mut state = NetworkState::default();
        state.scroll_details_down();
        assert_eq!(state.details_scroll_offset, 1);
        state.scroll_details_down();
        assert_eq!(state.details_scroll_offset, 2);
    }

    #[test]
    fn test_scroll_details_up_decrements() {
        let mut state = NetworkState {
            details_scroll_offset: 5,
            ..Default::default()
        };
        state.scroll_details_up();
        assert_eq!(state.details_scroll_offset, 4);
    }

    #[test]
    fn test_reset_clears_details_scroll_offset() {
        let mut state = NetworkState {
            details_scroll_offset: 10,
            ..Default::default()
        };
        state.reset();
        assert_eq!(
            state.details_scroll_offset, 0,
            "reset() must clear details_scroll_offset"
        );
    }

    #[test]
    fn test_clear_clears_details_scroll_offset() {
        let mut state = NetworkState {
            details_scroll_offset: 7,
            ..Default::default()
        };
        state.clear();
        assert_eq!(
            state.details_scroll_offset, 0,
            "clear() must clear details_scroll_offset"
        );
    }

    #[test]
    fn test_set_filter_clears_details_scroll_offset() {
        let mut state = NetworkState {
            details_scroll_offset: 3,
            ..Default::default()
        };
        state.set_filter("api".to_string());
        assert_eq!(
            state.details_scroll_offset, 0,
            "set_filter() must clear details_scroll_offset"
        );
    }

    #[test]
    fn test_select_prev_clears_details_scroll_offset() {
        let mut state = NetworkState {
            details_scroll_offset: 5,
            ..Default::default()
        };
        state.merge_entries(vec![
            make_entry("a", "GET", Some(200)),
            make_entry("b", "GET", Some(200)),
        ]);
        state.selected_index = Some(1);
        state.select_prev();
        assert_eq!(
            state.details_scroll_offset, 0,
            "select_prev() must reset details_scroll_offset"
        );
    }

    #[test]
    fn test_select_next_clears_details_scroll_offset() {
        let mut state = NetworkState {
            details_scroll_offset: 8,
            ..Default::default()
        };
        state.merge_entries(vec![
            make_entry("a", "GET", Some(200)),
            make_entry("b", "GET", Some(200)),
        ]);
        state.selected_index = Some(0);
        state.select_next();
        assert_eq!(
            state.details_scroll_offset, 0,
            "select_next() must reset details_scroll_offset"
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

    // ── C1 regression: stale wrap-cache body collision ────────────────────────

    /// Regression test for C1: two different bodies of EQUAL formatted length
    /// at the same (selected_index, tab, content_width) must produce distinct
    /// cache entries.  Without `body_hash` in the key, the second body would
    /// return the stale wrapped lines for the first body.
    #[test]
    fn test_wrap_cache_different_body_same_len_no_collision() {
        let mut state = NetworkState::default();

        // Two bodies that are exactly the same byte-length but different content.
        let body_a = "AAAAAAAAAA"; // 10 chars
        let body_b = "BBBBBBBBBB"; // 10 chars — equal length, different content
        assert_eq!(
            body_a.len(),
            body_b.len(),
            "Bodies must be equal length for this test"
        );

        let key_a = BodyWrapCacheKey {
            selected_index: Some(0),
            detail_tab: NetworkDetailTab::ResponseBody,
            content_width: 80,
            body_len: body_a.len(),
            body_hash: super::hash_body(body_a),
        };
        let key_b = BodyWrapCacheKey {
            selected_index: Some(0),
            detail_tab: NetworkDetailTab::ResponseBody,
            content_width: 80,
            body_len: body_b.len(),
            body_hash: super::hash_body(body_b),
        };

        // Keys must differ (different body_hash).
        assert_ne!(
            key_a, key_b,
            "Equal-length but distinct bodies must produce distinct keys"
        );

        // Warm cache with body_a.
        let lines_a = state.get_or_compute_wrapped_lines(key_a, body_a, 80);
        assert_eq!(
            lines_a,
            vec![body_a],
            "First body must wrap to itself at width 80"
        );

        // Invalidate (as the detail handler would do on body update).
        state.invalidate_wrap_cache();

        // Now cache body_b — must NOT return body_a's lines.
        let lines_b = state.get_or_compute_wrapped_lines(key_b, body_b, 80);
        assert_eq!(
            lines_b,
            vec![body_b],
            "Second body must return its own content, not the stale first body"
        );
        assert_ne!(
            lines_a, lines_b,
            "Different bodies must produce different wrapped lines"
        );
    }

    /// Verify that `hash_body` produces different hashes for distinct content
    /// (collision-safety smoke test for the discriminator we added to the key).
    #[test]
    fn test_hash_body_distinct_for_distinct_content() {
        assert_ne!(
            super::hash_body("AAAAAAAAAA"),
            super::hash_body("BBBBBBBBBB"),
            "hash_body must produce different hashes for different content of equal length"
        );
        assert_ne!(
            super::hash_body("hello world"),
            super::hash_body("world hello"),
            "hash_body must distinguish permutations"
        );
    }

    /// Verify that `hash_body` produces the same hash for the same content
    /// (determinism / cache-hit correctness).
    #[test]
    fn test_hash_body_deterministic_for_same_content() {
        let body = r#"{"key":"value","count":42}"#;
        assert_eq!(
            super::hash_body(body),
            super::hash_body(body),
            "hash_body must be deterministic for the same content"
        );
    }

    /// Simulates the full C1 failing scenario end-to-end using the cache's
    /// get_or_compute path:
    ///
    /// 1. Prime the cache with body #1 (no invalidation — as if detail never changed).
    /// 2. Call get_or_compute with body #2's key (same index/tab/width, same len,
    ///    different hash) — must return body #2, not the stale body #1.
    #[test]
    fn test_cache_key_with_body_hash_prevents_stale_render() {
        let state = NetworkState::default();

        let body1 = "response: ok!"; // 13 chars
        let body2 = "response: no!"; // 13 chars — equal length, different at position 11

        assert_eq!(body1.len(), body2.len());

        let key1 = BodyWrapCacheKey {
            selected_index: Some(5),
            detail_tab: NetworkDetailTab::ResponseBody,
            content_width: 40,
            body_len: body1.len(),
            body_hash: super::hash_body(body1),
        };
        let key2 = BodyWrapCacheKey {
            selected_index: Some(5),
            detail_tab: NetworkDetailTab::ResponseBody,
            content_width: 40,
            body_len: body2.len(),
            body_hash: super::hash_body(body2),
        };

        // Prime cache with body1.
        let result1 = state.get_or_compute_wrapped_lines(key1.clone(), body1, 40);
        assert!(
            result1.iter().any(|l| l.contains("ok")),
            "Cache must contain body1 content"
        );

        // Without explicit invalidation, ask for body2's key.
        // body_hash differs → cache miss → returns body2's content.
        let result2 = state.get_or_compute_wrapped_lines(key2, body2, 40);
        assert!(
            result2.iter().any(|l| l.contains("no")),
            "Should return body2 content, not stale body1; got: {result2:?}"
        );
        assert!(
            !result2
                .iter()
                .any(|l| l.contains("ok") && !l.contains("no")),
            "Must not contain stale 'ok' from body1; got: {result2:?}"
        );
    }
}
