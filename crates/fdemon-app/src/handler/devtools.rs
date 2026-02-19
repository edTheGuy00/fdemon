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
pub fn handle_exit_devtools_mode(state: &mut AppState) -> UpdateResult {
    state.exit_devtools_mode();
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
            // Guard on vm_connected to avoid hydration silently dropping the action.
            let selected_node_id = {
                let visible = state.devtools_view_state.inspector.visible_nodes();
                visible
                    .get(state.devtools_view_state.inspector.selected_index)
                    .and_then(|(node, _)| node.object_id.clone())
            };

            if let (Some(node_id), Some(handle)) =
                (selected_node_id, state.session_manager.selected())
            {
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
/// and opens it with the system default browser (or a custom one from settings).
pub fn handle_open_browser_devtools(state: &AppState) -> UpdateResult {
    let ws_uri = state
        .session_manager
        .selected()
        .and_then(|h| h.session.ws_uri.clone());

    let Some(ws_uri) = ws_uri else {
        tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
        return UpdateResult::none();
    };

    // Percent-encode the ws_uri for use as a query parameter value.
    let encoded_uri = percent_encode_uri(&ws_uri);
    let url = format!("https://devtools.flutter.dev/?uri={encoded_uri}");

    // Get custom browser from settings (empty = system default).
    let browser = &state.settings.devtools.browser;

    if let Err(e) = open_url_in_browser(&url, browser) {
        tracing::error!("Failed to open browser DevTools: {e}");
    }

    UpdateResult::none()
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
// Helper: Browser launcher
// ─────────────────────────────────────────────────────────────────────────────

/// Open a URL in the system browser (cross-platform, fire-and-forget).
///
/// If `browser` is non-empty, uses it as the browser command.
/// Otherwise uses the platform-default browser opener.
fn open_url_in_browser(url: &str, browser: &str) -> std::io::Result<()> {
    use std::process::Command;

    if !browser.is_empty() {
        // Custom browser specified in settings.
        Command::new(browser).arg(url).spawn()?;
        return Ok(());
    }

    // Platform-default browser.
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
    }

    Ok(())
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
}
