//! Status bar widget
//!
//! Displays app state, build config info, session timer, and reload status.

use crate::app::state::AppState;
use crate::config::FlutterMode;
use crate::core::AppPhase;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Status bar widget showing application state
pub struct StatusBar<'a> {
    state: &'a AppState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// Get the state indicator with appropriate styling
    fn state_indicator(&self) -> Span<'static> {
        match self.state.phase {
            AppPhase::Initializing => {
                Span::styled("○ Starting", Style::default().fg(Color::DarkGray))
            }
            AppPhase::Running if self.state.is_busy() => Span::styled(
                "↻ Reloading",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            AppPhase::Running => Span::styled(
                "● Running",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            AppPhase::Reloading => Span::styled(
                "↻ Reloading",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            AppPhase::Stopped => Span::styled("○ Stopped", Style::default().fg(Color::DarkGray)),
            AppPhase::Quitting => Span::styled("○ Stopping", Style::default().fg(Color::DarkGray)),
        }
    }

    /// Get build configuration info span (Debug/Profile/Release + optional flavor)
    fn config_info(&self) -> Option<Span<'static>> {
        // Get selected session's config
        let session = self.state.session_manager.selected()?;
        let session_data = &session.session;

        // Get mode and flavor from launch_config, default to Debug
        let (mode, flavor) = match &session_data.launch_config {
            Some(config) => (config.mode, config.flavor.clone()),
            None => (FlutterMode::Debug, None),
        };

        // Format the display string with capitalized mode name
        let mode_str = match mode {
            FlutterMode::Debug => "Debug",
            FlutterMode::Profile => "Profile",
            FlutterMode::Release => "Release",
        };

        let display = match flavor {
            Some(f) => format!("{} ({})", mode_str, f),
            None => mode_str.to_string(),
        };

        // Color based on mode
        let color = match mode {
            FlutterMode::Debug => Color::Green,
            FlutterMode::Profile => Color::Yellow,
            FlutterMode::Release => Color::Magenta,
        };

        Some(Span::styled(display, Style::default().fg(color)))
    }

    /// Get Flutter version span
    fn flutter_version(&self) -> Option<Span<'static>> {
        self.state
            .flutter_version
            .as_ref()
            .map(|v| Span::styled(format!("Flutter {}", v), Style::default().fg(Color::Blue)))
    }

    /// Get session timer span
    fn session_timer(&self) -> Option<Span<'static>> {
        self.state
            .session_duration_display()
            .map(|d| Span::styled(format!("⏱ {}", d), Style::default().fg(Color::Gray)))
    }

    /// Get last reload span
    fn last_reload(&self) -> Option<Span<'static>> {
        self.state
            .last_reload_display()
            .map(|t| Span::styled(format!("↻ {}", t), Style::default().fg(Color::DarkGray)))
    }

    /// Get scroll indicator span
    fn scroll_indicator(&self) -> Span<'static> {
        if self.state.log_view_state.auto_scroll {
            Span::styled("⬇ Auto", Style::default().fg(Color::Green))
        } else {
            Span::styled("⬆ Manual", Style::default().fg(Color::Yellow))
        }
    }

    /// Get log position string
    fn log_position(&self) -> String {
        let state = &self.state.log_view_state;
        if state.total_lines == 0 {
            "0/0".to_string()
        } else {
            let current = state.offset + 1;
            let end = (state.offset + state.visible_lines).min(state.total_lines);
            format!("{}-{}/{}", current, end, state.total_lines)
        }
    }

    /// Build all segments with separators
    fn build_segments(&self) -> Vec<Span<'static>> {
        let separator = Span::styled(" │ ", Style::default().fg(Color::DarkGray));

        let mut segments = Vec::new();

        // Left padding and state indicator (always show)
        segments.push(Span::raw(" "));
        segments.push(self.state_indicator());

        // Build config info (Debug/Profile/Release + flavor)
        if let Some(config) = self.config_info() {
            segments.push(separator.clone());
            segments.push(config);
        }

        // Flutter version
        if let Some(version) = self.flutter_version() {
            segments.push(separator.clone());
            segments.push(version);
        }

        // Session timer
        if let Some(timer) = self.session_timer() {
            segments.push(separator.clone());
            segments.push(timer);
        }

        // Last reload
        if let Some(reload) = self.last_reload() {
            segments.push(separator.clone());
            segments.push(reload);
        }

        // Scroll status and log position
        segments.push(separator.clone());
        segments.push(self.scroll_indicator());
        segments.push(Span::raw(" "));
        segments.push(Span::styled(
            self.log_position(),
            Style::default().fg(Color::DarkGray),
        ));

        segments.push(Span::raw(" ")); // Right padding

        segments
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create block with top border (looks like separator)
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Build and render the status line
        let segments = self.build_segments();
        let line = Line::from(segments);

        Paragraph::new(line).render(inner, buf);
    }
}

/// Compact status bar for narrow terminals (< 60 columns)
pub struct StatusBarCompact<'a> {
    state: &'a AppState,
}

impl<'a> StatusBarCompact<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl Widget for StatusBarCompact<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Compact: just state and timer
        let (state_char, color) = match self.state.phase {
            AppPhase::Running if self.state.is_busy() => ("↻", Color::Yellow),
            AppPhase::Running => ("●", Color::Green),
            AppPhase::Reloading => ("↻", Color::Yellow),
            _ => ("○", Color::DarkGray),
        };

        let timer = self.state.session_duration_display().unwrap_or_default();

        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                state_char,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(timer, Style::default().fg(Color::Gray)),
        ]);

        Paragraph::new(line).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LaunchConfig;
    use crate::daemon::Device;
    use chrono::{Duration, Local};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn create_test_state() -> AppState {
        AppState::new()
    }

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
    fn test_state_indicator_initializing() {
        let state = create_test_state();
        let bar = StatusBar::new(&state);
        let indicator = bar.state_indicator();

        assert!(indicator.style.fg == Some(Color::DarkGray));
        assert!(indicator.content.to_string().contains("Starting"));
    }

    #[test]
    fn test_state_indicator_running() {
        let mut state = create_test_state();
        state.phase = AppPhase::Running;

        let bar = StatusBar::new(&state);
        let indicator = bar.state_indicator();

        assert!(indicator.style.fg == Some(Color::Green));
        assert!(indicator.content.to_string().contains("Running"));
    }

    #[test]
    fn test_state_indicator_reloading() {
        let mut state = create_test_state();
        state.phase = AppPhase::Reloading;

        let bar = StatusBar::new(&state);
        let indicator = bar.state_indicator();

        assert!(indicator.style.fg == Some(Color::Yellow));
        assert!(indicator.content.to_string().contains("Reloading"));
    }

    #[test]
    fn test_state_indicator_quitting() {
        let mut state = create_test_state();
        state.phase = AppPhase::Quitting;

        let bar = StatusBar::new(&state);
        let indicator = bar.state_indicator();

        assert!(indicator.style.fg == Some(Color::DarkGray));
        assert!(indicator.content.to_string().contains("Stopping"));
    }

    #[test]
    fn test_config_info_debug_mode() {
        let mut state = create_test_state();
        let device = test_device("d1", "iPhone");
        let mut config = LaunchConfig::default();
        config.mode = FlutterMode::Debug;
        config.flavor = None;

        let id = state
            .session_manager
            .create_session_with_config(&device, config)
            .unwrap();
        state.session_manager.select_by_id(id);

        let bar = StatusBar::new(&state);
        let config_span = bar.config_info().unwrap();

        assert!(config_span.content.to_string().contains("Debug"));
        assert_eq!(config_span.style.fg, Some(Color::Green));
    }

    #[test]
    fn test_config_info_profile_mode() {
        let mut state = create_test_state();
        let device = test_device("d1", "iPhone");
        let mut config = LaunchConfig::default();
        config.mode = FlutterMode::Profile;
        config.flavor = None;

        let id = state
            .session_manager
            .create_session_with_config(&device, config)
            .unwrap();
        state.session_manager.select_by_id(id);

        let bar = StatusBar::new(&state);
        let config_span = bar.config_info().unwrap();

        assert!(config_span.content.to_string().contains("Profile"));
        assert_eq!(config_span.style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_config_info_release_with_flavor() {
        let mut state = create_test_state();
        let device = test_device("d1", "Pixel");
        let mut config = LaunchConfig::default();
        config.mode = FlutterMode::Release;
        config.flavor = Some("production".to_string());

        let id = state
            .session_manager
            .create_session_with_config(&device, config)
            .unwrap();
        state.session_manager.select_by_id(id);

        let bar = StatusBar::new(&state);
        let config_span = bar.config_info().unwrap();

        assert!(config_span.content.to_string().contains("Release"));
        assert!(config_span.content.to_string().contains("production"));
        assert_eq!(config_span.style.fg, Some(Color::Magenta));
    }

    #[test]
    fn test_config_info_no_session() {
        let state = create_test_state();
        let bar = StatusBar::new(&state);

        assert!(bar.config_info().is_none());
    }

    #[test]
    fn test_config_info_no_launch_config() {
        let mut state = create_test_state();
        let device = test_device("d1", "Device");
        let id = state.session_manager.create_session(&device).unwrap();
        state.session_manager.select_by_id(id);

        let bar = StatusBar::new(&state);
        let config_span = bar.config_info().unwrap();

        // Should default to Debug
        assert!(config_span.content.to_string().contains("Debug"));
        assert_eq!(config_span.style.fg, Some(Color::Green));
    }

    #[test]
    fn test_flutter_version() {
        let mut state = create_test_state();
        state.flutter_version = Some("3.19.0".to_string());

        let bar = StatusBar::new(&state);
        let version = bar.flutter_version().unwrap();

        assert!(version.content.to_string().contains("Flutter 3.19.0"));
    }

    #[test]
    fn test_session_timer() {
        let mut state = create_test_state();
        state.session_start = Some(Local::now() - Duration::seconds(3723)); // 1h 2m 3s

        let bar = StatusBar::new(&state);
        let timer = bar.session_timer().unwrap();

        assert!(timer.content.to_string().contains("01:02:03"));
    }

    #[test]
    fn test_last_reload() {
        let mut state = create_test_state();
        state.last_reload_time = Some(Local::now());

        let bar = StatusBar::new(&state);
        let reload = bar.last_reload();

        assert!(reload.is_some());
    }

    #[test]
    fn test_build_segments_minimal() {
        let state = create_test_state();
        let bar = StatusBar::new(&state);
        let segments = bar.build_segments();

        // Should have at least: padding, state, separator, scroll, pos, padding
        assert!(segments.len() >= 6);
    }

    #[test]
    fn test_build_segments_with_config() {
        let mut state = create_test_state();
        state.phase = AppPhase::Running;

        // Create a session with release config and flavor
        let device = test_device("d1", "Pixel");
        let mut config = LaunchConfig::default();
        config.mode = FlutterMode::Release;
        config.flavor = Some("staging".to_string());

        let id = state
            .session_manager
            .create_session_with_config(&device, config)
            .unwrap();
        state.session_manager.select_by_id(id);

        let bar = StatusBar::new(&state);
        let segments = bar.build_segments();

        // Collect all content
        let content: String = segments.iter().map(|s| s.content.to_string()).collect();

        assert!(content.contains("Running"));
        assert!(content.contains("Release"));
        assert!(content.contains("staging"));
    }

    #[test]
    fn test_status_bar_render() {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut state = create_test_state();
        state.phase = AppPhase::Running;

        // Create a session with config
        let device = test_device("d1", "Test Device");
        let mut config = LaunchConfig::default();
        config.mode = FlutterMode::Debug;

        let id = state
            .session_manager
            .create_session_with_config(&device, config)
            .unwrap();
        state.session_manager.select_by_id(id);

        terminal
            .draw(|frame| {
                let area = frame.area();
                let bar = StatusBar::new(&state);
                frame.render_widget(bar, area);
            })
            .unwrap();

        // Verify the buffer contains expected text
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        assert!(content.contains("Running"));
        assert!(content.contains("Debug")); // Config info now shows instead of device
    }

    #[test]
    fn test_compact_status_bar_render() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut state = create_test_state();
        state.phase = AppPhase::Running;
        state.session_start = Some(Local::now() - Duration::seconds(60));

        terminal
            .draw(|frame| {
                let area = frame.area();
                let bar = StatusBarCompact::new(&state);
                frame.render_widget(bar, area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Should contain the running indicator
        assert!(content.contains("●"));
    }

    #[test]
    fn test_log_position_empty() {
        let state = create_test_state();
        let bar = StatusBar::new(&state);

        assert_eq!(bar.log_position(), "0/0");
    }

    #[test]
    fn test_scroll_indicator_auto() {
        let mut state = create_test_state();
        state.log_view_state.auto_scroll = true;

        let bar = StatusBar::new(&state);
        let indicator = bar.scroll_indicator();

        assert!(indicator.content.to_string().contains("Auto"));
        assert!(indicator.style.fg == Some(Color::Green));
    }

    #[test]
    fn test_scroll_indicator_manual() {
        let mut state = create_test_state();
        state.log_view_state.auto_scroll = false;

        let bar = StatusBar::new(&state);
        let indicator = bar.scroll_indicator();

        assert!(indicator.content.to_string().contains("Manual"));
        assert!(indicator.style.fg == Some(Color::Yellow));
    }
}
