//! Device list widgets for rendering grouped devices with selection
//!
//! This module provides rendering widgets for connected and bootable device lists
//! with headers, selection state, and scrolling support.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Widget},
};

use super::device_groups::{
    flatten_groups, group_bootable_devices, group_connected_devices, DeviceListItem,
    GroupedBootableDevice,
};
use crate::daemon::{AndroidAvd, Device, IosSimulator, ToolAvailability};

/// Minimum width (in columns) to show verbose scroll indicators ("↑ more").
/// Below this threshold, compact indicators ("↑") are shown.
const VERBOSE_INDICATOR_WIDTH_THRESHOLD: u16 = 50;

/// Styling configuration for device list rendering.
///
/// Defines colors and styles for headers, devices, selection indicators,
/// and various device states (connected, disconnected, booting).
#[derive(Debug, Clone)]
pub struct DeviceListStyles {
    pub header: Style,
    pub device_normal: Style,
    pub device_selected: Style,
    pub device_selected_focused: Style,
    pub info: Style,
}

impl Default for DeviceListStyles {
    fn default() -> Self {
        Self {
            header: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            device_normal: Style::default(),
            device_selected: Style::default().add_modifier(Modifier::BOLD),
            device_selected_focused: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            info: Style::default().fg(Color::DarkGray),
        }
    }
}

/// Widget for rendering connected devices with grouping
pub struct ConnectedDeviceList<'a> {
    devices: &'a [Device],
    selected_index: usize,
    is_focused: bool,
    scroll_offset: usize,
    styles: DeviceListStyles,
}

impl<'a> ConnectedDeviceList<'a> {
    pub fn new(
        devices: &'a [Device],
        selected_index: usize,
        is_focused: bool,
        scroll_offset: usize,
    ) -> Self {
        Self {
            devices,
            selected_index,
            is_focused,
            scroll_offset,
            styles: DeviceListStyles::default(),
        }
    }

    fn render_item(
        &self,
        item: &DeviceListItem<&Device>,
        index: usize,
        area_width: u16,
    ) -> ListItem<'static> {
        match item {
            DeviceListItem::Header(header) => ListItem::new(Line::from(vec![
                Span::styled("  ", self.styles.device_normal),
                Span::styled(header.clone(), self.styles.header),
            ])),
            DeviceListItem::Device(device) => {
                let is_selected = index == self.selected_index;
                let style = if is_selected && self.is_focused {
                    self.styles.device_selected_focused
                } else if is_selected {
                    self.styles.device_selected
                } else {
                    self.styles.device_normal
                };

                let indicator = if is_selected { "▶ " } else { "  " };
                let device_type = if device.emulator {
                    device
                        .emulator_id
                        .as_ref()
                        .map(|_| "emulator")
                        .unwrap_or("simulator")
                } else {
                    "physical"
                };

                // Calculate available width for device name
                // Format: "▶ <name> (<type>)"
                let type_suffix = format!(" ({})", device_type);
                let reserved = indicator.len() + type_suffix.len();
                let available_width = (area_width as usize).saturating_sub(reserved);

                // Truncate device name if needed
                let name = if available_width > 0 {
                    super::truncate_with_ellipsis(&device.name, available_width)
                } else {
                    device.name.clone()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(name, style),
                    Span::styled(type_suffix, self.styles.info),
                ]))
            }
        }
    }

    fn render_scroll_indicators(
        &self,
        area: Rect,
        buf: &mut Buffer,
        start: usize,
        end: usize,
        total: usize,
    ) {
        // Use shorter indicators in narrow terminals
        let (up_indicator, down_indicator) = if area.width < VERBOSE_INDICATOR_WIDTH_THRESHOLD {
            ("↑", "↓")
        } else {
            ("↑ more", "↓ more")
        };

        // Show up indicator if scrolled down
        if start > 0 {
            let x = area.right().saturating_sub(up_indicator.len() as u16 + 1);
            buf.set_string(
                x,
                area.top(),
                up_indicator,
                Style::default().fg(Color::DarkGray),
            );
        }

        // Show down indicator if more items below
        if end < total {
            let x = area.right().saturating_sub(down_indicator.len() as u16 + 1);
            let y = area.bottom().saturating_sub(1);
            buf.set_string(x, y, down_indicator, Style::default().fg(Color::DarkGray));
        }
    }
}

impl Widget for ConnectedDeviceList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let groups = group_connected_devices(self.devices);
        let items = flatten_groups(&groups);

        // Calculate visible range
        let visible_height = area.height as usize;
        let start = self.scroll_offset.min(items.len().saturating_sub(1));
        let end = (start + visible_height).min(items.len());

        // Create list items only for visible range
        let list_items: Vec<ListItem> = items[start..end]
            .iter()
            .enumerate()
            .map(|(visible_idx, item)| {
                let actual_idx = start + visible_idx;
                self.render_item(item, actual_idx, area.width)
            })
            .collect();

        let list = List::new(list_items);
        list.render(area, buf);

        // Render scroll indicators
        self.render_scroll_indicators(area, buf, start, end, items.len());
    }
}

/// Widget for rendering bootable devices with grouping
pub struct BootableDeviceList<'a> {
    ios_simulators: &'a [IosSimulator],
    android_avds: &'a [AndroidAvd],
    selected_index: usize,
    is_focused: bool,
    scroll_offset: usize,
    tool_availability: &'a ToolAvailability,
    styles: DeviceListStyles,
}

impl<'a> BootableDeviceList<'a> {
    pub fn new(
        ios_simulators: &'a [IosSimulator],
        android_avds: &'a [AndroidAvd],
        selected_index: usize,
        is_focused: bool,
        scroll_offset: usize,
        tool_availability: &'a ToolAvailability,
    ) -> Self {
        Self {
            ios_simulators,
            android_avds,
            selected_index,
            is_focused,
            scroll_offset,
            tool_availability,
            styles: DeviceListStyles::default(),
        }
    }

    fn render_item(
        &self,
        item: &DeviceListItem<GroupedBootableDevice>,
        index: usize,
        area_width: u16,
    ) -> ListItem<'static> {
        match item {
            DeviceListItem::Header(header) => ListItem::new(Line::from(vec![
                Span::styled("  ", self.styles.device_normal),
                Span::styled(header.clone(), self.styles.header),
            ])),
            DeviceListItem::Device(device) => {
                let is_selected = index == self.selected_index;
                let style = if is_selected && self.is_focused {
                    self.styles.device_selected_focused
                } else if is_selected {
                    self.styles.device_selected
                } else {
                    self.styles.device_normal
                };

                let indicator = if is_selected { "▶ " } else { "  " };
                let runtime = device.runtime_info();

                // Calculate available width for device name
                // Format: "▶ <name> (<runtime>)"
                let runtime_suffix = format!(" ({})", runtime);
                let reserved = indicator.len() + runtime_suffix.len();
                let available_width = (area_width as usize).saturating_sub(reserved);

                // Truncate device name if needed
                let name = if available_width > 0 {
                    super::truncate_with_ellipsis(device.display_name(), available_width)
                } else {
                    device.display_name().to_string()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(name, style),
                    Span::styled(runtime_suffix, self.styles.info),
                ]))
            }
        }
    }

    fn render_unavailable_message(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::layout::Alignment;
        use ratatui::widgets::Paragraph;

        let mut messages = Vec::new();

        if let Some(msg) = self.tool_availability.ios_unavailable_message() {
            messages.push(Line::from(Span::styled(
                msg,
                Style::default().fg(Color::Yellow),
            )));
        }

        if let Some(msg) = self.tool_availability.android_unavailable_message() {
            messages.push(Line::from(Span::styled(
                msg,
                Style::default().fg(Color::Yellow),
            )));
        }

        if !messages.is_empty() {
            messages.insert(0, Line::from(""));
            let paragraph = Paragraph::new(messages).alignment(Alignment::Center);
            paragraph.render(area, buf);
        }
    }

    fn render_scroll_indicators(
        &self,
        area: Rect,
        buf: &mut Buffer,
        start: usize,
        end: usize,
        total: usize,
    ) {
        // Use shorter indicators in narrow terminals
        let (up_indicator, down_indicator) = if area.width < VERBOSE_INDICATOR_WIDTH_THRESHOLD {
            ("↑", "↓")
        } else {
            ("↑ more", "↓ more")
        };

        // Show up indicator if scrolled down
        if start > 0 {
            let x = area.right().saturating_sub(up_indicator.len() as u16 + 1);
            buf.set_string(
                x,
                area.top(),
                up_indicator,
                Style::default().fg(Color::DarkGray),
            );
        }

        // Show down indicator if more items below
        if end < total {
            let x = area.right().saturating_sub(down_indicator.len() as u16 + 1);
            let y = area.bottom().saturating_sub(1);
            buf.set_string(x, y, down_indicator, Style::default().fg(Color::DarkGray));
        }
    }
}

impl Widget for BootableDeviceList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Check if any tools are unavailable
        let ios_unavailable = !self.tool_availability.xcrun_simctl;
        let android_unavailable = !self.tool_availability.android_emulator;

        // If both are unavailable, show message
        if ios_unavailable && android_unavailable {
            self.render_unavailable_message(area, buf);
            return;
        }

        let groups = group_bootable_devices(self.ios_simulators, self.android_avds);
        let items = flatten_groups(&groups);

        if items.is_empty() {
            // No devices found
            use ratatui::layout::Alignment;
            use ratatui::widgets::Paragraph;

            let msg = Paragraph::new("No bootable devices found")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            msg.render(area, buf);
            return;
        }

        // Calculate visible range
        let visible_height = area.height as usize;
        let start = self.scroll_offset.min(items.len().saturating_sub(1));
        let end = (start + visible_height).min(items.len());

        // Create list items only for visible range
        let list_items: Vec<ListItem> = items[start..end]
            .iter()
            .enumerate()
            .map(|(visible_idx, item)| {
                let actual_idx = start + visible_idx;
                self.render_item(item, actual_idx, area.width)
            })
            .collect();

        let list = List::new(list_items);
        list.render(area, buf);

        // Render scroll indicators
        self.render_scroll_indicators(area, buf, start, end, items.len());
    }
}

/// Calculate scroll offset to keep selection visible
///
/// # Arguments
/// * `selected_index` - The currently selected item index
/// * `visible_height` - Number of items that can fit on screen
/// * `current_offset` - Current scroll offset
///
/// # Returns
/// The new scroll offset that keeps the selection visible
pub fn calculate_scroll_offset(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::SimulatorState;
    use crate::tui::test_utils::{test_device_full, TestTerminal};

    #[test]
    fn test_connected_device_list_renders() {
        let devices = vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "Pixel 8", "android", false),
        ];

        let mut terminal = TestTerminal::new();

        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();

        assert!(content.contains("iPhone 15"));
        assert!(content.contains("Pixel 8"));
        assert!(content.contains("iOS Devices"));
    }

    #[test]
    fn test_bootable_device_list_renders() {
        let ios_sims = vec![IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15 Pro".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15 Pro".to_string(),
        }];

        let android_avds = vec![AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        }];

        let tool_availability = ToolAvailability {
            xcrun_simctl: true,
            android_emulator: true,
            emulator_path: None,
        };

        let mut terminal = TestTerminal::new();

        terminal.draw_with(|f| {
            let list =
                BootableDeviceList::new(&ios_sims, &android_avds, 0, true, 0, &tool_availability);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();

        assert!(content.contains("iPhone 15 Pro"));
        assert!(content.contains("Pixel 6"));
    }

    #[test]
    fn test_bootable_device_list_unavailable_tools() {
        let ios_sims = vec![];
        let android_avds = vec![];

        let tool_availability = ToolAvailability {
            xcrun_simctl: false,
            android_emulator: false,
            emulator_path: None,
        };

        let mut terminal = TestTerminal::new();

        terminal.draw_with(|f| {
            let list =
                BootableDeviceList::new(&ios_sims, &android_avds, 0, true, 0, &tool_availability);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();

        // Should show unavailable message
        assert!(content.contains("Android SDK") || content.contains("Xcode"));
    }

    #[test]
    fn test_bootable_device_list_empty() {
        let ios_sims = vec![];
        let android_avds = vec![];

        let tool_availability = ToolAvailability {
            xcrun_simctl: true,
            android_emulator: true,
            emulator_path: None,
        };

        let mut terminal = TestTerminal::new();

        terminal.draw_with(|f| {
            let list =
                BootableDeviceList::new(&ios_sims, &android_avds, 0, true, 0, &tool_availability);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();

        // Should show empty state message
        assert!(content.contains("No bootable devices found"));
    }

    #[test]
    fn test_calculate_scroll_offset_selection_visible() {
        // Selection visible, no scroll needed
        assert_eq!(calculate_scroll_offset(5, 10, 0), 0);
    }

    #[test]
    fn test_calculate_scroll_offset_selection_above() {
        // Selection above visible area, scroll up
        assert_eq!(calculate_scroll_offset(2, 10, 5), 2);
    }

    #[test]
    fn test_calculate_scroll_offset_selection_below() {
        // Selection below visible area, scroll down
        assert_eq!(calculate_scroll_offset(15, 10, 0), 6);
    }

    #[test]
    fn test_calculate_scroll_offset_zero_height() {
        // Zero height should return 0
        assert_eq!(calculate_scroll_offset(5, 0, 3), 0);
    }

    #[test]
    fn test_calculate_scroll_offset_at_bottom_edge() {
        // Selection at bottom edge of visible area
        assert_eq!(calculate_scroll_offset(9, 10, 0), 0);
    }

    #[test]
    fn test_calculate_scroll_offset_at_top_edge() {
        // Selection at top edge of visible area
        assert_eq!(calculate_scroll_offset(5, 10, 5), 5);
    }

    #[test]
    fn test_bootable_device_display_name_ios() {
        let sim = IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        };

        let device = GroupedBootableDevice::IosSimulator(sim);
        assert_eq!(device.display_name(), "iPhone 15");
        assert_eq!(device.platform(), "iOS");
        assert_eq!(device.runtime_info(), "iOS 17.2");
    }

    #[test]
    fn test_bootable_device_display_name_android() {
        let avd = AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        };

        let device = GroupedBootableDevice::AndroidAvd(avd);
        assert_eq!(device.display_name(), "Pixel 6");
        assert_eq!(device.platform(), "Android");
        assert_eq!(device.runtime_info(), "API 33");
    }

    #[test]
    fn test_bootable_device_android_no_api() {
        let avd = AndroidAvd {
            name: "Custom".to_string(),
            display_name: "Custom AVD".to_string(),
            api_level: None,
            target: None,
        };

        let device = GroupedBootableDevice::AndroidAvd(avd);
        assert_eq!(device.runtime_info(), "Unknown API");
    }

    #[test]
    fn test_device_list_styles_default() {
        let styles = DeviceListStyles::default();
        assert_eq!(styles.header.fg, Some(Color::Yellow));
        assert_eq!(styles.device_selected_focused.bg, Some(Color::Cyan));
    }
}
