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
    widgets::{Block, Borders, Clear, LineGauge, List, ListItem, Paragraph, Widget},
};

use crate::daemon::Device;

/// State for the device selector UI
#[derive(Debug, Clone, Default)]
pub struct DeviceSelectorState {
    /// Available devices (current view)
    pub devices: Vec<Device>,

    /// Cached devices from last successful discovery
    /// Used for instant display on subsequent opens
    cached_devices: Option<Vec<Device>>,

    /// Currently highlighted index
    pub selected_index: usize,

    /// Whether the selector is visible
    pub visible: bool,

    /// Loading state (while discovering devices, no cache)
    pub loading: bool,

    /// Whether we're refreshing in background (has cache, show header LineGauge)
    pub refreshing: bool,

    /// Error message if device discovery failed
    pub error: Option<String>,

    /// Whether to show emulator launch options
    pub show_emulator_options: bool,

    /// Number of emulator options (added after devices)
    emulator_option_count: usize,

    /// Frame counter for loading animation
    pub animation_frame: u64,
}

impl DeviceSelectorState {
    /// Create a new device selector state
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            cached_devices: None,
            selected_index: 0,
            visible: false,
            loading: false,
            refreshing: false,
            error: None,
            show_emulator_options: true,
            emulator_option_count: 2, // Android + iOS
            animation_frame: 0,
        }
    }

    /// Advance animation frame (call on each tick)
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    /// Calculate indeterminate progress ratio (0.0 to 1.0)
    /// Creates a bouncing effect from left to right and back
    pub fn indeterminate_ratio(&self) -> f64 {
        // Complete cycle every 60 frames (about 1 second at 60fps)
        let cycle_length = 300;
        let position = self.animation_frame % cycle_length;

        // First half: 0.0 -> 1.0, Second half: 1.0 -> 0.0
        let half = cycle_length / 2;
        if position < half {
            position as f64 / half as f64
        } else {
            (cycle_length - position) as f64 / half as f64
        }
    }

    /// Show the selector with loading state (startup, no cache)
    pub fn show_loading(&mut self) {
        self.visible = true;
        self.loading = true;
        self.refreshing = false;
        self.error = None;
        self.devices.clear();
        self.selected_index = 0;
    }

    /// Show with cached devices, refresh in background
    pub fn show_refreshing(&mut self) {
        self.visible = true;
        self.animation_frame = 0;

        // Use cached devices if available
        if let Some(ref cached) = self.cached_devices {
            self.devices = cached.clone();
            self.loading = false;
            self.refreshing = true;
        } else {
            // No cache, fall back to loading
            self.loading = true;
            self.refreshing = false;
        }
        self.error = None;
    }

    /// Check if we have cached devices
    pub fn has_cache(&self) -> bool {
        self.cached_devices.is_some()
    }

    /// Clear cache (e.g., after error or explicit refresh request)
    pub fn clear_cache(&mut self) {
        self.cached_devices = None;
    }

    /// Update with discovered devices (updates cache)
    pub fn set_devices(&mut self, devices: Vec<Device>) {
        self.devices = devices.clone();
        self.cached_devices = Some(devices);
        self.loading = false;
        self.refreshing = false;
        self.error = None;
        self.selected_index = 0;
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.loading = false;
        self.refreshing = false;
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
    /// Whether there are running sessions (affects Esc behavior)
    has_running_sessions: bool,
}

impl<'a> DeviceSelector<'a> {
    /// Create a new device selector widget
    pub fn new(state: &'a DeviceSelectorState) -> Self {
        Self {
            state,
            has_running_sessions: false, // Default for backward compatibility
        }
    }

    /// Create with session awareness for conditional Esc display
    pub fn with_session_state(state: &'a DeviceSelectorState, has_running_sessions: bool) -> Self {
        Self {
            state,
            has_running_sessions,
        }
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

        // Determine layout based on state
        let (content_area, footer_area) = if self.state.refreshing {
            // Refreshing: header gauge + content + footer
            let chunks = Layout::vertical([
                Constraint::Length(1), // Refresh indicator
                Constraint::Min(3),    // Content (cached devices)
                Constraint::Length(2), // Footer
            ])
            .split(inner);

            // Render refresh indicator in header (yellow LineGauge)
            let ratio = self.state.indeterminate_ratio();
            let gauge_area = Rect {
                x: chunks[0].x.saturating_add(2),
                y: chunks[0].y,
                width: chunks[0].width.saturating_sub(4),
                height: 1,
            };

            let gauge = LineGauge::default()
                .ratio(ratio)
                .filled_style(Style::default().fg(Color::Yellow)) // Yellow for refresh
                .unfilled_style(Style::default().fg(Color::Black))
                .filled_symbol(symbols::line::NORMAL.horizontal) // Thinner for header
                .unfilled_symbol(symbols::line::NORMAL.horizontal);

            gauge.render(gauge_area, buf);

            (chunks[1], chunks[2])
        } else {
            // Normal or loading: content + footer
            let chunks = Layout::vertical([
                Constraint::Min(3),    // Content
                Constraint::Length(2), // Footer
            ])
            .split(inner);

            (chunks[0], chunks[1])
        };

        // Render content based on state
        if self.state.loading {
            // Loading state with animated LineGauge (centered)
            let loading_chunks = Layout::vertical([
                Constraint::Length(2), // Spacer
                Constraint::Length(1), // Text
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Gauge
                Constraint::Min(0),    // Rest
            ])
            .split(content_area);

            // "Discovering devices..." text
            let loading_text = Paragraph::new("Discovering devices...")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow));
            loading_text.render(loading_chunks[1], buf);

            // Animated LineGauge
            let ratio = self.state.indeterminate_ratio();

            // Create padded area for the gauge
            let gauge_area = Rect {
                x: loading_chunks[3].x.saturating_add(4),
                y: loading_chunks[3].y,
                width: loading_chunks[3].width.saturating_sub(8),
                height: 1,
            };

            let gauge = LineGauge::default()
                .ratio(ratio)
                .filled_style(Style::default().fg(Color::Cyan))
                .unfilled_style(Style::default().fg(Color::Black))
                .filled_symbol(symbols::line::THICK.horizontal)
                .unfilled_symbol(symbols::line::THICK.horizontal);

            gauge.render(gauge_area, buf);
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
            error_text.render(content_area, buf);
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
            no_devices.render(content_area, buf);
        } else {
            // Device list (either refreshing with cache or ready)
            let items = self.device_items();
            let list = List::new(items);
            list.render(content_area, buf);
        }

        // Build footer text conditionally - only show Esc when sessions are running
        let footer_text = if self.has_running_sessions {
            "↑↓ Navigate  Enter Select  Esc Cancel  r Refresh"
        } else {
            "↑↓ Navigate  Enter Select  r Refresh"
        };

        // Footer with keybindings - use Gray for visibility on DarkGray background
        let footer = Paragraph::new(footer_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));
        footer.render(footer_area, buf);
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

    #[test]
    fn test_animation_tick() {
        let mut state = DeviceSelectorState::new();
        assert_eq!(state.animation_frame, 0);

        state.tick();
        assert_eq!(state.animation_frame, 1);

        state.tick();
        assert_eq!(state.animation_frame, 2);
    }

    #[test]
    fn test_animation_frame_wrapping() {
        let mut state = DeviceSelectorState::new();
        state.animation_frame = u64::MAX;

        state.tick();
        assert_eq!(state.animation_frame, 0);
    }

    #[test]
    fn test_device_selector_render_loading_with_linegauge() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = DeviceSelectorState::new();
        state.show_loading();
        state.tick(); // Advance animation

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
        // Should contain gauge characters (thick horizontal lines)
        assert!(content.contains('━') || content.contains('─'));
    }

    #[test]
    fn test_indeterminate_ratio_bounds() {
        let mut state = DeviceSelectorState::new();

        // Test many frames
        for _ in 0..200 {
            state.tick();
            let ratio = state.indeterminate_ratio();

            // Ratio should always be 0.0 to 1.0
            assert!(ratio >= 0.0);
            assert!(ratio <= 1.0);
        }
    }

    #[test]
    fn test_indeterminate_ratio_oscillates() {
        let mut state = DeviceSelectorState::new();

        let mut ratios = Vec::new();
        for _ in 0..60 {
            state.tick();
            ratios.push(state.indeterminate_ratio());
        }

        // Should have both increasing and decreasing sections
        let has_increase = ratios.windows(2).any(|w| w[1] > w[0]);
        let has_decrease = ratios.windows(2).any(|w| w[1] < w[0]);

        assert!(has_increase);
        assert!(has_decrease);
    }

    #[test]
    fn test_device_selector_with_session_state() {
        let state = DeviceSelectorState::new();

        // Without sessions
        let selector = DeviceSelector::with_session_state(&state, false);
        assert!(!selector.has_running_sessions);

        // With sessions
        let selector = DeviceSelector::with_session_state(&state, true);
        assert!(selector.has_running_sessions);
    }

    #[test]
    fn test_footer_shows_esc_only_with_sessions() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![]); // Not loading, show footer

        // Without sessions - no Esc
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let selector = DeviceSelector::with_session_state(&state, false);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(!content.contains("Esc Cancel"));
        assert!(content.contains("Navigate"));

        // With sessions - shows Esc
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let selector = DeviceSelector::with_session_state(&state, true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(content.contains("Esc Cancel"));
    }

    // ─────────────────────────────────────────────────────────
    // Task 11a: Device Cache Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_initial_has_no_cache() {
        let state = DeviceSelectorState::new();
        assert!(!state.has_cache());
    }

    #[test]
    fn test_show_loading_no_cache() {
        let mut state = DeviceSelectorState::new();
        assert!(!state.has_cache());

        state.show_loading();

        assert!(state.loading);
        assert!(!state.refreshing);
        assert!(state.devices.is_empty());
    }

    #[test]
    fn test_show_refreshing_with_cache() {
        let mut state = DeviceSelectorState::new();

        // First discovery
        let devices = vec![test_device("iphone", "iPhone 15", false)];
        state.set_devices(devices.clone());

        assert!(state.has_cache());
        assert!(!state.loading);
        assert!(!state.refreshing);

        // Subsequent show
        state.hide();
        state.show_refreshing();

        assert!(!state.loading);
        assert!(state.refreshing);
        assert_eq!(state.devices.len(), 1);
    }

    #[test]
    fn test_show_refreshing_falls_back_to_loading() {
        let mut state = DeviceSelectorState::new();
        assert!(!state.has_cache());

        // No cache, should fall back to loading
        state.show_refreshing();

        assert!(state.loading);
        assert!(!state.refreshing);
    }

    #[test]
    fn test_set_devices_updates_cache() {
        let mut state = DeviceSelectorState::new();

        let devices = vec![
            test_device("device1", "Device 1", false),
            test_device("device2", "Device 2", true),
        ];
        state.set_devices(devices);

        assert!(state.has_cache());
        assert_eq!(state.devices.len(), 2);

        // Hide and show again
        state.hide();
        state.show_refreshing();

        // Should have cached devices
        assert_eq!(state.devices.len(), 2);
    }

    #[test]
    fn test_refresh_updates_device_list() {
        let mut state = DeviceSelectorState::new();

        // Initial devices
        state.set_devices(vec![test_device("device1", "Device 1", false)]);
        state.hide();
        state.show_refreshing();

        assert!(state.refreshing);
        assert_eq!(state.devices.len(), 1);

        // Discovery completes with new devices
        state.set_devices(vec![
            test_device("device1", "Device 1", false),
            test_device("device2", "Device 2 (new)", true),
        ]);

        assert!(!state.refreshing);
        assert_eq!(state.devices.len(), 2);
    }

    #[test]
    fn test_clear_cache() {
        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![test_device("device1", "Device 1", false)]);

        assert!(state.has_cache());

        state.clear_cache();

        assert!(!state.has_cache());
    }

    #[test]
    fn test_set_error_clears_refreshing() {
        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![test_device("d1", "D1", false)]);
        state.hide();
        state.show_refreshing();

        assert!(state.refreshing);

        state.set_error("Discovery failed".to_string());

        assert!(!state.refreshing);
        assert!(!state.loading);
        assert!(state.error.is_some());
    }

    #[test]
    fn test_render_refreshing_shows_header_gauge_and_devices() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = DeviceSelectorState::new();
        state.set_devices(vec![test_device("iphone", "iPhone 15", false)]);
        state.hide();
        state.show_refreshing();
        state.tick(); // Advance animation

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = DeviceSelector::new(&state);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should show device name
        assert!(content.contains("iPhone 15"));

        // Should have gauge characters (from header indicator)
        assert!(content.contains('━') || content.contains('─') || content.contains('─'));
    }
}
