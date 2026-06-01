//! Device list widgets for rendering grouped devices with selection
//!
//! This module provides rendering widgets for connected and bootable device lists
//! with headers, selection state, and scrolling support.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Widget},
};

use super::device_groups::{
    flatten_groups, group_bootable_devices, group_connected_devices, DeviceListItem,
    GroupedBootableDevice,
};
use fdemon_app::message::Message;
use fdemon_app::{AndroidAvd, Device, IosSimulator, ToolAvailability};
use fdemon_app::{MouseAction, MouseRect};

use crate::theme::palette;

/// Minimum width (in columns) to show verbose scroll indicators ("↑ more").
/// Below this threshold, compact indicators ("↑") are shown.
const VERBOSE_INDICATOR_WIDTH_THRESHOLD: u16 = 50;

/// Widget for rendering connected devices with grouping
pub struct ConnectedDeviceList<'a> {
    devices: &'a [Device],
    selected_index: usize,
    is_focused: bool,
    scroll_offset: usize,
    /// Device ids that are checked for multi-launch. Independent of the cursor.
    checked: Option<&'a std::collections::BTreeSet<String>>,
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
            checked: None,
        }
    }

    /// Attach the checked-device set (chainable builder).
    ///
    /// When set, each connected-device row displays a `[x]` or `[ ]` checkbox
    /// prefix that reflects membership in `checked`. Orthogonal to the cursor highlight.
    pub fn with_checked(mut self, checked: &'a std::collections::BTreeSet<String>) -> Self {
        self.checked = Some(checked);
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
                let is_checked = self
                    .checked
                    .map(|set| set.contains(&device.id))
                    .unwrap_or(false);

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

                // Checkbox prefix — only rendered when a checked set is attached
                let checkbox_span: Option<Span> = self.checked.map(|_| {
                    let glyph = if is_checked { "[x] " } else { "[ ] " };
                    let color = if is_checked {
                        palette::ACCENT
                    } else {
                        palette::TEXT_MUTED
                    };
                    Span::styled(glyph, Style::default().fg(color))
                });
                // Checkbox width (4 cols: "[x] " or "[ ] ") — 0 when not shown
                let checkbox_width: usize = if checkbox_span.is_some() { 4 } else { 0 };

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
                // Format: "[x] <name> (<type>)" or "<name> (<type>)"
                let type_suffix = format!(" ({})", device_type);
                let reserved = checkbox_width + type_suffix.len();
                let available_width = (area_width as usize).saturating_sub(reserved);

                // Truncate device name if needed
                let name = if available_width > 0 {
                    super::truncate_with_ellipsis(&device.name, available_width)
                } else {
                    device.name.clone()
                };

                let mut spans: Vec<Span> = Vec::new();
                if let Some(cb) = checkbox_span {
                    spans.push(cb);
                }
                spans.push(Span::styled(name, style));
                spans.push(Span::styled(
                    type_suffix,
                    Style::default().fg(palette::TEXT_MUTED),
                ));
                ListItem::new(Line::from(spans))
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
        connected_device_list_render_with_regions(area, buf, &self, None);
    }
}

/// Render [`ConnectedDeviceList`] and record clickable device-row regions.
///
/// One rect per visible *device* row (headers are skipped) is registered at
/// `z_index = 1`. The `abs_index` recorded is the flat-list index (headers
/// included), matching `TargetSelectorState::selected_index`.
pub fn connected_device_list_render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    list: &ConnectedDeviceList<'_>,
    ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
) {
    let groups = group_connected_devices(list.devices);
    let items = flatten_groups(&groups);

    // Count devices in the full list that are filtered out (not supported)
    let hidden = list.devices.iter().filter(|d| !d.is_supported).count();

    if items.is_empty() {
        let msg = if list.devices.is_empty() {
            "No connected devices"
        } else {
            // Devices were discovered but all are unsupported for this project.
            "Devices found but none runnable for this project — check enabled platforms"
        };
        Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette::TEXT_MUTED))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .render(area, buf);
        return;
    }

    // Split area to reserve a 1-row footer when there are hidden unsupported devices.
    // Layout: list area (Min(0)) + optional footer (Length(1))
    let (list_area, footer_area) = if hidden > 0 {
        let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // Calculate visible range using the (possibly reduced) list area
    let visible_height = list_area.height as usize;
    let start = list.scroll_offset.min(items.len().saturating_sub(1));
    let end = (start + visible_height).min(items.len());

    // Create list items only for visible range
    let list_items: Vec<ListItem> = items[start..end]
        .iter()
        .enumerate()
        .map(|(visible_idx, item)| {
            let actual_idx = start + visible_idx;
            list.render_item(item, actual_idx, list_area.width)
        })
        .collect();

    let rendered_list = List::new(list_items);
    rendered_list.render(list_area, buf);

    // Render scroll indicators into the list sub-area
    list.render_scroll_indicators(list_area, buf, start, end, items.len());

    // Render the hidden-devices footer when applicable
    if let Some(footer) = footer_area {
        let footer_text = format!("({hidden} hidden: not runnable for this project)");
        Paragraph::new(footer_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette::TEXT_MUTED))
            .render(footer, buf);
    }

    // Record click regions for device rows (skip headers).
    // Iterate only within the list_area rows — do NOT register a region over the footer.
    if let Some(c) = ctx {
        for screen_row in 0..visible_height {
            let abs_index = start + screen_row;
            if abs_index >= items.len() {
                break;
            }
            // Only record regions for device rows, not headers
            if matches!(items[abs_index], DeviceListItem::Device(_)) {
                let rect = MouseRect::new(
                    list_area.x,
                    list_area.y + screen_row as u16,
                    list_area.width,
                    1,
                );
                if !rect.is_empty() {
                    c.click_at_z(
                        rect,
                        MouseAction::emit(Message::NewSessionDialogSelectDeviceAt {
                            index: abs_index,
                        }),
                        1,
                    );
                }
            }
        }
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
        }
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

                let runtime = device.runtime_info();

                // Calculate available width for device name
                // Format: "<name> (<runtime>)"
                let runtime_suffix = format!(" ({})", runtime);
                let reserved = runtime_suffix.len();
                let available_width = (area_width as usize).saturating_sub(reserved);

                // Truncate device name if needed
                let name = if available_width > 0 {
                    super::truncate_with_ellipsis(device.display_name(), available_width)
                } else {
                    device.display_name().to_string()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(name, style),
                    Span::styled(runtime_suffix, Style::default().fg(palette::TEXT_MUTED)),
                ]))
            }
        }
    }

    fn render_unavailable_message(&self, area: Rect, buf: &mut Buffer) {
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
        bootable_device_list_render_with_regions(area, buf, &self, None);
    }
}

/// Render [`BootableDeviceList`] and record clickable device-row regions.
///
/// One rect per visible *device* row (headers are skipped) is registered at
/// `z_index = 1`. The `abs_index` recorded is the flat-list index (headers
/// included), matching `TargetSelectorState::selected_index`.
pub fn bootable_device_list_render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    list: &BootableDeviceList<'_>,
    ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
) {
    // Check if any tools are unavailable
    let ios_unavailable = !list.tool_availability.xcrun_simctl;
    let android_unavailable = !list.tool_availability.android_emulator;

    // If both are unavailable, show message (no regions)
    if ios_unavailable && android_unavailable {
        list.render_unavailable_message(area, buf);
        return;
    }

    let groups = group_bootable_devices(list.ios_simulators, list.android_avds);
    let items = flatten_groups(&groups);

    if items.is_empty() {
        let msg = Paragraph::new("No bootable devices found")
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette::TEXT_MUTED));
        msg.render(area, buf);
        return;
    }

    // Calculate visible range
    let visible_height = area.height as usize;
    let start = list.scroll_offset.min(items.len().saturating_sub(1));
    let end = (start + visible_height).min(items.len());

    // Create list items only for visible range
    let list_items: Vec<ListItem> = items[start..end]
        .iter()
        .enumerate()
        .map(|(visible_idx, item)| {
            let actual_idx = start + visible_idx;
            list.render_item(item, actual_idx, area.width)
        })
        .collect();

    let rendered_list = List::new(list_items);
    rendered_list.render(area, buf);

    // Render scroll indicators
    list.render_scroll_indicators(area, buf, start, end, items.len());

    // Record click regions for device rows (skip headers)
    if let Some(c) = ctx {
        for screen_row in 0..visible_height {
            let abs_index = start + screen_row;
            if abs_index >= items.len() {
                break;
            }
            if matches!(items[abs_index], DeviceListItem::Device(_)) {
                let rect = MouseRect::new(area.x, area.y + screen_row as u16, area.width, 1);
                if !rect.is_empty() {
                    c.click_at_z(
                        rect,
                        MouseAction::emit(Message::NewSessionDialogSelectDeviceAt {
                            index: abs_index,
                        }),
                        1,
                    );
                }
            }
        }
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
            ..Default::default()
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

        let tool_availability = ToolAvailability::default();

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
            ..Default::default()
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

    // ─── render_with_regions tests ───────────────────────────────────────────

    use crate::widgets::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::mouse_regions::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn connected_device_list_regions_device_rows_only() {
        // 2 connected iOS devices → flat list: 1 header + 2 devices
        let devices = vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "iPhone 16", "ios", false),
        ];

        let list = ConnectedDeviceList::new(&devices, 0, true, 0);
        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        let mut regions = MouseRegions::default();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            connected_device_list_render_with_regions(area, &mut buf, &list, Some(&mut ctx));
        }

        // Flat list: [Header("ios devices"), Device("iPhone 15"), Device("iPhone 16")]
        // Headers are skipped — expect 2 device regions
        assert_eq!(
            regions.len(),
            2,
            "expected 2 device regions (headers skipped)"
        );

        for entry in regions.iter() {
            assert_eq!(entry.z_index, 1, "device regions must be at z=1");
        }

        // Check that indices are the flat-list positions (1 and 2 for the two devices)
        let indices: Vec<usize> = regions
            .iter()
            .filter_map(|e| {
                e.on_left.as_ref()?.as_emit().and_then(|m| {
                    if let Message::NewSessionDialogSelectDeviceAt { index } = m {
                        Some(*index)
                    } else {
                        None
                    }
                })
            })
            .collect();
        assert!(
            indices.contains(&1),
            "expected flat-list index 1 (first device)"
        );
        assert!(
            indices.contains(&2),
            "expected flat-list index 2 (second device)"
        );
    }

    // ─── Checkbox rendering tests ────────────────────────────────────────────

    #[test]
    fn renders_checkbox_for_each_device() {
        // When with_checked() is provided with an empty set, every device row
        // shows the unchecked glyph "[ ]".
        let devices = vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "Pixel 8", "android", false),
        ];
        let checked = std::collections::BTreeSet::new();

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0).with_checked(&checked);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            content.contains("[ ]"),
            "unchecked rows should display '[ ]'; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            !content.contains("[x]"),
            "no device is checked — '[x]' must not appear"
        );
    }

    #[test]
    fn renders_checked_glyph_for_checked_device() {
        // When a device id is in the checked set its row shows "[x]".
        let devices = vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "Pixel 8", "android", false),
        ];
        let mut checked = std::collections::BTreeSet::new();
        checked.insert("1".to_string()); // iPhone 15 is checked

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0).with_checked(&checked);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            content.contains("[x]"),
            "checked device should display '[x]'; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            content.contains("[ ]"),
            "unchecked device should still display '[ ]'"
        );
    }

    #[test]
    fn renders_no_checkbox_when_checked_set_not_provided() {
        // Without with_checked(), no checkbox glyphs appear (backward compatibility).
        let devices = vec![test_device_full("1", "iPhone 15", "ios", false)];

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            !content.contains("[ ]") && !content.contains("[x]"),
            "without with_checked(), no checkbox glyphs should appear; got: {}",
            &content.chars().take(300).collect::<String>()
        );
    }

    #[test]
    fn header_rows_render_without_checkbox() {
        // Headers must never carry a checkbox prefix even when with_checked is set.
        let devices = vec![test_device_full("1", "iPhone 15", "ios", false)];
        let mut checked = std::collections::BTreeSet::new();
        checked.insert("1".to_string());

        // Render into a small buffer to isolate the first row (the header).
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        let list = ConnectedDeviceList::new(&devices, 0, true, 0).with_checked(&checked);
        connected_device_list_render_with_regions(area, &mut buf, &list, None);

        // The flat list is: [Header("IOS DEVICES"), Device("iPhone 15")]
        // At height=1, only the first row (header) is rendered.
        let header_content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            !header_content.contains("[x]") && !header_content.contains("[ ]"),
            "header row must not display a checkbox; got: {}",
            header_content
        );
    }

    // ─── Empty-state messaging tests (task 03) ──────────────────────────────

    #[test]
    fn connected_empty_shows_no_devices() {
        // devices: &[] → expect "No connected devices"
        let devices: Vec<Device> = vec![];

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            content.contains("No connected devices"),
            "empty device list should show 'No connected devices'; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            !content.contains("none runnable"),
            "'none runnable' must not appear when list is genuinely empty"
        );
    }

    #[test]
    fn connected_all_unsupported_shows_none_runnable() {
        // devices: one device with is_supported = false → expect "none runnable"
        // task 02's filter in group_connected_devices excludes unsupported devices,
        // so items is empty while list.devices is non-empty.
        let devices = vec![Device {
            id: "unsupported-1".to_string(),
            name: "Unsupported Phone".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: false,
            capabilities: None,
        }];

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            content.contains("none runnable"),
            "all-unsupported list should show 'none runnable' message; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            !content.contains("No connected devices"),
            "'No connected devices' must not appear when devices exist but are unsupported"
        );
    }

    #[test]
    fn connected_with_supported_device_renders_rows_not_empty_state() {
        // devices: one supported device → buffer contains the device name, not the empty message
        let devices = vec![test_device_full("1", "Pixel 9", "android", false)];

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            content.contains("Pixel 9"),
            "supported device name should appear; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            !content.contains("No connected devices"),
            "empty-state message must not appear when a supported device is present"
        );
        assert!(
            !content.contains("none runnable"),
            "none-runnable message must not appear when a supported device is present"
        );
    }

    #[test]
    fn connected_device_list_regions_scroll_offset_preserved() {
        // 5 devices, scroll offset = 2 → visible rows start at flat-list index 2
        // flat list: [Header(ios), D0, D1, D2, D3, D4]  (indices 0..5 where 0=header)
        let devices: Vec<Device> = (0..5)
            .map(|i| test_device_full(&format!("id{}", i), &format!("Dev{}", i), "ios", false))
            .collect();

        let list = ConnectedDeviceList::new(&devices, 0, true, 2); // scroll_offset = 2
        let area = Rect::new(0, 0, 50, 3); // 3 visible rows
        let mut buf = Buffer::empty(area);
        let mut regions = MouseRegions::default();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            connected_device_list_render_with_regions(area, &mut buf, &list, Some(&mut ctx));
        }

        // Visible range: items[2..5] = [D1, D2, D3] — all devices, abs indices 2, 3, 4
        let indices: Vec<usize> = regions
            .iter()
            .filter_map(|e| {
                e.on_left.as_ref()?.as_emit().and_then(|m| {
                    if let Message::NewSessionDialogSelectDeviceAt { index } = m {
                        Some(*index)
                    } else {
                        None
                    }
                })
            })
            .collect();

        assert_eq!(
            regions.len(),
            3,
            "3 visible device rows should produce 3 regions"
        );
        // The absolute indices must start at 2 (scroll_offset) and increase
        assert!(
            indices.iter().all(|&i| i >= 2),
            "all abs_indices must be >= scroll_offset 2, got {:?}",
            indices
        );
    }

    // ─── Hidden-footer tests (task 03) ──────────────────────────────────────

    #[test]
    fn connected_mixed_shows_hidden_footer() {
        // One supported device + one unsupported device → rows render AND footer appears.
        let supported = test_device_full("1", "Pixel 9", "android", false);
        let unsupported = Device {
            id: "u1".to_string(),
            name: "Legacy Phone".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: false,
            capabilities: None,
        };
        let devices = vec![supported, unsupported];

        let mut terminal = TestTerminal::new();
        terminal.draw_with(|f| {
            let list = ConnectedDeviceList::new(&devices, 0, true, 0);
            f.render_widget(list, f.area());
        });

        let content = terminal.content();
        assert!(
            content.contains("Pixel 9"),
            "supported device name should appear; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            content.contains("1 hidden"),
            "footer should contain '1 hidden'; got: {}",
            &content.chars().take(300).collect::<String>()
        );
        assert!(
            content.contains("not runnable"),
            "footer should contain 'not runnable'; got: {}",
            &content.chars().take(300).collect::<String>()
        );
    }

    #[test]
    fn connected_all_supported_has_no_hidden_footer() {
        // Two supported devices → no footer row should appear.
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
        assert!(
            !content.contains("hidden"),
            "all-supported list must NOT show 'hidden' footer; got: {}",
            &content.chars().take(300).collect::<String>()
        );
    }

    #[test]
    fn connected_click_maps_correctly_with_footer_present() {
        // Mixed case: one supported + one unsupported.
        // The supported device is at flat-list index 1 (after the header at index 0).
        // The footer row must NOT receive a click region.
        // Area height = 3: list_area = rows 0..2, footer = row 2.
        let supported = test_device_full("1", "Pixel 9", "android", false);
        let unsupported = Device {
            id: "u1".to_string(),
            name: "Legacy Phone".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: false,
            capabilities: None,
        };
        let devices = vec![supported, unsupported];

        // Height 3: Layout gives list_area height=2 and footer height=1
        let list = ConnectedDeviceList::new(&devices, 0, true, 0);
        let area = Rect::new(0, 0, 50, 3);
        let mut buf = Buffer::empty(area);
        let mut regions = MouseRegions::default();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            connected_device_list_render_with_regions(area, &mut buf, &list, Some(&mut ctx));
        }

        // Flat list: [Header("ANDROID DEVICES"), Device("Pixel 9")] — 1 device region
        assert_eq!(
            regions.len(),
            1,
            "only the supported device row should register a click region; got {}",
            regions.len()
        );

        let device_region = regions.iter().next().expect("expected at least one region");

        // The device region must be within list_area (y < area.bottom() - 1 = 2)
        let footer_y = area.y + area.height - 1;
        assert!(
            device_region.rect.y < footer_y,
            "device click region must not overlap the footer row (y={footer_y}); got y={}",
            device_region.rect.y
        );

        // The registered index must be the flat-list device index (1, after the header at 0)
        let registered_index = device_region
            .on_left
            .as_ref()
            .and_then(|a| a.as_emit())
            .and_then(|m| {
                if let Message::NewSessionDialogSelectDeviceAt { index } = m {
                    Some(*index)
                } else {
                    None
                }
            })
            .expect("region should carry NewSessionDialogSelectDeviceAt");
        assert_eq!(
            registered_index, 1,
            "click should map to flat-list index 1 (first device after header); got {registered_index}"
        );
    }
}
