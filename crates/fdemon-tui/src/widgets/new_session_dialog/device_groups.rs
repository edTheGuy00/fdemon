//! Device grouping logic for organizing devices by platform
//!
//! This module provides utilities for grouping connected and bootable devices
//! by platform, with support for flattening groups into navigable lists.
//!
//! NOTE: Core grouping types and logic have been moved to `app/new_session_dialog/device_groups.rs`
//! (Phase 1, Task 05). This module re-exports them for backward compatibility.

// Re-export all types and functions from app layer
pub use fdemon_app::new_session_dialog::device_groups::{
    flatten_groups, group_bootable_devices, group_connected_devices, next_selectable,
    prev_selectable, selectable_indices, BootablePlatformGroup, DeviceGroup, DeviceListItem,
    GroupedBootableDevice, PlatformGroup,
};

// Tests remain in this file for now (testing the re-exported functionality)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_device_full;
    use fdemon_daemon::SimulatorState;

    #[test]
    fn test_platform_group_from_device() {
        let ios_physical = test_device_full("id", "iPhone", "ios", false);
        assert_eq!(
            PlatformGroup::from_device(&ios_physical),
            PlatformGroup::IosPhysical
        );

        let android_emulator = test_device_full("id", "Pixel", "android", true);
        assert_eq!(
            PlatformGroup::from_device(&android_emulator),
            PlatformGroup::AndroidEmulator
        );

        let ios_simulator = test_device_full("id", "iPhone Sim", "ios", true);
        assert_eq!(
            PlatformGroup::from_device(&ios_simulator),
            PlatformGroup::IosSimulator
        );

        let android_physical = test_device_full("id", "Galaxy", "android", false);
        assert_eq!(
            PlatformGroup::from_device(&android_physical),
            PlatformGroup::AndroidPhysical
        );

        let web = test_device_full("id", "Chrome", "chrome", false);
        assert_eq!(PlatformGroup::from_device(&web), PlatformGroup::Web);

        let desktop = test_device_full("id", "macOS", "macos", false);
        assert_eq!(PlatformGroup::from_device(&desktop), PlatformGroup::Desktop);
    }

    #[test]
    fn test_platform_group_from_device_with_variants() {
        // Test iOS variants
        let ios_x64 = test_device_full("id", "iPhone", "ios_x64", true);
        assert_eq!(
            PlatformGroup::from_device(&ios_x64),
            PlatformGroup::IosSimulator
        );

        // Test Android variants
        let android_arm64 = test_device_full("id", "Pixel", "android-arm64", true);
        assert_eq!(
            PlatformGroup::from_device(&android_arm64),
            PlatformGroup::AndroidEmulator
        );

        let android_physical = test_device_full("id", "Galaxy", "android-arm", false);
        assert_eq!(
            PlatformGroup::from_device(&android_physical),
            PlatformGroup::AndroidPhysical
        );

        // Test web variants
        let web_js = test_device_full("id", "Chrome", "web-javascript", false);
        assert_eq!(PlatformGroup::from_device(&web_js), PlatformGroup::Web);

        // Test desktop variants
        let darwin = test_device_full("id", "macOS", "darwin", false);
        assert_eq!(PlatformGroup::from_device(&darwin), PlatformGroup::Desktop);

        let linux = test_device_full("id", "Linux", "linux", false);
        assert_eq!(PlatformGroup::from_device(&linux), PlatformGroup::Desktop);

        let windows = test_device_full("id", "Windows", "windows", false);
        assert_eq!(PlatformGroup::from_device(&windows), PlatformGroup::Desktop);
    }

    #[test]
    fn test_group_connected_devices() {
        let devices = vec![
            test_device_full("1", "iPhone", "ios", false),
            test_device_full("2", "iPad", "ios", false),
            test_device_full("3", "Pixel", "android", false),
            test_device_full("4", "Chrome", "chrome", false),
        ];

        let groups = group_connected_devices(&devices);

        assert_eq!(groups.len(), 3); // iOS Devices, Android Devices, Web
        assert_eq!(groups[0].header, "iOS Devices");
        assert_eq!(groups[0].devices.len(), 2);
        assert_eq!(groups[1].header, "Android Devices");
        assert_eq!(groups[1].devices.len(), 1);
        assert_eq!(groups[2].header, "Web");
        assert_eq!(groups[2].devices.len(), 1);
    }

    #[test]
    fn test_group_connected_devices_mixed_emulators() {
        let devices = vec![
            test_device_full("1", "iPhone Physical", "ios", false),
            test_device_full("2", "iPhone Sim", "ios", true),
            test_device_full("3", "Pixel Physical", "android", false),
            test_device_full("4", "Pixel Emulator", "android", true),
        ];

        let groups = group_connected_devices(&devices);

        // Should have 4 groups: iOS Devices, iOS Simulators, Android Devices, Android Emulators
        assert_eq!(groups.len(), 4);
    }

    #[test]
    fn test_group_connected_devices_empty() {
        let devices: Vec<fdemon_daemon::Device> = vec![];
        let groups = group_connected_devices(&devices);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_bootable_devices() {
        let ios_simulators = vec![
            fdemon_daemon::IosSimulator {
                udid: "sim1".to_string(),
                name: "iPhone 15 Pro".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 15 Pro".to_string(),
            },
            fdemon_daemon::IosSimulator {
                udid: "sim2".to_string(),
                name: "iPad Pro".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPad Pro".to_string(),
            },
        ];

        let android_avds = vec![fdemon_daemon::AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        }];

        let groups = group_bootable_devices(&ios_simulators, &android_avds);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].header, "iOS Simulators");
        assert_eq!(groups[0].devices.len(), 2);
        assert_eq!(groups[1].header, "Android AVDs");
        assert_eq!(groups[1].devices.len(), 1);
    }

    #[test]
    fn test_group_bootable_devices_empty_simulators() {
        let ios_simulators: Vec<fdemon_daemon::IosSimulator> = vec![];
        let android_avds = vec![fdemon_daemon::AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        }];

        let groups = group_bootable_devices(&ios_simulators, &android_avds);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].header, "Android AVDs");
    }

    #[test]
    fn test_group_bootable_devices_empty_avds() {
        let ios_simulators = vec![fdemon_daemon::IosSimulator {
            udid: "sim1".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }];
        let android_avds: Vec<fdemon_daemon::AndroidAvd> = vec![];

        let groups = group_bootable_devices(&ios_simulators, &android_avds);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].header, "iOS Simulators");
    }

    #[test]
    fn test_group_bootable_devices_all_empty() {
        let ios_simulators: Vec<fdemon_daemon::IosSimulator> = vec![];
        let android_avds: Vec<fdemon_daemon::AndroidAvd> = vec![];

        let groups = group_bootable_devices(&ios_simulators, &android_avds);

        assert!(groups.is_empty());
    }

    #[test]
    fn test_flatten_groups() {
        let groups = vec![
            DeviceGroup::new("Group A", vec!["a1", "a2"]),
            DeviceGroup::new("Group B", vec!["b1"]),
        ];

        let flat = flatten_groups(&groups);

        assert_eq!(flat.len(), 5); // 2 headers + 3 items
        assert!(matches!(&flat[0], DeviceListItem::Header(h) if h == "Group A"));
        assert!(matches!(&flat[1], DeviceListItem::Device(d) if *d == "a1"));
        assert!(matches!(&flat[2], DeviceListItem::Device(d) if *d == "a2"));
        assert!(matches!(&flat[3], DeviceListItem::Header(h) if h == "Group B"));
        assert!(matches!(&flat[4], DeviceListItem::Device(d) if *d == "b1"));
    }

    #[test]
    fn test_flatten_groups_empty() {
        let groups: Vec<DeviceGroup<&str>> = vec![];
        let flat = flatten_groups(&groups);
        assert!(flat.is_empty());
    }

    #[test]
    fn test_flatten_groups_with_empty_group() {
        let groups = vec![
            DeviceGroup::new("Group A", vec!["a1"]),
            DeviceGroup::new("Group B", Vec::<&str>::new()),
        ];

        let flat = flatten_groups(&groups);

        // Empty groups should not appear in flattened list
        assert_eq!(flat.len(), 2); // 1 header + 1 item
    }

    #[test]
    fn test_selectable_indices() {
        let items = vec![
            DeviceListItem::Header("Header".to_string()),
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
            DeviceListItem::Header("Header 2".to_string()),
            DeviceListItem::Device("c"),
        ];

        let selectable = selectable_indices(&items);
        assert_eq!(selectable, vec![1, 2, 4]);
    }

    #[test]
    fn test_selectable_indices_no_devices() {
        let items = vec![
            DeviceListItem::Header::<&str>("Header 1".to_string()),
            DeviceListItem::Header::<&str>("Header 2".to_string()),
        ];

        let selectable = selectable_indices(&items);
        assert!(selectable.is_empty());
    }

    #[test]
    fn test_selectable_indices_empty() {
        let items: Vec<DeviceListItem<&str>> = vec![];
        let selectable = selectable_indices(&items);
        assert!(selectable.is_empty());
    }

    #[test]
    fn test_next_selectable() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
        ];

        assert_eq!(next_selectable(&items, 1), 2);
        assert_eq!(next_selectable(&items, 2), 1); // Wrap around
    }

    #[test]
    fn test_next_selectable_single_item() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
        ];

        assert_eq!(next_selectable(&items, 1), 1); // Stays on same item
    }

    #[test]
    fn test_next_selectable_empty() {
        let items: Vec<DeviceListItem<&str>> = vec![];
        assert_eq!(next_selectable(&items, 0), 0);
    }

    #[test]
    fn test_next_selectable_no_devices() {
        let items = vec![DeviceListItem::Header::<&str>("H".to_string())];
        assert_eq!(next_selectable(&items, 0), 0);
    }

    #[test]
    fn test_prev_selectable() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
        ];

        assert_eq!(prev_selectable(&items, 2), 1);
        assert_eq!(prev_selectable(&items, 1), 2); // Wrap around
    }

    #[test]
    fn test_prev_selectable_single_item() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
        ];

        assert_eq!(prev_selectable(&items, 1), 1); // Stays on same item
    }

    #[test]
    fn test_prev_selectable_empty() {
        let items: Vec<DeviceListItem<&str>> = vec![];
        assert_eq!(prev_selectable(&items, 0), 0);
    }

    #[test]
    fn test_prev_selectable_no_devices() {
        let items = vec![DeviceListItem::Header::<&str>("H".to_string())];
        assert_eq!(prev_selectable(&items, 0), 0);
    }

    #[test]
    fn test_platform_group_header() {
        assert_eq!(PlatformGroup::IosPhysical.header(), "iOS Devices");
        assert_eq!(PlatformGroup::AndroidPhysical.header(), "Android Devices");
        assert_eq!(PlatformGroup::IosSimulator.header(), "iOS Simulators");
        assert_eq!(PlatformGroup::AndroidEmulator.header(), "Android Emulators");
        assert_eq!(PlatformGroup::Web.header(), "Web");
        assert_eq!(PlatformGroup::Desktop.header(), "Desktop");
        assert_eq!(PlatformGroup::Other.header(), "Other");
    }

    #[test]
    fn test_bootable_platform_group_header() {
        assert_eq!(
            BootablePlatformGroup::IosSimulators.header(),
            "iOS Simulators"
        );
        assert_eq!(BootablePlatformGroup::AndroidAvds.header(), "Android AVDs");
    }

    #[test]
    fn test_device_group_is_empty() {
        let empty_group: DeviceGroup<&str> = DeviceGroup::new("Empty", vec![]);
        assert!(empty_group.is_empty());

        let non_empty_group = DeviceGroup::new("Non-empty", vec!["item"]);
        assert!(!non_empty_group.is_empty());
    }

    #[test]
    fn test_bootable_device_enum_ios() {
        let sim = fdemon_daemon::IosSimulator {
            udid: "test".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        };

        let bootable = GroupedBootableDevice::IosSimulator(sim);
        assert!(matches!(bootable, GroupedBootableDevice::IosSimulator(_)));
    }

    #[test]
    fn test_bootable_device_enum_android() {
        let avd = fdemon_daemon::AndroidAvd {
            name: "Pixel_6".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        };

        let bootable = GroupedBootableDevice::AndroidAvd(avd);
        assert!(matches!(bootable, GroupedBootableDevice::AndroidAvd(_)));
    }
}
