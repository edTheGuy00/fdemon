//! Device selector modal widget
//!
//! Displays available Flutter devices in a centered modal overlay,
//! with support for keyboard navigation and device selection.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use crate::daemon::Device;

/// State for the device selector UI
#[derive(Debug, Clone, Default)]
pub struct DeviceSelectorState {
    /// Available devices
    pub devices: Vec<Device>,

    /// Currently highlighted index
    pub selected_index: usize,

    /// Whether the selector is visible
    pub visible: bool,

    /// Loading state (while discovering devices)
    pub loading: bool,

    /// Error message if device discovery failed
    pub error: Option<String>,

    /// Whether to show emulator launch options
    pub show_emulator_options: bool,

    /// Number of emulator options (added after devices)
    emulator_option_count: usize,
}

impl DeviceSelectorState {
    /// Create a new device selector state
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            selected_index: 0,
            visible: false,
            loading: false,
            error: None,
            show_emulator_options: true,
            emulator_option_count: 2, // Android + iOS
        }
    }

    /// Show the selector with loading state
    pub fn show_loading(&mut self) {
        self.visible = true;
        self.loading = true;
        self.error = None;
        self.devices.clear();
        self.selected_index = 0;
    }

    /// Update with discovered devices
    pub fn set_devices(&mut self, devices: Vec<Device>) {
        self.devices = devices;
        self.loading = false;
        self.error = None;
        self.selected_index = 0;
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.loading = false;
        self.error = Some(error);
    }

    /// Hide the selector
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Show the selector (without loading)
    pub fn show(&mut self) {
        self.visible = true;
        self.loading = false;
    }

    /// Total number of selectable items
    pub fn item_count(&self) -> usize {
        let device_count = self.devices.len();
        if self.show_emulator_options {
            device_count + self.emulator_option_count
        } else {
            device_count
        }
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.item_count() > 0 {
            self.selected_index = if self.selected_index == 0 {
                self.item_count() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.item_count() > 0 {
            self.selected_index = (self.selected_index + 1) % self.item_count();
        }
    }

    /// Check if current selection is a device
    pub fn is_device_selected(&self) -> bool {
        self.selected_index < self.devices.len()
    }

    /// Check if current selection is "Launch Android Emulator"
    pub fn is_android_emulator_selected(&self) -> bool {
        self.show_emulator_options && self.selected_index == self.devices.len()
    }

    /// Check if current selection is "Launch iOS Simulator"
    pub fn is_ios_simulator_selected(&self) -> bool {
        self.show_emulator_options && self.selected_index == self.devices.len() + 1
    }

    /// Get the currently selected device (if a device is selected)
    pub fn selected_device(&self) -> Option<&Device> {
        if self.is_device_selected() {
            self.devices.get(self.selected_index)
        } else {
            None
        }
    }

    /// Check if there are no devices and no options
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty() && !self.show_emulator_options
    }
}

/// Device selector modal widget
pub struct DeviceSelector<'a> {
    state: &'a DeviceSelectorState,
}

impl<'a> DeviceSelector<'a> {
    pub fn new(state: &'a DeviceSelectorState) -> Self {
        Self { state }
    }

    /// Calculate the modal area centered in the parent
    pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

        Layout::horizontal([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
    }

    /// Create list items from devices
    fn device_items(&self) -> Vec<ListItem<'static>> {
        let mut items = Vec::new();

        // Add devices
        for (i, device) in self.state.devices.iter().enumerate() {
            let is_selected = i == self.state.selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indicator = if is_selected { "▶ " } else { "  " };
            let device_tag = if device.emulator {
                format!("({})", device.emulator_type())
            } else {
                "(physical)".to_string()
            };

            let line = format!(
                "{}{:<30} {:>12}",
                indicator,
                truncate_string(&device.name, 28),
                device_tag
            );

            items.push(ListItem::new(line).style(style));
        }

        // Add separator if there are emulator options and devices
        if self.state.show_emulator_options && !self.state.devices.is_empty() {
            items.push(ListItem::new("  ─────────────────────────────────────"));
        }

        // Add emulator launch options
        if self.state.show_emulator_options {
            let android_idx = self.state.devices.len();
            let ios_idx = android_idx + 1;

            // Android emulator option
            let android_selected = self.state.selected_index == android_idx;
            let android_style = if android_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            };
            let android_indicator = if android_selected { "▶ " } else { "  " };
            items.push(
                ListItem::new(format!("{}+ Launch Android Emulator...", android_indicator))
                    .style(android_style),
            );

            // iOS simulator option
            let ios_selected = self.state.selected_index == ios_idx;
            let ios_style = if ios_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            };
            let ios_indicator = if ios_selected { "▶ " } else { "  " };
            items.push(
                ListItem::new(format!("{}+ Launch iOS Simulator...", ios_indicator))
                    .style(ios_style),
            );
        }

        items
    }
}

impl Widget for DeviceSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate modal size
        let modal_area = Self::centered_rect(60, 70, area);

        // Clear the area behind the modal
        Clear.render(modal_area, buf);

        // Create the modal block
        let block = Block::default()
            .title(" Select Target Device ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout: content area + footer
        let chunks = Layout::vertical([
            Constraint::Min(3),    // Content
            Constraint::Length(2), // Footer
        ])
        .split(inner);

        // Render content based on state
        if self.state.loading {
            // Loading state
            let loading = Paragraph::new("Discovering devices...")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow));
            loading.render(chunks[0], buf);
        } else if let Some(ref error) = self.state.error {
            // Error state
            let error_text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Error:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(error.as_str()),
                Line::from(""),
                Line::from("Press 'r' to retry or Esc to cancel"),
            ])
            .alignment(Alignment::Center);
            error_text.render(chunks[0], buf);
        } else if self.state.is_empty() {
            // No devices and no emulator options
            let no_devices = Paragraph::new(vec![
                Line::from(""),
                Line::from("No devices found."),
                Line::from(""),
                Line::from("Connect a device or start an emulator."),
            ])
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
            no_devices.render(chunks[0], buf);
        } else {
            // Device list
            let items = self.device_items();
            let list = List::new(items);
            list.render(chunks[0], buf);
        }

        // Footer with keybindings
        let footer = Paragraph::new("↑↓ Navigate  Enter Select  Esc Cancel  r Refresh")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        footer.render(chunks[1], buf);
    }
}

/// Truncate a string to a maximum length, adding ellipsis if needed
fn truncate_string(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        s.chars().take(max_len).collect()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device(id: &str, name: &str, emulator: bool) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    #[test]
    fn test_state_new() {
        let state = DeviceSelectorState::new();
        assert!(!state.visible);
        assert!(!state.loading);
        assert!(state.devices.is_empty());
        assert_eq!(state.selected_index, 0);
        assert!(state.show_emulator_options);
    }

    #[test]
    fn test_state_navigation() {
        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![
            test_device("d1", "Device 1", false),
            test_device("d2", "Device 2", true),
        ]);

        // 2 devices + 2 emulator options = 4 items
        assert_eq!(state.item_count(), 4);
        assert_eq!(state.selected_index, 0);

        state.select_next();
        assert_eq!(state.selected_index, 1);

        state.select_next();
        assert_eq!(state.selected_index, 2);
        assert!(state.is_android_emulator_selected());

        state.select_next();
        assert_eq!(state.selected_index, 3);
        assert!(state.is_ios_simulator_selected());

        state.select_next(); // Wrap around
        assert_eq!(state.selected_index, 0);
        assert!(state.is_device_selected());

        state.select_previous(); // Wrap around backwards
        assert_eq!(state.selected_index, 3);
    }

    #[test]
    fn test_selected_device() {
        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![
            test_device("d1", "Device 1", false),
            test_device("d2", "Device 2", true),
        ]);

        assert_eq!(state.selected_device().map(|d| d.id.as_str()), Some("d1"));

        state.select_next();
        assert_eq!(state.selected_device().map(|d| d.id.as_str()), Some("d2"));

        state.select_next(); // Now on emulator option
        assert!(state.selected_device().is_none());
    }

    #[test]
    fn test_loading_state() {
        let mut state = DeviceSelectorState::new();

        assert!(!state.visible);
        assert!(!state.loading);

        state.show_loading();
        assert!(state.visible);
        assert!(state.loading);

        state.set_devices(vec![test_device("d1", "D1", false)]);
        assert!(!state.loading);
        assert!(state.visible);
    }

    #[test]
    fn test_error_state() {
        let mut state = DeviceSelectorState::new();
        state.show_loading();
        state.set_error("Connection failed".to_string());

        assert!(!state.loading);
        assert_eq!(state.error, Some("Connection failed".to_string()));
    }

    #[test]
    fn test_hide_show() {
        let mut state = DeviceSelectorState::new();
        state.show();
        assert!(state.visible);

        state.hide();
        assert!(!state.visible);
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("Short", 10), "Short");
        assert_eq!(truncate_string("Exactly Ten", 11), "Exactly Ten");
        assert_eq!(
            truncate_string("This is a very long device name", 15),
            "This is a very…"
        );
        assert_eq!(truncate_string("ABC", 3), "ABC");
        assert_eq!(truncate_string("ABCD", 3), "AB…");
    }

    #[test]
    fn test_truncate_string_unicode() {
        // Ensure Unicode chars are counted properly
        assert_eq!(truncate_string("日本語テスト", 4), "日本語…");
        assert_eq!(truncate_string("日本", 2), "日本");
    }

    #[test]
    fn test_hide_emulator_options() {
        let mut state = DeviceSelectorState::new();
        state.show_emulator_options = false;
        state.set_devices(vec![test_device("d1", "Device 1", false)]);

        assert_eq!(state.item_count(), 1);
        assert!(!state.is_android_emulator_selected());
        assert!(!state.is_ios_simulator_selected());
    }

    #[test]
    fn test_is_empty() {
        let mut state = DeviceSelectorState::new();
        state.show_emulator_options = false;
        assert!(state.is_empty());

        state.set_devices(vec![test_device("d1", "D1", false)]);
        assert!(!state.is_empty());

        state.devices.clear();
        state.show_emulator_options = true;
        assert!(!state.is_empty()); // Has emulator options
    }

    #[test]
    fn test_navigation_empty_devices_with_emulator_options() {
        let mut state = DeviceSelectorState::new();
        state.show_emulator_options = true;
        // No devices, but still have emulator options

        assert_eq!(state.item_count(), 2);
        assert!(state.is_android_emulator_selected());

        state.select_next();
        assert!(state.is_ios_simulator_selected());

        state.select_next();
        assert!(state.is_android_emulator_selected());
    }

    #[test]
    fn test_navigation_no_items() {
        let mut state = DeviceSelectorState::new();
        state.show_emulator_options = false;

        assert_eq!(state.item_count(), 0);

        // Should not panic
        state.select_next();
        state.select_previous();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_device_selector_widget_creation() {
        let state = DeviceSelectorState::new();
        let _selector = DeviceSelector::new(&state);
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = DeviceSelector::centered_rect(50, 50, area);

        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.width < area.width);
        assert!(centered.height < area.height);
    }

    #[test]
    fn test_device_items_generation() {
        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![
            test_device("iphone", "iPhone 15 Pro", true),
            test_device("pixel", "Pixel 8", true),
        ]);

        let selector = DeviceSelector::new(&state);
        let items = selector.device_items();

        // 2 devices + separator + 2 emulator options = 5 items
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn test_device_items_no_separator_when_no_devices() {
        let state = DeviceSelectorState::new();
        // No devices, just emulator options

        let selector = DeviceSelector::new(&state);
        let items = selector.device_items();

        // Just 2 emulator options, no separator
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_device_selector_render() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = DeviceSelectorState::new();
        state.visible = true;
        state.set_devices(vec![
            test_device("iphone", "iPhone 15 Pro", true),
            test_device("pixel", "Pixel 8", true),
        ]);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let selector = DeviceSelector::new(&state);
                frame.render_widget(selector, frame.area());
            })
            .unwrap();

        // Verify content is rendered
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("iPhone 15 Pro"));
        assert!(content.contains("Pixel 8"));
        assert!(content.contains("Select Target Device"));
    }

    #[test]
    fn test_device_selector_render_loading() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = DeviceSelectorState::new();
        state.show_loading();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let selector = DeviceSelector::new(&state);
                frame.render_widget(selector, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Discovering devices"));
    }

    #[test]
    fn test_device_selector_render_error() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = DeviceSelectorState::new();
        state.visible = true;
        state.set_error("Network timeout".to_string());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let selector = DeviceSelector::new(&state);
                frame.render_widget(selector, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Error"));
        assert!(content.contains("Network timeout"));
    }
}
