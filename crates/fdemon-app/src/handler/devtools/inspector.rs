//! Inspector-specific DevTools handlers.
//!
//! Handles widget tree fetch results and inspector navigation for the
//! Widget Inspector panel in DevTools mode.

use crate::handler::UpdateResult;
use crate::message::InspectorNav;
use crate::session::SessionId;
use crate::state::{AppState, DevToolsError};

use super::map_rpc_error;

/// Handle widget tree fetch completion.
///
/// Updates the inspector state with the fetched root node and auto-expands it.
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
pub fn handle_inspector_navigate(state: &mut AppState, nav: InspectorNav) -> UpdateResult {
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
        }
        InspectorNav::Collapse => {
            if let Some(value_id) = value_id {
                inspector.expanded.remove(&value_id);
            }
        }
    }

    UpdateResult::none()
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
}
