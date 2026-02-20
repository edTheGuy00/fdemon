//! Inspector-specific DevTools handlers.
//!
//! Handles widget tree fetch results and inspector navigation for the
//! Widget Inspector panel in DevTools mode.

use std::time::Instant;

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::InspectorNav;
use crate::session::SessionId;
use crate::state::{AppState, DevToolsError, InspectorState};

use super::map_rpc_error;

/// Handle widget tree fetch completion.
///
/// Updates the inspector state with the fetched root node, auto-expands it,
/// and dispatches an initial layout fetch for the root node so the layout
/// panel shows data immediately without requiring a navigation event.
pub fn handle_widget_tree_fetched(
    state: &mut AppState,
    session_id: SessionId,
    root: Box<fdemon_core::DiagnosticsNode>,
) -> UpdateResult {
    // Only update if this is for the active session.
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        let root_node = *root;

        // Reset selection and expansion state so the user starts at the root.
        // Stale IDs from the previous tree are meaningless after a refresh.
        state.devtools_view_state.inspector.selected_index = 0;
        state.devtools_view_state.inspector.expanded.clear();

        // Auto-expand root node before storing.
        if let Some(ref value_id) = root_node.value_id {
            state
                .devtools_view_state
                .inspector
                .expanded
                .insert(value_id.clone());
        }

        state.devtools_view_state.inspector.root = Some(root_node);
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = None;
        state.devtools_view_state.inspector.has_object_group = true;

        // Clear stale layout data — value_ids from the previous tree are
        // meaningless after a refresh.
        state.devtools_view_state.inspector.layout = None;
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = None;
        state.devtools_view_state.inspector.last_fetched_node_id = None;
        state.devtools_view_state.inspector.pending_node_id = None;
        state.devtools_view_state.inspector.layout_last_fetch_time = None;

        // Auto-fetch layout for the initially selected node (root at index 0)
        // so the layout panel shows data immediately on Inspector entry.
        if let Some(node_id) = get_selected_value_id(&state.devtools_view_state.inspector) {
            state.devtools_view_state.inspector.layout_loading = true;
            state.devtools_view_state.inspector.pending_node_id = Some(node_id.clone());
            state.devtools_view_state.inspector.layout_last_fetch_time = Some(Instant::now());
            return UpdateResult::action(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle: None,
            });
        }
    }

    UpdateResult::none()
}

/// Handle widget tree fetch failure.
///
/// Maps the raw RPC error string to a user-friendly [`DevToolsError`] using
/// [`map_rpc_error`] so the TUI never displays a raw technical error.
pub fn handle_widget_tree_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = Some(map_rpc_error(&error));
    }

    UpdateResult::none()
}

/// Handle inspector tree navigation (Up/Down/Expand/Collapse).
///
/// On Up/Down navigation: clears stale layout data immediately (so the UI
/// shows a loading state), then dispatches a `FetchLayoutData` action for the
/// newly selected node unless debounced or already fetched.
///
/// On Expand/Collapse: no layout fetch is triggered (selection does not change).
pub fn handle_inspector_navigate(state: &mut AppState, nav: InspectorNav) -> UpdateResult {
    // Phase 1: read the visible node count and current selection, then handle navigation.
    // We scope the mutable borrow of `inspector` here so it ends before we access
    // `state.session_manager` below.
    let (old_index, new_index, should_fetch_layout) = {
        let inspector = &mut state.devtools_view_state.inspector;
        let visible = inspector.visible_nodes();
        let count = visible.len();

        if count == 0 {
            return UpdateResult::none();
        }

        // Collect the data we need before the mutable borrow below.
        let (value_id, has_children) = visible
            .get(inspector.selected_index)
            .and_then(|(node, _depth)| {
                node.value_id
                    .as_ref()
                    .map(|id| (id.clone(), !node.children.is_empty()))
            })
            .unzip();

        let old_index = inspector.selected_index;

        match nav {
            InspectorNav::Up => {
                if inspector.selected_index > 0 {
                    inspector.selected_index -= 1;
                }
            }
            InspectorNav::Down => {
                if inspector.selected_index < count.saturating_sub(1) {
                    inspector.selected_index += 1;
                }
            }
            InspectorNav::Expand => {
                if let (Some(value_id), Some(true)) = (value_id, has_children) {
                    if !inspector.is_expanded(&value_id) {
                        inspector.expanded.insert(value_id);
                    }
                }
                return UpdateResult::none();
            }
            InspectorNav::Collapse => {
                if let Some(value_id) = value_id {
                    inspector.expanded.remove(&value_id);
                }
                return UpdateResult::none();
            }
        }

        let new_index = inspector.selected_index;
        let selection_changed = new_index != old_index;

        if selection_changed {
            // Clear stale layout data immediately — user sees loading state.
            inspector.layout = None;
            inspector.layout_error = None;
        }

        (old_index, new_index, selection_changed)
    };
    // `inspector` borrow has ended here — we can now access other fields.

    // Phase 2: auto-fetch layout for the newly selected node (Up/Down only).
    if should_fetch_layout {
        let _ = (old_index, new_index); // suppress unused warning

        // Determine whether to fetch — collect node_id while holding the borrow.
        // The borrow of `inspector` ends at the close of this block, allowing us
        // to access `state.session_manager` afterwards.
        let fetch_node_id: Option<String> = {
            let inspector = &mut state.devtools_view_state.inspector;

            if inspector.is_layout_fetch_debounced() {
                None
            } else if let Some(node_id) = get_selected_value_id(inspector) {
                // Skip if the same node was already fetched (cache hit).
                if inspector.last_fetched_node_id.as_deref() == Some(node_id.as_str()) {
                    None
                } else {
                    inspector.layout_loading = true;
                    inspector.pending_node_id = Some(node_id.clone());
                    inspector.layout_last_fetch_time = Some(Instant::now());
                    Some(node_id)
                }
            } else {
                None
            }
        };
        // `inspector` borrow has ended — we can now access session_manager.

        if let Some(node_id) = fetch_node_id {
            let session_id = state.session_manager.selected().map(|h| h.session.id);
            if let Some(session_id) = session_id {
                return UpdateResult::action(UpdateAction::FetchLayoutData {
                    session_id,
                    node_id,
                    vm_handle: None, // hydrated by process.rs
                });
            }
        }
    }

    UpdateResult::none()
}

/// Extract the `value_id` of the currently selected visible node.
///
/// Returns `None` when the inspector has no tree loaded or the selected index
/// is out of range, or when the node has no `value_id`.
fn get_selected_value_id(inspector: &InspectorState) -> Option<String> {
    let visible = inspector.visible_nodes();
    visible
        .get(inspector.selected_index)
        .and_then(|(node, _depth)| node.value_id.clone())
}

/// Handle widget tree fetch timeout (Phase 5, Task 02).
///
/// Sets `inspector.loading = false` and stores an error message with a retry
/// hint, then marks `connection_status` as `TimedOut` so the tab bar can
/// indicate the degraded state.
pub fn handle_widget_tree_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    use crate::state::VmConnectionStatus;

    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = Some(DevToolsError::new(
            "Request timed out",
            "Press [r] to retry",
        ));
        state.devtools_view_state.connection_status = VmConnectionStatus::TimedOut;
    }

    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout data handlers (merged from layout.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle layout data fetch completion.
///
/// Updates the inspector state's layout fields with the fetched layout info.
/// Discards stale responses when the user has navigated away from the node
/// that this fetch was dispatched for.
pub fn handle_layout_data_fetched(
    state: &mut AppState,
    session_id: SessionId,
    layout: fdemon_core::LayoutInfo,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        // Guard against stale responses: if the user navigated away from the
        // node this fetch was dispatched for, discard the response so the UI
        // does not show layout data for the wrong node.
        let selected_id = get_selected_value_id(&state.devtools_view_state.inspector);
        let pending_id = state
            .devtools_view_state
            .inspector
            .pending_node_id
            .as_deref();
        if pending_id != selected_id.as_deref() {
            state.devtools_view_state.inspector.layout_loading = false;
            state.devtools_view_state.inspector.pending_node_id = None;
            return UpdateResult::none();
        }

        state.devtools_view_state.inspector.layout = Some(layout);
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = None;
        state.devtools_view_state.inspector.has_layout_object_group = true;
        // Promote pending node ID to last_fetched so repeated panel switches
        // for the same node skip redundant fetches.
        state.devtools_view_state.inspector.last_fetched_node_id =
            state.devtools_view_state.inspector.pending_node_id.take();
    }

    UpdateResult::none()
}

/// Handle layout data fetch failure.
///
/// Maps the raw RPC error string to a user-friendly [`DevToolsError`] using
/// [`map_rpc_error`] so the TUI never displays a raw technical error.
pub fn handle_layout_data_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = Some(map_rpc_error(&error));
        // Clear pending node ID so a subsequent switch will retry the fetch.
        state.devtools_view_state.inspector.pending_node_id = None;
    }

    UpdateResult::none()
}

/// Handle layout data fetch timeout.
///
/// Sets `inspector.layout_loading = false` and stores an error message with a
/// retry hint, then marks `connection_status` as `TimedOut`.
pub fn handle_layout_data_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    use crate::state::VmConnectionStatus;

    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = Some(DevToolsError::new(
            "Request timed out",
            "Press [r] to retry",
        ));
        state.devtools_view_state.connection_status = VmConnectionStatus::TimedOut;
        // Clear pending node ID so a subsequent switch will retry the fetch.
        state.devtools_view_state.inspector.pending_node_id = None;
    }

    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> AppState {
        AppState::new()
    }

    fn make_state_with_session() -> AppState {
        let mut state = AppState::new();
        let device = fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.session_manager.create_session(&device).unwrap();
        state
    }

    fn make_node(description: &str) -> fdemon_core::DiagnosticsNode {
        serde_json::from_value(serde_json::json!({
            "description": description
        }))
        .expect("valid DiagnosticsNode")
    }

    // ── widget tree ───────────────────────────────────────────────────────────

    #[test]
    fn test_handle_widget_tree_fetched_with_no_active_session_is_noop() {
        let mut state = make_state();
        let node = make_node("MaterialApp");

        // session_id 999 does not match any active session.
        handle_widget_tree_fetched(&mut state, 999, Box::new(node));

        assert!(state.devtools_view_state.inspector.root.is_none());
    }

    #[test]
    fn test_handle_widget_tree_fetch_failed_no_active_session_is_noop() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = true;

        // session_id 999 does not match any active session.
        handle_widget_tree_fetch_failed(&mut state, 999, "error".to_string());

        // Should not update state when session_id doesn't match.
        assert!(state.devtools_view_state.inspector.loading);
    }

    // ── inspector navigation ──────────────────────────────────────────────────

    #[test]
    fn test_handle_inspector_navigate_no_op_when_tree_empty() {
        let mut state = make_state();
        // No root → visible_nodes() returns empty.
        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);
        assert!(result.action.is_none());
        assert_eq!(state.devtools_view_state.inspector.selected_index, 0);
    }

    // ── Performance Polish: Tree Refresh Cooldown (Phase 5, Task 04) ──────────

    #[test]
    fn test_tree_refresh_debounce_while_loading() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = true;

        // is_fetch_debounced() returns true when loading
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active while loading"
        );
    }

    #[test]
    fn test_tree_refresh_debounce_cooldown() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.last_fetch_time = Some(std::time::Instant::now());

        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active within 2-second cooldown"
        );
    }

    #[test]
    fn test_tree_refresh_allowed_when_no_fetch_time() {
        let state = make_state();

        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be inactive with no previous fetch"
        );
    }

    #[test]
    fn test_tree_refresh_allowed_after_cooldown() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = false;
        // Set last_fetch_time to 3 seconds ago (past the 2-second cooldown).
        state.devtools_view_state.inspector.last_fetch_time =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(3));

        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be inactive after cooldown has elapsed"
        );
    }

    #[test]
    fn test_record_fetch_start_sets_loading_and_time() {
        let mut state = make_state();
        assert!(!state.devtools_view_state.inspector.loading);
        assert!(state
            .devtools_view_state
            .inspector
            .last_fetch_time
            .is_none());

        state.devtools_view_state.inspector.record_fetch_start();

        assert!(
            state.devtools_view_state.inspector.loading,
            "record_fetch_start should set loading = true"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_some(),
            "record_fetch_start should set last_fetch_time"
        );
    }

    #[test]
    fn test_inspector_reset_clears_last_fetch_time() {
        let mut state = make_state();
        state.devtools_view_state.inspector.record_fetch_start();
        assert!(state
            .devtools_view_state
            .inspector
            .last_fetch_time
            .is_some());

        state.devtools_view_state.inspector.reset();

        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_none(),
            "reset() should clear last_fetch_time"
        );
        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be inactive after reset"
        );
    }

    // ── Bug 2: Refresh resets selection ──────────────────────────────────────

    #[test]
    fn test_widget_tree_fetched_resets_selection_and_expanded() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate stale state from a previous tree.
        state.devtools_view_state.inspector.selected_index = 15;
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("stale-id-1".to_string());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("stale-id-2".to_string());

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MyApp",
            "valueId": "new-root"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert_eq!(
            state.devtools_view_state.inspector.selected_index, 0,
            "selected_index should be reset to 0 after refresh"
        );
        assert!(
            !state
                .devtools_view_state
                .inspector
                .expanded
                .contains("stale-id-1"),
            "Stale expanded IDs should be cleared"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded
                .contains("new-root"),
            "New root should be auto-expanded"
        );
        assert_eq!(
            state.devtools_view_state.inspector.expanded.len(),
            1,
            "Only the new root should be in expanded set"
        );
    }

    // ── Error integration ─────────────────────────────────────────────────────

    #[test]
    fn test_widget_tree_fetched_clears_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Pre-set an error.
        state.devtools_view_state.inspector.error = Some(DevToolsError::new("old error", "hint"));

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MyApp"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            state.devtools_view_state.inspector.error.is_none(),
            "error should be cleared after successful fetch"
        );
    }

    #[test]
    fn test_widget_tree_fetch_failed_stores_friendly_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_widget_tree_fetch_failed(
            &mut state,
            session_id,
            "Method not found: ext.flutter.inspector.getRootWidgetTree".to_string(),
        );

        let error = state
            .devtools_view_state
            .inspector
            .error
            .as_ref()
            .expect("error should be set");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading should be false after failure"
        );
    }

    #[test]
    fn test_timeout_stores_friendly_error_inspector() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_widget_tree_fetch_timeout(&mut state, session_id);

        let error = state
            .devtools_view_state
            .inspector
            .error
            .as_ref()
            .expect("error should be set after timeout");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
        assert!(!state.devtools_view_state.inspector.loading);
    }

    // ── Error classification (Phase 5, Task 03) ───────────────────────────────

    #[test]
    fn test_rpc_error_maps_extension_not_registered() {
        let error = map_rpc_error("Method not found: ext.flutter.inspector.getRootWidgetTree");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(
            error.hint.contains("debug mode"),
            "Hint should mention debug mode, got: {:?}",
            error.hint
        );
    }

    #[test]
    fn test_rpc_error_maps_extension_not_registered_variant() {
        let error = map_rpc_error("extension not registered: ext.flutter.inspector");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(error.hint.contains("debug mode"));
    }

    #[test]
    fn test_rpc_error_maps_isolate_not_found() {
        let error = map_rpc_error("Isolate not found: 123456");
        assert_eq!(error.message, "Flutter app isolate not found");
        assert!(error.hint.contains("[r]"), "Hint should include [r] key");
    }

    #[test]
    fn test_rpc_error_maps_timeout() {
        let error = map_rpc_error("Request timed out after 10 seconds");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_rpc_error_maps_connection_lost() {
        let error = map_rpc_error("WebSocket connection closed unexpectedly");
        assert_eq!(error.message, "VM Service connection lost");
        assert!(error.hint.contains("Reconnecting"));
    }

    #[test]
    fn test_rpc_error_maps_vm_handle_unavailable() {
        let error = map_rpc_error("VM Service handle unavailable");
        assert_eq!(error.message, "VM Service not available");
        assert!(error.hint.contains("debug mode"));
    }

    #[test]
    fn test_rpc_error_maps_object_group_expired() {
        let error = map_rpc_error("object group expired");
        assert_eq!(error.message, "Widget data expired");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_rpc_error_maps_parse_error() {
        let error = map_rpc_error("parse error: unexpected token at line 1");
        assert_eq!(error.message, "Unexpected response from Flutter");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_rpc_error_fallback_for_unknown_error() {
        let error = map_rpc_error("some completely unknown error xyz");
        assert_eq!(error.message, "DevTools request failed");
        assert!(error.hint.contains("[r]"));
    }

    // ── Layout data handlers ──────────────────────────────────────────────────

    #[test]
    fn test_layout_data_fetched_records_node_id() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Set up a tree so the selected node's value_id matches pending_node_id.
        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Widget",
            "valueId": "node-xyz"
        }))
        .expect("valid node");
        state.devtools_view_state.inspector.root = Some(node);
        state.devtools_view_state.inspector.selected_index = 0;

        // Simulate a pending fetch for "node-xyz".
        state.devtools_view_state.inspector.pending_node_id = Some("node-xyz".to_string());
        state.devtools_view_state.inspector.layout_loading = true;

        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, layout);

        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .as_deref(),
            Some("node-xyz"),
            "last_fetched_node_id should be set from pending_node_id on success"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .is_none(),
            "pending_node_id should be cleared after successful fetch"
        );
    }

    #[test]
    fn test_inspector_reset_clears_layout_node_ids() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.last_fetched_node_id = Some("node-1".to_string());
        state.devtools_view_state.inspector.pending_node_id = Some("node-2".to_string());

        state.devtools_view_state.inspector.reset();

        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .is_none(),
            "reset() should clear last_fetched_node_id"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .is_none(),
            "reset() should clear pending_node_id"
        );
    }

    // ── Error integration (layout) ─────────────────────────────────────────────

    #[test]
    fn test_layout_data_fetch_failed_stores_friendly_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_layout_data_fetch_failed(&mut state, session_id, "Isolate not found".to_string());

        let error = state
            .devtools_view_state
            .inspector
            .layout_error
            .as_ref()
            .expect("layout_error should be set");
        assert_eq!(error.message, "Flutter app isolate not found");
        assert!(!state.devtools_view_state.inspector.layout_loading);
    }

    #[test]
    fn test_timeout_stores_friendly_error_layout() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_layout_data_fetch_timeout(&mut state, session_id);

        let error = state
            .devtools_view_state
            .inspector
            .layout_error
            .as_ref()
            .expect("layout_error should be set after timeout");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
    }

    // ── Auto-fetch on navigation (Task 06) ────────────────────────────────────

    /// Build a minimal tree with a root node that has a value_id and one child.
    fn make_tree_with_children() -> fdemon_core::DiagnosticsNode {
        serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "root-id",
            "children": [{
                "description": "Child",
                "valueId": "child-id",
                "children": []
            }]
        }))
        .expect("valid DiagnosticsNode")
    }

    #[test]
    fn test_navigate_down_triggers_layout_fetch() {
        let mut state = make_state_with_session();

        // Set up a tree with the root expanded so that Down changes selection.
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
            "Should return FetchLayoutData action when navigating Down"
        );
        assert!(
            state.devtools_view_state.inspector.layout_loading,
            "layout_loading should be true while fetch is in flight"
        );
    }

    #[test]
    fn test_navigate_up_clears_stale_layout() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        // Start at child (index 1) so Up changes selection.
        state.devtools_view_state.inspector.selected_index = 1;

        // Pre-set some stale layout data.
        state.devtools_view_state.inspector.layout = Some(fdemon_core::LayoutInfo::default());
        state.devtools_view_state.inspector.layout_error =
            Some(DevToolsError::new("old error", "hint"));

        handle_inspector_navigate(&mut state, InspectorNav::Up);

        assert!(
            state.devtools_view_state.inspector.layout.is_none(),
            "Stale layout should be cleared on selection change"
        );
        assert!(
            state.devtools_view_state.inspector.layout_error.is_none(),
            "Stale layout_error should be cleared on selection change"
        );
    }

    #[test]
    fn test_navigate_debounced_skips_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        // Simulate a very recent fetch — debounce should suppress a new one.
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            result.action.is_none(),
            "Should not dispatch FetchLayoutData when debounced, got: {:?}",
            result.action
        );
    }

    #[test]
    fn test_navigate_same_node_skips_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        // Pre-set last_fetched_node_id to match the node we'll navigate TO (child-id).
        state.devtools_view_state.inspector.last_fetched_node_id = Some("child-id".to_string());

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            result.action.is_none(),
            "Should not re-fetch layout for a node already fetched (cache hit)"
        );
    }

    #[test]
    fn test_expand_does_not_trigger_layout_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Expand);

        assert!(
            result.action.is_none(),
            "Expand should not trigger layout fetch"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false after Expand"
        );
    }

    #[test]
    fn test_collapse_does_not_trigger_layout_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Collapse);

        assert!(
            result.action.is_none(),
            "Collapse should not trigger layout fetch"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false after Collapse"
        );
    }

    #[test]
    fn test_is_layout_fetch_debounced_while_loading() {
        let mut state = make_state();
        state.devtools_view_state.inspector.layout_loading = true;

        assert!(
            state
                .devtools_view_state
                .inspector
                .is_layout_fetch_debounced(),
            "Debounce should be active while layout_loading is true"
        );
    }

    #[test]
    fn test_is_layout_fetch_debounced_within_cooldown() {
        let mut state = make_state();
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        assert!(
            state
                .devtools_view_state
                .inspector
                .is_layout_fetch_debounced(),
            "Debounce should be active within 500ms cooldown"
        );
    }

    #[test]
    fn test_is_layout_fetch_debounced_inactive_initially() {
        let state = make_state();
        assert!(
            !state
                .devtools_view_state
                .inspector
                .is_layout_fetch_debounced(),
            "Debounce should be inactive with no previous fetch"
        );
    }

    #[test]
    fn test_inspector_reset_clears_layout_last_fetch_time() {
        let mut state = make_state();
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        state.devtools_view_state.inspector.reset();

        assert!(
            state
                .devtools_view_state
                .inspector
                .layout_last_fetch_time
                .is_none(),
            "reset() should clear layout_last_fetch_time"
        );
    }

    // ── Review fix: stale layout response guard ──────────────────────────────

    #[test]
    fn test_layout_data_fetched_discards_stale_response() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Set up tree with root expanded, child visible.
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());

        // Simulate: fetch was dispatched for "root-id" (index 0).
        state.devtools_view_state.inspector.pending_node_id = Some("root-id".to_string());
        state.devtools_view_state.inspector.layout_loading = true;

        // User navigated to child (index 1) before fetch completed.
        state.devtools_view_state.inspector.selected_index = 1;

        // Now the stale response arrives for "root-id".
        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, layout);

        // Response should be discarded — layout should remain None.
        assert!(
            state.devtools_view_state.inspector.layout.is_none(),
            "Stale layout response should be discarded when user navigated away"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should be cleared after discarding stale response"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .is_none(),
            "pending_node_id should be cleared after discarding stale response"
        );
    }

    #[test]
    fn test_layout_data_fetched_accepts_matching_response() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Set up tree, selected at root (index 0).
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        // Fetch was dispatched for "root-id" — matches current selection.
        state.devtools_view_state.inspector.pending_node_id = Some("root-id".to_string());
        state.devtools_view_state.inspector.layout_loading = true;

        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, layout);

        assert!(
            state.devtools_view_state.inspector.layout.is_some(),
            "Matching layout response should be accepted"
        );
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .as_deref(),
            Some("root-id"),
            "last_fetched_node_id should be promoted from pending"
        );
    }

    // ── Review fix: tree refresh clears layout cache ─────────────────────────

    #[test]
    fn test_widget_tree_fetched_clears_layout_fields() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Pre-set stale layout state from a previous tree.
        state.devtools_view_state.inspector.layout = Some(fdemon_core::LayoutInfo::default());
        state.devtools_view_state.inspector.layout_loading = true;
        state.devtools_view_state.inspector.layout_error =
            Some(DevToolsError::new("old error", "hint"));
        state.devtools_view_state.inspector.last_fetched_node_id = Some("old-node".to_string());
        state.devtools_view_state.inspector.pending_node_id = Some("old-pending".to_string());
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(10));

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "NewRoot",
            "valueId": "new-root-id"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        // Stale layout data should be cleared.
        assert!(
            state.devtools_view_state.inspector.layout_error.is_none(),
            "layout_error should be cleared after tree refresh"
        );
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .as_deref(),
            None,
            "last_fetched_node_id should be cleared after tree refresh \
             (pending_node_id is set for the initial fetch, not last_fetched)"
        );
    }

    // ── Review fix: initial layout fetch on tree load ────────────────────────

    #[test]
    fn test_widget_tree_fetched_dispatches_initial_layout_fetch() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "root-id"
        }))
        .unwrap();

        let result = handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
            "Should dispatch FetchLayoutData for the root node on tree load"
        );
        assert!(
            state.devtools_view_state.inspector.layout_loading,
            "layout_loading should be true after initial fetch dispatch"
        );
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .as_deref(),
            Some("root-id"),
            "pending_node_id should be set to root node for initial fetch"
        );
    }

    #[test]
    fn test_widget_tree_fetched_no_fetch_when_no_value_id() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Node without a value_id — cannot fetch layout.
        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root"
        }))
        .unwrap();

        let result = handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            result.action.is_none(),
            "Should not dispatch FetchLayoutData when root has no value_id"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false when no fetch is dispatched"
        );
    }
}
