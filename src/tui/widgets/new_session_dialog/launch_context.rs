//! Field widgets for the Launch Context pane
//!
//! This module provides individual field widgets used in the Launch Context pane:
//! - LaunchContextStyles: Styling constants for all field widgets
//! - DropdownField: Dropdown-style field that opens fuzzy modal
//! - ModeSelector: Radio button group for Debug/Profile/Release
//! - ActionField: Field that opens a modal (for Dart Defines)
//! - LaunchButton: Launch button with focused/enabled states

use crate::config::FlutterMode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

/// Styles for Launch Context fields
pub struct LaunchContextStyles {
    pub label: Style,
    pub value_normal: Style,
    pub value_focused: Style,
    pub value_disabled: Style,
    pub placeholder: Style,
    pub button_normal: Style,
    pub button_focused: Style,
    pub mode_selected: Style,
    pub mode_unselected: Style,
    pub suffix: Style,
}

impl Default for LaunchContextStyles {
    fn default() -> Self {
        Self {
            label: Style::default().fg(Color::Gray),
            value_normal: Style::default().fg(Color::White),
            value_focused: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            value_disabled: Style::default().fg(Color::DarkGray),
            placeholder: Style::default().fg(Color::DarkGray),
            button_normal: Style::default().fg(Color::Green),
            button_focused: Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
            mode_selected: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            mode_unselected: Style::default().fg(Color::DarkGray),
            suffix: Style::default().fg(Color::DarkGray),
        }
    }
}

/// A dropdown-style field that opens a fuzzy modal
pub struct DropdownField {
    label: String,
    value: String,
    is_focused: bool,
    is_disabled: bool,
    suffix: Option<String>,
    styles: LaunchContextStyles,
}

impl DropdownField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            is_focused: false,
            is_disabled: false,
            suffix: None,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }
}

impl Widget for DropdownField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Layout: label + value box
        let chunks = Layout::horizontal([
            Constraint::Length(15), // Label
            Constraint::Min(20),    // Value
        ])
        .split(area);

        // Render label
        let label = Paragraph::new(format!("  {}:", self.label)).style(self.styles.label);
        label.render(chunks[0], buf);

        // Determine value style
        let value_style = if self.is_disabled {
            self.styles.value_disabled
        } else if self.is_focused {
            self.styles.value_focused
        } else {
            self.styles.value_normal
        };

        // Format value with dropdown indicator and suffix
        let display_value = if self.value.is_empty() || self.value == "(none)" {
            "(none)".to_string()
        } else {
            self.value.clone()
        };

        let dropdown_indicator = if self.is_disabled { " " } else { " ▼" };
        let suffix_text = self.suffix.map(|s| format!("  {}", s)).unwrap_or_default();

        let value_line = Line::from(vec![
            Span::styled(format!("[ {}", display_value), value_style),
            Span::styled(dropdown_indicator, value_style),
            Span::styled(" ]", value_style),
            Span::styled(suffix_text, self.styles.suffix),
        ]);

        Paragraph::new(value_line).render(chunks[1], buf);
    }
}

/// Radio button group for Flutter mode selection
pub struct ModeSelector {
    selected: FlutterMode,
    is_focused: bool,
    is_disabled: bool,
    styles: LaunchContextStyles,
}

impl ModeSelector {
    pub fn new(selected: FlutterMode) -> Self {
        Self {
            selected,
            is_focused: false,
            is_disabled: false,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    fn mode_style(&self, mode: FlutterMode) -> Style {
        if self.is_disabled {
            self.styles.value_disabled
        } else if mode == self.selected && self.is_focused {
            self.styles.value_focused
        } else if mode == self.selected {
            self.styles.mode_selected
        } else {
            self.styles.mode_unselected
        }
    }

    fn mode_indicator(&self, mode: FlutterMode) -> &'static str {
        if mode == self.selected {
            "(●)"
        } else {
            "(○)"
        }
    }
}

impl Widget for ModeSelector {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Length(15), // Label
            Constraint::Min(40),    // Radio buttons
        ])
        .split(area);

        // Render label
        let label = Paragraph::new("  Mode:").style(self.styles.label);
        label.render(chunks[0], buf);

        // Render radio buttons
        let debug_style = self.mode_style(FlutterMode::Debug);
        let profile_style = self.mode_style(FlutterMode::Profile);
        let release_style = self.mode_style(FlutterMode::Release);

        let line = Line::from(vec![
            Span::styled(
                format!("{} Debug", self.mode_indicator(FlutterMode::Debug)),
                debug_style,
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Profile", self.mode_indicator(FlutterMode::Profile)),
                profile_style,
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Release", self.mode_indicator(FlutterMode::Release)),
                release_style,
            ),
        ]);

        Paragraph::new(line).render(chunks[1], buf);
    }
}

/// A field that opens a modal when activated
pub struct ActionField {
    label: String,
    value: String,
    action_indicator: &'static str,
    is_focused: bool,
    is_disabled: bool,
    styles: LaunchContextStyles,
}

impl ActionField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            action_indicator: "▶",
            is_focused: false,
            is_disabled: false,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }
}

impl Widget for ActionField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Length(15), // Label
            Constraint::Min(20),    // Value
        ])
        .split(area);

        // Render label
        let label = Paragraph::new(format!("  {}:", self.label)).style(self.styles.label);
        label.render(chunks[0], buf);

        // Determine value style
        let value_style = if self.is_disabled {
            self.styles.value_disabled
        } else if self.is_focused {
            self.styles.value_focused
        } else {
            self.styles.value_normal
        };

        let indicator = if self.is_disabled {
            " "
        } else {
            self.action_indicator
        };

        let value_line = Line::from(vec![
            Span::styled(format!("[ {} ", self.value), value_style),
            Span::styled(indicator, value_style),
            Span::styled(" ]", value_style),
        ]);

        Paragraph::new(value_line).render(chunks[1], buf);
    }
}

/// The launch button at the bottom of Launch Context
pub struct LaunchButton {
    is_focused: bool,
    is_enabled: bool,
    styles: LaunchContextStyles,
}

impl LaunchButton {
    pub fn new() -> Self {
        Self {
            is_focused: false,
            is_enabled: true,
            styles: LaunchContextStyles::default(),
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
}

impl Default for LaunchButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for LaunchButton {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = if !self.is_enabled {
            Style::default().fg(Color::DarkGray)
        } else if self.is_focused {
            self.styles.button_focused
        } else {
            self.styles.button_normal
        };

        let text = if self.is_enabled {
            "    LAUNCH (Enter)    "
        } else {
            "    SELECT DEVICE     "
        };

        let button = Paragraph::new(format!("[{}]", text))
            .style(style)
            .alignment(Alignment::Center);

        button.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_dropdown_field_renders() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = DropdownField::new("Config", "Development").focused(true);
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Config"));
        assert!(content.contains("Development"));
    }

    #[test]
    fn test_mode_selector_renders() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = ModeSelector::new(FlutterMode::Debug).focused(true);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Debug"));
        assert!(content.contains("Profile"));
        assert!(content.contains("Release"));
    }

    #[test]
    fn test_launch_button_renders() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let button = LaunchButton::new().focused(true);
                f.render_widget(button, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("LAUNCH"));
    }

    #[test]
    fn test_disabled_field_styling() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = DropdownField::new("Flavor", "dev")
                    .disabled(true)
                    .suffix("(from config)");
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("from config"));
    }

    #[test]
    fn test_action_field_renders() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = ActionField::new("Dart Defines", "2 defined").focused(true);
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Dart Defines"));
        assert!(content.contains("2 defined"));
    }

    #[test]
    fn test_dropdown_field_none_value() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = DropdownField::new("Config", "");
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("(none)"));
    }

    #[test]
    fn test_mode_selector_indicators() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let selector = ModeSelector::new(FlutterMode::Profile);
                f.render_widget(selector, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Check for filled/empty radio button indicators
        assert!(content.contains("(○)") || content.contains("(●)"));
    }

    #[test]
    fn test_launch_button_disabled_text() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let button = LaunchButton::new().enabled(false);
                f.render_widget(button, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("SELECT DEVICE"));
    }

    #[test]
    fn test_dropdown_field_disabled_no_indicator() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = DropdownField::new("Config", "test").disabled(true);
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Disabled field should not have dropdown indicator
        // Just check that content renders without panic
        assert!(content.contains("Config"));
    }

    #[test]
    fn test_action_field_disabled_no_indicator() {
        let backend = TestBackend::new(50, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let field = ActionField::new("Defines", "none").disabled(true);
                f.render_widget(field, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Disabled field should not have action indicator
        assert!(content.contains("Defines"));
    }
}

// ============================================================================
// Main LaunchContext Widget
// ============================================================================

use super::state::LaunchContextState;
use crate::config::ConfigSource;
use ratatui::widgets::{Block, Borders};

// ============================================================================
// Shared Helper Functions
// ============================================================================

/// Check if a field should show "(from config)" suffix
fn should_show_disabled_suffix(
    state: &LaunchContextState,
    field: super::state::LaunchContextField,
) -> bool {
    !state.is_field_editable(field)
        && matches!(state.selected_config_source(), Some(ConfigSource::VSCode))
}

/// Render the configuration dropdown field
fn render_config_field(area: Rect, buf: &mut Buffer, state: &LaunchContextState, is_focused: bool) {
    let config_focused =
        is_focused && state.focused_field == super::state::LaunchContextField::Config;
    let config_field =
        DropdownField::new("Configuration", state.config_display()).focused(config_focused);
    config_field.render(area, buf);
}

/// Render the mode selector (Debug/Profile/Release radio buttons)
fn render_mode_field(area: Rect, buf: &mut Buffer, state: &LaunchContextState, is_focused: bool) {
    let mode_focused = is_focused && state.focused_field == super::state::LaunchContextField::Mode;
    let mode_disabled = !state.is_mode_editable();
    let mode_selector = ModeSelector::new(state.mode)
        .focused(mode_focused)
        .disabled(mode_disabled);
    mode_selector.render(area, buf);
}

/// Render the flavor dropdown field
fn render_flavor_field(area: Rect, buf: &mut Buffer, state: &LaunchContextState, is_focused: bool) {
    let flavor_focused =
        is_focused && state.focused_field == super::state::LaunchContextField::Flavor;
    let flavor_disabled = !state.is_flavor_editable();
    let flavor_suffix =
        if should_show_disabled_suffix(state, super::state::LaunchContextField::Flavor) {
            Some("(from config)")
        } else {
            None
        };
    let mut flavor_field =
        DropdownField::new("Flavor", state.flavor_display()).focused(flavor_focused);
    flavor_field = flavor_field.disabled(flavor_disabled);
    if let Some(suffix) = flavor_suffix {
        flavor_field = flavor_field.suffix(suffix);
    }
    flavor_field.render(area, buf);
}

/// Render the dart defines action field
fn render_dart_defines_field(
    area: Rect,
    buf: &mut Buffer,
    state: &LaunchContextState,
    is_focused: bool,
) {
    let defines_focused =
        is_focused && state.focused_field == super::state::LaunchContextField::DartDefines;
    let defines_disabled = !state.are_dart_defines_editable();
    let defines_field = ActionField::new("Dart Defines", state.dart_defines_display())
        .focused(defines_focused)
        .disabled(defines_disabled);
    defines_field.render(area, buf);
}

/// Calculate the layout for all fields
fn calculate_fields_layout(inner: Rect) -> [Rect; 11] {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Config field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Mode field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Flavor field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Dart Defines field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Launch button
        Constraint::Min(0),    // Rest (empty)
    ])
    .split(inner);

    [
        chunks[0], chunks[1], chunks[2], chunks[3], chunks[4], chunks[5], chunks[6], chunks[7],
        chunks[8], chunks[9], chunks[10],
    ]
}

/// Render the border block and return the inner area
fn render_border(area: Rect, buf: &mut Buffer, is_focused: bool) -> Rect {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(" Launch Context ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    block.render(area, buf);
    inner
}

/// Render all common fields (config, mode, flavor, dart defines)
fn render_common_fields(
    chunks: &[Rect; 11],
    buf: &mut Buffer,
    state: &LaunchContextState,
    is_focused: bool,
) {
    render_config_field(chunks[1], buf, state, is_focused);
    render_mode_field(chunks[3], buf, state, is_focused);
    render_flavor_field(chunks[5], buf, state, is_focused);
    render_dart_defines_field(chunks[7], buf, state, is_focused);
}

// ============================================================================
// LaunchContext Widget
// ============================================================================

/// The Launch Context widget (right pane of NewSessionDialog)
pub struct LaunchContext<'a> {
    state: &'a LaunchContextState,
    is_focused: bool,
}

impl<'a> LaunchContext<'a> {
    pub fn new(state: &'a LaunchContextState, is_focused: bool) -> Self {
        Self { state, is_focused }
    }

    /// Calculate minimum height needed
    pub fn min_height() -> u16 {
        12 // 1 border + 10 content + 1 border
    }
}

impl Widget for LaunchContext<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = render_border(area, buf, self.is_focused);
        let chunks = calculate_fields_layout(inner);

        render_common_fields(&chunks, buf, self.state, self.is_focused);

        // Render Launch button
        let launch_focused =
            self.is_focused && self.state.focused_field == super::state::LaunchContextField::Launch;
        let launch_button = LaunchButton::new().focused(launch_focused);
        launch_button.render(chunks[9], buf);
    }
}

// ============================================================================
// LaunchContextWithDevice Widget
// ============================================================================

/// Launch Context with device selection awareness
pub struct LaunchContextWithDevice<'a> {
    state: &'a LaunchContextState,
    is_focused: bool,
    has_device_selected: bool,
    compact: bool,
}

impl<'a> LaunchContextWithDevice<'a> {
    pub fn new(state: &'a LaunchContextState, is_focused: bool, has_device_selected: bool) -> Self {
        Self {
            state,
            is_focused,
            has_device_selected,
            compact: false,
        }
    }

    /// Enable compact mode for narrow terminals
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Widget for LaunchContextWithDevice<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.compact {
            self.render_compact(area, buf);
        } else {
            self.render_full(area, buf);
        }
    }
}

impl LaunchContextWithDevice<'_> {
    /// Render full (horizontal layout) mode
    fn render_full(&self, area: Rect, buf: &mut Buffer) {
        let inner = render_border(area, buf, self.is_focused);
        let chunks = calculate_fields_layout(inner);

        render_common_fields(&chunks, buf, self.state, self.is_focused);

        // Render Launch button with device awareness
        let launch_focused =
            self.is_focused && self.state.focused_field == super::state::LaunchContextField::Launch;
        let launch_button = LaunchButton::new()
            .focused(launch_focused)
            .enabled(self.has_device_selected);
        launch_button.render(chunks[9], buf);
    }

    /// Render compact (vertical layout) mode - tighter spacing, inline mode selector
    fn render_compact(&self, area: Rect, buf: &mut Buffer) {
        // Compact layout: fewer spacers, inline mode
        let chunks = Layout::vertical([
            Constraint::Length(1), // Config field
            Constraint::Length(1), // Mode inline
            Constraint::Length(1), // Flavor field
            Constraint::Length(1), // Dart Defines field
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Launch button
            Constraint::Min(0),    // Rest
        ])
        .split(area);

        // Render config field
        render_config_field(chunks[0], buf, self.state, self.is_focused);

        // Render mode inline (abbreviated)
        self.render_mode_inline(chunks[1], buf);

        // Render flavor field
        render_flavor_field(chunks[2], buf, self.state, self.is_focused);

        // Render dart defines field
        render_dart_defines_field(chunks[3], buf, self.state, self.is_focused);

        // Render launch button
        let launch_focused =
            self.is_focused && self.state.focused_field == super::state::LaunchContextField::Launch;
        let launch_button = LaunchButton::new()
            .focused(launch_focused)
            .enabled(self.has_device_selected);
        launch_button.render(chunks[5], buf);
    }

    /// Render mode selector as inline radio buttons with abbreviated labels
    fn render_mode_inline(&self, area: Rect, buf: &mut Buffer) {
        let mode_focused =
            self.is_focused && self.state.focused_field == super::state::LaunchContextField::Mode;
        let mode_disabled = !self.state.is_mode_editable();

        let style_selected = if mode_disabled {
            Style::default().fg(Color::DarkGray)
        } else if mode_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        };

        let style_unselected = Style::default().fg(Color::DarkGray);

        let style_label = Style::default().fg(Color::Gray);

        use crate::config::FlutterMode;
        let mode_str = vec![
            ratatui::text::Span::styled("  Mode: ", style_label),
            ratatui::text::Span::styled(
                if self.state.mode == FlutterMode::Debug {
                    "(●)Dbg"
                } else {
                    "(○)Dbg"
                },
                if self.state.mode == FlutterMode::Debug {
                    style_selected
                } else {
                    style_unselected
                },
            ),
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                if self.state.mode == FlutterMode::Profile {
                    "(●)Prof"
                } else {
                    "(○)Prof"
                },
                if self.state.mode == FlutterMode::Profile {
                    style_selected
                } else {
                    style_unselected
                },
            ),
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                if self.state.mode == FlutterMode::Release {
                    "(●)Rel"
                } else {
                    "(○)Rel"
                },
                if self.state.mode == FlutterMode::Release {
                    style_selected
                } else {
                    style_unselected
                },
            ),
        ];

        let paragraph = Paragraph::new(Line::from(mode_str));
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod launch_context_tests {
    use super::*;
    use crate::config::{ConfigSource, LaunchConfig, LoadedConfigs, SourcedConfig};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_launch_context_renders() {
        let state = LaunchContextState::new(LoadedConfigs::default());

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Launch Context"));
        assert!(content.contains("Configuration"));
        assert!(content.contains("Mode"));
        assert!(content.contains("Flavor"));
        assert!(content.contains("Dart Defines"));
        assert!(content.contains("LAUNCH"));
    }

    #[test]
    fn test_launch_context_shows_disabled_suffix() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                flavor: Some("prod".to_string()),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.select_config(Some(0));

        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("from config"));
    }

    #[test]
    fn test_launch_context_focused_field() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.focused_field = super::super::state::LaunchContextField::Flavor;

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        // Verify Flavor field is highlighted (visual verification via styling)
        // The test passes if rendering doesn't panic
    }

    #[test]
    fn test_launch_context_with_device_renders() {
        let state = LaunchContextState::new(LoadedConfigs::default());

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContextWithDevice::new(&state, true, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Launch Context"));
        assert!(content.contains("LAUNCH"));
    }

    #[test]
    fn test_launch_context_with_device_no_selection() {
        let state = LaunchContextState::new(LoadedConfigs::default());

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContextWithDevice::new(&state, true, false);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("SELECT DEVICE"));
    }

    #[test]
    fn test_min_height() {
        assert_eq!(LaunchContext::min_height(), 12);
    }

    #[test]
    fn test_unfocused_border_color() {
        let state = LaunchContextState::new(LoadedConfigs::default());

        let backend = TestBackend::new(50, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, false);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        // Test passes if rendering doesn't panic
        // Border color is DarkGray when not focused
    }

    #[test]
    fn test_all_fields_disabled_for_vscode_config() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                flavor: Some("prod".to_string()),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.select_config(Some(0));

        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = LaunchContext::new(&state, true);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        // Test that all fields render correctly even when disabled
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Configuration"));
        assert!(content.contains("Mode"));
        assert!(content.contains("Flavor"));
        assert!(content.contains("Dart Defines"));
    }
}
