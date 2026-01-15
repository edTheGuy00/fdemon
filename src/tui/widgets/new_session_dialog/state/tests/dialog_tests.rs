//! Tests for NewSessionDialogState

use super::super::*;
use crate::config::FlutterMode;
use crate::core::{BootableDevice, DeviceState, Platform};
use crate::daemon::Device;

#[test]
fn test_new_session_dialog_state_default() {
    let state = NewSessionDialogState::new();
    assert_eq!(state.active_pane, DialogPane::Left);
    assert_eq!(state.target_tab, TargetTab::Connected);
    assert!(state.loading_connected);
    assert!(!state.has_modal_open());
}

#[test]
fn test_launch_context_field_navigation() {
    assert_eq!(LaunchContextField::Config.next(), LaunchContextField::Mode);
    assert_eq!(
        LaunchContextField::Launch.next(),
        LaunchContextField::Config
    );
    assert_eq!(
        LaunchContextField::Config.prev(),
        LaunchContextField::Launch
    );
}

#[test]
fn test_pane_navigation() {
    let mut state = NewSessionDialogState::new();
    assert_eq!(state.active_pane, DialogPane::Left);

    state.switch_pane();
    assert_eq!(state.active_pane, DialogPane::Right);

    state.switch_pane();
    assert_eq!(state.active_pane, DialogPane::Left);
}

#[test]
fn test_tab_switching() {
    let mut state = NewSessionDialogState::new();
    assert_eq!(state.target_tab, TargetTab::Connected);

    state.toggle_tab();
    assert_eq!(state.target_tab, TargetTab::Bootable);
    // Handler is responsible for setting loading flags, not state methods
    assert!(!state.loading_bootable);
}

#[test]
fn test_target_navigation_wrapping() {
    let mut state = NewSessionDialogState::new();
    state.connected_devices = vec![
        Device {
            id: "d1".into(),
            name: "Device 1".into(),
            platform: "ios".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        },
        Device {
            id: "d2".into(),
            name: "Device 2".into(),
            platform: "android".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        },
    ];
    state.loading_connected = false;

    assert_eq!(state.selected_target_index, 0);
    state.target_down();
    assert_eq!(state.selected_target_index, 1);
    state.target_down(); // Wrap
    assert_eq!(state.selected_target_index, 0);
    state.target_up(); // Wrap back
    assert_eq!(state.selected_target_index, 1);
}

#[test]
fn test_mode_cycling() {
    let mut state = NewSessionDialogState::new();
    assert_eq!(state.mode, FlutterMode::Debug);

    state.cycle_mode();
    assert_eq!(state.mode, FlutterMode::Profile);

    state.cycle_mode();
    assert_eq!(state.mode, FlutterMode::Release);

    state.cycle_mode();
    assert_eq!(state.mode, FlutterMode::Debug);
}

#[test]
fn test_mode_cycling_reverse() {
    let mut state = NewSessionDialogState::new();
    assert_eq!(state.mode, FlutterMode::Debug);

    state.cycle_mode_reverse();
    assert_eq!(state.mode, FlutterMode::Release);

    state.cycle_mode_reverse();
    assert_eq!(state.mode, FlutterMode::Profile);

    state.cycle_mode_reverse();
    assert_eq!(state.mode, FlutterMode::Debug);
}

#[test]
fn test_set_connected_devices() {
    let mut state = NewSessionDialogState::new();
    assert!(state.loading_connected);

    let devices = vec![Device {
        id: "d1".into(),
        name: "Device 1".into(),
        platform: "ios".into(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }];

    state.set_connected_devices(devices);
    assert!(!state.loading_connected);
    assert_eq!(state.connected_devices.len(), 1);
}

#[test]
fn test_set_connected_devices_clears_error() {
    let mut state = NewSessionDialogState::new();
    state.set_error("Previous error".to_string());
    assert!(state.error.is_some());

    let devices = vec![Device {
        id: "d1".into(),
        name: "Device 1".into(),
        platform: "ios".into(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }];

    state.set_connected_devices(devices);
    assert!(state.error.is_none()); // Error should be cleared on successful load
}

#[test]
fn test_error_handling() {
    let mut state = NewSessionDialogState::new();
    assert!(state.error.is_none());

    state.set_error("Test error".to_string());
    assert_eq!(state.error, Some("Test error".to_string()));

    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_selected_device_getters() {
    let mut state = NewSessionDialogState::new();
    state.connected_devices = vec![Device {
        id: "d1".into(),
        name: "Device 1".into(),
        platform: "ios".into(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }];
    state.bootable_devices = vec![BootableDevice {
        id: "sim1".into(),
        name: "Simulator 1".into(),
        platform: Platform::IOS,
        runtime: "iOS 17.2".into(),
        state: DeviceState::Shutdown,
    }];

    // Connected tab returns connected device
    state.target_tab = TargetTab::Connected;
    assert!(state.selected_connected_device().is_some());
    assert!(state.selected_bootable_device().is_none());

    // Bootable tab returns bootable device
    state.target_tab = TargetTab::Bootable;
    assert!(state.selected_connected_device().is_none());
    assert!(state.selected_bootable_device().is_some());
}

#[test]
fn test_modal_management() {
    let mut state = NewSessionDialogState::new();
    assert!(!state.has_modal_open());

    state.open_fuzzy_modal(
        FuzzyModalType::Config,
        vec!["config1".into(), "config2".into()],
    );
    assert!(state.has_modal_open());
    assert!(state.fuzzy_modal.is_some());

    state.close_fuzzy_modal();
    assert!(!state.has_modal_open());
    assert!(state.fuzzy_modal.is_none());
}

#[test]
fn test_dart_defines_modal() {
    let mut state = NewSessionDialogState::new();
    state.dart_defines = vec![DartDefine {
        key: "API_URL".into(),
        value: "https://api.com".into(),
    }];

    state.open_dart_defines_modal();
    assert!(state.dart_defines_modal.is_some());
    let modal = state.dart_defines_modal.as_ref().unwrap();
    assert_eq!(modal.defines.len(), 1);

    // Close saves changes
    state.close_dart_defines_modal();
    assert!(state.dart_defines_modal.is_none());
}

#[test]
fn test_switch_tab_skips_header() {
    let mut state = NewSessionDialogState::new();

    // Add devices to Connected tab (will be grouped with header at index 0)
    state.connected_devices = vec![
        Device {
            id: "d1".into(),
            name: "iPhone 15".into(),
            platform: "ios".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        },
        Device {
            id: "d2".into(),
            name: "Pixel 6".into(),
            platform: "android".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        },
    ];

    // Add devices to Bootable tab (will also be grouped with header at index 0)
    state.bootable_devices = vec![BootableDevice {
        id: "sim1".into(),
        name: "iPhone 15 Simulator".into(),
        platform: Platform::IOS,
        runtime: "iOS 17.2".into(),
        state: DeviceState::Shutdown,
    }];

    // Start on Connected tab
    state.target_tab = TargetTab::Connected;
    state.selected_target_index = 99; // Some arbitrary index

    // Switch to Bootable tab
    state.switch_tab(TargetTab::Bootable);

    // Selection should be at index 1 (first device), not 0 (header)
    // When devices are rendered with grouping, index 0 is the platform header
    assert_eq!(state.selected_target_index, 1);
    assert_eq!(state.target_tab, TargetTab::Bootable);

    // Switch back to Connected tab
    state.switch_tab(TargetTab::Connected);

    // Again, should skip to index 1 (first device after header)
    assert_eq!(state.selected_target_index, 1);
    assert_eq!(state.target_tab, TargetTab::Connected);
}

#[test]
fn test_switch_tab_empty_device_list() {
    let mut state = NewSessionDialogState::new();

    // Start with no devices
    state.target_tab = TargetTab::Connected;
    state.selected_target_index = 0;

    // Switch to Bootable tab (which is empty)
    state.switch_tab(TargetTab::Bootable);

    // With no devices, selection should be 0 (no header to skip)
    assert_eq!(state.selected_target_index, 0);
}

#[test]
fn test_rapid_tab_switching_no_race() {
    let mut state = NewSessionDialogState::new();

    // Rapid switch: Connected -> Bootable -> Connected -> Bootable
    state.switch_tab(TargetTab::Connected);
    assert!(!state.loading_bootable);

    state.switch_tab(TargetTab::Bootable);
    // Handler should set flag, not switch_tab
    assert!(!state.loading_bootable); // State method doesn't set it

    // Simulate handler setting flag
    state.loading_bootable = true;

    state.switch_tab(TargetTab::Connected);
    // Switching away shouldn't clear the flag (discovery still running)
    assert!(state.loading_bootable);
}
