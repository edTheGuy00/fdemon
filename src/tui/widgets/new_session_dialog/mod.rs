//! NewSessionDialog - Unified session launch dialog
//!
//! Replaces DeviceSelector and StartupDialog with a single dialog featuring:
//! - Target Selector (left pane): Connected/Bootable device tabs
//! - Launch Context (right pane): Config, mode, flavor, dart-defines
//! - Fuzzy search modals for config/flavor selection
//! - Dart defines master-detail modal

mod dart_defines_modal;
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
    style::{Color, Style},
    symbols,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::daemon::ToolAvailability;

/// Footer text shown when no modal is open
const FOOTER_MAIN: &str = "[1/2] Tab  [Tab] Pane  [↑↓] Navigate  [Enter] Select  [Esc] Close";

/// Footer text shown when fuzzy modal is open
const FOOTER_FUZZY_MODAL: &str = "[↑↓] Navigate  [Enter] Select  [Esc] Cancel  Type to filter";

/// Footer text shown when dart defines modal is open
const FOOTER_DART_DEFINES: &str = "[Tab] Pane  [↑↓] Navigate  [Enter] Edit  [Esc] Save & Close";

/// The main NewSessionDialog widget
pub struct NewSessionDialog<'a> {
    state: &'a NewSessionDialogState,
    tool_availability: &'a ToolAvailability,
}

impl<'a> NewSessionDialog<'a> {
    /// Minimum terminal width for dialog
    pub const MIN_WIDTH: u16 = 80;

    /// Minimum terminal height for dialog
    pub const MIN_HEIGHT: u16 = 24;

    pub fn new(state: &'a NewSessionDialogState, tool_availability: &'a ToolAvailability) -> Self {
        Self {
            state,
            tool_availability,
        }
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

    /// Get footer text based on current state
    fn footer_text(&self) -> &'static str {
        if self.state.is_fuzzy_modal_open() {
            FOOTER_FUZZY_MODAL
        } else if self.state.is_dart_defines_modal_open() {
            FOOTER_DART_DEFINES
        } else {
            FOOTER_MAIN
        }
    }

    /// Render main content (two panes)
    fn render_panes(&self, area: Rect, buf: &mut Buffer) {
        // Split into two equal panes
        let chunks = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Render Target Selector (left pane)
        let target_focused = self.state.is_target_selector_focused();
        let target_selector = TargetSelector::new(
            &self.state.target_selector,
            self.tool_availability,
            target_focused,
        );
        target_selector.render(chunks[0], buf);

        // Render Launch Context (right pane)
        let launch_focused = self.state.is_launch_context_focused();
        let has_device = self.state.is_ready_to_launch();
        let launch_context =
            LaunchContextWithDevice::new(&self.state.launch_context, launch_focused, has_device);
        launch_context.render(chunks[1], buf);
    }

    /// Render footer
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let text = Paragraph::new(self.footer_text())
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        text.render(area, buf);
    }

    /// Render fuzzy modal overlay (bottom 40% with dimmed background)
    fn render_fuzzy_modal_overlay(&self, dialog_area: Rect, buf: &mut Buffer) {
        let modal_state = match &self.state.fuzzy_modal {
            Some(state) => state,
            None => return,
        };

        // Dim the background (main dialog area)
        fuzzy_modal::render_dim_overlay(dialog_area, buf);

        // Render fuzzy modal widget (it calculates its own area)
        let fuzzy_modal = FuzzyModal::new(modal_state);
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

    /// Check if terminal is large enough
    pub fn fits_in_area(area: Rect) -> bool {
        area.width >= Self::MIN_WIDTH && area.height >= Self::MIN_HEIGHT
    }

    /// Render a "terminal too small" message
    fn render_too_small(area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let message = format!(
            "Terminal too small. Need at least {}x{} (current: {}x{})",
            Self::MIN_WIDTH,
            Self::MIN_HEIGHT,
            area.width,
            area.height
        );

        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);

        // Center vertically
        let y = area.y + area.height / 2;
        let centered = Rect::new(area.x, y, area.width, 1);
        paragraph.render(centered, buf);
    }
}

impl Widget for NewSessionDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !Self::fits_in_area(area) {
            Self::render_too_small(area, buf);
            return;
        }

        let dialog_area = Self::centered_rect(area);

        // Clear background
        Clear.render(dialog_area, buf);

        // Main dialog block
        let block = Block::default()
            .title(" New Session ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Layout: content + footer
        let chunks = Layout::vertical([
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Footer
        ])
        .split(inner);

        // Render main content (two panes)
        self.render_panes(chunks[0], buf);

        // Render footer
        self.render_footer(chunks[1], buf);

        // Render modal overlay if any
        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
        } else if self.state.is_fuzzy_modal_open() {
            self.render_fuzzy_modal_overlay(dialog_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoadedConfigs;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_dialog_renders() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability);
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
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 100, 40)));
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 80, 24)));
        assert!(!NewSessionDialog::fits_in_area(Rect::new(0, 0, 60, 20)));
    }

    #[test]
    fn test_too_small_message() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability);
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

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability);
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

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Manage Dart Defines"));
    }
}
