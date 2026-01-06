//! Startup dialog widget for comprehensive session launching
//!
//! Displays a centered modal with:
//! - Configuration selection (launch.toml + launch.json)
//! - Mode selector (Debug/Profile/Release)
//! - Flavor text input
//! - Dart-defines text input
//! - Device selection with emulator launch options

mod styles;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use crate::app::state::{DialogSection, StartupDialogState};
use crate::config::{ConfigSource, FlutterMode};

pub use styles::*;

/// Startup dialog widget for session launching
pub struct StartupDialog<'a> {
    state: &'a StartupDialogState,
    /// Whether there are running sessions (affects Esc behavior)
    has_running_sessions: bool,
}

impl<'a> StartupDialog<'a> {
    pub fn new(state: &'a StartupDialogState) -> Self {
        Self {
            state,
            has_running_sessions: false,
        }
    }

    pub fn with_session_state(state: &'a StartupDialogState, has_running_sessions: bool) -> Self {
        Self {
            state,
            has_running_sessions,
        }
    }

    /// Calculate centered modal area (80% width, 70% height)
    fn centered_rect(area: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(area);

        Layout::horizontal([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(popup_layout[1])[1]
    }

    /// Render config list section
    fn render_config_list(&self, area: Rect, buf: &mut Buffer) {
        let is_active = self.state.active_section == DialogSection::Configs;
        let border_color = if is_active {
            ACTIVE_BORDER
        } else {
            INACTIVE_BORDER
        };

        let block = Block::default()
            .title(" Configuration ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.configs.configs.is_empty() {
            let no_configs = Paragraph::new("No configurations found")
                .style(Style::default().fg(PLACEHOLDER_COLOR))
                .alignment(Alignment::Center);
            no_configs.render(inner, buf);
            return;
        }

        let mut items: Vec<ListItem> = Vec::new();

        for (i, config) in self.state.configs.configs.iter().enumerate() {
            let is_selected = self.state.selected_config == Some(i);
            let is_vscode_start = self.state.configs.vscode_start_index == Some(i);

            // Add divider before VSCode configs
            if is_vscode_start && i > 0 {
                items.push(
                    ListItem::new("  ─────────────────────────────────")
                        .style(Style::default().fg(DIVIDER_COLOR)),
                );
            }

            let style = if is_selected && is_active {
                Style::default()
                    .fg(SELECTED_FG)
                    .bg(SELECTED_BG)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indicator = if is_selected { "▶ " } else { "  " };
            let source_tag = match config.source {
                ConfigSource::VSCode => " (VSCode)",
                _ => "",
            };

            let line = format!("{}{}{}", indicator, config.display_name, source_tag);
            items.push(ListItem::new(line).style(style));
        }

        let list = List::new(items);
        list.render(inner, buf);
    }

    /// Render mode selector (horizontal radio buttons)
    fn render_mode_selector(&self, area: Rect, buf: &mut Buffer) {
        let is_active = self.state.active_section == DialogSection::Mode;

        let debug_style = self.mode_style(FlutterMode::Debug, is_active);
        let profile_style = self.mode_style(FlutterMode::Profile, is_active);
        let release_style = self.mode_style(FlutterMode::Release, is_active);

        let line = Line::from(vec![
            Span::raw("  Mode: "),
            Span::styled(self.mode_indicator(FlutterMode::Debug), debug_style),
            Span::raw(" "),
            Span::styled(self.mode_indicator(FlutterMode::Profile), profile_style),
            Span::raw(" "),
            Span::styled(self.mode_indicator(FlutterMode::Release), release_style),
        ]);

        Paragraph::new(line).render(area, buf);
    }

    fn mode_indicator(&self, mode: FlutterMode) -> String {
        let selected = self.state.mode == mode;
        let icon = if selected { "●" } else { "○" };
        format!("{}{}", icon, mode)
    }

    fn mode_style(&self, mode: FlutterMode, section_active: bool) -> Style {
        let selected = self.state.mode == mode;
        if selected && section_active {
            Style::default()
                .fg(SELECTED_FG)
                .bg(SELECTED_BG)
                .add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(PLACEHOLDER_COLOR)
        }
    }

    /// Render text input field
    fn render_input_field(
        &self,
        area: Rect,
        buf: &mut Buffer,
        label: &str,
        value: &str,
        section: DialogSection,
    ) {
        let is_active = self.state.active_section == section;
        let is_editing = is_active && self.state.editing;

        // Show cursor at end when editing
        let display_value = if is_editing {
            format!("{}|", value)
        } else if value.is_empty() {
            "(optional)".to_string()
        } else {
            value.to_string()
        };

        // Highlight background when editing
        let value_style = if is_editing {
            Style::default().fg(VALUE_COLOR).bg(Color::DarkGray)
        } else if is_active {
            Style::default().fg(VALUE_COLOR)
        } else {
            Style::default().fg(if value.is_empty() {
                PLACEHOLDER_COLOR
            } else {
                VALUE_COLOR
            })
        };

        let line = Line::from(vec![
            Span::raw(format!("  {}: ", label)),
            Span::styled(format!("[{}]", display_value), value_style),
        ]);

        Paragraph::new(line)
            .style(Style::default().fg(if is_active { VALUE_COLOR } else { LABEL_COLOR }))
            .render(area, buf);
    }

    /// Render device list section
    fn render_device_list(&self, area: Rect, buf: &mut Buffer) {
        let is_active = self.state.active_section == DialogSection::Devices;
        let border_color = if is_active {
            ACTIVE_BORDER
        } else {
            INACTIVE_BORDER
        };

        let block = Block::default()
            .title(" Device ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.loading {
            let loading = Paragraph::new("Discovering devices...")
                .style(Style::default().fg(LOADING_COLOR))
                .alignment(Alignment::Center);
            loading.render(inner, buf);
            return;
        }

        if let Some(ref error) = self.state.error {
            let error_text = Paragraph::new(error.as_str())
                .style(Style::default().fg(ERROR_COLOR))
                .alignment(Alignment::Center);
            error_text.render(inner, buf);
            return;
        }

        let mut items: Vec<ListItem> = Vec::new();

        for (i, device) in self.state.devices.iter().enumerate() {
            let is_selected = self.state.selected_device == Some(i);

            let style = if is_selected && is_active {
                Style::default()
                    .fg(SELECTED_FG)
                    .bg(SELECTED_BG)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indicator = if is_selected { "▶ " } else { "  " };
            let device_type = if device.emulator {
                device.emulator_type()
            } else {
                "physical"
            };

            let line = format!("{}{} ({})", indicator, device.name, device_type);
            items.push(ListItem::new(line).style(style));
        }

        // Add emulator launch options
        if !self.state.devices.is_empty() {
            items.push(
                ListItem::new("  ─────────────────────────────────")
                    .style(Style::default().fg(DIVIDER_COLOR)),
            );
        }

        let android_style = Style::default().fg(EMULATOR_ANDROID);
        items.push(ListItem::new("  + Launch Android Emulator").style(android_style));

        let ios_style = Style::default().fg(EMULATOR_IOS);
        items.push(ListItem::new("  + Launch iOS Simulator").style(ios_style));

        let list = List::new(items);
        list.render(inner, buf);
    }

    /// Render footer with keybindings
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = if self.has_running_sessions {
            "[Tab] Section  [↑↓] Navigate  [Enter] Launch  [Esc] Cancel  [r] Refresh"
        } else {
            "[Tab] Section  [↑↓] Navigate  [Enter] Launch  [r] Refresh"
        };

        Paragraph::new(hints)
            .alignment(Alignment::Center)
            .style(Style::default().fg(LABEL_COLOR))
            .render(area, buf);
    }
}

impl Widget for StartupDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_area = Self::centered_rect(area);

        // Clear background
        Clear.render(modal_area, buf);

        // Modal block
        let block = Block::default()
            .title(" Launch Session ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout sections
        let chunks = Layout::vertical([
            Constraint::Length(8), // Config list
            Constraint::Length(2), // Mode selector
            Constraint::Length(1), // Flavor input
            Constraint::Length(1), // Dart defines input
            Constraint::Length(1), // Spacer
            Constraint::Min(6),    // Device list
            Constraint::Length(2), // Footer
        ])
        .split(inner);

        self.render_config_list(chunks[0], buf);
        self.render_mode_selector(chunks[1], buf);
        self.render_input_field(
            chunks[2],
            buf,
            "Flavor",
            &self.state.flavor,
            DialogSection::Flavor,
        );
        self.render_input_field(
            chunks[3],
            buf,
            "Dart Defines",
            &self.state.dart_defines,
            DialogSection::DartDefines,
        );
        self.render_device_list(chunks[5], buf);
        self.render_footer(chunks[6], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LoadedConfigs, SourcedConfig};
    use crate::daemon::Device;
    use ratatui::{backend::TestBackend, Terminal};

    // Helper to create a test device
    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    #[test]
    fn test_startup_dialog_renders() {
        let state = StartupDialogState::new();
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Launch Session"));
        assert!(content.contains("Configuration"));
        assert!(content.contains("Device"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = StartupDialog::centered_rect(area);

        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.width < area.width);
        assert!(centered.height < area.height);
    }

    #[test]
    fn test_startup_dialog_with_devices() {
        let mut state = StartupDialogState::new();
        state.set_devices(vec![
            test_device("dev1", "Device 1"),
            test_device("dev2", "Device 2"),
        ]);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Device 1"));
        assert!(content.contains("Device 2"));
    }

    #[test]
    fn test_startup_dialog_loading_state() {
        let state = StartupDialogState::new(); // Default is loading

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Discovering devices"));
    }

    #[test]
    fn test_startup_dialog_error_state() {
        let mut state = StartupDialogState::new();
        state.set_error("Failed to discover devices".to_string());

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Failed to discover devices"));
    }

    #[test]
    fn test_startup_dialog_mode_selector() {
        let state = StartupDialogState::new();
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should show mode options
        assert!(content.contains("Mode:"));
        assert!(content.contains("debug"));
    }

    #[test]
    fn test_startup_dialog_with_session_state() {
        let state = StartupDialogState::new();

        // Without sessions
        let dialog = StartupDialog::with_session_state(&state, false);
        assert!(!dialog.has_running_sessions);

        // With sessions
        let dialog = StartupDialog::with_session_state(&state, true);
        assert!(dialog.has_running_sessions);
    }

    #[test]
    fn test_mode_indicator_selected() {
        let state = StartupDialogState::new();
        let dialog = StartupDialog::new(&state);

        // Debug is selected by default
        assert!(dialog.mode_indicator(FlutterMode::Debug).contains("●"));
        assert!(dialog.mode_indicator(FlutterMode::Profile).contains("○"));
        assert!(dialog.mode_indicator(FlutterMode::Release).contains("○"));
    }

    #[test]
    fn test_startup_dialog_with_configs() {
        use crate::config::LaunchConfig;

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Test Config".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Test Config".to_string(),
        });

        let state = StartupDialogState::with_configs(configs);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Test Config"));
    }

    #[test]
    fn test_startup_dialog_empty_configs() {
        let configs = LoadedConfigs::default();
        let state = StartupDialogState::with_configs(configs);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = StartupDialog::new(&state);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("No configurations found"));
    }
}
