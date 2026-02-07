//! Target Selector Widget
//!
//! Main widget combining tab bar and device list into the left pane of NewSessionDialog.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

// State types now live in app layer (moved in Phase 1, Task 05)
use super::device_list::{BootableDeviceList, ConnectedDeviceList};
use super::tab_bar::TabBar;
use super::TargetTab;
use fdemon_app::ToolAvailability;

// Re-export TargetSelectorState from app layer for backward compatibility
pub use fdemon_app::new_session_dialog::TargetSelectorState;

/// The Target Selector widget (left pane of NewSessionDialog)
pub struct TargetSelector<'a> {
    state: &'a TargetSelectorState,
    tool_availability: &'a ToolAvailability,
    is_focused: bool,
    compact: bool,
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
            compact: false,
        }
    }

    /// Enable compact mode for narrow terminals
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Widget for TargetSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.compact {
            self.render_compact(area, buf);
        } else {
            self.render_full(area, buf);
        }
    }
}

impl TargetSelector<'_> {
    /// Render full (horizontal layout) mode
    fn render_full(&self, area: Rect, buf: &mut Buffer) {
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
            Constraint::Length(3), // Tab bar
            Constraint::Min(5),    // Content (device list)
            Constraint::Length(1), // Footer hints
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
                        self.state.scroll_offset,
                    );
                    list.render(chunks[1], buf);
                }
                TargetTab::Bootable => {
                    let list = BootableDeviceList::new(
                        &self.state.ios_simulators,
                        &self.state.android_avds,
                        self.state.selected_index,
                        self.is_focused,
                        self.state.scroll_offset,
                        self.tool_availability,
                    );
                    list.render(chunks[1], buf);
                }
            }
        }

        // Render footer
        self.render_footer(chunks[2], buf);
    }

    /// Render compact (vertical layout) mode - with border, tighter spacing
    fn render_compact(&self, area: Rect, buf: &mut Buffer) {
        // Add border with title
        let border_style = if self.is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(" Target Selector ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        // Compact mode: smaller tab bar, tighter spacing, no footer
        let chunks = Layout::vertical([
            Constraint::Length(1), // Compact tab bar (single line)
            Constraint::Min(1),    // Device list (reduced from 3 to account for borders)
        ])
        .split(inner);

        // Render compact tab bar
        self.render_tabs_compact(chunks[0], buf);

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
                        self.state.scroll_offset,
                    );
                    list.render(chunks[1], buf);
                }
                TargetTab::Bootable => {
                    let list = BootableDeviceList::new(
                        &self.state.ios_simulators,
                        &self.state.android_avds,
                        self.state.selected_index,
                        self.is_focused,
                        self.state.scroll_offset,
                        self.tool_availability,
                    );
                    list.render(chunks[1], buf);
                }
            }
        }
    }

    /// Render compact tab bar (abbreviated labels, single line)
    fn render_tabs_compact(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::text::Span;

        let connected_active = self.state.active_tab == TargetTab::Connected;
        let bootable_active = self.state.active_tab == TargetTab::Bootable;

        let style_active = if self.is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD)
        };
        let style_inactive = Style::default().fg(Color::DarkGray);

        let tabs = vec![
            Span::styled(
                if connected_active {
                    "[1]Connected"
                } else {
                    " 1 Connected"
                },
                if connected_active {
                    style_active
                } else {
                    style_inactive
                },
            ),
            Span::raw("  "),
            Span::styled(
                if bootable_active {
                    "[2]Bootable"
                } else {
                    " 2 Bootable"
                },
                if bootable_active {
                    style_active
                } else {
                    style_inactive
                },
            ),
        ];

        let paragraph = Paragraph::new(Line::from(tabs)).alignment(Alignment::Center);
        paragraph.render(area, buf);
    }
}

impl TargetSelector<'_> {
    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        let text = Paragraph::new("Discovering devices...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        text.render(area, buf);
    }

    fn render_error(&self, area: Rect, buf: &mut Buffer, error: &str) {
        let text = Paragraph::new(error)
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        text.render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = match self.state.active_tab {
            TargetTab::Connected => "[Enter] Select  [r] Refresh",
            TargetTab::Bootable => "[Enter] Boot  [r] Refresh",
        };

        let text = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        text.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_device_full;
    use fdemon_daemon::{AndroidAvd, Device, IosSimulator};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_target_selector_state_default() {
        let state = TargetSelectorState::default();
        assert_eq!(state.active_tab, TargetTab::Connected);
        assert!(state.loading);
        assert!(state.connected_devices.is_empty());
    }

    #[test]
    fn test_target_selector_state_new() {
        let state = TargetSelectorState::new();
        assert_eq!(state.active_tab, TargetTab::Connected);
        assert!(state.loading);
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
    fn test_set_bootable_devices() {
        use fdemon_daemon::SimulatorState;

        let mut state = TargetSelectorState::default();
        let ios_sims = vec![IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }];
        let android_avds = vec![AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        }];

        state.set_bootable_devices(ios_sims, android_avds);

        assert!(!state.bootable_loading);
        assert_eq!(state.ios_simulators.len(), 1);
        assert_eq!(state.android_avds.len(), 1);
    }

    #[test]
    fn test_set_error() {
        let mut state = TargetSelectorState::default();
        state.set_error("Test error".to_string());

        assert_eq!(state.error, Some("Test error".to_string()));
        assert!(!state.loading);
    }

    #[test]
    fn test_set_bootable_devices_clears_error() {
        let mut state = TargetSelectorState::default();
        state.error = Some("Previous error".to_string());

        state.set_bootable_devices(vec![], vec![]);

        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_connected_devices_clears_error() {
        let mut state = TargetSelectorState::default();
        state.error = Some("Previous error".to_string());

        state.set_connected_devices(vec![]);

        assert!(state.error.is_none());
    }

    #[test]
    fn test_select_next_empty_list() {
        let mut state = TargetSelectorState::default();
        state.loading = false;

        state.select_next();

        // Should not panic on empty list
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_select_previous_empty_list() {
        let mut state = TargetSelectorState::default();
        state.loading = false;

        state.select_previous();

        // Should not panic on empty list
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_selected_connected_device_wrong_tab() {
        let mut state = TargetSelectorState::default();
        state.active_tab = TargetTab::Bootable;
        state.connected_devices = vec![test_device_full("1", "iPhone", "ios", false)];

        assert!(state.selected_connected_device().is_none());
    }

    #[test]
    fn test_selected_bootable_device_wrong_tab() {
        use fdemon_daemon::SimulatorState;

        let mut state = TargetSelectorState::default();
        state.active_tab = TargetTab::Connected;
        state.ios_simulators = vec![IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }];

        assert!(state.selected_bootable_device().is_none());
    }

    #[test]
    fn test_target_selector_renders() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![test_device_full("1", "iPhone 15", "ios", false)]);

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

    #[test]
    fn test_target_selector_renders_loading() {
        let state = TargetSelectorState::default(); // loading = true by default
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

        assert!(content.contains("Discovering devices"));
    }

    #[test]
    fn test_target_selector_renders_error() {
        let mut state = TargetSelectorState::default();
        state.set_error("Failed to discover devices".to_string());

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

        assert!(content.contains("Failed to discover devices"));
    }

    #[test]
    fn test_target_selector_renders_bootable_tab() {
        use fdemon_daemon::SimulatorState;

        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.active_tab = TargetTab::Bootable;
        state.set_bootable_devices(
            vec![IosSimulator {
                udid: "123".to_string(),
                name: "iPhone 15".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 15".to_string(),
            }],
            vec![],
        );

        let tool_availability = ToolAvailability {
            xcrun_simctl: true,
            android_emulator: false,
            emulator_path: None,
        };

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

        assert!(content.contains("Bootable"));
        assert!(content.contains("iPhone 15"));
    }

    #[test]
    fn test_target_selector_unfocused() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![test_device_full("1", "iPhone", "ios", false)]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(50, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = TargetSelector::new(&state, &tool_availability, false);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should still render, just with different border color
        assert!(content.contains("Target Selector"));
    }

    #[test]
    fn test_navigation_uses_cached_list() {
        let mut state = TargetSelectorState::default();
        state.loading = false;

        // Create 10 devices
        let devices: Vec<Device> = (0..10)
            .map(|i| test_device_full(&format!("id{}", i), &format!("Device {}", i), "ios", false))
            .collect();

        state.set_connected_devices(devices);

        // First access computes cache
        let list1 = state.flat_list();
        let ptr1 = list1.as_ptr();

        // Navigation uses cache (same pointer)
        state.select_next();
        let list2 = state.flat_list();
        let ptr2 = list2.as_ptr();

        assert_eq!(ptr1, ptr2, "Should use cached list, not reallocate");

        // Another navigation still uses cache
        state.select_previous();
        let list3 = state.flat_list();
        let ptr3 = list3.as_ptr();

        assert_eq!(ptr1, ptr3, "Should still use cached list");
    }

    #[test]
    fn test_cache_invalidated_on_device_update() {
        let mut state = TargetSelectorState::default();
        state.loading = false;

        let devices = vec![test_device_full("1", "iPhone", "ios", false)];
        state.set_connected_devices(devices);

        // Populate cache
        let _ = state.flat_list();
        assert!(state.cached_flat_list.is_some());

        // Update devices should invalidate cache
        state.set_connected_devices(vec![test_device_full("2", "Pixel", "android", false)]);
        assert!(
            state.cached_flat_list.is_none(),
            "Cache should be invalidated after device update"
        );
    }

    #[test]
    fn test_cache_invalidated_on_bootable_update() {
        use fdemon_daemon::SimulatorState;

        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.active_tab = TargetTab::Bootable;

        let ios_sims = vec![IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }];

        state.set_bootable_devices(ios_sims, vec![]);

        // Populate cache
        let _ = state.flat_list();
        assert!(state.cached_flat_list.is_some());

        // Update bootable devices should invalidate cache
        state.set_bootable_devices(vec![], vec![]);
        assert!(
            state.cached_flat_list.is_none(),
            "Cache should be invalidated after bootable device update"
        );
    }

    #[test]
    fn test_cache_invalidated_on_tab_switch() {
        use fdemon_daemon::SimulatorState;

        let mut state = TargetSelectorState::default();
        state.loading = false;

        // Set up both connected and bootable devices
        state.set_connected_devices(vec![test_device_full("1", "iPhone", "ios", false)]);
        state.set_bootable_devices(
            vec![IosSimulator {
                udid: "123".to_string(),
                name: "iPhone 15".to_string(),
                runtime: "iOS 17.2".to_string(),
                state: SimulatorState::Shutdown,
                device_type: "iPhone 15".to_string(),
            }],
            vec![],
        );

        // Start on Connected tab
        state.active_tab = TargetTab::Connected;
        let _ = state.flat_list();
        assert!(state.cached_flat_list.is_some());

        // Switch to Bootable tab should invalidate cache
        state.set_tab(TargetTab::Bootable);
        assert!(
            state.cached_flat_list.is_none(),
            "Cache should be invalidated after tab switch"
        );
    }

    #[test]
    fn test_cache_repopulates_after_invalidation() {
        let mut state = TargetSelectorState::default();
        state.loading = false;

        let devices = vec![test_device_full("1", "iPhone", "ios", false)];
        state.set_connected_devices(devices);

        // First access
        let _ = state.flat_list();
        assert!(state.cached_flat_list.is_some());

        // Invalidate
        state.set_connected_devices(vec![test_device_full("2", "Pixel", "android", false)]);
        assert!(state.cached_flat_list.is_none());

        // Access again should repopulate
        let _ = state.flat_list();
        assert!(
            state.cached_flat_list.is_some(),
            "Cache should be repopulated on next access"
        );
    }

    #[test]
    fn test_scroll_offset_default() {
        let state = TargetSelectorState::default();
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_adjust_scroll_keeps_selection_visible() {
        let mut state = TargetSelectorState::default();
        state.loading = false;

        // Add 20 devices
        let devices: Vec<Device> = (0..20)
            .map(|i| test_device_full(&format!("id{}", i), &format!("Device {}", i), "ios", false))
            .collect();
        state.set_connected_devices(devices);

        state.selected_index = 15;
        state.scroll_offset = 0;

        state.adjust_scroll(10); // 10 visible items

        // Selection at 15 should require scroll offset of at least 6
        assert!(state.scroll_offset >= 6);
        assert!(state.scroll_offset <= 15);
    }

    #[test]
    fn test_scroll_resets_on_tab_switch() {
        let mut state = TargetSelectorState::default();
        state.scroll_offset = 5;
        state.set_tab(TargetTab::Bootable);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_resets_on_connected_devices_update() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.scroll_offset = 10;

        state.set_connected_devices(vec![test_device_full("1", "iPhone", "ios", false)]);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_resets_on_bootable_devices_update() {
        use fdemon_daemon::SimulatorState;

        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.scroll_offset = 10;

        let ios_sims = vec![IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }];

        state.set_bootable_devices(ios_sims, vec![]);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_reset_scroll() {
        let mut state = TargetSelectorState::default();
        state.scroll_offset = 15;

        state.reset_scroll();
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_adjust_scroll_zero_height() {
        let mut state = TargetSelectorState::default();
        state.scroll_offset = 5;

        state.adjust_scroll(0);
        // Should not change when visible_height is 0
        assert_eq!(state.scroll_offset, 5);
    }

    // Tests for Task 01 - Compact Borders and Titles

    #[test]
    fn test_target_selector_compact_has_border() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![test_device_full("1", "iPhone", "ios", false)]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = TargetSelector::new(&state, &tool_availability, true).compact(true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Check that title is rendered
        assert!(
            content.contains("Target Selector"),
            "Compact mode should show 'Target Selector' title"
        );

        // Check for border characters (Plain style uses │ and ─)
        assert!(
            content.contains("│") || content.contains("─"),
            "Compact mode should have border characters"
        );
    }

    #[test]
    fn test_target_selector_compact_focused_border() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![test_device_full("1", "Device", "ios", false)]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        // Test focused
        terminal
            .draw(|f| {
                let selector = TargetSelector::new(&state, &tool_availability, true).compact(true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        // Visual test - focused border should be cyan (can't easily test color)
        // Test passes if rendering doesn't panic
    }

    #[test]
    fn test_target_selector_compact_unfocused_border() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![test_device_full("1", "Device", "ios", false)]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        // Test unfocused
        terminal
            .draw(|f| {
                let selector = TargetSelector::new(&state, &tool_availability, false).compact(true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        // Visual test - unfocused border should be dark gray (can't easily test color)
        // Test passes if rendering doesn't panic
    }

    #[test]
    fn test_target_selector_compact_content_readable() {
        let mut state = TargetSelectorState::default();
        state.loading = false;
        state.set_connected_devices(vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "Pixel 6", "android", false),
        ]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = TargetSelector::new(&state, &tool_availability, true).compact(true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Check that content is still readable within borders
        assert!(
            content.contains("iPhone 15") || content.contains("Pixel 6"),
            "Device names should be visible within borders"
        );
    }
}
