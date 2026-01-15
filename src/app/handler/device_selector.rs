//! Legacy device selector handlers
//!
//! These handlers will be replaced by NewSessionDialog in Phase 8.

use crate::app::state::{AppState, UiMode};
use crate::daemon::Device;

use super::{UpdateAction, UpdateResult};

/// Handle show device selector message
pub fn handle_show_device_selector(state: &mut AppState) -> UpdateResult {
    state.ui_mode = UiMode::DeviceSelector;

    // Use global cache if available for instant display (Task 08e)
    let cached_devices = state.get_cached_devices().cloned();
    if let Some(cached) = cached_devices {
        // Manually set devices and refreshing state to avoid clearing refreshing flag
        let cached_len = cached.len();
        state.device_selector.devices = cached;
        state.device_selector.visible = true;
        state.device_selector.loading = false;
        state.device_selector.refreshing = true;
        state.device_selector.error = None;
        state.device_selector.animation_frame = 0;
        if state.device_selector.selected_index >= cached_len {
            state.device_selector.selected_index = 0;
        }
    } else {
        state.device_selector.show_loading();
    }

    // Always trigger discovery to get fresh data
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Handle hide device selector message
pub fn handle_hide_device_selector(state: &mut AppState) -> UpdateResult {
    // Only hide if there are running sessions, otherwise stay on selector
    if state.session_manager.has_running_sessions() {
        state.device_selector.hide();
        state.ui_mode = UiMode::Normal;
    }
    UpdateResult::none()
}

/// Handle device selector up message
pub fn handle_device_selector_up(state: &mut AppState) -> UpdateResult {
    if state.ui_mode == UiMode::DeviceSelector {
        state.device_selector.select_previous();
    }
    UpdateResult::none()
}

/// Handle device selector down message
pub fn handle_device_selector_down(state: &mut AppState) -> UpdateResult {
    if state.ui_mode == UiMode::DeviceSelector {
        state.device_selector.select_next();
    }
    UpdateResult::none()
}

/// Handle device selected message
pub fn handle_device_selected(state: &mut AppState, device: Device) -> UpdateResult {
    // Check if device already has a running session
    if state
        .session_manager
        .find_by_device_id(&device.id)
        .is_some()
    {
        tracing::warn!("Device '{}' already has an active session", device.name);
        // Stay in device selector to pick another device
        return UpdateResult::none();
    }

    // Create session in manager FIRST
    match state.session_manager.create_session(&device) {
        Ok(session_id) => {
            tracing::info!(
                "Session created for {} (id: {}, device: {})",
                device.name,
                session_id,
                device.id
            );

            // Auto-switch to the newly created session
            state.session_manager.select_by_id(session_id);

            // Hide selector and switch to normal mode
            state.device_selector.hide();
            state.ui_mode = UiMode::Normal;

            // Return action to spawn session WITH the session_id
            UpdateResult::action(UpdateAction::SpawnSession {
                session_id,
                device,
                config: None,
            })
        }
        Err(e) => {
            // Max sessions reached or other error
            tracing::error!("Failed to create session: {}", e);
            UpdateResult::none()
        }
    }
}
