//! # Network Monitor Handlers
//!
//! TEA update functions for all network-related messages: HTTP profile
//! polling results, request detail fetching, navigation, filtering,
//! recording toggle, and clear operations.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::NetworkNav;
use crate::session::NetworkDetailTab;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::network::{HttpProfileEntry, HttpProfileEntryDetail};

/// Number of entries to skip per page-up or page-down navigation.
const NETWORK_PAGE_STEP: usize = 10;

/// Shared, abort-able handle to a background monitoring task.
///
/// The `Arc<Mutex<Option<...>>>` wrapper lets the spawn site store the handle
/// inside the `Arc` after the task is already running, then pass ownership to
/// the session handle via a message.  The `Option` allows the handle to be
/// moved out (`take()`) cleanly.
type SharedTaskHandle = std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>;

/// Handle incoming HTTP profile poll results.
///
/// Always advances `last_poll_timestamp` so the poller's `updatedSince`
/// cursor keeps moving. Entries are only merged into the session's network
/// state when recording is active, so pausing recording does not flood the
/// list with stale entries when recording is resumed.
pub(crate) fn handle_http_profile_received(
    state: &mut AppState,
    session_id: SessionId,
    timestamp: i64,
    entries: Vec<HttpProfileEntry>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Always advance the timestamp so the poller stays incremental.
        // This ensures that when recording resumes, only NEW requests appear
        // (not a flood of everything that arrived during the pause).
        handle.session.network.last_poll_timestamp = Some(timestamp);

        // Only merge entries when recording is active.
        if handle.session.network.recording {
            handle.session.network.merge_entries(entries);
        }
    }
    UpdateResult::none()
}

/// Handle full request detail received.
///
/// Stores the fetched detail and clears the loading flag for the session.
pub(crate) fn handle_http_request_detail_received(
    state: &mut AppState,
    session_id: SessionId,
    detail: Box<HttpProfileEntryDetail>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.network.loading_detail = false;
        handle.session.network.selected_detail = Some(detail);
    }
    UpdateResult::none()
}

/// Handle detail fetch failure.
///
/// Clears the loading flag and stores the error message for display.
pub(crate) fn handle_http_request_detail_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.network.loading_detail = false;
        handle.session.network.last_error = Some(error);
    }
    UpdateResult::none()
}

/// Handle network extensions unavailable (release mode).
///
/// Marks the session's network state as unavailable and disables recording
/// so the UI can show a "not available in release mode" message.
pub(crate) fn handle_network_extensions_unavailable(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.network.extensions_available = Some(false);
        handle.session.network.recording = false;
    }
    UpdateResult::none()
}

/// Handle network monitoring task started.
///
/// Stores the shutdown sender and task handle in the session handle so they
/// can be stopped cleanly on session close or VM disconnect.
pub(crate) fn handle_network_monitoring_started(
    state: &mut AppState,
    session_id: SessionId,
    shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
    task_handle: SharedTaskHandle,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Belt-and-suspenders: if a monitoring task is already running (e.g.
        // due to a race between the idempotency guard and task dispatch), stop
        // it before overwriting the handles so the old task does not become a
        // zombie.  This mirrors the teardown pattern used in
        // `VmServiceDisconnected` (update.rs).
        if let Some(h) = handle.network_task_handle.take() {
            h.abort();
        }
        if let Some(ref tx) = handle.network_shutdown_tx {
            let _ = tx.send(true);
        }

        handle.network_shutdown_tx = Some(shutdown_tx);
        handle.network_task_handle = task_handle.lock().ok().and_then(|mut g| g.take());
    }
    UpdateResult::none()
}

/// Navigate the request list.
///
/// Moves the selection up, down, page-up or page-down in the filtered
/// network request list and triggers a detail fetch for the newly
/// selected entry.
pub(crate) fn handle_network_navigate(state: &mut AppState, nav: NetworkNav) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    match nav {
        NetworkNav::Up => handle.session.network.select_prev(),
        NetworkNav::Down => handle.session.network.select_next(),
        NetworkNav::PageUp => {
            for _ in 0..NETWORK_PAGE_STEP {
                handle.session.network.select_prev();
            }
        }
        NetworkNav::PageDown => {
            for _ in 0..NETWORK_PAGE_STEP {
                handle.session.network.select_next();
            }
        }
    }

    // Trigger detail fetch for the newly selected request.
    fetch_selected_detail_action(state)
}

/// Select a specific request by index.
///
/// Clears the cached detail so a fresh fetch is triggered for the new
/// selection. If `index` is `None` the selection is cleared entirely.
pub(crate) fn handle_network_select_request(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    handle.session.network.selected_index = index;
    handle.session.network.selected_detail = None;

    if index.is_some() {
        fetch_selected_detail_action(state)
    } else {
        UpdateResult::none()
    }
}

/// Switch detail sub-tab.
///
/// Changes the active detail tab (General, Headers, RequestBody,
/// ResponseBody, or Timing) for the currently active session.
pub(crate) fn handle_network_switch_detail_tab(
    state: &mut AppState,
    tab: NetworkDetailTab,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.detail_tab = tab;
    }
    UpdateResult::none()
}

/// Toggle recording on/off.
///
/// Flips the `recording` flag. When recording is off, the polling task
/// continues but `handle_http_profile_received` discards incoming entries
/// while still advancing the timestamp cursor.
pub(crate) fn handle_toggle_network_recording(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    handle.session.network.recording = !handle.session.network.recording;
    UpdateResult::none()
}

/// Clear all recorded network entries.
///
/// Clears the local `NetworkState` immediately and returns a
/// `ClearHttpProfile` action to reset the VM-side request history.
pub(crate) fn handle_clear_network_profile(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.network.clear();
    }
    // Also clear on the VM side (vm_handle hydrated by process.rs).
    UpdateResult::action(UpdateAction::ClearHttpProfile {
        session_id,
        vm_handle: None,
    })
}

/// Update filter text.
///
/// Delegates to [`NetworkState::set_filter`], which clears the active selection
/// and scroll offset. This prevents the index domain mismatch between the
/// filtered-entry index stored in `selected_index` and the raw-entry index
/// assumed by the eviction loop in `merge_entries`.
pub(crate) fn handle_network_filter_changed(state: &mut AppState, filter: String) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.set_filter(filter);
    }
    UpdateResult::none()
}

/// Enter filter input mode — copy the current active filter into the input buffer.
///
/// Seeding the buffer with the existing filter lets the user edit in place
/// rather than always starting from an empty string.
pub(crate) fn handle_enter_filter_mode(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_buffer = handle.session.network.filter.clone();
        handle.session.network.filter_input_active = true;
    }
    UpdateResult::none()
}

/// Exit filter input mode — discard the buffer, keep the existing filter.
pub(crate) fn handle_exit_filter_mode(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_active = false;
        handle.session.network.filter_input_buffer.clear();
    }
    UpdateResult::none()
}

/// Commit the filter input — apply the buffer as the active filter and exit input mode.
///
/// Delegates to [`NetworkState::set_filter`] so the selection and scroll
/// offset are reset consistently with the direct `NetworkFilterChanged` path.
pub(crate) fn handle_commit_filter(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let new_filter = handle.session.network.filter_input_buffer.clone();
        handle.session.network.set_filter(new_filter);
        handle.session.network.filter_input_active = false;
        handle.session.network.filter_input_buffer.clear();
    }
    UpdateResult::none()
}

/// Append a character to the filter input buffer.
pub(crate) fn handle_filter_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_buffer.push(c);
    }
    UpdateResult::none()
}

/// Delete the last character from the filter input buffer.
pub(crate) fn handle_filter_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_buffer.pop();
    }
    UpdateResult::none()
}

/// Build a `FetchHttpRequestDetail` action for the currently selected entry.
///
/// Returns `UpdateResult::none()` when there is no active session, no
/// selection, or no entry at the selected index.
fn fetch_selected_detail_action(state: &AppState) -> UpdateResult {
    let Some(session_id) = state.session_manager.selected_id() else {
        return UpdateResult::none();
    };
    let Some(handle) = state.session_manager.get(session_id) else {
        return UpdateResult::none();
    };
    let Some(entry) = handle.session.network.selected_entry() else {
        return UpdateResult::none();
    };

    let request_id = entry.id.clone();
    UpdateResult::action(UpdateAction::FetchHttpRequestDetail {
        session_id,
        request_id,
        vm_handle: None, // hydrated by process.rs
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use fdemon_core::network::HttpProfileEntry;

    fn make_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    fn make_devtools_state() -> AppState {
        let mut state = AppState::new();
        let device = make_device();
        state.session_manager.create_session(&device).unwrap();
        state
    }

    fn make_devtools_state_with_entries(count: usize) -> AppState {
        let mut state = make_devtools_state();
        let entries: Vec<HttpProfileEntry> = (0..count)
            .map(|i| make_entry(&i.to_string(), "GET", Some(200)))
            .collect();
        let session_id = state.session_manager.selected_id().unwrap();
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.network.merge_entries(entries);
        }
        state
    }

    fn make_entry(id: &str, method: &str, status: Option<u16>) -> HttpProfileEntry {
        HttpProfileEntry {
            id: id.to_string(),
            method: method.to_string(),
            uri: format!("https://example.com/{id}"),
            status_code: status,
            content_type: Some("application/json".to_string()),
            start_time_us: 1_000_000,
            end_time_us: status.map(|_| 1_050_000),
            request_content_length: None,
            response_content_length: Some(128),
            error: None,
        }
    }

    fn active_session_id(state: &AppState) -> SessionId {
        state.session_manager.selected_id().unwrap()
    }

    #[test]
    fn test_handle_http_profile_received_stores_entries() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        let entries = vec![make_entry("1", "GET", Some(200))];
        let result = handle_http_profile_received(&mut state, session_id, 5000, entries);
        assert!(result.action.is_none());
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.session.network.entries.len(), 1);
        assert_eq!(handle.session.network.last_poll_timestamp, Some(5000));
    }

    #[test]
    fn test_handle_http_profile_received_merges_existing() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        let entries = vec![make_entry("1", "GET", None)]; // pending
        handle_http_profile_received(&mut state, session_id, 1000, entries);
        let entries2 = vec![make_entry("1", "GET", Some(200))]; // completed
        handle_http_profile_received(&mut state, session_id, 2000, entries2);
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.session.network.entries.len(), 1); // not duplicated
        assert_eq!(handle.session.network.entries[0].status_code, Some(200));
        assert_eq!(handle.session.network.last_poll_timestamp, Some(2000));
    }

    #[test]
    fn test_handle_http_request_detail_received_stores_detail() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        let detail = Box::new(HttpProfileEntryDetail {
            entry: make_entry("req-1", "GET", Some(200)),
            request_headers: vec![],
            response_headers: vec![],
            request_body: vec![],
            response_body: vec![],
            events: vec![],
            connection_info: None,
        });
        let result = handle_http_request_detail_received(&mut state, session_id, detail);
        assert!(result.action.is_none());
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(!handle.session.network.loading_detail);
        assert!(handle.session.network.selected_detail.is_some());
    }

    #[test]
    fn test_handle_http_request_detail_failed_clears_loading() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        // Set loading flag manually.
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .network
            .loading_detail = true;
        let result =
            handle_http_request_detail_failed(&mut state, session_id, "timeout".to_string());
        assert!(result.action.is_none());
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(!handle.session.network.loading_detail);
        assert_eq!(
            handle.session.network.last_error,
            Some("timeout".to_string())
        );
    }

    #[test]
    fn test_handle_toggle_recording() {
        let mut state = make_devtools_state();
        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .network
                .recording
        );
        handle_toggle_network_recording(&mut state);
        assert!(
            !state
                .session_manager
                .selected()
                .unwrap()
                .session
                .network
                .recording
        );
        handle_toggle_network_recording(&mut state);
        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .network
                .recording
        );
    }

    #[test]
    fn test_handle_clear_returns_action() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        // Add some entries first.
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .network
            .merge_entries(vec![make_entry("1", "GET", Some(200))]);
        let result = handle_clear_network_profile(&mut state, session_id);
        assert!(matches!(
            result.action,
            Some(UpdateAction::ClearHttpProfile { .. })
        ));
        // Local state is cleared immediately.
        let handle = state.session_manager.get(session_id).unwrap();
        assert!(handle.session.network.entries.is_empty());
    }

    #[test]
    fn test_handle_navigate_down_selects_first() {
        let mut state = make_devtools_state_with_entries(3);
        let result = handle_network_navigate(&mut state, NetworkNav::Down);
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.selected_index, Some(0));
        // Should trigger a detail fetch action.
        assert!(matches!(
            result.action,
            Some(UpdateAction::FetchHttpRequestDetail { .. })
        ));
    }

    #[test]
    fn test_handle_navigate_up_on_empty_is_noop() {
        let mut state = make_devtools_state();
        let result = handle_network_navigate(&mut state, NetworkNav::Up);
        assert!(result.action.is_none());
        assert!(state
            .session_manager
            .selected()
            .unwrap()
            .session
            .network
            .selected_index
            .is_none());
    }

    #[test]
    fn test_handle_navigate_page_down() {
        let mut state = make_devtools_state_with_entries(15);
        handle_network_navigate(&mut state, NetworkNav::PageDown);
        let handle = state.session_manager.selected().unwrap();
        // After PageDown from no selection, selects index 0 then steps 9 more = 9.
        // But actual behavior: each select_next from None→0, 1, 2, ... up to max.
        // After 10 calls from None: 0, 1, 2, ..., 9
        assert_eq!(handle.session.network.selected_index, Some(9));
    }

    #[test]
    fn test_handle_switch_detail_tab() {
        let mut state = make_devtools_state();
        handle_network_switch_detail_tab(&mut state, NetworkDetailTab::Headers);
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.detail_tab, NetworkDetailTab::Headers);
    }

    #[test]
    fn test_handle_switch_detail_tab_all_variants() {
        let mut state = make_devtools_state();
        for tab in [
            NetworkDetailTab::General,
            NetworkDetailTab::Headers,
            NetworkDetailTab::RequestBody,
            NetworkDetailTab::ResponseBody,
            NetworkDetailTab::Timing,
        ] {
            handle_network_switch_detail_tab(&mut state, tab);
            let handle = state.session_manager.selected().unwrap();
            assert_eq!(handle.session.network.detail_tab, tab);
        }
    }

    #[test]
    fn test_handle_filter_resets_selection() {
        let mut state = make_devtools_state_with_entries(3);
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .selected_index = Some(1);
        handle_network_filter_changed(&mut state, "POST".to_string());
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.filter, "POST");
        assert!(handle.session.network.selected_index.is_none());
        assert_eq!(handle.session.network.scroll_offset, 0);
    }

    #[test]
    fn test_handle_filter_empty_string_resets() {
        let mut state = make_devtools_state_with_entries(3);
        // Apply filter then clear it.
        handle_network_filter_changed(&mut state, "GET".to_string());
        handle_network_filter_changed(&mut state, String::new());
        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.network.filter.is_empty());
    }

    #[test]
    fn test_handle_extensions_unavailable() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        handle_network_extensions_unavailable(&mut state, session_id);
        let handle = state.session_manager.get(session_id).unwrap();
        assert_eq!(handle.session.network.extensions_available, Some(false));
        assert!(!handle.session.network.recording);
    }

    #[test]
    fn test_handle_select_request_sets_index() {
        let mut state = make_devtools_state_with_entries(3);
        let result = handle_network_select_request(&mut state, Some(2));
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.selected_index, Some(2));
        // Should return a fetch action since index.is_some().
        assert!(matches!(
            result.action,
            Some(UpdateAction::FetchHttpRequestDetail { .. })
        ));
    }

    #[test]
    fn test_handle_select_request_none_clears() {
        let mut state = make_devtools_state_with_entries(3);
        // Select first.
        handle_network_select_request(&mut state, Some(0));
        // Then deselect.
        let result = handle_network_select_request(&mut state, None);
        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.network.selected_index.is_none());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_fetch_selected_detail_no_session_returns_none() {
        let state = AppState::new(); // no sessions
        let result = fetch_selected_detail_action(&state);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_fetch_selected_detail_no_selection_returns_none() {
        let state = make_devtools_state_with_entries(3);
        // No selection set.
        let result = fetch_selected_detail_action(&state);
        assert!(result.action.is_none());
    }

    // ── Filter input mode tests ────────────────────────────────────────────────

    #[test]
    fn test_enter_filter_mode_copies_existing_filter() {
        let mut state = make_devtools_state();
        // Set a pre-existing filter.
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .filter = "api".to_string();
        handle_enter_filter_mode(&mut state);
        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.network.filter_input_active);
        assert_eq!(handle.session.network.filter_input_buffer, "api");
    }

    #[test]
    fn test_enter_filter_mode_starts_with_empty_buffer_when_no_filter() {
        let mut state = make_devtools_state();
        handle_enter_filter_mode(&mut state);
        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.network.filter_input_active);
        assert!(handle.session.network.filter_input_buffer.is_empty());
    }

    #[test]
    fn test_exit_filter_mode_discards_buffer() {
        let mut state = make_devtools_state();
        // Enter filter mode and type something.
        handle_enter_filter_mode(&mut state);
        handle_filter_input(&mut state, 'a');
        handle_filter_input(&mut state, 'p');
        handle_filter_input(&mut state, 'i');
        // Exit without committing.
        handle_exit_filter_mode(&mut state);
        let handle = state.session_manager.selected().unwrap();
        assert!(!handle.session.network.filter_input_active);
        assert!(handle.session.network.filter_input_buffer.is_empty());
        // Active filter should remain unchanged (was empty).
        assert!(handle.session.network.filter.is_empty());
    }

    #[test]
    fn test_commit_filter_applies_buffer() {
        let mut state = make_devtools_state_with_entries(5);
        handle_enter_filter_mode(&mut state);
        handle_filter_input(&mut state, 'G');
        handle_filter_input(&mut state, 'E');
        handle_filter_input(&mut state, 'T');
        handle_commit_filter(&mut state);
        let handle = state.session_manager.selected().unwrap();
        assert!(!handle.session.network.filter_input_active);
        assert!(handle.session.network.filter_input_buffer.is_empty());
        assert_eq!(handle.session.network.filter, "GET");
        // Selection and scroll should have been reset by set_filter.
        assert!(handle.session.network.selected_index.is_none());
        assert_eq!(handle.session.network.scroll_offset, 0);
    }

    #[test]
    fn test_filter_input_appends_char() {
        let mut state = make_devtools_state();
        handle_enter_filter_mode(&mut state);
        handle_filter_input(&mut state, 'a');
        handle_filter_input(&mut state, 'p');
        handle_filter_input(&mut state, 'i');
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.filter_input_buffer, "api");
    }

    #[test]
    fn test_filter_backspace_removes_char() {
        let mut state = make_devtools_state();
        handle_enter_filter_mode(&mut state);
        handle_filter_input(&mut state, 'a');
        handle_filter_input(&mut state, 'p');
        handle_filter_input(&mut state, 'i');
        handle_filter_backspace(&mut state);
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.filter_input_buffer, "ap");
    }

    #[test]
    fn test_filter_backspace_on_empty_is_noop() {
        let mut state = make_devtools_state();
        handle_enter_filter_mode(&mut state);
        // Backspace on empty buffer should not panic.
        let result = handle_filter_backspace(&mut state);
        assert!(result.action.is_none());
        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.network.filter_input_buffer.is_empty());
    }

    #[test]
    fn test_filter_mode_noop_without_session() {
        let mut state = AppState::new(); // no sessions
                                         // All filter-mode handlers should return UpdateResult::none() without panic.
        assert!(handle_enter_filter_mode(&mut state).action.is_none());
        assert!(handle_exit_filter_mode(&mut state).action.is_none());
        assert!(handle_commit_filter(&mut state).action.is_none());
        assert!(handle_filter_input(&mut state, 'x').action.is_none());
        assert!(handle_filter_backspace(&mut state).action.is_none());
    }

    #[test]
    fn test_enter_and_exit_leaves_filter_unchanged() {
        let mut state = make_devtools_state();
        // Set a pre-existing filter directly.
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .set_filter("GET".to_string());
        handle_enter_filter_mode(&mut state);
        handle_filter_input(&mut state, 'P');
        handle_filter_input(&mut state, 'O');
        handle_filter_input(&mut state, 'S');
        handle_filter_input(&mut state, 'T');
        // Cancel — active filter should still be "GET".
        handle_exit_filter_mode(&mut state);
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(handle.session.network.filter, "GET");
        assert!(!handle.session.network.filter_input_active);
    }

    #[test]
    fn test_handle_monitoring_started_stores_handles() {
        use std::sync::{Arc, Mutex};
        use tokio::sync::watch;

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut state = make_devtools_state();
            let session_id = active_session_id(&state);

            let (tx, _rx) = watch::channel(false);
            let shutdown_tx = Arc::new(tx);
            let task: tokio::task::JoinHandle<()> =
                tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(1)).await });
            let task_handle = Arc::new(Mutex::new(Some(task)));

            let result =
                handle_network_monitoring_started(&mut state, session_id, shutdown_tx, task_handle);
            assert!(result.action.is_none());
            let handle = state.session_manager.get(session_id).unwrap();
            assert!(handle.network_shutdown_tx.is_some());
            // The JoinHandle should have been moved out of the Arc<Mutex<Option<>>>.
            assert!(handle.network_task_handle.is_some());
        });
    }
}
