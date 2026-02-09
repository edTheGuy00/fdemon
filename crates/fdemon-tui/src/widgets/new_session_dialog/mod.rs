//! NewSessionDialog - Unified session launch dialog
//!
//! Replaces DeviceSelector and StartupDialog with a single dialog featuring:
//! - Target Selector (left pane): Connected/Bootable device tabs
//! - Launch Context (right pane): Config, mode, flavor, dart-defines
//! - Fuzzy search modals for config/flavor selection
//! - Dart defines master-detail modal

mod dart_defines_modal;
// device_groups is a thin re-export to app layer, intentionally shadows glob re-export
#[allow(hidden_glob_reexports)]
mod device_groups;
mod device_list;
pub mod fuzzy_modal; // Public for App layer to access fuzzy_filter function
mod launch_context;
mod state;
mod tab_bar;
pub mod target_selector; // Public for App layer to re-export TargetSelectorState

pub use dart_defines_modal::*;
pub use device_groups::*;
pub use device_list::*;
pub use fuzzy_modal::*;
pub use launch_context::*;
pub use state::*; // Re-exports from App layer
pub use tab_bar::*;
pub use target_selector::*;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

use fdemon_app::ToolAvailability;

use crate::theme::{icons::IconSet, palette, styles};
use crate::widgets::modal_overlay;

// ============================================================================
// Text Truncation Utilities
// ============================================================================

/// Truncates a string to fit within `max_width` characters, adding "..." suffix if truncated.
///
/// # Behavior
/// - Returns the original string if it fits within `max_width`
/// - For `max_width <= 3`, returns dots only (no meaningful text fits)
/// - For longer strings, truncates and adds "..." suffix
///
/// # Character Handling
/// Uses character count, not byte length, to safely handle multi-byte UTF-8
/// characters (emoji, CJK, etc.) without panicking.
///
/// # Examples
/// ```
/// # use fdemon_tui::widgets::new_session_dialog::truncate_with_ellipsis;
/// assert_eq!(truncate_with_ellipsis("Hello", 10), "Hello");
/// assert_eq!(truncate_with_ellipsis("Hello World", 8), "Hello...");
/// assert_eq!(truncate_with_ellipsis("Test", 3), "...");
/// assert_eq!(truncate_with_ellipsis("iPhone 🔥", 9), "iPhone 🔥");
/// ```
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        let truncated: String = text.chars().take(max_width - 3).collect();
        format!("{}...", truncated)
    }
}

/// Truncates a string by removing middle characters, keeping start and end visible.
///
/// Useful for paths or identifiers where both prefix and suffix are meaningful.
/// The result format is: `<start>...<end>`
///
/// # Behavior
/// - Returns the original string if it fits within `max_width`
/// - For `max_width <= 3`, returns dots only (no meaningful text fits)
/// - For longer strings, keeps roughly equal parts from start and end
/// - If odd number of available chars, extra char goes to the start
///
/// # Character Handling
/// Uses character count, not byte length, to safely handle multi-byte UTF-8
/// characters (emoji, CJK, etc.) without panicking.
///
/// # Examples
/// ```
/// # use fdemon_tui::widgets::new_session_dialog::truncate_middle;
/// assert_eq!(truncate_middle("Hello World", 11), "Hello World");
/// assert_eq!(truncate_middle("Hello World", 9), "Hel...rld");
/// assert_eq!(truncate_middle("abcdef", 3), "...");
/// ```
pub fn truncate_middle(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        // Reserve space for "..." (3 chars)
        let available = max_width - 3;
        let half = available / 2;
        let extra = available % 2; // Give extra char to start

        let start: String = text.chars().take(half + extra).collect();
        let end: String = text.chars().skip(char_count - half).collect();
        format!("{}...{}", start, end)
    }
}

/// Minimum terminal width for horizontal (two-pane) layout
const MIN_HORIZONTAL_WIDTH: u16 = 70;

/// Minimum terminal height for horizontal (two-pane) layout
const MIN_HORIZONTAL_HEIGHT: u16 = 20;

/// Minimum terminal width for vertical (stacked) layout
const MIN_VERTICAL_WIDTH: u16 = 40;

/// Minimum terminal height for vertical (stacked) layout
const MIN_VERTICAL_HEIGHT: u16 = 20;

/// Absolute minimum dimensions (below this shows "too small" message)
const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 20;

/// Layout mode for NewSessionDialog based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Two-pane horizontal layout (Target Selector | Launch Context)
    /// Requires width >= 70
    Horizontal,

    /// Stacked vertical layout (Target Selector above Launch Context)
    /// For narrow terminals (width 40-69)
    Vertical,

    /// Terminal too small to render dialog meaningfully
    /// Below 40x20
    TooSmall,
}

/// The main NewSessionDialog widget
pub struct NewSessionDialog<'a> {
    state: &'a NewSessionDialogState,
    tool_availability: &'a ToolAvailability,
    icons: &'a IconSet,
}

impl<'a> NewSessionDialog<'a> {
    /// Minimum terminal width for dialog (updated to match MIN_WIDTH constant)
    pub const MIN_WIDTH: u16 = MIN_WIDTH;

    /// Minimum terminal height for dialog (updated to match MIN_HEIGHT constant)
    pub const MIN_HEIGHT: u16 = MIN_HEIGHT;

    pub fn new(
        state: &'a NewSessionDialogState,
        tool_availability: &'a ToolAvailability,
        icons: &'a IconSet,
    ) -> Self {
        Self {
            state,
            tool_availability,
            icons,
        }
    }

    /// Determine the appropriate layout mode for the given area
    pub fn layout_mode(area: Rect) -> LayoutMode {
        if area.width >= MIN_HORIZONTAL_WIDTH && area.height >= MIN_HORIZONTAL_HEIGHT {
            LayoutMode::Horizontal
        } else if area.width >= MIN_VERTICAL_WIDTH && area.height >= MIN_VERTICAL_HEIGHT {
            LayoutMode::Vertical
        } else {
            LayoutMode::TooSmall
        }
    }

    /// Check if area supports at least vertical layout
    pub fn fits_in_area(area: Rect) -> bool {
        Self::layout_mode(area) != LayoutMode::TooSmall
    }

    /// Calculate centered dialog area (80% width, 70% height)
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

    /// Get footer hints based on current state
    fn footer_hints(&self) -> Vec<(&str, &str)> {
        if self.state.is_fuzzy_modal_open() {
            vec![("↑↓", "Navigate"), ("Enter", "Select"), ("Esc", "Cancel")]
        } else if self.state.is_dart_defines_modal_open() {
            vec![
                ("Tab", "Pane"),
                ("↑↓", "Navigate"),
                ("Enter", "Edit"),
                ("Esc", "Close"),
            ]
        } else {
            vec![
                ("1/2", "Tab"),
                ("Tab", "Pane"),
                ("↑↓", "Navigate"),
                ("Enter", "Select"),
                ("Esc", "Close"),
            ]
        }
    }

    /// Render header area with title, subtitle, and close hint
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        // Row 1: "New Session" (left) + "[Esc] Close" (right)
        let title_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "New Session",
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let close_hint = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(" "),
            Span::styled("Close", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw("  "),
        ]);

        // Split area for title (left) and close hint (right)
        let title_area = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(title_line).render(title_area, buf);
        Paragraph::new(close_hint)
            .alignment(Alignment::Right)
            .render(title_area, buf);

        // Row 2: Subtitle
        let subtitle = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Configure deployment target and runtime flags.",
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
        ]);
        let subtitle_area = Rect::new(area.x, area.y + 1, area.width, 1);
        Paragraph::new(subtitle).render(subtitle_area, buf);
    }

    /// Render a horizontal separator line
    fn render_separator(area: Rect, buf: &mut Buffer) {
        let separator = "─".repeat(area.width as usize);
        buf.set_string(
            area.x,
            area.y,
            &separator,
            Style::default().fg(palette::BORDER_DIM),
        );
    }

    /// Render compact header for vertical layout (2 lines: title + close hint only)
    fn render_header_compact(&self, area: Rect, buf: &mut Buffer) {
        // Row 1: "New Session" (left) + "[Esc]" (right)
        let title_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "New Session",
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let close_hint = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw("  "),
        ]);

        // Split area for title (left) and close hint (right)
        let title_area = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(title_line).render(title_area, buf);
        Paragraph::new(close_hint)
            .alignment(Alignment::Right)
            .render(title_area, buf);

        // For compact mode, skip subtitle to save space
    }

    /// Render main content (two panes)
    fn render_panes(&self, area: Rect, buf: &mut Buffer) {
        // Split into 40% (Target Selector) + 1 col (separator) + 60% (Launch Context)
        let chunks = Layout::horizontal([
            Constraint::Percentage(40), // Target Selector
            Constraint::Length(1),      // Vertical separator
            Constraint::Percentage(60), // Launch Context
        ])
        .split(area);

        // Render Target Selector (left pane)
        let target_focused = self.state.is_target_selector_focused();
        let target_selector = TargetSelector::new(
            &self.state.target_selector,
            self.tool_availability,
            target_focused,
        );
        target_selector.render(chunks[0], buf);

        // Render vertical separator
        Self::render_vertical_separator(chunks[1], buf);

        // Render Launch Context (right pane)
        let launch_focused = self.state.is_launch_context_focused();
        let has_device = self.state.is_ready_to_launch();
        let launch_context = LaunchContextWithDevice::new(
            &self.state.launch_context,
            launch_focused,
            has_device,
            self.icons,
        );
        launch_context.render(chunks[2], buf);
    }

    /// Render a vertical separator line
    fn render_vertical_separator(area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_char('│');
                cell.set_style(Style::default().fg(palette::BORDER_DIM));
            }
        }
    }

    /// Render footer with kbd-style shortcut hints
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        // Fill background with SURFACE color
        let bg_block = Block::default().style(Style::default().bg(palette::SURFACE));
        bg_block.render(area, buf);

        let hints = self.footer_hints();

        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, label)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    "  ·  ",
                    Style::default().fg(palette::BORDER_DIM),
                ));
            }
            spans.push(Span::styled(
                format!("[{}]", key),
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                *label,
                Style::default().fg(palette::TEXT_MUTED),
            ));
        }

        let line = Line::from(spans);
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .render(area, buf);
    }

    /// Render fuzzy modal overlay (bottom 40% with dimmed background)
    fn render_fuzzy_modal_overlay(&self, dialog_area: Rect, buf: &mut Buffer) {
        let modal_state = match &self.state.fuzzy_modal {
            Some(state) => state,
            None => return,
        };

        // Dim the background (main dialog area)
        modal_overlay::dim_background(buf, dialog_area);

        // Check if this is an entry point modal that's loading
        let is_loading = modal_state.modal_type == super::FuzzyModalType::EntryPoint
            && self.state.launch_context.entry_points_loading;

        // Render fuzzy modal widget (it calculates its own area)
        let fuzzy_modal = FuzzyModal::new(modal_state).loading(is_loading);
        fuzzy_modal.render(dialog_area, buf);
    }

    /// Render dart defines modal (full-screen overlay)
    fn render_dart_defines_modal(&self, dialog_area: Rect, buf: &mut Buffer) {
        let modal_state = match &self.state.dart_defines_modal {
            Some(state) => state,
            None => return,
        };

        // Full-screen overlay (replaces main dialog)
        Clear.render(dialog_area, buf);

        // Render dart defines modal widget
        let dart_defines_modal = DartDefinesModal::new(modal_state);
        dart_defines_modal.render(dialog_area, buf);
    }

    /// Render a "terminal too small" message
    fn render_too_small(area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let message = format!(
            "Terminal too small. Need at least {}x{} (current: {}x{})",
            MIN_WIDTH, MIN_HEIGHT, area.width, area.height
        );

        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(palette::STATUS_RED))
            .alignment(Alignment::Center);

        // Center vertically
        let y = area.y + area.height / 2;
        let centered = Rect::new(area.x, y, area.width, 1);
        paragraph.render(centered, buf);
    }

    /// Render horizontal (two-pane) layout
    fn render_horizontal(&self, area: Rect, buf: &mut Buffer) {
        // Step 1: Dim the background (background content already rendered by render/mod.rs)
        modal_overlay::dim_background(buf, area);

        // Step 2: Calculate centered dialog area
        let dialog_area = Self::centered_rect(area);

        // Step 3: Render shadow (1-cell offset right+bottom)
        modal_overlay::render_shadow(buf, dialog_area);

        // Step 4: Clear dialog area (prepare for dialog content)
        modal_overlay::clear_area(buf, dialog_area);

        // Step 5: Main dialog block (no title on border)
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Layout: header + separator + content + separator + footer
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header (title + subtitle + blank line)
            Constraint::Length(1), // Separator
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Separator
            Constraint::Length(1), // Footer
        ])
        .split(inner);

        // Render header
        self.render_header(chunks[0], buf);

        // Render separator
        Self::render_separator(chunks[1], buf);

        // Render main content (two panes)
        self.render_panes(chunks[2], buf);

        // Render separator
        Self::render_separator(chunks[3], buf);

        // Render footer
        self.render_footer(chunks[4], buf);

        // Render modal overlay if any
        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
        } else if self.state.is_fuzzy_modal_open() {
            self.render_fuzzy_modal_overlay(dialog_area, buf);
        }
    }

    /// Render vertical (stacked) layout for narrow terminals
    fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
        // Step 1: Dim the background (background content already rendered by render/mod.rs)
        modal_overlay::dim_background(buf, area);

        // Step 2: Use more of the available space in vertical mode (90% width, 85% height)
        let dialog_area = Self::centered_rect_custom(90, 85, area);

        // Step 3: Render shadow (1-cell offset right+bottom)
        modal_overlay::render_shadow(buf, dialog_area);

        // Step 4: Clear dialog area (prepare for dialog content)
        modal_overlay::clear_area(buf, dialog_area);

        // Step 5: Main dialog block (no title on border)
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Vertical split: header + separator + Target Selector + separator + Launch Context + separator + footer
        let chunks = Layout::vertical([
            Constraint::Length(2),      // Header (compact: title only)
            Constraint::Length(1),      // Separator
            Constraint::Percentage(45), // Target Selector (top, compact mode)
            Constraint::Length(1),      // Separator line
            Constraint::Min(10),        // Launch Context (bottom)
            Constraint::Length(1),      // Separator
            Constraint::Length(1),      // Footer
        ])
        .split(inner);

        // Render compact header
        self.render_header_compact(chunks[0], buf);

        // Render separator
        Self::render_separator(chunks[1], buf);

        // Render Target Selector (top, compact mode)
        let target_focused = self.state.is_target_selector_focused();
        let target_selector = TargetSelector::new(
            &self.state.target_selector,
            self.tool_availability,
            target_focused,
        )
        .compact(true);
        target_selector.render(chunks[2], buf);

        // Render separator line
        Self::render_separator(chunks[3], buf);

        // Render Launch Context (bottom, compact mode)
        let launch_focused = self.state.is_launch_context_focused();
        let has_device = self.state.is_ready_to_launch();
        let launch_context = LaunchContextWithDevice::new(
            &self.state.launch_context,
            launch_focused,
            has_device,
            self.icons,
        )
        .compact(true);
        launch_context.render(chunks[4], buf);

        // Render separator
        Self::render_separator(chunks[5], buf);

        // Render compact footer
        self.render_footer_compact(chunks[6], buf);

        // Render modal overlay if any
        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
        } else if self.state.is_fuzzy_modal_open() {
            self.render_fuzzy_modal_overlay(dialog_area, buf);
        }
    }

    /// Calculate centered dialog area with custom percentages
    fn centered_rect_custom(width_percent: u16, height_percent: u16, area: Rect) -> Rect {
        let v_margin = (100 - height_percent) / 2;
        let popup_layout = Layout::vertical([
            Constraint::Percentage(v_margin),
            Constraint::Percentage(height_percent),
            Constraint::Percentage(v_margin),
        ])
        .split(area);

        let h_margin = (100 - width_percent) / 2;
        Layout::horizontal([
            Constraint::Percentage(h_margin),
            Constraint::Percentage(width_percent),
            Constraint::Percentage(h_margin),
        ])
        .split(popup_layout[1])[1]
    }

    /// Render footer with abbreviated keybindings (for vertical layout)
    fn render_footer_compact(&self, area: Rect, buf: &mut Buffer) {
        // Fill background with SURFACE color
        let bg_block = Block::default().style(Style::default().bg(palette::SURFACE));
        bg_block.render(area, buf);

        // Shorter keybinding hints for narrow terminals
        let hints = if self.state.is_fuzzy_modal_open() {
            vec![("↑↓", "Nav"), ("Enter", "Sel"), ("Esc", "Close")]
        } else if self.state.is_dart_defines_modal_open() {
            vec![("Tab", "Pane"), ("↑↓", "Nav"), ("Esc", "Close")]
        } else {
            vec![
                ("1/2", "Tab"),
                ("Tab", "Pane"),
                ("↑↓", "Nav"),
                ("Esc", "Close"),
            ]
        };

        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, label)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " · ",
                    Style::default().fg(palette::BORDER_DIM),
                ));
            }
            spans.push(Span::styled(
                format!("[{}]", key),
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                *label,
                Style::default().fg(palette::TEXT_MUTED),
            ));
        }

        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .render(area, buf);
    }
}

impl Widget for NewSessionDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match Self::layout_mode(area) {
            LayoutMode::TooSmall => {
                Self::render_too_small(area, buf);
            }
            LayoutMode::Horizontal => {
                self.render_horizontal(area, buf);
            }
            LayoutMode::Vertical => {
                self.render_vertical(area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::config::{IconMode, LoadedConfigs};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_dialog_renders() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("New Session"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = NewSessionDialog::centered_rect(area);

        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.width < area.width);
        assert!(centered.height < area.height);
    }

    #[test]
    fn test_fits_in_area() {
        // Should fit with horizontal layout
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 100, 40)));
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 80, 24)));
        // Should fit with vertical layout
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 60, 20)));
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 40, 20)));
        // Should not fit (too small)
        assert!(!NewSessionDialog::fits_in_area(Rect::new(0, 0, 30, 15)));
    }

    #[test]
    fn test_layout_mode_horizontal() {
        let area = Rect::new(0, 0, 100, 40);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
    }

    #[test]
    fn test_layout_mode_vertical() {
        let area = Rect::new(0, 0, 50, 30);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
    }

    #[test]
    fn test_layout_mode_too_small() {
        let area = Rect::new(0, 0, 30, 15);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
    }

    #[test]
    fn test_layout_mode_boundary_horizontal() {
        let area = Rect::new(0, 0, 70, 20);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
    }

    #[test]
    fn test_layout_mode_boundary_vertical() {
        let area = Rect::new(0, 0, 69, 20);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
    }

    #[test]
    fn test_layout_mode_boundary_too_small_width() {
        let area = Rect::new(0, 0, 39, 20);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
    }

    #[test]
    fn test_layout_mode_boundary_too_small_height() {
        let area = Rect::new(0, 0, 70, 19);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
    }

    #[test]
    fn test_too_small_message() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("too small"));
    }

    #[test]
    fn test_dialog_with_fuzzy_modal() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_flavor_modal(vec!["dev".to_string(), "prod".to_string()]);

        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Select Flavor"));
        assert!(content.contains("dev"));
        assert!(content.contains("prod"));
    }

    #[test]
    fn test_dialog_with_dart_defines_modal() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_dart_defines_modal();

        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Manage Dart Defines"));
    }

    #[test]
    fn test_truncate_with_ellipsis_no_truncation() {
        let result = truncate_with_ellipsis("short", 10);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_with_ellipsis_exact_fit() {
        let result = truncate_with_ellipsis("exactly10!", 10);
        assert_eq!(result, "exactly10!");
    }

    #[test]
    fn test_truncate_with_ellipsis_truncates() {
        let result = truncate_with_ellipsis("this is a very long text", 10);
        assert_eq!(result, "this is...");
    }

    #[test]
    fn test_truncate_with_ellipsis_very_short() {
        let result = truncate_with_ellipsis("text", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_with_ellipsis_minimal() {
        let result = truncate_with_ellipsis("text", 2);
        assert_eq!(result, "..");
    }

    #[test]
    fn test_truncate_middle_no_truncation() {
        let result = truncate_middle("short", 10);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_middle_truncates() {
        let result = truncate_middle("this_is_a_very_long_device_name", 15);
        assert_eq!(result, "this_i...e_name");
    }

    #[test]
    fn test_truncate_middle_very_short() {
        let result = truncate_middle("longtext", 5);
        // With max_width=5: available=2, half=1, start="l", end="t"
        assert_eq!(result, "l...t");
    }

    #[test]
    fn test_truncate_middle_minimal() {
        let result = truncate_middle("text", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_with_ellipsis_utf8() {
        // Emoji (4 bytes per char) - "iPhone 🔥" is 8 chars, fits at width 8
        assert_eq!(truncate_with_ellipsis("iPhone 🔥", 7), "iPho...");
        assert_eq!(truncate_with_ellipsis("iPhone 🔥", 8), "iPhone 🔥");
        assert_eq!(truncate_with_ellipsis("iPhone 🔥", 9), "iPhone 🔥");

        // Multi-byte chars (3 bytes per char) - "日本語テスト" is 6 chars
        assert_eq!(truncate_with_ellipsis("日本語テスト", 6), "日本語テスト");
        assert_eq!(truncate_with_ellipsis("日本語テスト", 5), "日本...");

        // Mixed ASCII and emoji - "Test 🚀 Device" is 13 chars
        assert_eq!(truncate_with_ellipsis("Test 🚀 Device", 10), "Test 🚀 ...");
        assert_eq!(truncate_with_ellipsis("Test 🚀 Device", 9), "Test 🚀...");
    }

    #[test]
    fn test_truncate_middle_utf8() {
        // Emoji in name - "🔥Hot🔥Device🔥" is 12 chars
        assert_eq!(truncate_middle("🔥Hot🔥Device🔥", 10), "🔥Hot...ce🔥");
        assert_eq!(truncate_middle("🔥Hot🔥Device🔥", 8), "🔥Ho...e🔥");

        // Multi-byte chars - "日本語デバイス" is 7 chars
        assert_eq!(truncate_middle("日本語デバイス", 7), "日本語デバイス");
        assert_eq!(truncate_middle("日本語デバイス", 6), "日本...ス");
    }
}
