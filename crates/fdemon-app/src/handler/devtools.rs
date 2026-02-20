//! DevTools mode message handlers.
//!
//! This module implements all handler functions for DevTools mode messages
//! including panel switching, widget inspector navigation, browser DevTools
//! launching, and debug overlay toggling.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::{DebugOverlayKind, InspectorNav};
use crate::session::SessionId;
use crate::state::{AppState, DevToolsError, DevToolsPanel, VmConnectionStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Error classification
// ─────────────────────────────────────────────────────────────────────────────

/// Map a raw RPC error string to a user-friendly [`DevToolsError`].
///
/// Flutter VM Service errors often contain technical strings like
/// `"Method not found: ext.flutter.inspector.getRootWidgetTree"` or
/// `"Isolate not found"` that are not useful to the end user. This function
/// maps known patterns to concise messages with actionable hints.
///
/// The matching order is important: more specific patterns are checked first.
///
/// # Mapping table
///
/// | Raw error pattern | User message | Hint |
/// |---|---|---|
/// | `"extension not registered"` / `"method not found"` | "Widget inspector not available in this mode" | "Try running in debug mode" |
/// | `"isolate not found"` | "Flutter app isolate not found" | "The app may have restarted. Press [r] to retry" |
/// | `"timed out"` | "Request timed out" | "Press [r] to retry" |
/// | `"connection"` / `"closed"` | "VM Service connection lost" | "Reconnecting automatically..." |
/// | `"no vm"` / `"uri"` / `"not available"` | "VM Service not available" | "Ensure the app is running in debug mode" |
/// | `"object group"` / `"group expired"` | "Widget data expired" | "Press [r] to refresh" |
/// | `"parse"` / `"unexpected"` / `"invalid json"` | "Unexpected response from Flutter" | "Press [r] to retry, or press [b] for browser DevTools" |
/// | Anything else | "DevTools request failed" | "Press [r] to retry" |
pub fn map_rpc_error(raw: &str) -> DevToolsError {
    let lower = raw.to_lowercase();

    // Extension / service extension not registered → debug mode hint.
    // Check this before "not found" to avoid matching "isolate not found".
    if lower.contains("extension not registered")
        || lower.contains("method not found")
        || lower.contains("ext.flutter")
    {
        return DevToolsError::new(
            "Widget inspector not available in this mode",
            "Try running in debug mode",
        );
    }

    // Isolate not found → app restart hint.
    if lower.contains("isolate not found") || lower.contains("isolate_not_found") {
        return DevToolsError::new(
            "Flutter app isolate not found",
            "The app may have restarted. Press [r] to retry",
        );
    }

    // Timeout → retry hint.
    if lower.contains("timed out") || lower.contains("timeout") {
        return DevToolsError::new("Request timed out", "Press [r] to retry");
    }

    // Connection lost.
    if lower.contains("connection") || lower.contains("closed") || lower.contains("websocket") {
        return DevToolsError::new(
            "VM Service connection lost",
            "Reconnecting automatically...",
        );
    }

    // No VM URI / service not available.
    if lower.contains("no vm")
        || lower.contains("vm service not available")
        || lower.contains("handle unavailable")
    {
        return DevToolsError::new(
            "VM Service not available",
            "Ensure the app is running in debug mode",
        );
    }

    // Object group expired.
    if lower.contains("object group") || lower.contains("group expired") {
        return DevToolsError::new("Widget data expired", "Press [r] to refresh");
    }

    // Parse / protocol errors.
    if lower.contains("parse")
        || lower.contains("unexpected")
        || lower.contains("invalid json")
        || lower.contains("deserialization")
    {
        return DevToolsError::new(
            "Unexpected response from Flutter",
            "Press [r] to retry, or press [b] for browser DevTools",
        );
    }

    // Fallback: show a generic message so the raw error never surfaces.
    DevToolsError::new("DevTools request failed", "Press [r] to retry")
}

/// Map a `default_panel` config string to a [`DevToolsPanel`] enum variant.
///
/// `"layout"` → [`DevToolsPanel::Layout`],
/// `"performance"` → [`DevToolsPanel::Performance`],
/// anything else (including `"inspector"`) → [`DevToolsPanel::Inspector`].
pub fn parse_default_panel(panel: &str) -> DevToolsPanel {
    match panel {
        "layout" => DevToolsPanel::Layout,
        "performance" => DevToolsPanel::Performance,
        _ => DevToolsPanel::Inspector,
    }
}

/// Handle entering DevTools mode from Normal mode.
///
/// Transitions `ui_mode` to `DevTools`. The initial panel is determined by
/// `state.settings.devtools.default_panel`. If the Inspector panel is active,
/// no tree has been loaded yet, and the VM Service is connected,
/// automatically triggers a widget tree fetch.
pub fn handle_enter_devtools_mode(state: &mut AppState) -> UpdateResult {
    // Apply the configured default panel before entering mode.
    let default_panel = parse_default_panel(&state.settings.devtools.default_panel.clone());
    state.devtools_view_state.active_panel = default_panel;

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
                    tree_max_depth: state.settings.devtools.tree_max_depth,
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
                            tree_max_depth: state.settings.devtools.tree_max_depth,
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
                // Staleness check: skip the fetch when the selected node has not changed
                // since the last layout fetch. This prevents redundant RPC calls when the
                // user rapidly switches between Inspector and Layout panels.
                let already_fetched = state
                    .devtools_view_state
                    .layout_explorer
                    .last_fetched_node_id
                    .as_deref()
                    == Some(node_id.as_str());

                if already_fetched && !state.devtools_view_state.layout_explorer.loading {
                    // Data is still fresh for the selected node — no fetch needed.
                } else if let Some(handle) = state.session_manager.selected() {
                    if handle.session.vm_connected {
                        let session_id = handle.session.id;
                        state.devtools_view_state.layout_explorer.loading = true;
                        // Track which node we are fetching so we can record it on success.
                        state.devtools_view_state.layout_explorer.pending_node_id =
                            Some(node_id.clone());
                        return UpdateResult::action(UpdateAction::FetchLayoutData {
                            session_id,
                            node_id,
                            vm_handle: None, // hydrated by process.rs
                        });
                    }
                }
            } else {
                state.devtools_view_state.layout_explorer.error = Some(DevToolsError::new(
                    "No widget selected",
                    "Select a widget in the Inspector panel first",
                ));
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

/// Handle VM Service reconnection attempt (Phase 5, Task 02).
///
/// Updates the `DevToolsViewState::connection_status` field to
/// `Reconnecting { attempt, max_attempts }` so the tab bar indicator can
/// show "Reconnecting (attempt/max_attempts)".
pub fn handle_vm_service_reconnecting(
    state: &mut AppState,
    session_id: SessionId,
    attempt: u32,
    max_attempts: u32,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.connection_status = VmConnectionStatus::Reconnecting {
            attempt,
            max_attempts,
        };
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

/// Handle layout data fetch timeout (Phase 5, Task 02).
///
/// Sets `layout_explorer.loading = false` and stores an error message with a
/// retry hint, then marks `connection_status` as `TimedOut`.
pub fn handle_layout_data_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
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
///
/// Uppercase hex digits are used per RFC 3986 §2.1 recommendation
/// (e.g., `%3A` not `%3a`).
fn percent_encode_uri(input: &str) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            // Unreserved characters per RFC 3986 §2.3
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            // Everything else gets percent-encoded with uppercase hex (RFC 3986 §2.1).
            _ => {
                write!(encoded, "%{:02X}", byte).unwrap();
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

    // ── parse_default_panel ───────────────────────────────────────────────────

    #[test]
    fn test_default_panel_maps_to_devtools_panel_enum() {
        assert_eq!(parse_default_panel("inspector"), DevToolsPanel::Inspector);
        assert_eq!(parse_default_panel("layout"), DevToolsPanel::Layout);
        assert_eq!(
            parse_default_panel("performance"),
            DevToolsPanel::Performance
        );
        assert_eq!(parse_default_panel("invalid"), DevToolsPanel::Inspector); // fallback
        assert_eq!(parse_default_panel(""), DevToolsPanel::Inspector); // empty fallback
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
    fn test_handle_enter_devtools_mode_uses_default_panel_config() {
        let mut state = make_state();
        state.settings.devtools.default_panel = "performance".to_string();
        handle_enter_devtools_mode(&mut state);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance,
            "Should use default_panel config to set initial panel"
        );
    }

    #[test]
    fn test_handle_enter_devtools_mode_layout_panel() {
        let mut state = make_state();
        state.settings.devtools.default_panel = "layout".to_string();
        handle_enter_devtools_mode(&mut state);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Layout,
        );
    }

    #[test]
    fn test_handle_enter_devtools_mode_invalid_panel_defaults_inspector() {
        let mut state = make_state();
        state.settings.devtools.default_panel = "unknown_panel".to_string();
        handle_enter_devtools_mode(&mut state);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector,
        );
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
    fn test_percent_encode_uri_uppercase_hex() {
        let uri = "ws://127.0.0.1:12345/abc=/ws";
        let encoded = percent_encode_uri(uri);
        // RFC 3986 §2.1 recommends uppercase hex digits.
        assert!(
            encoded.contains("%3A"),
            "colon should encode as %3A (got: {encoded})"
        );
        assert!(
            encoded.contains("%2F"),
            "slash should encode as %2F (got: {encoded})"
        );
        assert!(
            !encoded.contains("%3a"),
            "no lowercase %3a allowed (got: {encoded})"
        );
        assert!(
            !encoded.contains("%2f"),
            "no lowercase %2f allowed (got: {encoded})"
        );
    }

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
            // The ws:// scheme characters must be percent-encoded with uppercase hex (RFC 3986 §2.1).
            assert!(
                url.contains("ws%3A%2F%2F"),
                "Encoded URI must contain uppercase percent-encoded ws:// scheme (got: {url})"
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

        let result = handle_switch_panel(&mut state, DevToolsPanel::Layout);

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

        let result = handle_switch_panel(&mut state, DevToolsPanel::Layout);

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

        let result = handle_switch_panel(&mut state, DevToolsPanel::Layout);

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
        handle_switch_panel(&mut state, DevToolsPanel::Layout);

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
}
