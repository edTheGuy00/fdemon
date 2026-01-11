# Task: Target Selector Widget

## Summary

Create the main Target Selector widget that combines the tab bar and device list into the left pane of the NewSessionDialog.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Create |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (add export) |

## Implementation

### 1. Target selector state

```rust
// src/tui/widgets/new_session_dialog/target_selector.rs

use super::tab_bar::TargetTab;
use super::device_groups::{DeviceListItem, flatten_groups, group_connected_devices, group_bootable_devices, next_selectable, prev_selectable};
use crate::daemon::{Device, IosSimulator, AndroidAvd, BootableDevice, ToolAvailability};

/// State for the Target Selector pane
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

    /// Scroll offset for long lists
    pub scroll_offset: usize,

    /// Loading state for device discovery
    pub loading: bool,

    /// Loading state for bootable device discovery
    pub bootable_loading: bool,

    /// Error message if discovery failed
    pub error: Option<String>,
}

impl Default for TargetSelectorState {
    fn default() -> Self {
        Self {
            active_tab: TargetTab::Connected,
            connected_devices: Vec::new(),
            ios_simulators: Vec::new(),
            android_avds: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: true,
            bootable_loading: false,
            error: None,
        }
    }
}
```

### 2. Navigation methods

```rust
impl TargetSelectorState {
    /// Switch to a specific tab
    pub fn set_tab(&mut self, tab: TargetTab) {
        if self.active_tab != tab {
            self.active_tab = tab;
            self.selected_index = self.first_selectable_index();
            self.scroll_offset = 0;
        }
    }

    /// Toggle between tabs
    pub fn toggle_tab(&mut self) {
        self.set_tab(self.active_tab.toggle());
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let items = self.current_flat_list();
        if !items.is_empty() {
            self.selected_index = next_selectable(&items, self.selected_index);
        }
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        let items = self.current_flat_list();
        if !items.is_empty() {
            self.selected_index = prev_selectable(&items, self.selected_index);
        }
    }

    /// Get first selectable index in current tab
    fn first_selectable_index(&self) -> usize {
        let items = self.current_flat_list();
        items
            .iter()
            .enumerate()
            .find_map(|(i, item)| match item {
                DeviceListItem::Device(_) => Some(i),
                DeviceListItem::Header(_) => None,
            })
            .unwrap_or(0)
    }

    /// Get flattened list for current tab
    fn current_flat_list(&self) -> Vec<DeviceListItem<String>> {
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
                        DeviceListItem::Device(d) => DeviceListItem::Device(d.id().to_string()),
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
    pub fn selected_bootable_device(&self) -> Option<BootableDevice> {
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
        self.error = None;

        // Reset selection if it's now invalid
        if self.active_tab == TargetTab::Connected {
            let max_index = self.current_flat_list().len().saturating_sub(1);
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

        // Reset selection if on bootable tab
        if self.active_tab == TargetTab::Bootable {
            let max_index = self.current_flat_list().len().saturating_sub(1);
            if self.selected_index > max_index {
                self.selected_index = self.first_selectable_index();
            }
        }
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
    }
}
```

### 3. Target selector widget

```rust
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::tab_bar::TabBar;
use super::device_list::{ConnectedDeviceList, BootableDeviceList};

/// The Target Selector widget (left pane of NewSessionDialog)
pub struct TargetSelector<'a> {
    state: &'a TargetSelectorState,
    tool_availability: &'a ToolAvailability,
    is_focused: bool,
}

impl<'a> TargetSelector<'a> {
    pub fn new(
        state: &'a TargetSelectorState,
        tool_availability: &'a ToolAvailability,
        is_focused: bool,
    ) -> Self {
        Self {
            state,
            tool_availability,
            is_focused,
        }
    }
}

impl Widget for TargetSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Main block
        let border_color = if self.is_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .title(" Target Selector ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout: tab bar + content + footer
        let chunks = Layout::vertical([
            Constraint::Length(3),  // Tab bar
            Constraint::Min(5),     // Content (device list)
            Constraint::Length(1),  // Footer hints
        ])
        .split(inner);

        // Render tab bar
        let tab_bar = TabBar::new(self.state.active_tab, self.is_focused);
        tab_bar.render(chunks[0], buf);

        // Render content based on active tab
        if self.state.loading {
            self.render_loading(chunks[1], buf);
        } else if let Some(ref error) = self.state.error {
            self.render_error(chunks[1], buf, error);
        } else {
            match self.state.active_tab {
                TargetTab::Connected => {
                    let list = ConnectedDeviceList::new(
                        &self.state.connected_devices,
                        self.state.selected_index,
                        self.is_focused,
                    );
                    list.render(chunks[1], buf);
                }
                TargetTab::Bootable => {
                    let list = BootableDeviceList::new(
                        &self.state.ios_simulators,
                        &self.state.android_avds,
                        self.state.selected_index,
                        self.is_focused,
                        self.tool_availability,
                    );
                    list.render(chunks[1], buf);
                }
            }
        }

        // Render footer
        self.render_footer(chunks[2], buf);
    }
}

impl TargetSelector<'_> {
    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        let text = Paragraph::new("Discovering devices...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(ratatui::layout::Alignment::Center);
        text.render(area, buf);
    }

    fn render_error(&self, area: Rect, buf: &mut Buffer, error: &str) {
        let text = Paragraph::new(error)
            .style(Style::default().fg(Color::Red))
            .alignment(ratatui::layout::Alignment::Center);
        text.render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = match self.state.active_tab {
            TargetTab::Connected => "[Enter] Select  [r] Refresh",
            TargetTab::Bootable => "[Enter] Boot  [r] Refresh",
        };

        let text = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        text.render(area, buf);
    }
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
    fn test_target_selector_state_default() {
        let state = TargetSelectorState::default();
        assert_eq!(state.active_tab, TargetTab::Connected);
        assert!(state.loading);
        assert!(state.connected_devices.is_empty());
    }

    #[test]
    fn test_set_tab_resets_selection() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.selected_index = 5;

        state.set_tab(TargetTab::Bootable);

        assert_eq!(state.active_tab, TargetTab::Bootable);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_toggle_tab() {
        let mut state = TargetSelectorState::default();

        state.toggle_tab();
        assert_eq!(state.active_tab, TargetTab::Bootable);

        state.toggle_tab();
        assert_eq!(state.active_tab, TargetTab::Connected);
    }

    #[test]
    fn test_set_connected_devices() {
        let mut state = TargetSelectorState::default();
        let devices = vec![
            test_device_full("1", "iPhone", "ios", false),
            test_device_full("2", "Pixel", "android", false),
        ];

        state.set_connected_devices(devices);

        assert!(!state.loading);
        assert_eq!(state.connected_devices.len(), 2);
    }

    #[test]
    fn test_target_selector_renders() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![
            test_device_full("1", "iPhone 15", "ios", false),
        ]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(50, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = TargetSelector::new(&state, &tool_availability, true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Target Selector"));
        assert!(content.contains("Connected"));
        assert!(content.contains("iPhone 15"));
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test target_selector && cargo clippy -- -D warnings
```

## Notes

- Widget combines tab bar, device list, and footer
- State tracks selection separately for each tab
- Loading/error states are handled gracefully
- Footer hints change based on active tab
