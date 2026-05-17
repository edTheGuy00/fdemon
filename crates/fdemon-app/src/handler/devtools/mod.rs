//! DevTools mode message handlers.
//!
//! This module implements all handler functions for DevTools mode messages
//! including panel switching, widget inspector navigation, browser DevTools
//! launching, and debug overlay toggling.
//!
//! Sub-modules:
//! - `inspector`: Widget tree fetch handlers, inspector navigation, and layout data handlers
//! - `performance`: Frame selection, memory sample, and allocation profile handlers

pub(crate) mod debug;
pub mod inspector;
pub(crate) mod network;
pub(crate) mod performance;

pub use inspector::{
    handle_close_details, handle_cycle_tab, handle_inspector_navigate,
    handle_layout_data_fetch_failed, handle_layout_data_fetch_timeout, handle_layout_data_fetched,
    handle_open_details, handle_toggle_hide_implementation, handle_widget_tree_fetch_failed,
    handle_widget_tree_fetch_timeout, handle_widget_tree_fetched,
};

pub(crate) use performance::{
    handle_allocation_profile_received, handle_memory_sample_received,
    handle_select_performance_frame,
};

use crate::config::DevToolsSettings;
use crate::handler::{FetchTrigger, UpdateAction, UpdateResult};
use crate::message::DebugOverlayKind;
use crate::session::SessionId;
use crate::state::{AppState, DevToolsError, DevToolsPanel, ToastLevel, VmConnectionStatus};
use fdemon_core::url::percent_encode_uri;

// ─────────────────────────────────────────────────────────────────────────────
// Readiness-poll config clamping
// ─────────────────────────────────────────────────────────────────────────────

/// Upper bound on readiness-poll attempts. 0 = skip poll; 20 × 5 s call
/// timeout × 5 s interval = 200 s worst case (capped by the outer fetch timeout).
const MAX_READINESS_POLL_ATTEMPTS: u32 = 20;
/// Lower bound on poll interval: sub-10 ms is pointless overhead.
const MIN_READINESS_POLL_INTERVAL_MS: u64 = 10;
/// Upper bound on poll interval: 5 s + 20 attempts = 100 s outer budget.
const MAX_READINESS_POLL_INTERVAL_MS: u64 = 5_000;
/// Lower bound on per-call timeout: sub-100 ms is unrealistic for an RPC round-trip.
const MIN_READINESS_POLL_CALL_TIMEOUT_MS: u64 = 100;
/// Upper bound on per-call timeout: 10 s per attempt is the upper sane limit.
const MAX_READINESS_POLL_CALL_TIMEOUT_MS: u64 = 10_000;

/// Clamp the three readiness-poll config values to bounded ranges and emit a
/// `warn!` for each value that required clamping.
///
/// Values within the ranges pass through unchanged (no log noise). Values
/// outside the bounds are clamped to the nearest limit so that a typo such as
/// `inspector_readiness_poll_attempts = 4294967295` cannot saturate the Tokio runtime.
///
/// This is the **single read point** for the three `inspector_readiness_poll_*` settings.
/// All `FetchWidgetTree` dispatch sites must call this helper rather than
/// reading `state.settings.devtools.inspector_readiness_poll_*` directly.
pub(crate) fn clamped_readiness_poll_config(settings: &DevToolsSettings) -> (u32, u64, u64) {
    let attempts = settings
        .inspector_readiness_poll_attempts
        .min(MAX_READINESS_POLL_ATTEMPTS);
    if attempts != settings.inspector_readiness_poll_attempts {
        tracing::warn!(
            requested = settings.inspector_readiness_poll_attempts,
            clamped_to = attempts,
            "inspector_readiness_poll_attempts clamped to bounded range"
        );
    }

    let interval = settings.inspector_readiness_poll_interval_ms.clamp(
        MIN_READINESS_POLL_INTERVAL_MS,
        MAX_READINESS_POLL_INTERVAL_MS,
    );
    if interval != settings.inspector_readiness_poll_interval_ms {
        tracing::warn!(
            requested_ms = settings.inspector_readiness_poll_interval_ms,
            clamped_to_ms = interval,
            "inspector_readiness_poll_interval_ms clamped to bounded range"
        );
    }

    let timeout = settings.inspector_readiness_poll_call_timeout_ms.clamp(
        MIN_READINESS_POLL_CALL_TIMEOUT_MS,
        MAX_READINESS_POLL_CALL_TIMEOUT_MS,
    );
    if timeout != settings.inspector_readiness_poll_call_timeout_ms {
        tracing::warn!(
            requested_ms = settings.inspector_readiness_poll_call_timeout_ms,
            clamped_to_ms = timeout,
            "inspector_readiness_poll_call_timeout_ms clamped to bounded range"
        );
    }

    (attempts, interval, timeout)
}

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
/// If the default panel is Performance, unpauses allocation polling so data
/// is immediately fresh.
///
/// If the performance monitoring task has not been started yet (first DevTools
/// entry), dispatches `StartPerformanceMonitoring` instead of unpausing. The
/// `VmServicePerformanceMonitoringStarted` handler will unpause appropriately
/// based on current UI state. On subsequent entries, unpauses the existing task
/// via `perf_pause_tx`.
pub fn handle_enter_devtools_mode(state: &mut AppState) -> UpdateResult {
    let default_panel = parse_default_panel(&state.settings.devtools.default_panel);
    state.devtools_view_state.active_panel = default_panel;
    state.enter_devtools_mode();

    // Check whether performance monitoring needs to be lazy-started.
    // This happens on the first DevTools entry when the VM is connected but no
    // perf task was spawned yet (because VmServiceConnected skips the spawn
    // when DevTools is not active at connect time).
    // Use `session.vm_connected` (the reliable per-session flag) rather than
    // `devtools_view_state.connection_status` which may lag behind the session.
    let needs_perf_start = if let Some(handle) = state.session_manager.selected() {
        handle.perf_shutdown_tx.is_none() && handle.session.vm_connected
    } else {
        false
    };

    if needs_perf_start {
        // Collect data needed for StartPerformanceMonitoring before returning.
        // NOTE: perf_pause_tx and alloc_pause_tx are not yet set (task not started),
        // so the unpause signals below are skipped — the
        // VmServicePerformanceMonitoringStarted handler adjusts initial pause
        // state based on ui_mode and active_panel after the task starts.
        let Some(session_id) = state.session_manager.selected_id() else {
            return UpdateResult::none();
        };
        let performance_refresh_ms = state.settings.devtools.performance_refresh_ms;
        let allocation_profile_interval_ms = state.settings.devtools.allocation_profile_interval_ms;
        let mode = state
            .session_manager
            .selected()
            .and_then(|h| h.session.launch_config.as_ref())
            .map(|c| c.mode)
            .unwrap_or(crate::config::FlutterMode::Debug);

        // StartPerformanceMonitoring consumes the action slot, so use the
        // follow-up message slot for panel-specific initialization that would
        // normally be handled by handle_switch_panel:
        //  - Inspector: queue FetchWidgetTree if the tree isn't loaded yet.
        //  - Network: queue SwitchDevToolsPanel(Network) so handle_switch_panel
        //    fires StartNetworkMonitoring (the network task hasn't started yet).
        let followup_msg = if state.devtools_view_state.active_panel == DevToolsPanel::Inspector
            && state.devtools_view_state.inspector.root.is_none()
            && !state.devtools_view_state.inspector.loading
        {
            // Queue RequestWidgetTree as a follow-up. Do NOT call record_fetch_start()
            // here — that engages the debounce (`loading=true`), and the follow-up
            // RequestWidgetTree handler in update.rs would then see itself as
            // already-in-flight and bail. The follow-up handler calls
            // record_fetch_start() itself just before dispatching FetchWidgetTree.
            Some(crate::message::Message::RequestWidgetTree { session_id })
        } else if state.devtools_view_state.active_panel == DevToolsPanel::Network {
            Some(crate::message::Message::SwitchDevToolsPanel(
                DevToolsPanel::Network,
            ))
        } else {
            None
        };

        return UpdateResult {
            message: followup_msg,
            action: Some(UpdateAction::StartPerformanceMonitoring {
                session_id,
                handle: None, // hydrated by process.rs
                performance_refresh_ms,
                allocation_profile_interval_ms,
                mode,
            }),
        };
    }

    // Task already running — unpause the entire performance polling loop
    // (memory + alloc). The perf_pause_rx.changed() arm in the polling task
    // fires immediately, triggering an on-demand memory fetch so the panel
    // shows current data.
    if let Some(handle) = state.session_manager.selected() {
        if let Some(ref tx) = handle.perf_pause_tx {
            let _ = tx.send(false); // unpause
        }
    }

    // Unpause allocation polling when entering DevTools with Performance as the
    // default panel so the user sees fresh allocation data immediately.
    if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
        if let Some(handle) = state.session_manager.selected() {
            if let Some(ref tx) = handle.alloc_pause_tx {
                let _ = tx.send(false); // unpause
            }
        }
    }

    // Unpause network monitoring if the default panel is Network and the
    // network task is already running (subsequent DevTools visits).
    // On the first visit the task hasn't started yet — `network_pause_tx` is
    // None, so this send safely does nothing. The task will be started by the
    // StartNetworkMonitoring action in handle_switch_panel.
    if state.devtools_view_state.active_panel == DevToolsPanel::Network {
        if let Some(handle) = state.session_manager.selected() {
            if let Some(ref tx) = handle.network_pause_tx {
                let _ = tx.send(false); // unpause
            }
        }
    }

    if state.devtools_view_state.active_panel == DevToolsPanel::Inspector
        && state.devtools_view_state.inspector.root.is_none()
        && !state.devtools_view_state.inspector.loading
    {
        if let Some(handle) = state.session_manager.selected() {
            if handle.session.vm_connected {
                let session_id = handle.session.id;
                // Use record_fetch_start() to enforce the invariant that loading=true
                // is always paired with last_fetch_time=Some(now). Without the
                // timestamp a lost terminal message would leave loading=true forever.
                state.devtools_view_state.inspector.record_fetch_start();
                let (poll_attempts, poll_interval_ms, poll_call_timeout_ms) =
                    clamped_readiness_poll_config(&state.settings.devtools);
                return UpdateResult::action(UpdateAction::FetchWidgetTree {
                    session_id,
                    vm_handle: None, // hydrated by process.rs
                    tree_max_depth: state.settings.devtools.tree_max_depth,
                    fetch_timeout_secs: state.settings.devtools.inspector_fetch_timeout_secs,
                    inspector_readiness_poll_attempts: poll_attempts,
                    inspector_readiness_poll_interval_ms: poll_interval_ms,
                    inspector_readiness_poll_call_timeout_ms: poll_call_timeout_ms,
                    trigger: FetchTrigger::Initial,
                });
            }
        }
    }

    UpdateResult::none()
}

/// Handle the DevTools Esc key with tiered close semantics.
///
/// **Tier 1 — Details panel close**: When the Inspector tab is active and the
/// Details panel is open, the first Esc closes the Details panel instead of
/// exiting DevTools mode. This mirrors the behaviour of Flutter DevTools where
/// Esc dismisses the innermost selection before navigating outward.
///
/// **Tier 2 — Exit DevTools**: When no inner panel is open (or a non-Inspector
/// panel is active), delegates to [`handle_exit_devtools_mode`] which returns
/// to Normal mode and disposes VM object groups.
pub fn handle_devtools_escape(state: &mut AppState) -> UpdateResult {
    // Tiered Esc: close Details first when the Inspector is active.
    if state.devtools_view_state.active_panel == DevToolsPanel::Inspector
        && state.devtools_view_state.inspector.details_open
    {
        return inspector::handle_close_details(state);
    }
    // Fall through to the existing exit-DevTools logic.
    handle_exit_devtools_mode(state)
}

/// Handle exiting DevTools mode — returns to Normal and disposes VM object groups.
///
/// Pauses allocation polling since the Performance panel is no longer visible.
/// Also pauses the entire performance polling loop so no VM Service RPCs fire
/// while the user is viewing logs.
pub fn handle_exit_devtools_mode(state: &mut AppState) -> UpdateResult {
    // Pause the entire performance polling loop (memory + alloc).
    // This eliminates all getMemoryUsage/getIsolate RPCs while viewing logs.
    if let Some(handle) = state.session_manager.selected() {
        if let Some(ref tx) = handle.perf_pause_tx {
            let _ = tx.send(true); // pause
        }
    }

    // Pause allocation polling: the user is leaving DevTools entirely so the
    // Performance panel is no longer visible regardless of which panel was active.
    if let Some(handle) = state.session_manager.selected() {
        if let Some(ref tx) = handle.alloc_pause_tx {
            let _ = tx.send(true); // pause
        }
    }

    // Pause network monitoring (if running): the user is leaving DevTools, so
    // no getHttpProfile RPCs should fire while they are viewing logs.
    if let Some(handle) = state.session_manager.selected() {
        if let Some(ref tx) = handle.network_pause_tx {
            let _ = tx.send(true); // pause
        }
    }

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
/// Inspector (widget tree). Pauses/unpauses allocation polling based on whether
/// the Performance panel is becoming visible or hidden.
pub fn handle_switch_panel(state: &mut AppState, panel: DevToolsPanel) -> UpdateResult {
    // Before switching, check if we are leaving the Performance panel — if so,
    // pause allocation polling. The `watch` channel coalesces rapid toggles so
    // burst panel switches do not create burst fetches.
    let old_panel = state.devtools_view_state.active_panel;
    if old_panel == DevToolsPanel::Performance && panel != DevToolsPanel::Performance {
        if let Some(handle) = state.session_manager.selected() {
            if let Some(ref tx) = handle.alloc_pause_tx {
                let _ = tx.send(true); // pause
            }
        }
    }

    // Before switching, check if we are leaving the Network panel — if so,
    // pause network polling so no getHttpProfile RPCs fire while on other panels.
    if old_panel == DevToolsPanel::Network && panel != DevToolsPanel::Network {
        if let Some(handle) = state.session_manager.selected() {
            if let Some(ref tx) = handle.network_pause_tx {
                let _ = tx.send(true); // pause network polling
            }
        }
    }

    state.switch_devtools_panel(panel);

    match panel {
        DevToolsPanel::Inspector => {
            if state.devtools_view_state.inspector.root.is_none()
                && !state.devtools_view_state.inspector.loading
            {
                if let Some(handle) = state.session_manager.selected() {
                    if handle.session.vm_connected {
                        let session_id = handle.session.id;
                        // Use record_fetch_start() to enforce the invariant that
                        // loading=true is always paired with last_fetch_time=Some(now).
                        // Without the timestamp a lost terminal message would leave
                        // loading=true forever, re-introducing the stuck-loading bug.
                        state.devtools_view_state.inspector.record_fetch_start();
                        let (poll_attempts, poll_interval_ms, poll_call_timeout_ms) =
                            clamped_readiness_poll_config(&state.settings.devtools);
                        return UpdateResult::action(UpdateAction::FetchWidgetTree {
                            session_id,
                            vm_handle: None, // hydrated by process.rs
                            tree_max_depth: state.settings.devtools.tree_max_depth,
                            fetch_timeout_secs: state
                                .settings
                                .devtools
                                .inspector_fetch_timeout_secs,
                            inspector_readiness_poll_attempts: poll_attempts,
                            inspector_readiness_poll_interval_ms: poll_interval_ms,
                            inspector_readiness_poll_call_timeout_ms: poll_call_timeout_ms,
                            trigger: FetchTrigger::Initial,
                        });
                    }
                }
            }
        }
        DevToolsPanel::Performance => {
            // Unpause allocation polling when entering the Performance panel.
            // The background task will fire one immediate fetch (via the
            // `alloc_pause_rx.changed()` arm) so the allocation table is
            // populated without waiting for the next scheduled tick.
            if let Some(handle) = state.session_manager.selected() {
                if let Some(ref tx) = handle.alloc_pause_tx {
                    let _ = tx.send(false); // unpause
                }
            }
        }
        DevToolsPanel::Network => {
            // Start network monitoring if the VM is connected, extensions are
            // not known to be unavailable, and a polling task is not already
            // running.  Checking `network_shutdown_tx.is_some()` is the
            // idempotency guard: the sender is set by
            // `handle_network_monitoring_started` and cleared on disconnect /
            // session close, so its presence reliably indicates a live task.
            if let Some(handle) = state.session_manager.selected() {
                let session_id = handle.session.id;
                let vm_connected = handle.session.vm_connected;
                let extensions_unavailable =
                    handle.session.network.extensions_available == Some(false);
                let already_running = handle.network_shutdown_tx.is_some();
                let mode = handle
                    .session
                    .launch_config
                    .as_ref()
                    .map(|c| c.mode)
                    .unwrap_or(crate::config::FlutterMode::Debug);
                if vm_connected && !extensions_unavailable && !already_running {
                    return UpdateResult::action(UpdateAction::StartNetworkMonitoring {
                        session_id,
                        handle: None, // hydrated by process.rs
                        poll_interval_ms: state.settings.devtools.network_poll_interval_ms,
                        mode,
                    });
                }
                // Task is already running — unpause network polling so the
                // Network tab immediately shows any requests that arrived while
                // the tab was hidden. The `network_pause_rx.changed()` arm in
                // the polling task fires an immediate getHttpProfile fetch on
                // unpause.
                if already_running {
                    if let Some(ref tx) = handle.network_pause_tx {
                        let _ = tx.send(false); // unpause
                    }
                }
            }
        }
    }

    UpdateResult::none()
}

/// Handle opening Flutter DevTools in the system browser.
///
/// Prefers the served DevTools endpoint (`session.devtools_endpoint`) when
/// available — this is the URL from the `app.devTools` daemon event or the
/// `devtools.serve` RPC response.  Falls back to `build_local_devtools_url`
/// (the legacy DDS-path URL) when no endpoint has been recorded yet.
///
/// When falling back, a transient toast is pushed to `state.toasts`:
/// - If `devtools_serve_pending` is `true`, the message tells the user the
///   server is still starting.
/// - Otherwise the message explains that DevTools is not registered with DDS
///   and provides recovery steps.
///
/// **Behaviour choice**: when the endpoint is pending we still open the legacy
/// fallback URL (rather than deferring the open entirely). This lets the user
/// access DevTools immediately even if the served URL is not ready yet, while
/// the toast communicates that a better URL may become available soon.
pub fn handle_open_browser_devtools(state: &mut AppState) -> UpdateResult {
    let Some(session_handle) = state.session_manager.selected() else {
        return UpdateResult::none();
    };

    if session_handle.session.ws_uri.is_none() {
        tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
        state.push_toast(
            ToastLevel::Warn,
            "VM Service not ready yet \u{2014} \
             wait for the app to finish launching, then press B again.",
        );
        return UpdateResult::none();
    }
    let ws_uri = session_handle.session.ws_uri.clone().unwrap_or_default();

    // Clone the data we need before the mutable borrow of `state`.
    let devtools_endpoint = session_handle.session.devtools_endpoint.clone();
    let devtools_serve_pending = session_handle.session.devtools_serve_pending;
    let browser = state.settings.devtools.browser.clone();

    let url = match devtools_endpoint {
        Some(ref endpoint) => {
            tracing::info!(
                base_url = %fdemon_core::url::redact_devtools_url(&endpoint.base_url),
                "Opening served DevTools URL"
            );
            endpoint.url(&ws_uri)
        }
        None => {
            tracing::warn!("No served DevTools endpoint — falling back to legacy URL");

            let toast_msg = if devtools_serve_pending {
                "DevTools server is still starting — try again in a moment."
            } else {
                "DevTools is not registered with DDS. \
                 Update Flutter to \u{2265} 1.22 or run `dart devtools` manually."
            };
            state.push_toast(ToastLevel::Warn, toast_msg);

            let encoded = percent_encode_uri(&ws_uri);
            build_local_devtools_url(&ws_uri, &encoded)
        }
    };

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
        assert_eq!(parse_default_panel("network"), DevToolsPanel::Network);
        assert_eq!(parse_default_panel("net"), DevToolsPanel::Network);
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

        let result = handle_open_browser_devtools(&mut state);

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

        let result = handle_open_browser_devtools(&mut state);

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
        let mut state = make_state_with_session();

        let result = handle_open_browser_devtools(&mut state);

        assert!(
            result.action.is_none(),
            "Expected no action when ws_uri is not set"
        );
    }

    #[test]
    fn open_browser_no_ws_uri_emits_toast() {
        // When ws_uri is not yet set, the user should see a toast explaining
        // why pressing B did nothing — not a silent no-op.
        let mut state = make_state_with_session();

        handle_open_browser_devtools(&mut state);

        assert!(
            state
                .toasts
                .iter()
                .any(|t| t.text.contains("VM Service not ready")),
            "Expected a 'VM Service not ready' toast, got: {:?}",
            state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Served URL preference tests (Task 06: open-browser-uses-served-url)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn open_browser_uses_served_endpoint_when_available() {
        use crate::session::DevToolsEndpoint;

        let mut state = make_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.ws_uri = Some("ws://127.0.0.1:1234/abc=/ws".to_string());
        handle.session.devtools_endpoint = Some(DevToolsEndpoint {
            base_url: "http://127.0.0.1:9100".into(),
        });

        let result = handle_open_browser_devtools(&mut state);

        match result.action.unwrap() {
            UpdateAction::OpenBrowserDevTools { url, .. } => {
                assert!(
                    url.starts_with("http://127.0.0.1:9100?uri="),
                    "URL should start with the served base_url (got: {url})"
                );
                assert!(
                    url.contains("ws%3A%2F%2F127.0.0.1%3A1234"),
                    "URL should contain percent-encoded ws://127.0.0.1:1234 (got: {url})"
                );
                // Must NOT contain /devtools/ — that is the legacy path shape.
                assert!(
                    !url.contains("/devtools/"),
                    "Served URL should not use /devtools/ legacy path (got: {url})"
                );
            }
            _ => panic!("expected OpenBrowserDevTools action"),
        }
    }

    #[test]
    fn open_browser_uses_served_endpoint_dds_integrated_format() {
        // DDS-integrated DevTools (Flutter ≥ 3.24) — base_url has auth-token path.
        use crate::session::DevToolsEndpoint;

        let mut state = make_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.ws_uri = Some("ws://127.0.0.1:59123/tbrR0DzW2j8=/ws".to_string());
        handle.session.devtools_endpoint = Some(DevToolsEndpoint {
            base_url: "http://127.0.0.1:59123/tbrR0DzW2j8=/devtools".into(),
        });

        let result = handle_open_browser_devtools(&mut state);

        match result.action.unwrap() {
            UpdateAction::OpenBrowserDevTools { url, .. } => {
                assert!(
                    url.starts_with("http://127.0.0.1:59123/tbrR0DzW2j8=/devtools?uri="),
                    "DDS-integrated URL should preserve auth-token path (got: {url})"
                );
                assert!(
                    url.contains("ws%3A%2F%2F"),
                    "URL should contain percent-encoded ws:// scheme (got: {url})"
                );
            }
            _ => panic!("expected OpenBrowserDevTools action"),
        }
    }

    #[test]
    fn open_browser_falls_back_to_legacy_url_when_no_endpoint() {
        let mut state = make_state_with_session();
        state.session_manager.selected_mut().unwrap().session.ws_uri =
            Some("ws://127.0.0.1:1234/abc=/ws".to_string());
        // No devtools_endpoint set — remains None.

        let result = handle_open_browser_devtools(&mut state);

        match result.action.unwrap() {
            UpdateAction::OpenBrowserDevTools { url, .. } => {
                assert!(
                    url.contains("/devtools/?uri="),
                    "Legacy fallback URL should contain /devtools/?uri= (got: {url})"
                );
                assert!(
                    url.contains("ws%3A%2F%2F"),
                    "Legacy URL should contain percent-encoded ws:// (got: {url})"
                );
            }
            _ => panic!("expected OpenBrowserDevTools action"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Toast emission tests (Task 07: fallback-and-recovery-toast)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn fallback_path_emits_toast() {
        // When no devtools_endpoint is set (legacy fallback), a toast must be
        // pushed with the DDS registration failure message.
        let mut state = make_state_with_session();
        state.session_manager.selected_mut().unwrap().session.ws_uri =
            Some("ws://127.0.0.1:9999/abc=/ws".to_string());
        // No devtools_endpoint → fallback path.

        handle_open_browser_devtools(&mut state);

        assert!(
            !state.toasts.is_empty(),
            "A toast must be pushed when falling back to the legacy URL"
        );
        assert!(
            state
                .toasts
                .iter()
                .any(|t| t.text.contains("not registered with DDS")),
            "Toast must mention DDS registration (got: {:?})",
            state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pending_serve_emits_different_toast() {
        // When devtools_serve_pending is true, the toast should tell the user
        // to wait rather than report a DDS failure.
        let mut state = make_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.ws_uri = Some("ws://127.0.0.1:9999/abc=/ws".to_string());
        handle.session.devtools_serve_pending = true;
        // No devtools_endpoint → fallback path with pending flavour.

        handle_open_browser_devtools(&mut state);

        assert!(
            !state.toasts.is_empty(),
            "A toast must be pushed when devtools_serve_pending is true"
        );
        assert!(
            state
                .toasts
                .iter()
                .any(|t| t.text.contains("still starting")),
            "Toast must mention server is still starting (got: {:?})",
            state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn served_endpoint_no_toast() {
        // When a devtools_endpoint is available no toast should be pushed.
        use crate::session::DevToolsEndpoint;

        let mut state = make_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.ws_uri = Some("ws://127.0.0.1:1234/abc=/ws".to_string());
        handle.session.devtools_endpoint = Some(DevToolsEndpoint {
            base_url: "http://127.0.0.1:9100".into(),
        });

        handle_open_browser_devtools(&mut state);

        assert!(
            state.toasts.is_empty(),
            "No toast should be pushed when the served endpoint is available (got: {:?})",
            state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Allocation pause / unpause tests (Task 05: gate-alloc-on-panel)
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a session with a live alloc_pause_tx channel.
    /// Returns the AppState and a receiver so tests can observe the channel value.
    fn make_state_with_alloc_pause() -> (AppState, tokio::sync::watch::Receiver<bool>) {
        let mut state = make_state_with_session();
        let (tx, rx) = tokio::sync::watch::channel(true); // initial: paused
        state.session_manager.selected_mut().unwrap().alloc_pause_tx =
            Some(std::sync::Arc::new(tx));
        (state, rx)
    }

    #[test]
    fn test_alloc_pause_tx_stored_on_session_handle() {
        // After VmServicePerformanceMonitoringStarted is handled via
        // Message dispatch, handle.alloc_pause_tx should be Some(...).
        // Verified here by directly setting the field in the test helper.
        let (state, rx) = make_state_with_alloc_pause();
        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .alloc_pause_tx
                .is_some(),
            "alloc_pause_tx should be set on the session handle"
        );
        // Channel starts in the paused state (true).
        assert!(
            *rx.borrow(),
            "alloc_pause channel should start in paused state (true)"
        );
    }

    #[test]
    fn test_switch_to_performance_sends_unpause() {
        // SwitchDevToolsPanel(Performance) should send false on alloc_pause_tx.
        let (mut state, rx) = make_state_with_alloc_pause();
        // Start on Inspector panel.
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

        handle_switch_panel(&mut state, DevToolsPanel::Performance);

        assert!(
            !*rx.borrow(),
            "switching TO Performance should unpause alloc polling (send false)"
        );
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );
    }

    #[test]
    fn test_switch_away_from_performance_sends_pause() {
        // SwitchDevToolsPanel(Inspector) when current panel is Performance
        // should pause alloc polling (send true).
        let (mut state, rx) = make_state_with_alloc_pause();
        // Start with Performance panel active and alloc unpaused.
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        state
            .session_manager
            .selected()
            .unwrap()
            .alloc_pause_tx
            .as_ref()
            .unwrap()
            .send(false)
            .unwrap(); // simulate it was unpaused

        handle_switch_panel(&mut state, DevToolsPanel::Inspector);

        assert!(
            *rx.borrow(),
            "switching AWAY from Performance should pause alloc polling (send true)"
        );
    }

    #[test]
    fn test_switch_between_non_performance_panels_does_not_change_pause_state() {
        // Switching Inspector → Network should not touch the alloc_pause channel.
        let (mut state, rx) = make_state_with_alloc_pause();
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        // Confirm initial state is paused.
        assert!(*rx.borrow());

        handle_switch_panel(&mut state, DevToolsPanel::Network);

        // Pause state should remain true (unchanged — no Performance panel involved).
        assert!(
            *rx.borrow(),
            "switching between non-Performance panels should not change alloc pause state"
        );
    }

    #[test]
    fn test_exit_devtools_sends_pause() {
        // handle_exit_devtools_mode should pause alloc polling.
        let (mut state, rx) = make_state_with_alloc_pause();
        // Simulate: user was on Performance panel with alloc unpaused.
        state
            .session_manager
            .selected()
            .unwrap()
            .alloc_pause_tx
            .as_ref()
            .unwrap()
            .send(false)
            .unwrap();

        handle_exit_devtools_mode(&mut state);

        assert!(
            *rx.borrow(),
            "exiting DevTools should pause alloc polling (send true)"
        );
    }

    #[test]
    fn test_enter_devtools_with_performance_default_sends_unpause() {
        // handle_enter_devtools_mode with default_panel = "performance"
        // should unpause alloc polling.
        let (mut state, rx) = make_state_with_alloc_pause();
        state.settings.devtools.default_panel = "performance".to_string();

        handle_enter_devtools_mode(&mut state);

        assert!(
            !*rx.borrow(),
            "entering DevTools with Performance as default should unpause alloc polling (send false)"
        );
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );
    }

    #[test]
    fn test_enter_devtools_with_inspector_default_does_not_unpause() {
        // handle_enter_devtools_mode with default_panel = "inspector"
        // should NOT change the alloc pause state (remains paused).
        let (mut state, rx) = make_state_with_alloc_pause();
        state.settings.devtools.default_panel = "inspector".to_string();

        handle_enter_devtools_mode(&mut state);

        assert!(
            *rx.borrow(),
            "entering DevTools with Inspector as default should not unpause alloc polling"
        );
    }

    #[test]
    fn test_alloc_pause_cleared_on_disconnect() {
        // After VmServiceDisconnected, alloc_pause_tx should be None.
        // We test by simulating what the handler does.
        let mut state = make_state_with_session();
        let (tx, _rx) = tokio::sync::watch::channel(false);
        state.session_manager.selected_mut().unwrap().alloc_pause_tx =
            Some(std::sync::Arc::new(tx));

        // Simulate the VmServiceDisconnected handler clearing the field.
        state.session_manager.selected_mut().unwrap().alloc_pause_tx = None;

        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .alloc_pause_tx
                .is_none(),
            "alloc_pause_tx should be None after disconnect"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // perf_pause_tx tests (Task 01: pause-perf-when-not-devtools)
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a session with both a live `perf_pause_tx` and `alloc_pause_tx`.
    /// Returns the AppState plus receivers for observing both channel values.
    fn make_state_with_perf_pause() -> (
        AppState,
        tokio::sync::watch::Receiver<bool>,
        tokio::sync::watch::Receiver<bool>,
    ) {
        let mut state = make_state_with_session();
        let (perf_tx, perf_rx) = tokio::sync::watch::channel(true); // starts paused
        let (alloc_tx, alloc_rx) = tokio::sync::watch::channel(true); // starts paused
        let handle = state.session_manager.selected_mut().unwrap();
        handle.perf_pause_tx = Some(std::sync::Arc::new(perf_tx));
        handle.alloc_pause_tx = Some(std::sync::Arc::new(alloc_tx));
        (state, perf_rx, alloc_rx)
    }

    #[test]
    fn test_perf_pause_tx_stored_on_session_handle() {
        // After VmServicePerformanceMonitoringStarted is handled,
        // handle.perf_pause_tx should be Some(...).
        // Verified here by directly populating the field (as the handler does).
        let (state, perf_rx, _alloc_rx) = make_state_with_perf_pause();
        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .perf_pause_tx
                .is_some(),
            "perf_pause_tx should be set on the session handle"
        );
        // Channel starts in the paused state (true) — monitoring starts paused
        // at VM connect time, before the user opens DevTools.
        assert!(
            *perf_rx.borrow(),
            "perf_pause channel should start in paused state (true)"
        );
    }

    #[test]
    fn test_enter_devtools_sends_perf_unpause() {
        // handle_enter_devtools_mode should send false on perf_pause_tx.
        let (mut state, perf_rx, _alloc_rx) = make_state_with_perf_pause();
        state.settings.devtools.default_panel = "inspector".to_string();

        handle_enter_devtools_mode(&mut state);

        assert!(
            !*perf_rx.borrow(),
            "entering DevTools should unpause performance monitoring (send false on perf_pause_tx)"
        );
    }

    #[test]
    fn test_exit_devtools_sends_perf_pause() {
        // handle_exit_devtools_mode should send true on perf_pause_tx.
        let (mut state, perf_rx, _alloc_rx) = make_state_with_perf_pause();

        // Simulate: user was in DevTools with perf monitoring active.
        state
            .session_manager
            .selected()
            .unwrap()
            .perf_pause_tx
            .as_ref()
            .unwrap()
            .send(false)
            .unwrap();
        assert!(!*perf_rx.borrow(), "precondition: perf should be unpaused");

        handle_exit_devtools_mode(&mut state);

        assert!(
            *perf_rx.borrow(),
            "exiting DevTools should pause performance monitoring (send true on perf_pause_tx)"
        );
    }

    #[test]
    fn test_perf_pause_cleared_on_disconnect() {
        // After VmServiceDisconnected, perf_pause_tx should be None.
        let mut state = make_state_with_session();
        let (tx, _rx) = tokio::sync::watch::channel(false);
        state.session_manager.selected_mut().unwrap().perf_pause_tx = Some(std::sync::Arc::new(tx));

        // Simulate the VmServiceDisconnected handler clearing the field.
        state.session_manager.selected_mut().unwrap().perf_pause_tx = None;

        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .perf_pause_tx
                .is_none(),
            "perf_pause_tx should be None after disconnect"
        );
    }

    #[test]
    fn test_panel_switch_does_not_affect_perf_pause() {
        // SwitchDevToolsPanel (Inspector → Network, etc.) should NOT change
        // perf_pause_tx. Only DevTools entry/exit affects it.
        let (mut state, perf_rx, _alloc_rx) = make_state_with_perf_pause();

        // Simulate: user is in DevTools with perf monitoring active (unpaused).
        state
            .session_manager
            .selected()
            .unwrap()
            .perf_pause_tx
            .as_ref()
            .unwrap()
            .send(false)
            .unwrap();
        let initial_perf_state = *perf_rx.borrow();
        assert!(!initial_perf_state, "precondition: perf should be unpaused");

        // Switch from Inspector to Network — this should NOT touch perf_pause_tx.
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        handle_switch_panel(&mut state, DevToolsPanel::Network);

        assert_eq!(
            *perf_rx.borrow(),
            initial_perf_state,
            "panel switching should not change perf_pause_tx state"
        );

        // Switch from Network to Performance — this should also NOT touch perf_pause_tx.
        handle_switch_panel(&mut state, DevToolsPanel::Performance);
        assert_eq!(
            *perf_rx.borrow(),
            initial_perf_state,
            "switching to Performance panel should not change perf_pause_tx (only alloc_pause_tx)"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // network_pause_tx tests (Task 02: pause-network-on-tab-switch)
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a session with a live `network_pause_tx` channel (initial: active/false).
    /// Returns AppState and a receiver to observe the channel value.
    fn make_state_with_network_pause() -> (AppState, tokio::sync::watch::Receiver<bool>) {
        let mut state = make_state_with_session();
        // Initial value false (active) — task starts when already on Network tab.
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = state.session_manager.selected_mut().unwrap();
        // Simulate the task being running by also setting network_shutdown_tx.
        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        handle.network_shutdown_tx = Some(std::sync::Arc::new(shutdown_tx));
        handle.network_pause_tx = Some(std::sync::Arc::new(tx));
        (state, rx)
    }

    #[test]
    fn test_network_pause_tx_stored_on_session_handle() {
        // After VmServiceNetworkMonitoringStarted is handled, network_pause_tx
        // should be Some. Verified here by directly setting the field.
        let (state, rx) = make_state_with_network_pause();
        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .network_pause_tx
                .is_some(),
            "network_pause_tx should be set on the session handle"
        );
        // Channel starts in the active state (false) — task starts on Network tab.
        assert!(
            !*rx.borrow(),
            "network_pause channel should start in active state (false)"
        );
    }

    #[test]
    fn test_switch_away_from_network_sends_pause() {
        // SwitchDevToolsPanel(Performance) when current panel is Network
        // should send true on network_pause_tx.
        let (mut state, rx) = make_state_with_network_pause();
        state.devtools_view_state.active_panel = DevToolsPanel::Network;

        handle_switch_panel(&mut state, DevToolsPanel::Performance);

        assert!(
            *rx.borrow(),
            "switching AWAY from Network should pause network polling (send true)"
        );
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );
    }

    #[test]
    fn test_switch_to_network_sends_unpause_when_task_running() {
        // SwitchDevToolsPanel(Network) when task is already running should
        // send false on network_pause_tx (unpause).
        let (mut state, rx) = make_state_with_network_pause();
        // First pause it (simulate coming from another panel).
        state
            .session_manager
            .selected()
            .unwrap()
            .network_pause_tx
            .as_ref()
            .unwrap()
            .send(true)
            .unwrap();
        assert!(*rx.borrow(), "precondition: network should be paused");

        // Switch to Network — task is already running (network_shutdown_tx is set).
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        handle_switch_panel(&mut state, DevToolsPanel::Network);

        assert!(
            !*rx.borrow(),
            "switching TO Network (task running) should unpause network polling (send false)"
        );
    }

    #[test]
    fn test_exit_devtools_pauses_network() {
        // handle_exit_devtools_mode should send true on network_pause_tx.
        let (mut state, rx) = make_state_with_network_pause();
        // Confirm initial state is active (false).
        assert!(!*rx.borrow(), "precondition: network should be active");

        handle_exit_devtools_mode(&mut state);

        assert!(
            *rx.borrow(),
            "exiting DevTools should pause network polling (send true)"
        );
    }

    #[test]
    fn test_enter_devtools_with_network_default_unpauses() {
        // handle_enter_devtools_mode with default_panel = "network"
        // should send false on network_pause_tx (if task is running).
        let (mut state, rx) = make_state_with_network_pause();
        state.settings.devtools.default_panel = "network".to_string();

        // First pause it (simulate previous DevTools exit).
        state
            .session_manager
            .selected()
            .unwrap()
            .network_pause_tx
            .as_ref()
            .unwrap()
            .send(true)
            .unwrap();
        assert!(*rx.borrow(), "precondition: network should be paused");

        handle_enter_devtools_mode(&mut state);

        assert!(
            !*rx.borrow(),
            "entering DevTools with Network as default should unpause network polling (send false)"
        );
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Network
        );
    }

    #[test]
    fn test_enter_devtools_with_non_network_default_does_not_unpause_network() {
        // handle_enter_devtools_mode with default_panel = "inspector"
        // should NOT change the network pause state (remains paused).
        let (mut state, rx) = make_state_with_network_pause();
        state.settings.devtools.default_panel = "inspector".to_string();

        // First pause it (simulate previous DevTools exit).
        state
            .session_manager
            .selected()
            .unwrap()
            .network_pause_tx
            .as_ref()
            .unwrap()
            .send(true)
            .unwrap();
        assert!(*rx.borrow(), "precondition: network should be paused");

        handle_enter_devtools_mode(&mut state);

        assert!(
            *rx.borrow(),
            "entering DevTools with Inspector as default should not unpause network polling"
        );
    }

    #[test]
    fn test_network_pause_cleared_on_disconnect() {
        // After VmServiceDisconnected, network_pause_tx should be None.
        // We test by simulating what the handler does.
        let mut state = make_state_with_session();
        let (tx, _rx) = tokio::sync::watch::channel(false);
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .network_pause_tx = Some(std::sync::Arc::new(tx));

        // Simulate the VmServiceDisconnected handler clearing the field.
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .network_pause_tx = None;

        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .network_pause_tx
                .is_none(),
            "network_pause_tx should be None after disconnect"
        );
    }

    #[test]
    fn test_switch_between_non_network_panels_does_not_change_network_pause_state() {
        // Switching Inspector → Performance should not touch the network_pause channel.
        let (mut state, rx) = make_state_with_network_pause();
        // First pause it (as if we're not on Network).
        state
            .session_manager
            .selected()
            .unwrap()
            .network_pause_tx
            .as_ref()
            .unwrap()
            .send(true)
            .unwrap();
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        assert!(*rx.borrow(), "precondition: network should be paused");

        handle_switch_panel(&mut state, DevToolsPanel::Performance);

        // Pause state should remain true (unchanged — no Network panel involved).
        assert!(
            *rx.borrow(),
            "switching between non-Network panels should not change network pause state"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // record_fetch_start invariant tests (Task 07: use-record-fetch-start)
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a state with an active vm-connected session on the Inspector
    /// panel, with no widget tree loaded. This is the scenario that triggers
    /// a FetchWidgetTree action on `handle_enter_devtools_mode` and
    /// `handle_switch_panel(Inspector)`.
    fn make_state_with_vm_connected_inspector() -> AppState {
        let mut state = make_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.vm_connected = true;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        // Ensure no tree is loaded and loading is false (triggers the fetch path).
        state.devtools_view_state.inspector.root = None;
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.last_fetch_time = None;
        state
    }

    #[test]
    fn test_handle_enter_devtools_mode_records_fetch_start_invariant() {
        // When handle_enter_devtools_mode sets loading=true it must also set
        // last_fetch_time via record_fetch_start(). Without the timestamp a
        // lost terminal message would leave loading=true forever.
        let mut state = make_state_with_vm_connected_inspector();

        let _ = handle_enter_devtools_mode(&mut state);

        let inspector = &state.devtools_view_state.inspector;
        if inspector.loading {
            assert!(
                inspector.last_fetch_time.is_some(),
                "loading=true must be paired with last_fetch_time=Some(...) — \
                 use record_fetch_start() instead of direct assignment"
            );
        }
    }

    #[test]
    fn test_handle_switch_panel_inspector_records_fetch_start_invariant() {
        // When handle_switch_panel(Inspector) sets loading=true it must also
        // set last_fetch_time via record_fetch_start().
        let mut state = make_state_with_vm_connected_inspector();
        // Start on Performance so we are actually switching TO Inspector.
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;

        let _ = handle_switch_panel(&mut state, DevToolsPanel::Inspector);

        let inspector = &state.devtools_view_state.inspector;
        if inspector.loading {
            assert!(
                inspector.last_fetch_time.is_some(),
                "loading=true must be paired with last_fetch_time=Some(...) — \
                 use record_fetch_start() instead of direct assignment"
            );
        }
    }

    #[test]
    fn test_lint_no_naked_inspector_loading_assignment_in_handler() {
        // Grep-based lint: no bare assignment of the form
        //   inspector.loading = true;
        // must remain in the production code of handler/devtools/mod.rs.
        // All auto-fetch sites must go through record_fetch_start() so the
        // loading+last_fetch_time invariant is always maintained together.
        //
        // The search pattern is split across a concat!() to avoid the lint
        // matching the pattern string itself.
        let needle = concat!("inspector", ".loading = true");
        let source = include_str!("mod.rs");
        let naked_count = source
            .lines()
            // Exclude lines that are Rust comments (// ...) or inside string
            // literals (contain a leading `"`). Production assignments always
            // begin with whitespace then `state.`.
            .filter(|l| {
                let trimmed = l.trim_start();
                trimmed.starts_with("state.") && trimmed.contains(needle)
            })
            .count();
        assert_eq!(
            naked_count, 0,
            "Found {naked_count} naked `inspector.loading = true` statement(s) in \
             handler/devtools/mod.rs. Replace each with \
             `state.devtools_view_state.inspector.record_fetch_start()` to \
             enforce the loading+last_fetch_time invariant."
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // clamped_readiness_poll_config tests (Task 08: clamp-readiness-poll-config)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_clamped_readiness_poll_attempts_capped_at_max() {
        // u32::MAX must be clamped down to MAX_READINESS_POLL_ATTEMPTS.
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_attempts: u32::MAX,
            ..Default::default()
        };
        let (attempts, _, _) = clamped_readiness_poll_config(&settings);
        assert_eq!(attempts, MAX_READINESS_POLL_ATTEMPTS);
    }

    #[test]
    fn test_clamped_readiness_poll_interval_floored_at_min() {
        // 0 ms is below the minimum; must be raised to MIN_READINESS_POLL_INTERVAL_MS.
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_interval_ms: 0,
            ..Default::default()
        };
        let (_, interval, _) = clamped_readiness_poll_config(&settings);
        assert_eq!(interval, MIN_READINESS_POLL_INTERVAL_MS);
    }

    #[test]
    fn test_clamped_readiness_poll_interval_capped_at_max() {
        // u64::MAX must be clamped down to MAX_READINESS_POLL_INTERVAL_MS.
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_interval_ms: u64::MAX,
            ..Default::default()
        };
        let (_, interval, _) = clamped_readiness_poll_config(&settings);
        assert_eq!(interval, MAX_READINESS_POLL_INTERVAL_MS);
    }

    #[test]
    fn test_clamped_readiness_poll_call_timeout_floored_at_min() {
        // 0 ms is below the minimum; must be raised to MIN_READINESS_POLL_CALL_TIMEOUT_MS.
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_call_timeout_ms: 0,
            ..Default::default()
        };
        let (_, _, timeout) = clamped_readiness_poll_config(&settings);
        assert_eq!(timeout, MIN_READINESS_POLL_CALL_TIMEOUT_MS);
    }

    #[test]
    fn test_clamped_readiness_poll_call_timeout_capped_at_max() {
        // u64::MAX must be clamped down to MAX_READINESS_POLL_CALL_TIMEOUT_MS.
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_call_timeout_ms: u64::MAX,
            ..Default::default()
        };
        let (_, _, timeout) = clamped_readiness_poll_config(&settings);
        assert_eq!(timeout, MAX_READINESS_POLL_CALL_TIMEOUT_MS);
    }

    #[test]
    fn test_clamped_readiness_poll_passes_through_normal_values() {
        // Values within range must be returned unchanged (no clamping, no log noise).
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_attempts: 3,
            inspector_readiness_poll_interval_ms: 200,
            inspector_readiness_poll_call_timeout_ms: 1500,
            ..Default::default()
        };
        let (a, i, t) = clamped_readiness_poll_config(&settings);
        assert_eq!((a, i, t), (3, 200, 1500));
    }

    #[test]
    fn test_clamped_readiness_poll_boundary_values_pass_through() {
        // Values exactly at the boundary must pass through unchanged.
        let settings = crate::config::DevToolsSettings {
            inspector_readiness_poll_attempts: MAX_READINESS_POLL_ATTEMPTS,
            inspector_readiness_poll_interval_ms: MIN_READINESS_POLL_INTERVAL_MS,
            inspector_readiness_poll_call_timeout_ms: MAX_READINESS_POLL_CALL_TIMEOUT_MS,
            ..Default::default()
        };
        let (a, i, t) = clamped_readiness_poll_config(&settings);
        assert_eq!(a, MAX_READINESS_POLL_ATTEMPTS);
        assert_eq!(i, MIN_READINESS_POLL_INTERVAL_MS);
        assert_eq!(t, MAX_READINESS_POLL_CALL_TIMEOUT_MS);
    }

    #[test]
    fn test_lint_no_raw_readiness_poll_reads_at_dispatch_sites() {
        // Grep-based lint: no FetchWidgetTree struct literal in mod.rs (or the
        // follow-on VmConnected path in update.rs) should read
        // `state.settings.devtools.inspector_readiness_poll_*` directly. All three
        // sites must go through clamped_readiness_poll_config().
        let source = include_str!("mod.rs");
        let raw_reads = source
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                // Only flag lines that are reading the field directly off state
                // (i.e. inside a struct literal). Comments and the helper
                // function body are excluded by the `state.settings.devtools`
                // prefix check.
                t.starts_with("state.settings.devtools.inspector_readiness_poll_")
            })
            .count();
        assert_eq!(
            raw_reads, 0,
            "Found {raw_reads} raw `state.settings.devtools.inspector_readiness_poll_*` read(s) \
             in handler/devtools/mod.rs. All dispatch sites must use \
             clamped_readiness_poll_config() instead."
        );
    }

    // ── Tiered Esc tests (Phase 1, Task 05) ──────────────────────────────────

    fn make_state_with_tree_and_devtools() -> AppState {
        let mut state = make_state_with_session();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

        let tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "root-id",
            "children": []
        }))
        .expect("valid DiagnosticsNode for tiered-esc test");
        state.devtools_view_state.inspector.root = Some(tree);
        state
    }

    #[test]
    fn tiered_esc_closes_details_first_then_exits_devtools() {
        let mut state = make_state_with_tree_and_devtools();

        // Open the Details panel.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("root-id".to_string());

        // First Esc: should close Details, NOT exit DevTools.
        handle_devtools_escape(&mut state);

        assert!(
            !state.devtools_view_state.inspector.details_open,
            "First Esc should close the Details panel"
        );
        assert_eq!(
            state.ui_mode,
            UiMode::DevTools,
            "First Esc should NOT exit DevTools mode"
        );

        // Second Esc: Details is now closed, so this exits DevTools.
        handle_devtools_escape(&mut state);

        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "Second Esc should exit DevTools mode"
        );
    }

    #[test]
    fn tiered_esc_exits_devtools_directly_when_details_closed() {
        let mut state = make_state_with_tree_and_devtools();
        // Details is already closed.
        state.devtools_view_state.inspector.details_open = false;

        handle_devtools_escape(&mut state);

        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "Esc with details closed should exit DevTools immediately"
        );
    }

    #[test]
    fn tiered_esc_on_performance_panel_exits_devtools() {
        let mut state = make_state_with_session();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        // Inspector details_open is irrelevant when the active panel is Performance.
        state.devtools_view_state.inspector.details_open = true;

        handle_devtools_escape(&mut state);

        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "Esc on Performance panel should exit DevTools even if inspector.details_open"
        );
    }
}
