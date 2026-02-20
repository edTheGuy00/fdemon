//! DevTools mode message handlers.
//!
//! This module implements all handler functions for DevTools mode messages
//! including panel switching, widget inspector navigation, browser DevTools
//! launching, and debug overlay toggling.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::{DebugOverlayKind, InspectorNav};
use crate::session::SessionId;
use crate::state::{AppState, DevToolsPanel};

/// Handle entering DevTools mode from Normal mode.
///
/// Transitions `ui_mode` to `DevTools`. If the Inspector panel is active,
/// no tree has been loaded yet, and the VM Service is connected,
/// automatically triggers a widget tree fetch.
pub fn handle_enter_devtools_mode(state: &mut AppState) -> UpdateResult {
    state.enter_devtools_mode();

    // If Inspector panel is active and no tree loaded, auto-fetch —
    // but only if the VM is connected (otherwise the hydration layer
    // will silently discard the action and loading will be stuck).
    if state.devtools_view_state.active_panel == DevToolsPanel::Inspector
        && state.devtools_view_state.inspector.root.is_none()
        && !state.devtools_view_state.inspector.loading
    {
        if let Some(handle) = state.session_manager.selected() {
            if handle.session.vm_connected {
                let session_id = handle.session.id;
                state.devtools_view_state.inspector.loading = true;
                return UpdateResult::action(UpdateAction::FetchWidgetTree {
                    session_id,
                    vm_handle: None, // hydrated by process.rs
                });
            }
        }
    }

    UpdateResult::none()
}

/// Handle exiting DevTools mode (return to Normal).
///
/// Transitions `ui_mode` back to `Normal` and, if a VM Service connection is
/// active, returns a [`UpdateAction::DisposeDevToolsGroups`] action to release
/// the `"fdemon-inspector-1"` and `"devtools-layout"` object groups on the
/// Flutter VM. Disposal failures are non-fatal.
pub fn handle_exit_devtools_mode(state: &mut AppState) -> UpdateResult {
    state.exit_devtools_mode();

    // Dispose both VM object groups if the VM is connected.
    // This prevents memory from accumulating on the Flutter VM side
    // when the user exits DevTools without re-entering.
    if let Some(handle) = state.session_manager.selected() {
        if handle.session.vm_connected {
            let session_id = handle.session.id;
            return UpdateResult::action(UpdateAction::DisposeDevToolsGroups {
                session_id,
                vm_handle: None, // hydrated by process.rs
            });
        }
    }

    UpdateResult::none()
}

/// Handle switching DevTools sub-panel.
///
/// When switching to Inspector with no loaded tree, automatically triggers a
/// widget tree fetch. When switching to Layout or Performance, no fetch is
/// triggered here (handled in later tasks).
pub fn handle_switch_panel(state: &mut AppState, panel: DevToolsPanel) -> UpdateResult {
    state.switch_devtools_panel(panel);

    match panel {
        DevToolsPanel::Inspector => {
            // Auto-fetch tree if not already loaded and not loading.
            // Guard on vm_connected to avoid hydration silently dropping the action.
            if state.devtools_view_state.inspector.root.is_none()
                && !state.devtools_view_state.inspector.loading
            {
                if let Some(handle) = state.session_manager.selected() {
                    if handle.session.vm_connected {
                        let session_id = handle.session.id;
                        state.devtools_view_state.inspector.loading = true;
                        return UpdateResult::action(UpdateAction::FetchWidgetTree {
                            session_id,
                            vm_handle: None, // hydrated by process.rs
                        });
                    }
                }
            }
        }
        DevToolsPanel::Layout => {
            // Auto-fetch layout data for the currently selected widget.
            // The getLayoutExplorerNode RPC expects `valueId`, not `objectId`.
            // Guard on vm_connected to avoid hydration silently dropping the action.
            let selected_node_id = {
                let visible = state.devtools_view_state.inspector.visible_nodes();
                visible
                    .get(state.devtools_view_state.inspector.selected_index)
                    .and_then(|(node, _)| node.value_id.clone())
            };

            if let Some(node_id) = selected_node_id {
                if let Some(handle) = state.session_manager.selected() {
                    if handle.session.vm_connected {
                        let session_id = handle.session.id;
                        state.devtools_view_state.layout_explorer.loading = true;
                        return UpdateResult::action(UpdateAction::FetchLayoutData {
                            session_id,
                            node_id,
                            vm_handle: None, // hydrated by process.rs
                        });
                    }
                }
            } else {
                state.devtools_view_state.layout_explorer.error =
                    Some("Select a widget in the Inspector to view its layout".to_string());
            }
        }
        DevToolsPanel::Performance => {
            // Performance data is already streaming via Phase 3 — nothing to fetch.
        }
    }

    UpdateResult::none()
}

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
pub fn handle_widget_tree_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = Some(error);
    }

    UpdateResult::none()
}

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
    }

    UpdateResult::none()
}

/// Handle layout data fetch failure.
pub fn handle_layout_data_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.layout_explorer.loading = false;
        state.devtools_view_state.layout_explorer.error = Some(error);
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

/// Handle opening Flutter DevTools in the system browser.
///
/// Constructs the DevTools URL from the active session's VM Service WebSocket URI
/// and returns an [`UpdateAction::OpenBrowserDevTools`] for the event loop to
/// dispatch. The actual browser launch happens in `actions.rs`, keeping the
/// TEA update path free of I/O side effects.
pub fn handle_open_browser_devtools(state: &AppState) -> UpdateResult {
    let ws_uri = state
        .session_manager
        .selected()
        .and_then(|h| h.session.ws_uri.clone());

    let Some(ws_uri) = ws_uri else {
        tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
        return UpdateResult::none();
    };

    // Derive local DevTools URL from the VM Service WebSocket URI.
    // DDS serves DevTools at the same host/port under /devtools/.
    let encoded_uri = percent_encode_uri(&ws_uri);
    let url = build_local_devtools_url(&ws_uri, &encoded_uri);

    // Get custom browser from settings (empty = system default).
    let browser = state.settings.devtools.browser.clone();

    UpdateResult::action(UpdateAction::OpenBrowserDevTools { url, browser })
}

/// Handle debug overlay toggle result from VM Service.
pub fn handle_debug_overlay_toggled(
    state: &mut AppState,
    extension: DebugOverlayKind,
    enabled: bool,
) -> UpdateResult {
    match extension {
        DebugOverlayKind::RepaintRainbow => {
            state.devtools_view_state.overlay_repaint_rainbow = enabled;
        }
        DebugOverlayKind::DebugPaint => {
            state.devtools_view_state.overlay_debug_paint = enabled;
        }
        DebugOverlayKind::PerformanceOverlay => {
            state.devtools_view_state.overlay_performance = enabled;
        }
    }
    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: URL encoding
// ─────────────────────────────────────────────────────────────────────────────

/// Build a local DevTools URL from a VM Service WebSocket URI.
///
/// Flutter's DDS (Dart Development Service) serves DevTools locally at the same
/// host/port as the VM Service. The auth token path segment must be preserved.
///
/// Conversion: `ws://127.0.0.1:12345/abc=/ws` →
/// `http://127.0.0.1:12345/abc=/devtools/?uri=<percent_encoded_ws_uri>`
fn build_local_devtools_url(ws_uri: &str, encoded_ws_uri: &str) -> String {
    // Convert WebSocket scheme to HTTP scheme.
    let http_base = if ws_uri.starts_with("wss://") {
        ws_uri.replacen("wss://", "https://", 1)
    } else {
        ws_uri.replacen("ws://", "http://", 1)
    };

    // Strip the trailing `/ws` suffix to get the base URL with auth token.
    // e.g. "http://127.0.0.1:12345/abc=/ws" → "http://127.0.0.1:12345/abc="
    let base = http_base.trim_end_matches("/ws");

    // Ensure a trailing slash before appending devtools path.
    let base = base.trim_end_matches('/');

    format!("{base}/devtools/?uri={encoded_ws_uri}")
}

/// Percent-encode a URI string so it can be used as a query parameter value.
///
/// Encodes all characters except ASCII alphanumerics and the unreserved
/// characters `-`, `_`, `.`, `~`. This is a conservative encoding suitable
/// for embedding a full URI inside another URI's query string.
fn percent_encode_uri(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            // Unreserved characters per RFC 3986 §2.3
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            // Everything else gets percent-encoded.
            _ => {
                encoded.push('%');
                encoded.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
                encoded.push(char::from_digit((byte & 0x0F) as u32, 16).unwrap_or('0'));
            }
        }
    }
    encoded
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::UiMode;

    fn make_state() -> AppState {
        AppState::new()
    }

    /// Helper: create a state with one session (no ws_uri set).
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

    // ── enter / exit ─────────────────────────────────────────────────────────

    #[test]
    fn test_handle_enter_devtools_mode_transitions_ui_mode() {
        let mut state = make_state();
        state.ui_mode = UiMode::Normal;

        let result = handle_enter_devtools_mode(&mut state);

        assert_eq!(state.ui_mode, UiMode::DevTools);
        // No session active → no FetchWidgetTree action.
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    #[test]
    fn test_handle_exit_devtools_mode_returns_to_normal() {
        let mut state = make_state();
        state.ui_mode = UiMode::DevTools;

        handle_exit_devtools_mode(&mut state);

        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    // ── panel switching ───────────────────────────────────────────────────────

    #[test]
    fn test_handle_switch_panel_changes_active_panel() {
        let mut state = make_state();

        handle_switch_panel(&mut state, DevToolsPanel::Performance);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );

        handle_switch_panel(&mut state, DevToolsPanel::Layout);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Layout
        );

        handle_switch_panel(&mut state, DevToolsPanel::Inspector);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector
        );
    }

    // ── widget tree ───────────────────────────────────────────────────────────

    fn make_node(description: &str) -> fdemon_core::DiagnosticsNode {
        serde_json::from_value(serde_json::json!({
            "description": description
        }))
        .expect("valid DiagnosticsNode")
    }

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

    // ── overlay toggled ───────────────────────────────────────────────────────

    #[test]
    fn test_handle_debug_overlay_toggled_repaint_rainbow() {
        let mut state = make_state();
        assert!(!state.devtools_view_state.overlay_repaint_rainbow);

        handle_debug_overlay_toggled(&mut state, DebugOverlayKind::RepaintRainbow, true);
        assert!(state.devtools_view_state.overlay_repaint_rainbow);

        handle_debug_overlay_toggled(&mut state, DebugOverlayKind::RepaintRainbow, false);
        assert!(!state.devtools_view_state.overlay_repaint_rainbow);
    }

    #[test]
    fn test_handle_debug_overlay_toggled_debug_paint() {
        let mut state = make_state();
        handle_debug_overlay_toggled(&mut state, DebugOverlayKind::DebugPaint, true);
        assert!(state.devtools_view_state.overlay_debug_paint);
    }

    #[test]
    fn test_handle_debug_overlay_toggled_performance_overlay() {
        let mut state = make_state();
        handle_debug_overlay_toggled(&mut state, DebugOverlayKind::PerformanceOverlay, true);
        assert!(state.devtools_view_state.overlay_performance);
    }

    // ── percent encoding ──────────────────────────────────────────────────────

    #[test]
    fn test_percent_encode_uri_encodes_colons_and_slashes() {
        let input = "ws://127.0.0.1:8181/ws";
        let encoded = percent_encode_uri(input);
        // Colons, slashes, and dots must be encoded.
        assert!(!encoded.contains(':'));
        assert!(!encoded.contains('/'));
        // Dots are unreserved, but IP address digits and letters pass through.
        assert!(encoded.contains('.'));
    }

    #[test]
    fn test_percent_encode_uri_unreserved_chars_pass_through() {
        let input = "abc-def_ghi.jkl~mno";
        let encoded = percent_encode_uri(input);
        assert_eq!(encoded, input);
    }

    #[test]
    fn test_percent_encode_uri_empty_string() {
        assert_eq!(percent_encode_uri(""), "");
    }

    #[test]
    fn test_percent_encode_uri_space_becomes_percent_20() {
        let input = "hello world";
        let encoded = percent_encode_uri(input);
        assert_eq!(encoded, "hello%20world");
    }

    // ── open browser devtools ─────────────────────────────────────────────────

    #[test]
    fn test_open_browser_devtools_returns_action() {
        let mut state = make_state_with_session();
        // Set ws_uri on the selected session.
        state.session_manager.selected_mut().unwrap().session.ws_uri =
            Some("ws://127.0.0.1:12345/abc=/ws".to_string());

        let result = handle_open_browser_devtools(&state);

        assert!(result.action.is_some(), "Expected an action to be returned");

        if let Some(UpdateAction::OpenBrowserDevTools { url, browser: _ }) = result.action {
            // URL should use local DDS-served DevTools, not the hosted one.
            assert!(
                url.starts_with("http://127.0.0.1:"),
                "URL should use local HTTP scheme (got: {url})"
            );
            assert!(
                url.contains("/devtools/"),
                "URL should contain /devtools/ path (got: {url})"
            );
            // The ws:// scheme characters must be percent-encoded in the query param.
            let lower = url.to_lowercase();
            assert!(
                lower.contains("ws%3a%2f%2f"),
                "Encoded URI must contain percent-encoded ws:// scheme (got: {url})"
            );
        } else {
            panic!("Expected UpdateAction::OpenBrowserDevTools");
        }
    }

    #[test]
    fn test_open_browser_devtools_wss_uri_uses_https() {
        let mut state = make_state_with_session();
        state.session_manager.selected_mut().unwrap().session.ws_uri =
            Some("wss://127.0.0.1:9999/auth=/ws".to_string());

        let result = handle_open_browser_devtools(&state);

        if let Some(UpdateAction::OpenBrowserDevTools { url, .. }) = result.action {
            assert!(
                url.starts_with("https://127.0.0.1:"),
                "wss:// should become https:// (got: {url})"
            );
            assert!(
                url.contains("/auth=/devtools/"),
                "URL should preserve auth token and contain /devtools/ (got: {url})"
            );
        } else {
            panic!("Expected OpenBrowserDevTools action");
        }
    }

    #[test]
    fn test_build_local_devtools_url_preserves_auth_token() {
        let ws_uri = "ws://127.0.0.1:12345/abc=/ws";
        let encoded = percent_encode_uri(ws_uri);
        let url = build_local_devtools_url(ws_uri, &encoded);

        assert_eq!(
            url.split("?uri=").next().unwrap(),
            "http://127.0.0.1:12345/abc=/devtools/",
            "Should preserve auth token path and append /devtools/: {url}"
        );
    }

    #[test]
    fn test_build_local_devtools_url_no_auth_token() {
        let ws_uri = "ws://127.0.0.1:12345/ws";
        let encoded = percent_encode_uri(ws_uri);
        let url = build_local_devtools_url(ws_uri, &encoded);

        assert_eq!(
            url.split("?uri=").next().unwrap(),
            "http://127.0.0.1:12345/devtools/",
            "Should work without auth token path: {url}"
        );
    }

    // ── Bug 1: Layout tab uses value_id ─────────────────────────────────────

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

        let result = handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            state.devtools_view_state.layout_explorer.loading,
            "Should trigger layout fetch when value_id is present"
        );
        assert!(
            matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
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

        let result = handle_switch_panel(&mut state, DevToolsPanel::Layout);

        assert!(
            result.action.is_none(),
            "Should not fetch layout when value_id is missing"
        );
        assert!(
            state.devtools_view_state.layout_explorer.error.is_some(),
            "Should set layout_explorer.error when value_id is missing"
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

    #[test]
    fn test_open_browser_devtools_no_ws_uri_returns_none() {
        // A session exists but has no ws_uri set.
        let state = make_state_with_session();

        let result = handle_open_browser_devtools(&state);

        assert!(
            result.action.is_none(),
            "Expected no action when ws_uri is not set"
        );
    }
}
