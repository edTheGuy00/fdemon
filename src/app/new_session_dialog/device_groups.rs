//! Device grouping types for the new session dialog.
//!
//! These types organize connected and bootable devices for display in the
//! target selector. This module contains pure data transformation logic with
//! no ratatui dependencies.

use crate::daemon::{AndroidAvd, Device, IosSimulator};
use std::collections::BTreeMap;

// ─────────────────────────────────────────────────────────────────────────────
// Core Types
// ─────────────────────────────────────────────────────────────────────────────

/// Bootable device wrapper for grouping.
///
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

/// Item in a flat device list (either header or device)
#[derive(Debug, Clone)]
pub enum DeviceListItem<T> {
    Header(String),
    Device(T),
}

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

// ─────────────────────────────────────────────────────────────────────────────
// Platform Grouping
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Flattening and Navigation
// ─────────────────────────────────────────────────────────────────────────────

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
