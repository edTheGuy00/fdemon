# Task: Device List Widget

## Summary

Create a device list widget that renders grouped devices with headers, selection state, and scrolling support.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/device_list.rs` | Create |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (add export) |

## Implementation

### 1. Device list widget

```rust
// src/tui/widgets/new_session_dialog/device_list.rs

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use super::device_groups::DeviceListItem;

/// Styles for the device list
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
```

### 2. Connected device list

```rust
use crate::daemon::Device;
use super::device_groups::{group_connected_devices, flatten_groups, DeviceListItem};

/// Widget for rendering connected devices with grouping
pub struct ConnectedDeviceList<'a> {
    devices: &'a [Device],
    selected_index: usize,
    is_focused: bool,
    styles: DeviceListStyles,
}

impl<'a> ConnectedDeviceList<'a> {
    pub fn new(devices: &'a [Device], selected_index: usize, is_focused: bool) -> Self {
        Self {
            devices,
            selected_index,
            is_focused,
            styles: DeviceListStyles::default(),
        }
    }

    fn render_item(&self, item: &DeviceListItem<&Device>, index: usize) -> ListItem<'static> {
        match item {
            DeviceListItem::Header(header) => {
                ListItem::new(Line::from(vec![
                    Span::styled("  ", self.styles.device_normal),
                    Span::styled(header.clone(), self.styles.header),
                ]))
            }
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
                    device.emulator_type()
                } else {
                    "physical"
                };

                ListItem::new(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(device.name.clone(), style),
                    Span::styled(format!(" ({})", device_type), self.styles.info),
                ]))
            }
        }
    }
}

impl Widget for ConnectedDeviceList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let groups = group_connected_devices(self.devices);
        let items = flatten_groups(&groups);

        let list_items: Vec<ListItem> = items
            .iter()
            .enumerate()
            .map(|(i, item)| self.render_item(item, i))
            .collect();

        let list = List::new(list_items);
        list.render(area, buf);
    }
}
```

### 3. Bootable device list

```rust
use crate::daemon::{BootableDevice, IosSimulator, AndroidAvd, ToolAvailability};
use super::device_groups::{group_bootable_devices, flatten_groups};

/// Widget for rendering bootable devices with grouping
pub struct BootableDeviceList<'a> {
    ios_simulators: &'a [IosSimulator],
    android_avds: &'a [AndroidAvd],
    selected_index: usize,
    is_focused: bool,
    tool_availability: &'a ToolAvailability,
    styles: DeviceListStyles,
}

impl<'a> BootableDeviceList<'a> {
    pub fn new(
        ios_simulators: &'a [IosSimulator],
        android_avds: &'a [AndroidAvd],
        selected_index: usize,
        is_focused: bool,
        tool_availability: &'a ToolAvailability,
    ) -> Self {
        Self {
            ios_simulators,
            android_avds,
            selected_index,
            is_focused,
            tool_availability,
            styles: DeviceListStyles::default(),
        }
    }

    fn render_item(&self, item: &DeviceListItem<BootableDevice>, index: usize) -> ListItem<'static> {
        match item {
            DeviceListItem::Header(header) => {
                ListItem::new(Line::from(vec![
                    Span::styled("  ", self.styles.device_normal),
                    Span::styled(header.clone(), self.styles.header),
                ]))
            }
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

                ListItem::new(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(device.display_name().to_string(), style),
                    Span::styled(format!(" ({})", runtime), self.styles.info),
                ]))
            }
        }
    }

    fn render_unavailable_message(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::Paragraph;
        use ratatui::layout::Alignment;

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
            use ratatui::widgets::Paragraph;
            use ratatui::layout::Alignment;

            let msg = Paragraph::new("No bootable devices found")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            msg.render(area, buf);
            return;
        }

        let list_items: Vec<ListItem> = items
            .iter()
            .enumerate()
            .map(|(i, item)| self.render_item(item, i))
            .collect();

        let list = List::new(list_items);
        list.render(area, buf);
    }
}
```

### 4. Scrolling support

```rust
/// Calculate scroll offset to keep selection visible
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
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use crate::tui::test_utils::test_device_full;

    #[test]
    fn test_connected_device_list_renders() {
        let devices = vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "Pixel 8", "android", false),
        ];

        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let list = ConnectedDeviceList::new(&devices, 0, true);
                f.render_widget(list, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

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

        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let list = BootableDeviceList::new(
                    &ios_sims,
                    &android_avds,
                    0,
                    true,
                    &tool_availability,
                );
                f.render_widget(list, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("iPhone 15 Pro"));
        assert!(content.contains("Pixel 6"));
    }

    #[test]
    fn test_calculate_scroll_offset() {
        // Selection visible
        assert_eq!(calculate_scroll_offset(5, 10, 0), 0);

        // Selection above visible area
        assert_eq!(calculate_scroll_offset(2, 10, 5), 2);

        // Selection below visible area
        assert_eq!(calculate_scroll_offset(15, 10, 0), 6);
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test device_list && cargo clippy -- -D warnings
```

## Notes

- Headers are rendered but not selectable
- Selection wraps around at list boundaries
- Scroll offset adjusts to keep selection visible
- Different styling for focused vs unfocused selection
