//! Device grouping logic for organizing devices by platform
//!
//! This module provides utilities for grouping connected and bootable devices
//! by platform, with support for flattening groups into navigable lists.

use crate::daemon::{AndroidAvd, Device, IosSimulator};
use std::collections::BTreeMap;

/// A group of devices with a header
#[derive(Debug, Clone)]
pub struct DeviceGroup<T> {
    pub header: String,
    pub devices: Vec<T>,
}

impl<T> DeviceGroup<T> {
    pub fn new(header: impl Into<String>, devices: Vec<T>) -> Self {
        Self {
            header: header.into(),
            devices,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

/// Platform category for grouping connected devices
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlatformGroup {
    IosPhysical,
    AndroidPhysical,
    IosSimulator,
    AndroidEmulator,
    Web,
    Desktop,
    Other,
}

impl PlatformGroup {
    pub fn header(&self) -> &'static str {
        match self {
            PlatformGroup::IosPhysical => "iOS Devices",
            PlatformGroup::AndroidPhysical => "Android Devices",
            PlatformGroup::IosSimulator => "iOS Simulators",
            PlatformGroup::AndroidEmulator => "Android Emulators",
            PlatformGroup::Web => "Web",
            PlatformGroup::Desktop => "Desktop",
            PlatformGroup::Other => "Other",
        }
    }

    /// Determine platform group from a Device
    pub fn from_device(device: &Device) -> Self {
        let platform = device.platform.to_lowercase();
        let is_emulator = device.emulator;

        match (platform.as_str(), is_emulator) {
            ("ios", false) => PlatformGroup::IosPhysical,
            ("ios", true) => PlatformGroup::IosSimulator,
            (p, false) if p.starts_with("ios") => PlatformGroup::IosPhysical,
            (p, true) if p.starts_with("ios") => PlatformGroup::IosSimulator,
            ("android", false) => PlatformGroup::AndroidPhysical,
            ("android", true) => PlatformGroup::AndroidEmulator,
            (p, false) if p.starts_with("android") => PlatformGroup::AndroidPhysical,
            (p, true) if p.starts_with("android") => PlatformGroup::AndroidEmulator,
            ("chrome" | "web" | "web-javascript", _) => PlatformGroup::Web,
            ("linux" | "macos" | "darwin" | "windows", _) => PlatformGroup::Desktop,
            _ => PlatformGroup::Other,
        }
    }
}

/// Group connected devices by platform
pub fn group_connected_devices(devices: &[Device]) -> Vec<DeviceGroup<&Device>> {
    let mut groups: BTreeMap<PlatformGroup, Vec<&Device>> = BTreeMap::new();

    for device in devices {
        let group = PlatformGroup::from_device(device);
        groups.entry(group).or_default().push(device);
    }

    groups
        .into_iter()
        .filter(|(_, devices)| !devices.is_empty())
        .map(|(platform, devices)| DeviceGroup::new(platform.header(), devices))
        .collect()
}

/// Platform group for bootable devices
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BootablePlatformGroup {
    IosSimulators,
    AndroidAvds,
}

impl BootablePlatformGroup {
    pub fn header(&self) -> &'static str {
        match self {
            BootablePlatformGroup::IosSimulators => "iOS Simulators",
            BootablePlatformGroup::AndroidAvds => "Android AVDs",
        }
    }
}

/// Bootable device wrapper for grouping in the TUI layer.
/// This enum wraps IosSimulator and AndroidAvd for device list rendering.
/// Note: Distinct from `core::BootableDevice` domain type which has different structure.
#[derive(Debug, Clone)]
pub enum GroupedBootableDevice {
    IosSimulator(IosSimulator),
    AndroidAvd(AndroidAvd),
}

impl GroupedBootableDevice {
    /// Get the display name for this bootable device
    pub fn display_name(&self) -> &str {
        match self {
            GroupedBootableDevice::IosSimulator(sim) => &sim.name,
            GroupedBootableDevice::AndroidAvd(avd) => &avd.display_name,
        }
    }

    /// Get runtime information as a string
    pub fn runtime_info(&self) -> String {
        match self {
            GroupedBootableDevice::IosSimulator(sim) => sim.runtime.clone(),
            GroupedBootableDevice::AndroidAvd(avd) => avd
                .api_level
                .map(|api| format!("API {}", api))
                .unwrap_or_else(|| "Unknown API".to_string()),
        }
    }

    /// Get platform name
    pub fn platform(&self) -> &'static str {
        match self {
            GroupedBootableDevice::IosSimulator(_) => "iOS",
            GroupedBootableDevice::AndroidAvd(_) => "Android",
        }
    }
}

/// Group bootable devices (iOS simulators and Android AVDs)
pub fn group_bootable_devices(
    ios_simulators: &[IosSimulator],
    android_avds: &[AndroidAvd],
) -> Vec<DeviceGroup<GroupedBootableDevice>> {
    let mut groups = Vec::new();

    // iOS Simulators group
    if !ios_simulators.is_empty() {
        let devices: Vec<GroupedBootableDevice> = ios_simulators
            .iter()
            .cloned()
            .map(GroupedBootableDevice::IosSimulator)
            .collect();
        groups.push(DeviceGroup::new(
            BootablePlatformGroup::IosSimulators.header(),
            devices,
        ));
    }

    // Android AVDs group
    if !android_avds.is_empty() {
        let devices: Vec<GroupedBootableDevice> = android_avds
            .iter()
            .cloned()
            .map(GroupedBootableDevice::AndroidAvd)
            .collect();
        groups.push(DeviceGroup::new(
            BootablePlatformGroup::AndroidAvds.header(),
            devices,
        ));
    }

    groups
}

/// Item in a flat device list (either header or device)
#[derive(Debug, Clone)]
pub enum DeviceListItem<T> {
    Header(String),
    Device(T),
}

/// Flatten grouped devices into a list with headers
pub fn flatten_groups<T: Clone>(groups: &[DeviceGroup<T>]) -> Vec<DeviceListItem<T>> {
    let mut items = Vec::new();

    for group in groups {
        if !group.is_empty() {
            items.push(DeviceListItem::Header(group.header.clone()));
            for device in &group.devices {
                items.push(DeviceListItem::Device(device.clone()));
            }
        }
    }

    items
}

/// Get only selectable indices (devices, not headers)
pub fn selectable_indices<T>(items: &[DeviceListItem<T>]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            DeviceListItem::Device(_) => Some(i),
            DeviceListItem::Header(_) => None,
        })
        .collect()
}

/// Check if an index points to a header
fn is_header<T>(items: &[DeviceListItem<T>], index: usize) -> bool {
    items
        .get(index)
        .map(|item| matches!(item, DeviceListItem::Header(_)))
        .unwrap_or(false)
}

/// Find nearest selectable index (not a header)
///
/// If the current index points to a header, finds the nearest device.
/// Tries forward first, then backward, then returns 0 as fallback.
fn nearest_selectable<T>(items: &[DeviceListItem<T>], index: usize) -> usize {
    let selectable = selectable_indices(items);
    if selectable.is_empty() {
        return 0;
    }

    // If index is already selectable, return it
    if selectable.contains(&index) {
        return index;
    }

    // Try forward first
    for &i in &selectable {
        if i >= index {
            return i;
        }
    }

    // Then backward (return last selectable)
    selectable[selectable.len() - 1]
}

/// Navigate to next selectable item
pub fn next_selectable<T>(items: &[DeviceListItem<T>], current: usize) -> usize {
    let selectable = selectable_indices(items);
    if selectable.is_empty() {
        return 0;
    }

    // Defensive check: if current is a header, find nearest selectable first
    let start = if is_header(items, current) {
        nearest_selectable(items, current)
    } else {
        current
    };

    // Find current position in selectable list
    let current_pos = selectable.iter().position(|&i| i == start).unwrap_or(0);
    let next_pos = (current_pos + 1) % selectable.len();
    selectable[next_pos]
}

/// Navigate to previous selectable item
pub fn prev_selectable<T>(items: &[DeviceListItem<T>], current: usize) -> usize {
    let selectable = selectable_indices(items);
    if selectable.is_empty() {
        return 0;
    }

    // Defensive check: if current is a header, find nearest selectable first
    let start = if is_header(items, current) {
        nearest_selectable(items, current)
    } else {
        current
    };

    let current_pos = selectable.iter().position(|&i| i == start).unwrap_or(0);
    let prev_pos = if current_pos == 0 {
        selectable.len() - 1
    } else {
        current_pos - 1
    };
    selectable[prev_pos]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::SimulatorState;
    use crate::tui::test_utils::test_device_full;

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
        let devices: Vec<Device> = vec![];
        let groups = group_connected_devices(&devices);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_bootable_devices() {
        let ios_simulators = vec![
            IosSimulator {
                udid: "sim1".to_string(),
                name: "iPhone 15 Pro".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 15 Pro".to_string(),
            },
            IosSimulator {
                udid: "sim2".to_string(),
                name: "iPad Pro".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPad Pro".to_string(),
            },
        ];

        let android_avds = vec![AndroidAvd {
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
        let ios_simulators: Vec<IosSimulator> = vec![];
        let android_avds = vec![AndroidAvd {
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
        let ios_simulators = vec![IosSimulator {
            udid: "sim1".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }];
        let android_avds: Vec<AndroidAvd> = vec![];

        let groups = group_bootable_devices(&ios_simulators, &android_avds);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].header, "iOS Simulators");
    }

    #[test]
    fn test_group_bootable_devices_all_empty() {
        let ios_simulators: Vec<IosSimulator> = vec![];
        let android_avds: Vec<AndroidAvd> = vec![];

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
        let sim = IosSimulator {
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
        let avd = AndroidAvd {
            name: "Pixel_6".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        };

        let bootable = GroupedBootableDevice::AndroidAvd(avd);
        assert!(matches!(bootable, GroupedBootableDevice::AndroidAvd(_)));
    }

    #[test]
    fn test_navigation_from_header_position_next() {
        // Simulate corrupted state: selection on header
        let items = vec![
            DeviceListItem::Header("Group A".to_string()),
            DeviceListItem::Device("a1"),
            DeviceListItem::Device("a2"),
            DeviceListItem::Header("Group B".to_string()),
            DeviceListItem::Device("b1"),
        ];

        // Starting from header at index 0
        // nearest_selectable(0) = 1 (first device)
        // next_selectable from 1 = 2 (next device)
        let result = next_selectable(&items, 0);
        // Should return a device, not stay on header
        assert!(!is_header(&items, result));
        assert_eq!(result, 2); // Next device after nearest (1 -> 2)
    }

    #[test]
    fn test_navigation_from_header_position_prev() {
        let items = vec![
            DeviceListItem::Header("Group A".to_string()),
            DeviceListItem::Device("a1"),
            DeviceListItem::Device("a2"),
            DeviceListItem::Header("Group B".to_string()),
            DeviceListItem::Device("b1"),
        ];

        // Starting from header at index 3
        // nearest_selectable(3) = 4 (nearest device forward)
        // prev_selectable from 4 = 2 (wraps around to previous device)
        let result = prev_selectable(&items, 3);
        // Should return a device, not stay on header
        assert!(!is_header(&items, result));
        assert_eq!(result, 2); // Previous device from nearest (4 -> 2)
    }

    #[test]
    fn test_nearest_selectable_forward() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
        ];

        // From header, should go forward to first device
        assert_eq!(nearest_selectable(&items, 0), 1);
    }

    #[test]
    fn test_nearest_selectable_backward() {
        let items = vec![
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
            DeviceListItem::Header("H".to_string()),
        ];

        // From header at end, should go backward to last device
        assert_eq!(nearest_selectable(&items, 2), 1);
    }

    #[test]
    fn test_nearest_selectable_already_selectable() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
        ];

        // Already on a device, should return same index
        assert_eq!(nearest_selectable(&items, 1), 1);
        assert_eq!(nearest_selectable(&items, 2), 2);
    }

    #[test]
    fn test_nearest_selectable_empty() {
        let items: Vec<DeviceListItem<&str>> = vec![];
        assert_eq!(nearest_selectable(&items, 0), 0);
    }

    #[test]
    fn test_nearest_selectable_no_devices() {
        let items = vec![
            DeviceListItem::Header::<&str>("H1".to_string()),
            DeviceListItem::Header::<&str>("H2".to_string()),
        ];
        assert_eq!(nearest_selectable(&items, 0), 0);
    }

    #[test]
    fn test_is_header_true() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
        ];

        assert!(is_header(&items, 0));
    }

    #[test]
    fn test_is_header_false() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
        ];

        assert!(!is_header(&items, 1));
    }

    #[test]
    fn test_is_header_out_of_bounds() {
        let items = vec![DeviceListItem::Device("a")];

        assert!(!is_header(&items, 99));
    }
}
