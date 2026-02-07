//! Status bar widget
//!
//! Displays app state, build config info, session timer, and reload status.

use fdemon_app::config::FlutterMode;
use fdemon_app::state::AppState;
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
        let session = self.state.session_manager.selected();
        let (icon, label, style) = match session {
            None => crate::theme::styles::phase_indicator_disconnected(),
            Some(s) if s.session.is_busy() => crate::theme::styles::phase_indicator_busy(),
            Some(s) => crate::theme::styles::phase_indicator(&s.session.phase),
        };
        Span::styled(format!("{} {}", icon, label), style)
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

    /// Get session timer span (from selected session)
    fn session_timer(&self) -> Option<Span<'static>> {
        self.state
            .session_manager
            .selected()
            .and_then(|h| h.session.duration_display())
            .map(|d| Span::styled(format!("⏱ {}", d), Style::default().fg(Color::Gray)))
    }

    /// Get last reload span (from selected session)
    fn last_reload(&self) -> Option<Span<'static>> {
        self.state
            .session_manager
            .selected()
            .and_then(|h| h.session.last_reload_display())
            .map(|t| Span::styled(format!("↻ {}", t), Style::default().fg(Color::DarkGray)))
    }

    /// Get scroll indicator span (from selected session)
    fn scroll_indicator(&self) -> Span<'static> {
        let auto_scroll = self
            .state
            .session_manager
            .selected()
            .map(|h| h.session.log_view_state.auto_scroll)
            .unwrap_or(true);

        if auto_scroll {
            Span::styled("⬇ Auto", Style::default().fg(Color::Green))
        } else {
            Span::styled("⬆ Manual", Style::default().fg(Color::Yellow))
        }
    }

    /// Get error count span (from selected session)
    fn error_count(&self) -> Span<'static> {
        let error_count = self
            .state
            .session_manager
            .selected()
            .map(|h| h.session.error_count())
            .unwrap_or(0);

        if error_count == 0 {
            // No errors - dim green indicator
            Span::styled("✓ No errors", Style::default().fg(Color::DarkGray))
        } else {
            // Has errors - red, bold, attention-grabbing
            let text = if error_count == 1 {
                "✗ 1 error".to_string()
            } else {
                format!("✗ {} errors", error_count)
            };

            Span::styled(
                text,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        }
    }

    /// Get log position string (from selected session)
    fn log_position(&self) -> String {
        if let Some(handle) = self.state.session_manager.selected() {
            let state = &handle.session.log_view_state;
            if state.total_lines == 0 {
                "0/0".to_string()
            } else {
                let current = state.offset + 1;
                let end = (state.offset + state.visible_lines).min(state.total_lines);
                format!("{}-{}/{}", current, end, state.total_lines)
            }
        } else {
            "0/0".to_string()
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

        // Error count (always show if session exists)
        if self.state.session_manager.selected().is_some() {
            segments.push(separator.clone());
            segments.push(self.error_count());
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

        let session = self.state.session_manager.selected();
        let (icon, label, style) = match session {
            Some(s) if s.session.is_busy() => crate::theme::styles::phase_indicator_busy(),
            Some(s) => crate::theme::styles::phase_indicator(&s.session.phase),
            None => crate::theme::styles::phase_indicator_disconnected(),
        };

        // Show "Not Connected" label when no sessions exist
        if session.is_none() {
            let spans = vec![
                Span::raw(" "),
                Span::styled(format!("{} {}", icon, label), style),
            ];
            let line = Line::from(spans);
            Paragraph::new(line).render(inner, buf);
            return;
        }

        let timer = self
            .state
            .session_manager
            .selected()
            .and_then(|h| h.session.duration_display())
            .unwrap_or_default();

        // Get error count for compact display
        let error_count = self
            .state
            .session_manager
            .selected()
            .map(|h| h.session.error_count())
            .unwrap_or(0);

        let mut spans = vec![
            Span::raw(" "),
            Span::styled(icon, style),
            Span::raw(" "),
            Span::styled(timer, Style::default().fg(Color::Gray)),
        ];

        // Add compact error indicator if there are errors
        if error_count > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!("✗{}", error_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }

        let line = Line::from(spans);
        Paragraph::new(line).render(inner, buf);
    }
}

#[cfg(test)]
mod tests;
