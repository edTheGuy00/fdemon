//! Status bar widget
//!
//! Displays app state, device info, session timer, and reload status.

use crate::app::state::AppState;
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

    /// Get device info span
    fn device_info(&self) -> Option<Span<'static>> {
        match (&self.state.device_name, &self.state.platform) {
            (Some(name), Some(platform)) => Some(Span::styled(
                format!("{} ({})", name, platform),
                Style::default().fg(Color::Cyan),
            )),
            (Some(name), None) => {
                Some(Span::styled(name.clone(), Style::default().fg(Color::Cyan)))
            }
            (None, Some(platform)) => Some(Span::styled(
                format!("({})", platform),
                Style::default().fg(Color::Cyan),
            )),
            _ => None,
        }
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

        // Device info
        if let Some(device) = self.device_info() {
            segments.push(separator.clone());
            segments.push(device);
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
    use chrono::{Duration, Local};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn create_test_state() -> AppState {
        AppState::new()
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
    fn test_device_info_both() {
        let mut state = create_test_state();
        state.device_name = Some("iPhone 15 Pro".to_string());
        state.platform = Some("ios".to_string());

        let bar = StatusBar::new(&state);
        let device = bar.device_info().unwrap();

        assert!(device.content.to_string().contains("iPhone 15 Pro"));
        assert!(device.content.to_string().contains("ios"));
    }

    #[test]
    fn test_device_info_name_only() {
        let mut state = create_test_state();
        state.device_name = Some("Pixel".to_string());

        let bar = StatusBar::new(&state);
        let device = bar.device_info().unwrap();

        assert!(device.content.to_string().contains("Pixel"));
    }

    #[test]
    fn test_device_info_none() {
        let state = create_test_state();
        let bar = StatusBar::new(&state);

        assert!(bar.device_info().is_none());
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
    fn test_build_segments_with_device() {
        let mut state = create_test_state();
        state.phase = AppPhase::Running;
        state.device_name = Some("Pixel".to_string());
        state.platform = Some("android".to_string());

        let bar = StatusBar::new(&state);
        let segments = bar.build_segments();

        // Collect all content
        let content: String = segments.iter().map(|s| s.content.to_string()).collect();

        assert!(content.contains("Running"));
        assert!(content.contains("Pixel"));
        assert!(content.contains("android"));
    }

    #[test]
    fn test_status_bar_render() {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut state = create_test_state();
        state.phase = AppPhase::Running;
        state.device_name = Some("Test Device".to_string());
        state.platform = Some("test".to_string());

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
        assert!(content.contains("Test Device"));
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
