//! DevTools mode message handlers.
//!
//! This module implements all handler functions for DevTools mode messages
//! including panel switching, widget inspector navigation, browser DevTools
//! launching, and debug overlay toggling.
//!
//! Sub-modules:
//! - `inspector`: Widget tree fetch handlers, inspector navigation, and layout data handlers
//! - `performance`: Frame selection, memory sample, and allocation profile handlers

pub mod inspector;
pub(crate) mod network;
pub(crate) mod performance;

pub use inspector::{
    handle_inspector_navigate, handle_layout_data_fetch_failed, handle_layout_data_fetch_timeout,
    handle_layout_data_fetched, handle_widget_tree_fetch_failed, handle_widget_tree_fetch_timeout,
    handle_widget_tree_fetched,
};

pub(crate) use performance::{
    handle_allocation_profile_received, handle_memory_sample_received,
    handle_select_performance_frame,
};

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::DebugOverlayKind;
use crate::session::SessionId;
use crate::state::{AppState, DevToolsError, DevToolsPanel, VmConnectionStatus};

/// Map a raw RPC error string to a user-friendly [`DevToolsError`].
///
/// Matches known patterns (checked most-specific first) to concise messages
/// with actionable hints. Unknown errors get a generic fallback.
pub fn map_rpc_error(raw: &str) -> DevToolsError {
    let lower = raw.to_lowercase();

    // Extension not registered → debug mode hint (before "not found" to avoid matching "isolate not found").
    if lower.contains("extension not registered")
        || lower.contains("method not found")
        || lower.contains("ext.flutter")
    {
        return DevToolsError::new(
            "Widget inspector not available in this mode",
            "Try running in debug mode",
        );
    }
    if lower.contains("isolate not found") || lower.contains("isolate_not_found") {
        return DevToolsError::new(
            "Flutter app isolate not found",
            "The app may have restarted. Press [r] to retry",
        );
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return DevToolsError::new("Request timed out", "Press [r] to retry");
    }
    if lower.contains("connection") || lower.contains("closed") || lower.contains("websocket") {
        return DevToolsError::new(
            "VM Service connection lost",
            "Reconnecting automatically...",
        );
    }
    if lower.contains("no vm")
        || lower.contains("vm service not available")
        || lower.contains("handle unavailable")
    {
        return DevToolsError::new(
            "VM Service not available",
            "Ensure the app is running in debug mode",
        );
    }
    if lower.contains("object group") || lower.contains("group expired") {
        return DevToolsError::new("Widget data expired", "Press [r] to refresh");
    }
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
    DevToolsError::new("DevTools request failed", "Press [r] to retry")
}

/// Map a `default_panel` config string to a [`DevToolsPanel`] variant.
///
/// `"layout"` is mapped to `Inspector` as a backward-compatible fallback for
/// users who had `default_panel = "layout"` in their config file.
pub fn parse_default_panel(panel: &str) -> DevToolsPanel {
    match panel {
        "performance" => DevToolsPanel::Performance,
        "network" | "net" => DevToolsPanel::Network,
        _ => DevToolsPanel::Inspector, // "layout" falls through to Inspector
    }
}

/// Handle entering DevTools mode from Normal mode.
///
/// Sets default panel from config, transitions to DevTools mode, and
/// auto-fetches widget tree if Inspector panel is active and VM connected.
pub fn handle_enter_devtools_mode(state: &mut AppState) -> UpdateResult {
    let default_panel = parse_default_panel(&state.settings.devtools.default_panel);
    state.devtools_view_state.active_panel = default_panel;
    state.enter_devtools_mode();

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

/// Handle exiting DevTools mode — returns to Normal and disposes VM object groups.
pub fn handle_exit_devtools_mode(state: &mut AppState) -> UpdateResult {
    state.exit_devtools_mode();

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

/// Handle switching DevTools sub-panel. Auto-fetches data when switching to
/// Inspector (widget tree).
pub fn handle_switch_panel(state: &mut AppState, panel: DevToolsPanel) -> UpdateResult {
    state.switch_devtools_panel(panel);

    match panel {
        DevToolsPanel::Inspector => {
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
        DevToolsPanel::Performance => {}
        DevToolsPanel::Network => {
            // Start network monitoring if the VM is connected and extensions
            // are not known to be unavailable.
            if let Some(handle) = state.session_manager.selected() {
                let session_id = handle.session.id;
                let vm_connected = handle.session.vm_connected;
                let extensions_unavailable =
                    handle.session.network.extensions_available == Some(false);
                if vm_connected && !extensions_unavailable {
                    return UpdateResult::action(UpdateAction::StartNetworkMonitoring {
                        session_id,
                        handle: None, // hydrated by process.rs
                        poll_interval_ms: 1000,
                    });
                }
            }
        }
    }

    UpdateResult::none()
}

/// Handle opening Flutter DevTools in the system browser.
pub fn handle_open_browser_devtools(state: &AppState) -> UpdateResult {
    let ws_uri = state
        .session_manager
        .selected()
        .and_then(|h| h.session.ws_uri.clone());

    let Some(ws_uri) = ws_uri else {
        tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
        return UpdateResult::none();
    };

    let encoded_uri = percent_encode_uri(&ws_uri);
    let url = build_local_devtools_url(&ws_uri, &encoded_uri);
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

/// Handle VM Service reconnection attempt — updates connection status indicator.
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

/// Build a local DevTools URL from a VM Service WebSocket URI.
fn build_local_devtools_url(ws_uri: &str, encoded_ws_uri: &str) -> String {
    let http_base = if ws_uri.starts_with("wss://") {
        ws_uri.replacen("wss://", "https://", 1)
    } else {
        ws_uri.replacen("ws://", "http://", 1)
    };
    let base = http_base.trim_end_matches("/ws");
    let base = base.trim_end_matches('/');

    format!("{base}/devtools/?uri={encoded_ws_uri}")
}

/// Percent-encode a URI for use as a query parameter (RFC 3986).
fn percent_encode_uri(input: &str) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            // write! to String is infallible
            _ => {
                let _ = write!(encoded, "%{:02X}", byte);
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::UiMode;

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

    #[test]
    fn test_default_panel_maps_to_devtools_panel_enum() {
        assert_eq!(parse_default_panel("inspector"), DevToolsPanel::Inspector);
        // "layout" falls back to Inspector for backward compatibility
        assert_eq!(parse_default_panel("layout"), DevToolsPanel::Inspector);
        assert_eq!(
            parse_default_panel("performance"),
            DevToolsPanel::Performance
        );
        assert_eq!(parse_default_panel("invalid"), DevToolsPanel::Inspector); // fallback
        assert_eq!(parse_default_panel(""), DevToolsPanel::Inspector); // empty fallback
    }

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
    fn test_handle_enter_devtools_mode_layout_panel_falls_back_to_inspector() {
        let mut state = make_state();
        state.settings.devtools.default_panel = "layout".to_string();
        handle_enter_devtools_mode(&mut state);
        // "layout" is no longer a valid panel — falls back to Inspector
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector,
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

    #[test]
    fn test_handle_switch_panel_changes_active_panel() {
        let mut state = make_state();

        handle_switch_panel(&mut state, DevToolsPanel::Performance);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );

        handle_switch_panel(&mut state, DevToolsPanel::Inspector);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector
        );
    }

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
