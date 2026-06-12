//! Service-level DevTools monitoring handlers.
//!
//! These handlers back the `StartDevToolsMonitoring` / `StopDevToolsMonitoring`
//! messages dispatched by [`crate::services::DevToolsService`] so headless
//! consumers (e.g. an MCP server embedding the Engine) can collect telemetry
//! without the TUI user entering DevTools mode. They reuse exactly the same
//! `UpdateAction`s and pause channels as the TUI handlers in
//! [`super`] (`handle_enter_devtools_mode` / `handle_switch_panel`).

use crate::handler::{UpdateAction, UpdateResult};
use crate::session::SessionId;
use crate::state::AppState;
use crate::state::UiMode;

/// Handle `Message::StartDevToolsMonitoring`.
///
/// Marks the session for service-level monitoring and ensures the performance
/// (memory sampling) and network (HTTP profile) polling tasks are running and
/// unpaused:
///
/// - If a polling task is not running yet, the corresponding
///   `StartPerformanceMonitoring` / `StartNetworkMonitoring` action is
///   returned — the same actions the TUI dispatches on DevTools entry.
/// - If a task is already running, its pause channel is sent `false`.
/// - If the VM Service is not connected yet, only the flag is set; the
///   `VmServiceConnected` handler starts monitoring once the VM attaches.
///
/// Frame timings ([`crate::session::PerformanceState::frame_history`]) are
/// collected passively from the VM Service Extension stream whenever the VM is
/// connected and do not require this handler.
pub fn handle_start_devtools_monitoring(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    // Read settings before the mutable session borrow.
    let performance_refresh_ms = state.settings.devtools.performance_refresh_ms;
    let allocation_profile_interval_ms = state.settings.devtools.allocation_profile_interval_ms;
    let network_poll_interval_ms = state.settings.devtools.network_poll_interval_ms;

    let Some(handle) = state.session_manager.get_mut(session_id) else {
        tracing::warn!(
            session_id = session_id,
            "StartDevToolsMonitoring: unknown session — ignoring"
        );
        return UpdateResult::none();
    };

    handle.devtools_service_monitoring = true;

    if !handle.session.vm_connected {
        tracing::info!(
            session_id = session_id,
            "StartDevToolsMonitoring: VM Service not connected yet — monitoring \
             will start automatically on VmServiceConnected"
        );
        return UpdateResult::none();
    }

    let mode = handle
        .session
        .launch_config
        .as_ref()
        .map(|c| c.mode)
        .unwrap_or(crate::config::FlutterMode::Debug);

    let mut actions = Vec::new();

    // Performance polling (memory samples + allocation profile).
    if handle.perf_shutdown_tx.is_none() {
        actions.push(UpdateAction::StartPerformanceMonitoring {
            session_id,
            handle: None, // hydrated by process.rs
            performance_refresh_ms,
            allocation_profile_interval_ms,
            mode,
        });
    } else if let Some(ref tx) = handle.perf_pause_tx {
        let _ = tx.send(false); // unpause
    }

    // Network polling (HTTP profile). Skip when the `ext.dart.io.*` extensions
    // are known to be unavailable (release mode) or a task is already running.
    let network_extensions_unavailable = handle.session.network.extensions_available == Some(false);
    if handle.network_shutdown_tx.is_none() {
        if !network_extensions_unavailable {
            actions.push(UpdateAction::StartNetworkMonitoring {
                session_id,
                handle: None, // hydrated by process.rs
                poll_interval_ms: network_poll_interval_ms,
                mode,
            });
        }
    } else if let Some(ref tx) = handle.network_pause_tx {
        let _ = tx.send(false); // unpause
    }

    UpdateResult::actions_vec(actions)
}

/// Handle `Message::StopDevToolsMonitoring`.
///
/// Clears the service-monitoring flag. When the TUI user is *not* currently
/// viewing this session in DevTools mode, the performance and network polling
/// loops are paused (the same pause channels `handle_exit_devtools_mode`
/// uses). When the TUI *is* in DevTools mode on this session, pause state is
/// left untouched so the TUI panels keep receiving data.
pub fn handle_stop_devtools_monitoring(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    let tui_devtools_active = state.ui_mode == UiMode::DevTools
        && state.session_manager.selected_id() == Some(session_id);

    let Some(handle) = state.session_manager.get_mut(session_id) else {
        tracing::warn!(
            session_id = session_id,
            "StopDevToolsMonitoring: unknown session — ignoring"
        );
        return UpdateResult::none();
    };

    handle.devtools_service_monitoring = false;

    if !tui_devtools_active {
        if let Some(ref tx) = handle.perf_pause_tx {
            let _ = tx.send(true); // pause
        }
        if let Some(ref tx) = handle.network_pause_tx {
            let _ = tx.send(true); // pause
        }
    }

    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::UpdateAction;
    use crate::state::UiMode;

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
            is_supported: true,
            capabilities: None,
        };
        state.session_manager.create_session(&device).unwrap();
        state
    }

    fn session_id(state: &AppState) -> SessionId {
        state.session_manager.selected_id().unwrap()
    }

    #[test]
    fn test_start_monitoring_unknown_session_is_noop() {
        let mut state = make_state_with_session();
        let result = handle_start_devtools_monitoring(&mut state, 9999);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    #[test]
    fn test_start_monitoring_vm_not_connected_sets_flag_only() {
        let mut state = make_state_with_session();
        let id = session_id(&state);

        let result = handle_start_devtools_monitoring(&mut state, id);

        assert!(
            result.action.is_none(),
            "no actions before the VM Service connects"
        );
        assert!(
            state
                .session_manager
                .get(id)
                .unwrap()
                .devtools_service_monitoring,
            "flag must be set so VmServiceConnected starts monitoring later"
        );
    }

    #[test]
    fn test_start_monitoring_dispatches_perf_and_network_actions() {
        let mut state = make_state_with_session();
        let id = session_id(&state);
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .vm_connected = true;

        let result = handle_start_devtools_monitoring(&mut state, id);

        let actions = result.actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::StartPerformanceMonitoring { session_id, .. } if *session_id == id)),
            "expected StartPerformanceMonitoring, got {:?}",
            actions
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::StartNetworkMonitoring { session_id, .. } if *session_id == id)),
            "expected StartNetworkMonitoring, got {:?}",
            actions
        );
        assert!(
            state
                .session_manager
                .get(id)
                .unwrap()
                .devtools_service_monitoring
        );
    }

    #[test]
    fn test_start_monitoring_skips_network_when_extensions_unavailable() {
        let mut state = make_state_with_session();
        let id = session_id(&state);
        {
            let handle = state.session_manager.get_mut(id).unwrap();
            handle.session.vm_connected = true;
            handle.session.network.extensions_available = Some(false);
        }

        let result = handle_start_devtools_monitoring(&mut state, id);

        let actions = result.actions();
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, UpdateAction::StartNetworkMonitoring { .. })),
            "network monitoring must not start when extensions are unavailable"
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::StartPerformanceMonitoring { .. })),
            "performance monitoring should still start"
        );
    }

    #[test]
    fn test_start_monitoring_unpauses_running_tasks_instead_of_restarting() {
        let mut state = make_state_with_session();
        let id = session_id(&state);

        let (perf_shutdown_tx, _perf_shutdown_rx) = tokio::sync::watch::channel(false);
        let (perf_pause_tx, perf_pause_rx) = tokio::sync::watch::channel(true);
        let (net_shutdown_tx, _net_shutdown_rx) = tokio::sync::watch::channel(false);
        let (net_pause_tx, net_pause_rx) = tokio::sync::watch::channel(true);
        {
            let handle = state.session_manager.get_mut(id).unwrap();
            handle.session.vm_connected = true;
            handle.perf_shutdown_tx = Some(std::sync::Arc::new(perf_shutdown_tx));
            handle.perf_pause_tx = Some(std::sync::Arc::new(perf_pause_tx));
            handle.network_shutdown_tx = Some(std::sync::Arc::new(net_shutdown_tx));
            handle.network_pause_tx = Some(std::sync::Arc::new(net_pause_tx));
        }

        let result = handle_start_devtools_monitoring(&mut state, id);

        assert!(
            result.action.is_none(),
            "tasks already running — no new start actions expected"
        );
        assert!(!*perf_pause_rx.borrow(), "perf polling must be unpaused");
        assert!(!*net_pause_rx.borrow(), "network polling must be unpaused");
    }

    #[test]
    fn test_stop_monitoring_clears_flag_and_pauses_outside_devtools() {
        let mut state = make_state_with_session();
        let id = session_id(&state);
        state.ui_mode = UiMode::Normal;

        let (perf_pause_tx, perf_pause_rx) = tokio::sync::watch::channel(false);
        let (net_pause_tx, net_pause_rx) = tokio::sync::watch::channel(false);
        {
            let handle = state.session_manager.get_mut(id).unwrap();
            handle.devtools_service_monitoring = true;
            handle.perf_pause_tx = Some(std::sync::Arc::new(perf_pause_tx));
            handle.network_pause_tx = Some(std::sync::Arc::new(net_pause_tx));
        }

        handle_stop_devtools_monitoring(&mut state, id);

        let handle = state.session_manager.get(id).unwrap();
        assert!(!handle.devtools_service_monitoring, "flag must be cleared");
        assert!(*perf_pause_rx.borrow(), "perf polling must be paused");
        assert!(*net_pause_rx.borrow(), "network polling must be paused");
    }

    #[test]
    fn test_stop_monitoring_leaves_pause_state_when_tui_in_devtools() {
        let mut state = make_state_with_session();
        let id = session_id(&state);
        state.ui_mode = UiMode::DevTools;

        let (perf_pause_tx, perf_pause_rx) = tokio::sync::watch::channel(false);
        {
            let handle = state.session_manager.get_mut(id).unwrap();
            handle.devtools_service_monitoring = true;
            handle.perf_pause_tx = Some(std::sync::Arc::new(perf_pause_tx));
        }

        handle_stop_devtools_monitoring(&mut state, id);

        let handle = state.session_manager.get(id).unwrap();
        assert!(!handle.devtools_service_monitoring, "flag must be cleared");
        assert!(
            !*perf_pause_rx.borrow(),
            "pause state must be left to the TUI when DevTools is active on this session"
        );
    }

    #[test]
    fn test_stop_monitoring_unknown_session_is_noop() {
        let mut state = make_state_with_session();
        let result = handle_stop_devtools_monitoring(&mut state, 9999);
        assert!(result.action.is_none());
    }
}
