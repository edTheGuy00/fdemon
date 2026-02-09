//! Device list widgets for rendering grouped devices with selection
//!
//! This module provides rendering widgets for connected and bootable device lists
//! with headers, selection state, and scrolling support.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Widget},
};

use super::device_groups::{
    flatten_groups, group_bootable_devices, group_connected_devices, DeviceListItem,
    GroupedBootableDevice,
};
use fdemon_app::{config::IconMode, AndroidAvd, Device, IosSimulator, ToolAvailability};

use crate::theme::{icons::IconSet, palette};

/// Minimum width (in columns) to show verbose scroll indicators ("↑ more").
/// Below this threshold, compact indicators ("↑") are shown.
const VERBOSE_INDICATOR_WIDTH_THRESHOLD: u16 = 50;

/// Determine icon for a device based on platform_type
fn device_icon(platform_type: &str, _is_emulator: bool, icons: &IconSet) -> &'static str {
    let platform_lower = platform_type.to_lowercase();

    if platform_lower.contains("ios") || platform_lower.contains("android") {
        icons.smartphone()
    } else if platform_lower.contains("web") || platform_lower.contains("chrome") {
        icons.globe()
    } else if platform_lower.contains("macos")
        || platform_lower.contains("linux")
        || platform_lower.contains("windows")
        || platform_lower.contains("darwin")
    {
        icons.monitor()
    } else {
        icons.cpu()
    }
}

/// Determine icon for a bootable device based on platform
fn bootable_device_icon(platform: &str, icons: &IconSet) -> &'static str {
    let platform_lower = platform.to_lowercase();

    if platform_lower.contains("ios") || platform_lower.contains("android") {
        icons.smartphone()
    } else if platform_lower.contains("web") {
        icons.globe()
    } else {
        icons.cpu()
    }
}

/// Widget for rendering connected devices with grouping
pub struct ConnectedDeviceList<'a> {
    devices: &'a [Device],
    selected_index: usize,
    is_focused: bool,
    scroll_offset: usize,
    icons: IconSet,
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
            icons: IconSet::new(IconMode::Unicode), // Default to Unicode for compatibility
        }
    }

    /// Set the icon mode (chainable builder)
    pub fn with_icons(mut self, icon_mode: IconMode) -> Self {
        self.icons = IconSet::new(icon_mode);
        self
    }

    fn render_item(
        &self,
        item: &DeviceListItem<&Device>,
        index: usize,
        area_width: u16,
    ) -> ListItem<'static> {
        match item {
            DeviceListItem::Header(header) => {
                // Uppercase header with ACCENT_DIM color
                let header_style = Style::default()
                    .fg(palette::ACCENT_DIM)
                    .add_modifier(Modifier::BOLD);
                let header_upper = header.to_uppercase();
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(header_upper, header_style),
                ]))
            }
            DeviceListItem::Device(device) => {
                let is_selected = index == self.selected_index;

                // Updated selection highlighting
                let style = if is_selected && self.is_focused {
                    Style::default()
                        .fg(palette::TEXT_BRIGHT)
                        .bg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default()
                        .fg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
                };

                // Platform icon - use platform_type if available, fallback to platform
                let platform = device.platform_type.as_deref().unwrap_or(&device.platform);
                let icon = device_icon(platform, device.emulator, &self.icons);
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
                // Format: " <icon> <name> (<type>)"
                let prefix = format!(" {} ", icon);
                let type_suffix = format!(" ({})", device_type);
                let reserved = prefix.len() + type_suffix.len();
                let available_width = (area_width as usize).saturating_sub(reserved);

                // Truncate device name if needed
                let name = if available_width > 0 {
                    super::truncate_with_ellipsis(&device.name, available_width)
                } else {
                    device.name.clone()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(name, style),
                    Span::styled(type_suffix, Style::default().fg(palette::TEXT_MUTED)),
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
                Style::default().fg(palette::BORDER_DIM),
            );
        }

        // Show down indicator if more items below
        if end < total {
            let x = area.right().saturating_sub(down_indicator.len() as u16 + 1);
            let y = area.bottom().saturating_sub(1);
            buf.set_string(
                x,
                y,
                down_indicator,
                Style::default().fg(palette::BORDER_DIM),
            );
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
    icons: IconSet,
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
            icons: IconSet::new(IconMode::Unicode), // Default to Unicode for compatibility
        }
    }

    /// Set the icon mode (chainable builder)
    pub fn with_icons(mut self, icon_mode: IconMode) -> Self {
        self.icons = IconSet::new(icon_mode);
        self
    }

    fn render_item(
        &self,
        item: &DeviceListItem<GroupedBootableDevice>,
        index: usize,
        area_width: u16,
    ) -> ListItem<'static> {
        match item {
            DeviceListItem::Header(header) => {
                // Uppercase header with ACCENT_DIM color
                let header_style = Style::default()
                    .fg(palette::ACCENT_DIM)
                    .add_modifier(Modifier::BOLD);
                let header_upper = header.to_uppercase();
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(header_upper, header_style),
                ]))
            }
            DeviceListItem::Device(device) => {
                let is_selected = index == self.selected_index;

                // Updated selection highlighting
                let style = if is_selected && self.is_focused {
                    Style::default()
                        .fg(palette::TEXT_BRIGHT)
                        .bg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default()
                        .fg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
                };

                // Platform icon
                let icon = bootable_device_icon(device.platform(), &self.icons);
                let runtime = device.runtime_info();

                // Calculate available width for device name
                // Format: " <icon> <name> (<runtime>)"
                let prefix = format!(" {} ", icon);
                let runtime_suffix = format!(" ({})", runtime);
                let reserved = prefix.len() + runtime_suffix.len();
                let available_width = (area_width as usize).saturating_sub(reserved);

                // Truncate device name if needed
                let name = if available_width > 0 {
                    super::truncate_with_ellipsis(device.display_name(), available_width)
                } else {
                    device.display_name().to_string()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(name, style),
                    Span::styled(runtime_suffix, Style::default().fg(palette::TEXT_MUTED)),
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
                Style::default().fg(palette::STATUS_YELLOW),
            )));
        }

        if let Some(msg) = self.tool_availability.android_unavailable_message() {
            messages.push(Line::from(Span::styled(
                msg,
                Style::default().fg(palette::STATUS_YELLOW),
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
                Style::default().fg(palette::BORDER_DIM),
            );
        }

        // Show down indicator if more items below
        if end < total {
            let x = area.right().saturating_sub(down_indicator.len() as u16 + 1);
            let y = area.bottom().saturating_sub(1);
            buf.set_string(
                x,
                y,
                down_indicator,
                Style::default().fg(palette::BORDER_DIM),
            );
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
                .style(Style::default().fg(palette::TEXT_MUTED));
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
    use crate::test_utils::{test_device_full, TestTerminal};
    use fdemon_daemon::SimulatorState;

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
        // Headers are rendered in uppercase in the new design
        assert!(content.contains("IOS DEVICES") || content.contains("ANDROID DEVICES"));
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

    // Removed test_device_list_styles_default - DeviceListStyles struct was removed in theme migration
}
