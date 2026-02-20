//! Layout explorer DevTools handlers.
//!
//! Handles layout data fetch results for the Layout Explorer panel in
//! DevTools mode. In Phase 2, these handlers will be merged into the
//! inspector module when the Layout tab is absorbed.

use crate::handler::UpdateResult;
use crate::session::SessionId;
use crate::state::{AppState, DevToolsError};

use super::map_rpc_error;

/// Handle layout data fetch completion.
///
/// Updates the layout explorer state with the fetched layout info.
pub fn handle_layout_data_fetched(
    state: &mut AppState,
    session_id: SessionId,
    layout: fdemon_core::LayoutInfo,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.layout_explorer.layout = Some(layout);
        state.devtools_view_state.layout_explorer.loading = false;
        state.devtools_view_state.layout_explorer.error = None;
        state.devtools_view_state.layout_explorer.has_object_group = true;
        // Promote pending node ID to last_fetched so repeated panel switches
        // for the same node skip redundant fetches.
        state
            .devtools_view_state
            .layout_explorer
            .last_fetched_node_id = state
            .devtools_view_state
            .layout_explorer
            .pending_node_id
            .take();
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
        state.devtools_view_state.layout_explorer.loading = false;
        state.devtools_view_state.layout_explorer.error = Some(map_rpc_error(&error));
        // Clear pending node ID so a subsequent switch will retry the fetch.
        state.devtools_view_state.layout_explorer.pending_node_id = None;
    }

    UpdateResult::none()
}

/// Handle layout data fetch timeout (Phase 5, Task 02).
///
/// Sets `layout_explorer.loading = false` and stores an error message with a
/// retry hint, then marks `connection_status` as `TimedOut`.
pub fn handle_layout_data_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    use crate::state::VmConnectionStatus;

    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.layout_explorer.loading = false;
        state.devtools_view_state.layout_explorer.error = Some(DevToolsError::new(
            "Request timed out",
            "Press [r] to retry",
        ));
        state.devtools_view_state.connection_status = VmConnectionStatus::TimedOut;
        // Clear pending node ID so a subsequent switch will retry the fetch.
        state.devtools_view_state.layout_explorer.pending_node_id = None;
    }

    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DevToolsPanel;

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

    // ── Performance Polish: Layout Fetch Debounce (Phase 5, Task 04) ─────────

    #[test]
    fn test_switch_to_layout_skips_fetch_for_same_node() {
        let mut state = make_state_with_session();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .vm_connected = true;

        // Build a tree with a known value_id.
        let root: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MaterialApp",
            "valueId": "node-abc"
        }))
        .unwrap();
        state.devtools_view_state.inspector.root = Some(root);
        state.devtools_view_state.inspector.selected_index = 0;

        // Simulate that layout data for "node-abc" was already fetched.
        state
            .devtools_view_state
            .layout_explorer
            .last_fetched_node_id = Some("node-abc".to_string());
        state.devtools_view_state.layout_explorer.loading = false;

        let result = super::super::handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            result.action.is_none(),
            "Should skip layout fetch when selected node has not changed"
        );
    }

    #[test]
    fn test_switch_to_layout_fetches_when_node_changes() {
        let mut state = make_state_with_session();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .vm_connected = true;

        // Build a tree with a new node.
        let root: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MaterialApp",
            "valueId": "node-new"
        }))
        .unwrap();
        state.devtools_view_state.inspector.root = Some(root);
        state.devtools_view_state.inspector.selected_index = 0;

        // Simulate that layout data was fetched for a different node.
        state
            .devtools_view_state
            .layout_explorer
            .last_fetched_node_id = Some("node-old".to_string());
        state.devtools_view_state.layout_explorer.loading = false;

        let result = super::super::handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            result.action.is_some(),
            "Should trigger layout fetch when node has changed"
        );
        assert!(
            state.devtools_view_state.layout_explorer.loading,
            "Should set loading = true when fetch starts"
        );
        assert_eq!(
            state
                .devtools_view_state
                .layout_explorer
                .pending_node_id
                .as_deref(),
            Some("node-new"),
            "Should set pending_node_id to the new node"
        );
    }

    #[test]
    fn test_switch_to_layout_fetches_when_no_previous_fetch() {
        let mut state = make_state_with_session();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .vm_connected = true;

        let root: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MaterialApp",
            "valueId": "node-first"
        }))
        .unwrap();
        state.devtools_view_state.inspector.root = Some(root);
        state.devtools_view_state.inspector.selected_index = 0;

        // No previous fetch.
        assert!(state
            .devtools_view_state
            .layout_explorer
            .last_fetched_node_id
            .is_none());

        let result = super::super::handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            result.action.is_some(),
            "Should trigger layout fetch when no previous fetch exists"
        );
    }

    #[test]
    fn test_layout_data_fetched_records_node_id() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate a pending fetch for "node-xyz".
        state.devtools_view_state.layout_explorer.pending_node_id = Some("node-xyz".to_string());
        state.devtools_view_state.layout_explorer.loading = true;

        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, layout);

        assert_eq!(
            state
                .devtools_view_state
                .layout_explorer
                .last_fetched_node_id
                .as_deref(),
            Some("node-xyz"),
            "last_fetched_node_id should be set from pending_node_id on success"
        );
        assert!(
            state
                .devtools_view_state
                .layout_explorer
                .pending_node_id
                .is_none(),
            "pending_node_id should be cleared after successful fetch"
        );
    }

    #[test]
    fn test_layout_explorer_reset_clears_node_ids() {
        use crate::state::LayoutExplorerState;

        let mut layout = LayoutExplorerState::default();
        layout.last_fetched_node_id = Some("node-1".to_string());
        layout.pending_node_id = Some("node-2".to_string());

        layout.reset();

        assert!(
            layout.last_fetched_node_id.is_none(),
            "reset() should clear last_fetched_node_id"
        );
        assert!(
            layout.pending_node_id.is_none(),
            "reset() should clear pending_node_id"
        );
    }

    // ── Error integration ─────────────────────────────────────────────────────

    #[test]
    fn test_layout_data_fetch_failed_stores_friendly_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_layout_data_fetch_failed(&mut state, session_id, "Isolate not found".to_string());

        let error = state
            .devtools_view_state
            .layout_explorer
            .error
            .as_ref()
            .expect("error should be set");
        assert_eq!(error.message, "Flutter app isolate not found");
        assert!(!state.devtools_view_state.layout_explorer.loading);
    }

    #[test]
    fn test_timeout_stores_friendly_error_layout() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_layout_data_fetch_timeout(&mut state, session_id);

        let error = state
            .devtools_view_state
            .layout_explorer
            .error
            .as_ref()
            .expect("error should be set after timeout");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_switch_to_layout_no_selection_shows_friendly_error() {
        let mut state = make_state_with_session();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .vm_connected = true;

        // No inspector tree loaded — no selected node.
        super::super::handle_switch_panel(&mut state, DevToolsPanel::Layout);

        let error = state
            .devtools_view_state
            .layout_explorer
            .error
            .as_ref()
            .expect("error should be set when no widget is selected");
        assert!(
            error.message.contains("No widget") || error.hint.contains("Inspector"),
            "Should prompt user to select a widget in Inspector, got: {:?}",
            error
        );
    }

    // ── Bug 1: Layout tab uses value_id ──────────────────────────────────────

    #[test]
    fn test_switch_to_layout_uses_value_id() {
        let mut state = make_state_with_session();
        // Mark VM as connected.
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .vm_connected = true;

        // Build a tree with value_id set (and object_id = None, as is common).
        let root: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MaterialApp",
            "valueId": "widget-123"
        }))
        .unwrap();
        state.devtools_view_state.inspector.root = Some(root);
        state.devtools_view_state.inspector.selected_index = 0;

        let result = super::super::handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            state.devtools_view_state.layout_explorer.loading,
            "Should trigger layout fetch when value_id is present"
        );
        assert!(
            matches!(
                result.action,
                Some(crate::handler::UpdateAction::FetchLayoutData { .. })
            ),
            "Should return FetchLayoutData action"
        );
    }

    #[test]
    fn test_switch_to_layout_no_value_id_sets_error() {
        let mut state = make_state_with_session();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .vm_connected = true;

        // A node without value_id.
        let root: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Placeholder"
        }))
        .unwrap();
        state.devtools_view_state.inspector.root = Some(root);
        state.devtools_view_state.inspector.selected_index = 0;

        let result = super::super::handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            result.action.is_none(),
            "Should not fetch layout when value_id is missing"
        );
        assert!(
            state.devtools_view_state.layout_explorer.error.is_some(),
            "Should set layout_explorer.error when value_id is missing"
        );
    }
}
