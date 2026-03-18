//! NewSessionDialog target selector handlers
//!
//! Handles device list navigation, selection, booting, and device discovery.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::DiscoveryType;
use crate::state::AppState;
use fdemon_daemon::Device;
use tracing::warn;

/// Default estimated visible height for scroll calculations.
/// Used as a fallback on the first frame before the renderer has
/// written the actual visible height to `last_known_visible_height`.
const DEFAULT_ESTIMATED_VISIBLE_HEIGHT: usize = 10;

/// Get the effective visible height for scroll calculations.
///
/// Returns the actual visible height from the last render frame,
/// or falls back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` if no render
/// has occurred yet (first frame).
fn effective_visible_height(state: &AppState) -> usize {
    let height = state
        .new_session_dialog_state
        .target_selector
        .last_known_visible_height
        .get();
    if height > 0 {
        height
    } else {
        DEFAULT_ESTIMATED_VISIBLE_HEIGHT
    }
}

/// Handle device list navigation up
pub fn handle_device_up(state: &mut AppState) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .select_previous();
    // Use actual visible height from last render, fall back to estimate on first frame
    let height = effective_visible_height(state);
    state
        .new_session_dialog_state
        .target_selector
        .adjust_scroll(height);
    UpdateResult::none()
}

/// Handle device list navigation down
pub fn handle_device_down(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.select_next();
    // Use actual visible height from last render, fall back to estimate on first frame
    let height = effective_visible_height(state);
    state
        .new_session_dialog_state
        .target_selector
        .adjust_scroll(height);
    UpdateResult::none()
}

/// Handle device selection (Enter on device)
pub fn handle_device_select(state: &mut AppState) -> UpdateResult {
    use crate::new_session_dialog::TargetTab;
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
                use crate::new_session_dialog::GroupedBootableDevice;
                use fdemon_core::Platform;
                let (device_id, platform) = match device {
                    GroupedBootableDevice::IosSimulator(sim) => (sim.udid.clone(), Platform::IOS),
                    GroupedBootableDevice::AndroidAvd(avd) => (avd.name.clone(), Platform::Android),
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
    use crate::new_session_dialog::TargetTab;
    match state.new_session_dialog_state.target_selector.active_tab {
        TargetTab::Connected => {
            let Some(flutter) = state.flutter_executable() else {
                tracing::warn!("handle_refresh_devices: no Flutter SDK — cannot discover devices");
                return UpdateResult::none();
            };
            state.new_session_dialog_state.target_selector.loading = true;
            UpdateResult::action(UpdateAction::DiscoverDevices { flutter })
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
    ios_simulators: Vec<fdemon_daemon::IosSimulator>,
    android_avds: Vec<fdemon_daemon::AndroidAvd>,
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
    use crate::new_session_dialog::TargetTab;
    // Switch to Connected tab and trigger device refresh
    state
        .new_session_dialog_state
        .target_selector
        .set_tab(TargetTab::Connected);
    let Some(flutter) = state.flutter_executable() else {
        tracing::warn!("handle_boot_completed: no Flutter SDK — cannot discover devices");
        return UpdateResult::none();
    };
    state.new_session_dialog_state.target_selector.loading = true;
    UpdateResult::action(UpdateAction::DiscoverDevices { flutter })
}

/// Handle boot failed notification
pub fn handle_boot_failed(state: &mut AppState, device_id: String, error: String) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .set_error(format!("Failed to boot device {}: {}", device_id, error));
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoadedConfigs;
    use crate::new_session_dialog::TargetTab;
    use crate::state::{AppState, UiMode};
    use fdemon_core::Platform;
    use fdemon_daemon::test_utils::fake_flutter_sdk;
    use fdemon_daemon::{AndroidAvd, IosSimulator, SimulatorState};
    use std::path::PathBuf;

    fn test_app_state() -> AppState {
        let mut state = AppState::with_settings(
            PathBuf::from("/test/project"),
            crate::config::Settings::default(),
        );
        state.project_name = Some("TestProject".to_string());
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());
        // Inject a fake SDK so handlers that require flutter_executable() work in tests
        state.resolved_sdk = Some(fake_flutter_sdk());
        state
    }

    fn test_app_state_with_bootable_devices() -> AppState {
        let mut state = test_app_state();

        // Add iOS simulators
        let ios_sims = vec![
            IosSimulator {
                udid: "ios-sim-1".to_string(),
                name: "iPhone 15 Pro".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 15 Pro".to_string(),
            },
            IosSimulator {
                udid: "ios-sim-2".to_string(),
                name: "iPhone 14".to_string(),
                runtime: "iOS 17.0".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 14".to_string(),
            },
        ];

        // Add Android AVDs
        let android_avds = vec![
            AndroidAvd {
                name: "Pixel_6_API_33".to_string(),
                display_name: "Pixel 6".to_string(),
                api_level: Some(33),
                target: None,
            },
            AndroidAvd {
                name: "Pixel_7_API_34".to_string(),
                display_name: "Pixel 7".to_string(),
                api_level: Some(34),
                target: None,
            },
        ];

        state
            .new_session_dialog_state
            .target_selector
            .set_bootable_devices(ios_sims, android_avds);
        state
    }

    #[test]
    fn test_boot_ios_simulator_uses_platform_enum() {
        let mut state = test_app_state_with_bootable_devices();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Bootable);
        // Index 0 is header "iOS Simulators", first device is at index 1
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_device_select(&mut state);

        if let Some(UpdateAction::BootDevice {
            device_id: _,
            platform,
        }) = result.action
        {
            assert_eq!(platform, Platform::IOS);
        } else {
            panic!("Expected BootDevice action with Platform::IOS");
        }
    }

    #[test]
    fn test_boot_android_avd_uses_platform_enum() {
        let mut state = test_app_state_with_bootable_devices();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Bootable);
        // Flat list: [iOS Header, iOS1, iOS2, Android Header, Android1, Android2]
        // Select first Android AVD (at index 4)
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 4;

        let result = handle_device_select(&mut state);

        if let Some(UpdateAction::BootDevice {
            device_id: _,
            platform,
        }) = result.action
        {
            assert_eq!(platform, Platform::Android);
        } else {
            panic!("Expected BootDevice action with Platform::Android");
        }
    }

    #[test]
    fn test_boot_device_id_correct() {
        let mut state = test_app_state_with_bootable_devices();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Bootable);
        // Index 1 is first iOS simulator (index 0 is header)
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_device_select(&mut state);

        if let Some(UpdateAction::BootDevice {
            device_id,
            platform: _,
        }) = result.action
        {
            assert_eq!(device_id, "ios-sim-1");
        } else {
            panic!("Expected BootDevice action");
        }
    }

    #[test]
    fn test_device_select_on_connected_tab_no_action() {
        let mut state = test_app_state();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Connected);

        let result = handle_device_select(&mut state);

        assert!(
            result.action.is_none(),
            "Should not trigger boot action on Connected tab"
        );
    }

    #[test]
    fn test_refresh_devices_connected_tab() {
        let mut state = test_app_state();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Connected);
        state.new_session_dialog_state.target_selector.loading = false;

        let result = handle_refresh_devices(&mut state);

        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverDevices { .. })
        ));
    }

    #[test]
    fn test_refresh_devices_bootable_tab() {
        let mut state = test_app_state();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Bootable);
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = false;

        let result = handle_refresh_devices(&mut state);

        assert!(
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverBootableDevices)
        ));
    }

    #[test]
    fn test_boot_completed_switches_to_connected_tab() {
        let mut state = test_app_state();
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Bootable);

        let result = handle_boot_completed(&mut state);

        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Connected
        );
        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverDevices { .. })
        ));
    }

    #[test]
    fn test_boot_failed_sets_error() {
        let mut state = test_app_state();

        handle_boot_failed(&mut state, "test-device".to_string(), "timeout".to_string());

        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
        let error = state
            .new_session_dialog_state
            .target_selector
            .error
            .unwrap();
        assert!(error.contains("test-device"));
        assert!(error.contains("timeout"));
    }

    #[test]
    fn test_device_discovery_failed_connected() {
        let mut state = test_app_state();
        state.new_session_dialog_state.target_selector.loading = true;

        handle_device_discovery_failed(
            &mut state,
            "Discovery failed".to_string(),
            crate::message::DiscoveryType::Connected,
        );

        assert!(!state.new_session_dialog_state.target_selector.loading);
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
    }

    #[test]
    fn test_device_discovery_failed_bootable() {
        let mut state = test_app_state();
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = true;

        handle_device_discovery_failed(
            &mut state,
            "Discovery failed".to_string(),
            crate::message::DiscoveryType::Bootable,
        );

        assert!(
            !state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert!(state
            .new_session_dialog_state
            .target_selector
            .error
            .is_some());
    }

    #[test]
    fn test_handle_device_down_uses_default_height_on_first_frame() {
        let mut state = test_app_state();
        // Add 20 devices to require scrolling
        let devices: Vec<Device> = (0..20)
            .map(|i| {
                fdemon_daemon::test_utils::test_device_full(
                    &format!("d{}", i),
                    &format!("Device {}", i),
                    "ios",
                    false,
                )
            })
            .collect();
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(devices);

        // last_known_visible_height is 0 (no render yet) — handler should fall back to
        // DEFAULT_ESTIMATED_VISIBLE_HEIGHT (10)
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .last_known_visible_height
                .get(),
            0
        );

        // Navigate down 12 times — past the default estimated viewport of 10
        for _ in 0..12 {
            handle_device_down(&mut state);
        }

        // scroll_offset should have adjusted to keep selection visible
        assert!(
            state.new_session_dialog_state.target_selector.scroll_offset > 0,
            "scroll_offset should be > 0 after navigating past estimated viewport"
        );
    }

    #[test]
    fn test_handle_device_down_uses_actual_height_after_render() {
        let mut state = test_app_state();
        let devices: Vec<Device> = (0..20)
            .map(|i| {
                fdemon_daemon::test_utils::test_device_full(
                    &format!("d{}", i),
                    &format!("Device {}", i),
                    "ios",
                    false,
                )
            })
            .collect();
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(devices);

        // Simulate the renderer writing an actual visible height of 5
        state
            .new_session_dialog_state
            .target_selector
            .last_known_visible_height
            .set(5);

        // Navigate down 6 times — past the 5-row viewport
        for _ in 0..6 {
            handle_device_down(&mut state);
        }

        // With visible_height=5, scrolling should start after 5 items visible.
        // After 6 navigations the selection is past the viewport boundary.
        assert!(
            state.new_session_dialog_state.target_selector.scroll_offset > 0,
            "scroll_offset should be > 0 after navigating past actual 5-row viewport"
        );
    }

    #[test]
    fn test_handle_device_up_uses_actual_height() {
        let mut state = test_app_state();
        let devices: Vec<Device> = (0..20)
            .map(|i| {
                fdemon_daemon::test_utils::test_device_full(
                    &format!("d{}", i),
                    &format!("Device {}", i),
                    "ios",
                    false,
                )
            })
            .collect();
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(devices);

        // Simulate the renderer writing an actual visible height of 5
        state
            .new_session_dialog_state
            .target_selector
            .last_known_visible_height
            .set(5);

        // Navigate down 10 times to scroll the list
        for _ in 0..10 {
            handle_device_down(&mut state);
        }
        let scroll_after_down = state.new_session_dialog_state.target_selector.scroll_offset;
        assert!(
            scroll_after_down > 0,
            "scroll_offset should be > 0 after navigating down"
        );

        // Navigate back up 10 times — should return to top
        for _ in 0..10 {
            handle_device_up(&mut state);
        }

        // Selection should be back at the first selectable item.
        // The flat list is [header, dev0, dev1, ...], so the first selectable
        // index is 1 (the header at 0 is not selectable).
        let sel = state
            .new_session_dialog_state
            .target_selector
            .selected_index;
        let offset = state.new_session_dialog_state.target_selector.scroll_offset;
        // Selection returned to first device (flat index 1, after the header at 0)
        assert_eq!(sel, 1, "Selection should be at flat index 1 (first device)");
        // Selection must be visible: selected_index is within [offset, offset + visible_height)
        assert!(
            sel >= offset && sel < offset + 5,
            "selected_index ({sel}) should be visible in viewport [offset={offset}, offset+5)"
        );
    }
}
