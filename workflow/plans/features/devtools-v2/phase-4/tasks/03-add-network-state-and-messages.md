## Task: Add Network State and Messages

**Objective**: Add all network-related state types, message variants, and UpdateAction variants to `fdemon-app`. This establishes the TEA data model for the Network Monitor: per-session network state with request history, UI interaction state, and all the message types that handlers and the engine will process.

**Depends on**: Task 01 (add-network-domain-types)

### Scope

- `crates/fdemon-app/src/session/network.rs`: **NEW** — `NetworkState` struct
- `crates/fdemon-app/src/session/session.rs`: Add `pub network: NetworkState` field
- `crates/fdemon-app/src/session/mod.rs`: Add `pub mod network;` and re-export
- `crates/fdemon-app/src/state.rs`: Add `DevToolsPanel::Network` variant, extend `DevToolsViewState` reset
- `crates/fdemon-app/src/message.rs`: Add all network message variants
- `crates/fdemon-app/src/handler/mod.rs`: Add `UpdateAction` variants for network operations

### Details

#### Create `session/network.rs`

Follow the `session/performance.rs` pattern. Network state is per-session because each Flutter session has its own VM Service and HTTP profile.

```rust
//! # Network Monitor State
//!
//! Per-session state for HTTP/WebSocket network profiling.
//! Stores the rolling request history, selected request detail,
//! and UI interaction state (filter, sort, recording toggle).

use fdemon_core::network::{
    HttpProfileEntry, HttpProfileEntryDetail, NetworkDetailTab, SocketEntry,
};
use fdemon_core::performance::RingBuffer;

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
        self.entries.iter().filter(|e| {
            e.method.to_lowercase().contains(&filter_lower)
                || e.uri.to_lowercase().contains(&filter_lower)
                || e.status_code.map_or(false, |s| s.to_string().contains(&filter_lower))
                || e.content_type.as_deref().map_or(false, |ct| ct.to_lowercase().contains(&filter_lower))
        }).collect()
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
        if count == 0 { return; }
        self.selected_index = Some(match self.selected_index {
            Some(0) | None => 0,
            Some(i) => i - 1,
        });
        self.selected_detail = None; // invalidate cached detail
    }

    /// Navigate selection down.
    pub fn select_next(&mut self) {
        let count = self.filtered_count();
        if count == 0 { return; }
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
```

#### Add `NetworkState` to `Session`

In `crates/fdemon-app/src/session/session.rs`, add:

```rust
use crate::session::network::NetworkState;

pub struct Session {
    // ... existing fields ...
    pub network: NetworkState,    // NEW
}
```

Update `Session::new()` and `Session::default()` (if they exist) to include `network: NetworkState::default()`.

#### Add module declaration

In `crates/fdemon-app/src/session/mod.rs`:

```rust
pub mod network;
pub use network::NetworkState;
```

#### Add `DevToolsPanel::Network` variant

In `crates/fdemon-app/src/state.rs`:

```rust
pub enum DevToolsPanel {
    #[default]
    Inspector,
    Performance,
    Network,        // NEW
}
```

Update `DevToolsViewState::reset()` — no new fields needed in `DevToolsViewState` itself since network state is per-session on `Session.network`. But if `DevToolsViewState` caches any network UI state, reset it here.

#### Add network message variants

In `crates/fdemon-app/src/message.rs`, add a new section:

```rust
// ── VM Service Network Messages (Phase 4, Network Monitor) ──────────────

/// HTTP profile poll results arrived.
VmServiceHttpProfileReceived {
    session_id: SessionId,
    timestamp: i64,
    entries: Vec<HttpProfileEntry>,
},

/// Full detail for a single HTTP request arrived.
VmServiceHttpRequestDetailReceived {
    session_id: SessionId,
    detail: Box<HttpProfileEntryDetail>,
},

/// Detail fetch failed.
VmServiceHttpRequestDetailFailed {
    session_id: SessionId,
    error: String,
},

/// Network monitoring background task started.
VmServiceNetworkMonitoringStarted {
    session_id: SessionId,
    network_shutdown_tx: Arc<watch::Sender<bool>>,
    network_task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
},

/// Network extensions not available (e.g., release mode).
VmServiceNetworkExtensionsUnavailable {
    session_id: SessionId,
},

// ── Network Monitor UI Messages ──────────────────────────────────────────

/// Navigate the network request list.
NetworkNavigate(NetworkNav),

/// Select a specific request by index.
NetworkSelectRequest {
    index: Option<usize>,
},

/// Switch detail sub-tab.
NetworkSwitchDetailTab(NetworkDetailTab),

/// Toggle recording on/off.
ToggleNetworkRecording,

/// Clear all recorded network entries.
ClearNetworkProfile {
    session_id: SessionId,
},

/// Update filter text.
NetworkFilterChanged(String),
```

Add the `NetworkNav` helper enum (before the main `Message` enum, alongside `InspectorNav`):

```rust
/// Navigation actions for the network request list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkNav {
    Up,
    Down,
    PageUp,
    PageDown,
}
```

#### Add `UpdateAction` variants

In `crates/fdemon-app/src/handler/mod.rs`:

```rust
/// Start the network monitoring polling task.
StartNetworkMonitoring {
    session_id: SessionId,
    handle: Option<VmRequestHandle>,    // hydrated by process.rs
    poll_interval_ms: u64,
},

/// Fetch full detail for a specific HTTP request.
FetchHttpRequestDetail {
    session_id: SessionId,
    request_id: String,
    vm_handle: Option<VmRequestHandle>, // hydrated by process.rs
},

/// Clear the HTTP profile on the VM.
ClearHttpProfile {
    session_id: SessionId,
    vm_handle: Option<VmRequestHandle>, // hydrated by process.rs
},
```

### Acceptance Criteria

1. `NetworkState` struct exists with all fields (entries, selected_index, selected_detail, recording, filter, detail_tab, etc.)
2. `NetworkState::merge_entries()` correctly merges and evicts entries
3. `NetworkState::filtered_entries()` applies text filter across method, URI, status, content type
4. `NetworkState::select_prev()` / `select_next()` handle empty and boundary cases
5. `NetworkState::clear()` resets entries and selection
6. `Session` has `pub network: NetworkState` field
7. `DevToolsPanel::Network` variant exists
8. All network `Message` variants compile
9. All `UpdateAction` variants for network operations compile
10. `NetworkNav` enum exists with `Up`, `Down`, `PageUp`, `PageDown`
11. `cargo check -p fdemon-app` passes
12. `cargo test -p fdemon-app` passes (existing tests updated for new enum variant / struct field)

### Testing

Add tests in `session/network.rs`:

```rust
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
```

### Notes

- **Per-session state, not global**: Network state lives on `Session.network`, not `DevToolsViewState`. This ensures each Flutter session has independent network history. When the user switches sessions, the active session's network state is displayed. `DevToolsViewState::reset()` is NOT used for network state.
- **`recording: true` by default**: Following Flutter DevTools behavior, recording is enabled when the Network tab is first activated. The user can toggle it off with Space.
- **`entries: Vec<HttpProfileEntry>` not `RingBuffer`**: Using `Vec` with manual eviction (instead of `RingBuffer`) because `merge_entries` needs to update existing entries by ID. `RingBuffer` doesn't support random-access updates. Eviction is oldest-first with `remove(0)` when `max_entries` exceeded.
- **`selected_detail: Option<Box<...>>`**: Boxed to avoid bloating the struct size. Detail is fetched on-demand via `FetchHttpRequestDetail` action when a request is selected.
- **Breaking changes to existing types**: Adding `DevToolsPanel::Network` requires updating all `match` arms on `DevToolsPanel`. The compiler will catch these — update each with a `Network => ...` arm (typically mirroring the `Performance` arm behavior).
