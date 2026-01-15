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
    state.new_session_dialog_state.target_up();
    UpdateResult::none()
}

/// Handle device list navigation down
pub fn handle_device_down(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_down();
    UpdateResult::none()
}

/// Handle device selection (Enter on device)
pub fn handle_device_select(state: &mut AppState) -> UpdateResult {
    use crate::tui::widgets::TargetTab;
    match state.new_session_dialog_state.target_tab {
        TargetTab::Connected => {
            // Select device for launch - actual launch happens in Launch Context
            // For now, just acknowledge the selection
            if state
                .new_session_dialog_state
                .selected_connected_device()
                .is_none()
            {
                warn!("Cannot select device: no device selected on Connected tab");
            }
            UpdateResult::none()
        }
        TargetTab::Bootable => {
            // Boot the selected device
            if let Some(device) = state.new_session_dialog_state.selected_bootable_device() {
                let device_id = device.id.clone();
                let platform = device.platform.to_string();
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
    use crate::tui::widgets::TargetTab;
    match state.new_session_dialog_state.target_tab {
        TargetTab::Connected => {
            state.new_session_dialog_state.loading_connected = true;
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }
        TargetTab::Bootable => {
            state.new_session_dialog_state.loading_bootable = true;
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
        .set_connected_devices(devices);
    UpdateResult::none()
}

/// Handle bootable devices received from discovery
pub fn handle_bootable_devices_received(
    state: &mut AppState,
    ios_simulators: Vec<crate::daemon::IosSimulator>,
    android_avds: Vec<crate::daemon::AndroidAvd>,
) -> UpdateResult {
    // Convert to BootableDevice using BootCommand
    let mut bootable_devices = Vec::new();

    for sim in ios_simulators {
        let cmd = crate::daemon::BootCommand::IosSimulator(sim);
        bootable_devices.push(cmd.into());
    }

    for avd in android_avds {
        let cmd = crate::daemon::BootCommand::AndroidAvd(avd);
        bootable_devices.push(cmd.into());
    }

    state
        .new_session_dialog_state
        .set_bootable_devices(bootable_devices);
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
            state.new_session_dialog_state.loading_connected = false;
        }
        DiscoveryType::Bootable => {
            state.new_session_dialog_state.loading_bootable = false;
        }
    }
    state.new_session_dialog_state.set_error(error);
    UpdateResult::none()
}

/// Handle boot started notification
pub fn handle_boot_started(state: &mut AppState, device_id: String) -> UpdateResult {
    state
        .new_session_dialog_state
        .mark_device_booting(&device_id);
    UpdateResult::none()
}

/// Handle boot completed notification
pub fn handle_boot_completed(state: &mut AppState) -> UpdateResult {
    // Switch to Connected tab and trigger device refresh
    state.new_session_dialog_state.handle_device_booted();
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Handle boot failed notification
pub fn handle_boot_failed(state: &mut AppState, device_id: String, error: String) -> UpdateResult {
    state
        .new_session_dialog_state
        .set_error(format!("Failed to boot device {}: {}", device_id, error));
    UpdateResult::none()
}
