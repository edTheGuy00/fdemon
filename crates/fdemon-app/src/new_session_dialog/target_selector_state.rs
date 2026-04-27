//! Target selector state for the new session dialog.
//!
//! This module contains the state management for the target selector,
//! which allows users to choose between connected and bootable devices.

use std::cell::Cell;

use super::device_groups::{
    flatten_groups, group_bootable_devices, group_connected_devices, next_selectable,
    prev_selectable, DeviceListItem, GroupedBootableDevice,
};
use super::TargetTab;
use fdemon_daemon::{AndroidAvd, Device, IosSimulator};

/// State for the Target Selector pane.
///
/// Note: `last_known_visible_height` uses `Cell<usize>` interior mutability and is
/// written by the renderer each frame as a render-hint feedback channel. It must not
/// be used as a correctness input to business logic or participate in state equality
/// comparisons. See `docs/REVIEW_FOCUS.md` "Approved TEA Exception" for rationale.
#[derive(Debug, Clone)]
pub struct TargetSelectorState {
    /// Currently active tab
    pub active_tab: TargetTab,

    /// Connected devices (from flutter devices)
    pub connected_devices: Vec<Device>,

    /// iOS simulators (from xcrun simctl)
    pub ios_simulators: Vec<IosSimulator>,

    /// Android AVDs (from emulator -list-avds)
    pub android_avds: Vec<AndroidAvd>,

    /// Selected index in current tab's flattened list
    pub selected_index: usize,

    /// Loading state for device discovery
    pub loading: bool,

    /// Loading state for bootable device discovery
    pub bootable_loading: bool,

    /// Background refresh in progress for connected devices.
    ///
    /// Distinct from `loading`: `loading` shows a full-screen spinner with no
    /// content, whereas `refreshing` is set when the cached list is already shown
    /// and a background discovery is updating it in place. Cleared by
    /// `set_connected_devices()` and `set_error()`.
    pub refreshing: bool,

    /// Background refresh in progress for bootable devices.
    ///
    /// Mirror of `refreshing` for the bootable tab. Cleared by
    /// `set_bootable_devices()`.
    pub bootable_refreshing: bool,

    /// Error message if discovery failed
    pub error: Option<String>,

    /// Scroll offset for device list (number of items scrolled past)
    pub scroll_offset: usize,

    /// Last-known visible height of the device list area (in rows).
    ///
    /// Written by the renderer each frame via interior mutability (`Cell`).
    /// Read by the handler to compute accurate scroll offsets.
    /// Defaults to 0, which signals "no render has occurred yet" — the handler
    /// falls back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` when this is 0.
    pub last_known_visible_height: Cell<usize>,

    /// Cached flattened device list, invalidated on device updates
    pub cached_flat_list: Option<Vec<DeviceListItem<String>>>,
}

impl Default for TargetSelectorState {
    fn default() -> Self {
        Self {
            active_tab: TargetTab::Connected,
            connected_devices: Vec::new(),
            ios_simulators: Vec::new(),
            android_avds: Vec::new(),
            selected_index: 0,
            loading: true,
            bootable_loading: true,
            refreshing: false,
            bootable_refreshing: false,
            error: None,
            scroll_offset: 0,
            last_known_visible_height: Cell::new(0),
            cached_flat_list: None,
        }
    }
}

impl TargetSelectorState {
    /// Creates a new TargetSelectorState with default settings.
    ///
    /// Starts on the Connected tab with no devices loaded.
    /// Selection is initially at index 0 (will be adjusted when devices load).
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch to a specific tab
    pub fn set_tab(&mut self, tab: TargetTab) {
        if self.active_tab != tab {
            self.active_tab = tab;
            self.invalidate_cache();
            self.selected_index = self.first_selectable_index();
            self.scroll_offset = 0; // Reset scroll when switching tabs
        }
    }

    /// Toggle between tabs
    pub fn toggle_tab(&mut self) {
        self.set_tab(self.active_tab.toggle());
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let current = self.selected_index;
        let items = self.flat_list();
        if !items.is_empty() {
            self.selected_index = next_selectable(items, current);
        }
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        let current = self.selected_index;
        let items = self.flat_list();
        if !items.is_empty() {
            self.selected_index = prev_selectable(items, current);
        }
    }

    /// Get first selectable index in current tab
    fn first_selectable_index(&self) -> usize {
        let items = self.compute_flat_list();
        items
            .iter()
            .enumerate()
            .find_map(|(i, item)| match item {
                DeviceListItem::Device(_) => Some(i),
                DeviceListItem::Header(_) => None,
            })
            .unwrap_or(0)
    }

    /// Returns cached flat list, computing if necessary
    pub fn flat_list(&mut self) -> &[DeviceListItem<String>] {
        if self.cached_flat_list.is_none() {
            self.cached_flat_list = Some(self.compute_flat_list());
        }
        self.cached_flat_list.as_ref().unwrap()
    }

    /// Invalidate the cached flat list
    fn invalidate_cache(&mut self) {
        self.cached_flat_list = None;
    }

    /// Compute flattened list for current tab (internal helper)
    fn compute_flat_list(&self) -> Vec<DeviceListItem<String>> {
        match self.active_tab {
            TargetTab::Connected => {
                let groups = group_connected_devices(&self.connected_devices);
                flatten_groups(&groups)
                    .into_iter()
                    .map(|item| match item {
                        DeviceListItem::Header(h) => DeviceListItem::Header(h),
                        DeviceListItem::Device(d) => DeviceListItem::Device(d.id.clone()),
                    })
                    .collect()
            }
            TargetTab::Bootable => {
                let groups = group_bootable_devices(&self.ios_simulators, &self.android_avds);
                flatten_groups(&groups)
                    .into_iter()
                    .map(|item| match item {
                        DeviceListItem::Header(h) => DeviceListItem::Header(h),
                        DeviceListItem::Device(d) => match d {
                            GroupedBootableDevice::IosSimulator(sim) => {
                                DeviceListItem::Device(sim.udid.clone())
                            }
                            GroupedBootableDevice::AndroidAvd(avd) => {
                                DeviceListItem::Device(avd.name.clone())
                            }
                        },
                    })
                    .collect()
            }
        }
    }

    /// Get currently selected connected device
    pub fn selected_connected_device(&self) -> Option<&Device> {
        if self.active_tab != TargetTab::Connected {
            return None;
        }

        let groups = group_connected_devices(&self.connected_devices);
        let items = flatten_groups(&groups);

        items.get(self.selected_index).and_then(|item| match item {
            DeviceListItem::Device(device) => Some(*device),
            DeviceListItem::Header(_) => None,
        })
    }

    /// Get currently selected bootable device
    pub fn selected_bootable_device(&self) -> Option<GroupedBootableDevice> {
        if self.active_tab != TargetTab::Bootable {
            return None;
        }

        let groups = group_bootable_devices(&self.ios_simulators, &self.android_avds);
        let items = flatten_groups(&groups);

        items.get(self.selected_index).and_then(|item| match item {
            DeviceListItem::Device(device) => Some(device.clone()),
            DeviceListItem::Header(_) => None,
        })
    }

    /// Set connected devices
    pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
        self.connected_devices = devices;
        self.loading = false;
        self.refreshing = false;
        self.error = None;
        self.invalidate_cache();
        self.scroll_offset = 0; // Reset scroll when devices change

        // Reset selection if it's now invalid
        if self.active_tab == TargetTab::Connected {
            let max_index = self.compute_flat_list().len().saturating_sub(1);
            if self.selected_index > max_index {
                self.selected_index = self.first_selectable_index();
            }
        }
    }

    /// Set bootable devices
    pub fn set_bootable_devices(
        &mut self,
        ios_simulators: Vec<IosSimulator>,
        android_avds: Vec<AndroidAvd>,
    ) {
        self.ios_simulators = ios_simulators;
        self.android_avds = android_avds;
        self.bootable_loading = false;
        self.bootable_refreshing = false;
        // NOTE: do NOT clear self.error here — bootable device discovery uses
        // xcrun/emulator tools that are independent of the Flutter SDK.
        // SDK-level errors (e.g. "Flutter SDK not found") must persist until
        // set_connected_devices() confirms a working SDK by succeeding.
        self.invalidate_cache();
        self.scroll_offset = 0; // Reset scroll when devices change

        // Reset selection if on bootable tab
        if self.active_tab == TargetTab::Bootable {
            let max_index = self.compute_flat_list().len().saturating_sub(1);
            if self.selected_index > max_index {
                self.selected_index = self.first_selectable_index();
            }
        }
    }

    /// Set a new-session dialog error state.
    ///
    /// This helper is used by many new-session error paths, not just the
    /// connected-device foreground discovery failure path. Callers include device
    /// discovery failures, session creation failures, boot failures, config save
    /// errors, "no Flutter SDK" surfaces from the launch context and dialog open,
    /// and several validation paths.
    ///
    /// It records the error and clears the connected-side `loading` and
    /// `refreshing` flags so the UI does not remain stuck in a connected
    /// in-progress state after an error is surfaced.
    ///
    /// `bootable_loading` and `bootable_refreshing` are intentionally **not**
    /// cleared here. Bootable discovery is independent (xcrun/emulator tools,
    /// not the Flutter SDK) and its in-flight flags are managed by their own
    /// success/failure paths. Callers that need to clear bootable indicators on
    /// a particular error must do so themselves.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
        self.refreshing = false;
    }

    /// Adjust scroll offset to keep selected item visible
    ///
    /// # Arguments
    /// * `visible_height` - Number of items that can be displayed
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }

        self.scroll_offset =
            calculate_scroll_offset(self.selected_index, visible_height, self.scroll_offset);
    }

    /// Reset scroll offset (called when switching tabs or updating device list)
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    // Selection Preservation Helpers (Task 04 - Device Cache Usage)
    // ─────────────────────────────────────────────────────────────

    /// Get the currently selected device ID (if any)
    ///
    /// Returns the device ID of the currently selected item in the Connected tab.
    /// Returns None if on Bootable tab or no device is selected.
    pub fn selected_device_id(&self) -> Option<String> {
        if self.active_tab != TargetTab::Connected {
            return None;
        }

        let groups = group_connected_devices(&self.connected_devices);
        let items = flatten_groups(&groups);

        items.get(self.selected_index).and_then(|item| match item {
            DeviceListItem::Device(d) => Some(d.id.clone()),
            DeviceListItem::Header(_) => None,
        })
    }

    /// Select device by ID if it exists in the list
    ///
    /// Searches the connected devices list for a device with the given ID
    /// and updates the selection index if found.
    ///
    /// Returns true if the device was found and selected, false otherwise.
    pub fn select_device_by_id(&mut self, device_id: &str) -> bool {
        if self.active_tab != TargetTab::Connected {
            return false;
        }

        let groups = group_connected_devices(&self.connected_devices);
        let items = flatten_groups(&groups);

        for (index, item) in items.iter().enumerate() {
            if let DeviceListItem::Device(d) = item {
                if d.id == device_id {
                    self.selected_index = index;
                    return true;
                }
            }
        }
        false
    }

    /// Reset selection to first selectable device in current tab
    ///
    /// Useful after device list changes when previous selection is no longer valid.
    pub fn reset_selection_to_first(&mut self) {
        self.selected_index = self.first_selectable_index();
    }
}

/// Calculate scroll offset to keep selection visible
///
/// # Arguments
/// * `selected_index` - Index of currently selected item
/// * `visible_height` - Number of items that fit in the visible area
/// * `current_offset` - Current scroll offset
///
/// # Returns
/// The new scroll offset that keeps the selection visible
// TODO: deduplicate with device_list::calculate_scroll_offset — move to fdemon-core
fn calculate_scroll_offset(
    selected_index: usize,
    visible_height: usize,
    current_offset: usize,
) -> usize {
    if visible_height == 0 {
        return 0;
    }

    // If selection is above visible area, scroll up
    if selected_index < current_offset {
        return selected_index;
    }

    // If selection is below visible area, scroll down
    if selected_index >= current_offset + visible_height {
        return selected_index - visible_height + 1;
    }

    // Selection is visible, keep current offset
    current_offset
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_known_visible_height_default_is_zero() {
        let state = TargetSelectorState::default();
        assert_eq!(state.last_known_visible_height.get(), 0);
    }

    #[test]
    fn test_last_known_visible_height_set_and_get() {
        let state = TargetSelectorState::default();
        state.last_known_visible_height.set(15);
        assert_eq!(state.last_known_visible_height.get(), 15);
    }

    #[test]
    fn test_last_known_visible_height_survives_clone() {
        let state = TargetSelectorState::default();
        state.last_known_visible_height.set(20);
        let cloned = state.clone();
        assert_eq!(cloned.last_known_visible_height.get(), 20);
    }

    #[test]
    fn test_last_known_visible_height_writable_through_shared_ref() {
        let state = TargetSelectorState::default();
        let shared: &TargetSelectorState = &state;
        shared.last_known_visible_height.set(12);
        assert_eq!(state.last_known_visible_height.get(), 12);
    }

    #[test]
    fn test_set_bootable_devices_does_not_clear_error() {
        let mut state = TargetSelectorState::default();
        state.set_error("Flutter SDK not found".to_string());
        assert!(state.error.is_some());

        state.set_bootable_devices(vec![], vec![]);

        // Error should persist — bootable discovery is independent of SDK
        assert!(state.error.is_some());
        assert_eq!(state.error.as_deref(), Some("Flutter SDK not found"));
        assert!(!state.bootable_loading);
    }

    #[test]
    fn test_set_connected_devices_clears_error() {
        let mut state = TargetSelectorState::default();
        state.set_error("Flutter SDK not found".to_string());
        assert!(state.error.is_some());

        state.set_connected_devices(vec![]);

        // Error should be cleared — successful device discovery means SDK is working
        assert!(state.error.is_none());
        assert!(!state.loading);
    }

    #[test]
    fn test_refreshing_default_false() {
        let state = TargetSelectorState::default();
        assert!(!state.refreshing);
        assert!(!state.bootable_refreshing);
    }

    #[test]
    fn test_set_connected_devices_clears_refreshing() {
        let mut state = TargetSelectorState {
            refreshing: true,
            ..Default::default()
        };
        state.set_connected_devices(vec![]);
        assert!(!state.refreshing);
    }

    #[test]
    fn test_set_bootable_devices_clears_bootable_refreshing() {
        let mut state = TargetSelectorState {
            bootable_refreshing: true,
            ..Default::default()
        };
        state.set_bootable_devices(vec![], vec![]);
        assert!(!state.bootable_refreshing);
    }

    #[test]
    fn test_set_error_clears_refreshing() {
        let mut state = TargetSelectorState {
            refreshing: true,
            ..Default::default()
        };
        state.set_error("boom".to_string());
        assert!(!state.refreshing);
    }
}
