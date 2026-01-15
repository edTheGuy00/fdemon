# Task: Device Grouping

## Summary

Implement device grouping logic to organize connected devices and bootable devices by platform with section headers.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/device_groups.rs` | Create |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (add export) |

## Implementation

### 1. Define device group structure

```rust
// src/tui/widgets/new_session_dialog/device_groups.rs

use crate::daemon::{Device, BootableDevice, IosSimulator, AndroidAvd};

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
```

### 2. Platform detection for connected devices

```rust
/// Platform category for grouping
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
            ("android", false) => PlatformGroup::AndroidPhysical,
            ("android", true) => PlatformGroup::AndroidEmulator,
            ("chrome" | "web", _) => PlatformGroup::Web,
            ("linux" | "macos" | "windows", _) => PlatformGroup::Desktop,
            _ => PlatformGroup::Other,
        }
    }
}
```

### 3. Group connected devices

```rust
use std::collections::BTreeMap;

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
```

### 4. Group bootable devices

```rust
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

/// Group bootable devices (iOS simulators and Android AVDs)
pub fn group_bootable_devices(
    ios_simulators: &[IosSimulator],
    android_avds: &[AndroidAvd],
) -> Vec<DeviceGroup<BootableDevice>> {
    let mut groups = Vec::new();

    // iOS Simulators group
    if !ios_simulators.is_empty() {
        let devices: Vec<BootableDevice> = ios_simulators
            .iter()
            .cloned()
            .map(BootableDevice::IosSimulator)
            .collect();
        groups.push(DeviceGroup::new(
            BootablePlatformGroup::IosSimulators.header(),
            devices,
        ));
    }

    // Android AVDs group
    if !android_avds.is_empty() {
        let devices: Vec<BootableDevice> = android_avds
            .iter()
            .cloned()
            .map(BootableDevice::AndroidAvd)
            .collect();
        groups.push(DeviceGroup::new(
            BootablePlatformGroup::AndroidAvds.header(),
            devices,
        ));
    }

    groups
}
```

### 5. Flat list with headers for navigation

```rust
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

/// Navigate to next selectable item
pub fn next_selectable<T>(items: &[DeviceListItem<T>], current: usize) -> usize {
    let selectable = selectable_indices(items);
    if selectable.is_empty() {
        return 0;
    }

    // Find current position in selectable list
    let current_pos = selectable.iter().position(|&i| i == current).unwrap_or(0);
    let next_pos = (current_pos + 1) % selectable.len();
    selectable[next_pos]
}

/// Navigate to previous selectable item
pub fn prev_selectable<T>(items: &[DeviceListItem<T>], current: usize) -> usize {
    let selectable = selectable_indices(items);
    if selectable.is_empty() {
        return 0;
    }

    let current_pos = selectable.iter().position(|&i| i == current).unwrap_or(0);
    let prev_pos = if current_pos == 0 {
        selectable.len() - 1
    } else {
        current_pos - 1
    };
    selectable[prev_pos]
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_utils::test_device_full;

    #[test]
    fn test_platform_group_from_device() {
        let ios_physical = test_device_full("id", "iPhone", "ios", false);
        assert_eq!(PlatformGroup::from_device(&ios_physical), PlatformGroup::IosPhysical);

        let android_emulator = test_device_full("id", "Pixel", "android", true);
        assert_eq!(PlatformGroup::from_device(&android_emulator), PlatformGroup::AndroidEmulator);
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
    fn test_prev_selectable() {
        let items = vec![
            DeviceListItem::Header("H".to_string()),
            DeviceListItem::Device("a"),
            DeviceListItem::Device("b"),
        ];

        assert_eq!(prev_selectable(&items, 2), 1);
        assert_eq!(prev_selectable(&items, 1), 2); // Wrap around
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test device_groups && cargo clippy -- -D warnings
```

## Notes

- Empty groups are automatically hidden
- Headers are not selectable in navigation
- Navigation wraps around the selectable items
- Group order is consistent (uses BTreeMap for sorting)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/device_groups.rs` | Created new module with device grouping logic: `DeviceGroup<T>` struct, `PlatformGroup` and `BootablePlatformGroup` enums, `BootableDevice` enum wrapper, grouping functions (`group_connected_devices`, `group_bootable_devices`), and navigation helpers (`flatten_groups`, `selectable_indices`, `next_selectable`, `prev_selectable`). Includes 28 comprehensive unit tests. |
| `src/tui/widgets/new_session_dialog/mod.rs` | Added `device_groups` module declaration and public exports. |

### Notable Decisions/Tradeoffs

1. **Local BootableDevice enum**: Created a local `BootableDevice` enum in the `device_groups` module to wrap `IosSimulator` and `AndroidAvd` for grouping purposes. This is distinct from `core::BootableDevice` (which is a struct) and `daemon::BootCommand` (which has similar variants but different purpose). The TUI layer needs its own representation for UI grouping.

2. **Platform detection robustness**: Enhanced `PlatformGroup::from_device()` to handle platform string variants (e.g., "ios_x64", "android-arm64", "web-javascript", "darwin") in addition to base platforms, ensuring correct grouping regardless of platform string format.

3. **BTreeMap for consistent ordering**: Used `BTreeMap` in `group_connected_devices()` to ensure consistent group ordering based on the `Ord` implementation of `PlatformGroup` enum variants.

4. **Edge case handling**: Added comprehensive edge case handling in navigation functions (`next_selectable`, `prev_selectable`) to handle empty lists, single items, and wrap-around behavior.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test device_groups` - Passed (28 tests)
- `cargo clippy -- -D warnings` - Passed

### Test Coverage

All functionality is fully tested:
- Platform detection for all device types and variants (6 tests)
- Connected device grouping with various scenarios (3 tests)
- Bootable device grouping with various scenarios (4 tests)
- Group flattening and empty group handling (3 tests)
- Selectable indices extraction (3 tests)
- Next/previous navigation with edge cases (6 tests)
- Header string validation (2 tests)
- BootableDevice enum variant construction (2 tests)

### Risks/Limitations

None identified. The implementation follows the task specification exactly and handles all edge cases appropriately.
