//! NewSessionDialog target selector handlers
//!
//! Handles device list navigation, selection, booting, and device discovery.

use crate::app::handler::{UpdateAction, UpdateResult};
use crate::app::message::DiscoveryType;
use crate::app::state::AppState;
use crate::daemon::Device;
use tracing::warn;

/// Handle device list navigation up
pub fn handle_device_up(state: &mut AppState) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .select_previous();
    UpdateResult::none()
}

/// Handle device list navigation down
pub fn handle_device_down(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.select_next();
    UpdateResult::none()
}

/// Handle device selection (Enter on device)
pub fn handle_device_select(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::TargetTab;
    match state.new_session_dialog_state.target_selector.active_tab {
        TargetTab::Connected => {
            // Select device for launch - actual launch happens in Launch Context
            // For now, just acknowledge the selection
            if state
                .new_session_dialog_state
                .target_selector
                .selected_connected_device()
                .is_none()
            {
                warn!("Cannot select device: no device selected on Connected tab");
            }
            UpdateResult::none()
        }
        TargetTab::Bootable => {
            // Boot the selected device
            if let Some(device) = state
                .new_session_dialog_state
                .target_selector
                .selected_bootable_device()
            {
                use crate::tui::widgets::GroupedBootableDevice;
                let (device_id, platform) = match device {
                    GroupedBootableDevice::IosSimulator(sim) => {
                        (sim.udid.clone(), "ios".to_string())
                    }
                    GroupedBootableDevice::AndroidAvd(avd) => {
                        (avd.name.clone(), "android".to_string())
                    }
                };
                return UpdateResult::action(UpdateAction::BootDevice {
                    device_id,
                    platform,
                });
            }
            warn!("Cannot boot device: no device selected on Bootable tab");
            UpdateResult::none()
        }
    }
}

/// Handle device refresh (r key)
pub fn handle_refresh_devices(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::TargetTab;
    match state.new_session_dialog_state.target_selector.active_tab {
        TargetTab::Connected => {
            state.new_session_dialog_state.target_selector.loading = true;
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }
        TargetTab::Bootable => {
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading = true;
            UpdateResult::action(UpdateAction::DiscoverBootableDevices)
        }
    }
}

/// Handle connected devices received from discovery
pub fn handle_connected_devices_received(
    state: &mut AppState,
    devices: Vec<Device>,
) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .set_connected_devices(devices);
    UpdateResult::none()
}

/// Handle bootable devices received from discovery
pub fn handle_bootable_devices_received(
    state: &mut AppState,
    ios_simulators: Vec<crate::daemon::IosSimulator>,
    android_avds: Vec<crate::daemon::AndroidAvd>,
) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .set_bootable_devices(ios_simulators, android_avds);
    UpdateResult::none()
}

/// Handle device discovery failure
pub fn handle_device_discovery_failed(
    state: &mut AppState,
    error: String,
    discovery_type: DiscoveryType,
) -> UpdateResult {
    // Only clear the loading flag for the type that failed
    match discovery_type {
        DiscoveryType::Connected => {
            state.new_session_dialog_state.target_selector.loading = false;
        }
        DiscoveryType::Bootable => {
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading = false;
        }
    }
    state
        .new_session_dialog_state
        .target_selector
        .set_error(error);
    UpdateResult::none()
}

/// Handle boot started notification
pub fn handle_boot_started(_state: &mut AppState, _device_id: String) -> UpdateResult {
    // Boot started, no state change needed yet
    // Device state tracking happens in TargetSelectorState
    UpdateResult::none()
}

/// Handle boot completed notification
pub fn handle_boot_completed(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::TargetTab;
    // Switch to Connected tab and trigger device refresh
    state
        .new_session_dialog_state
        .target_selector
        .set_tab(TargetTab::Connected);
    state.new_session_dialog_state.target_selector.loading = true;
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Handle boot failed notification
pub fn handle_boot_failed(state: &mut AppState, device_id: String, error: String) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .set_error(format!("Failed to boot device {}: {}", device_id, error));
    UpdateResult::none()
}
