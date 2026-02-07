//! Test utilities for TUI rendering verification
//!
//! Provides helpers for testing widgets and full-screen rendering
//! using ratatui's TestBackend. These tests are fast (~1ms) and
//! 100% reliable unlike PTY-based tests.
//!
//! # Example
//!
//! ```rust
//! use crate::tui::test_utils::TestTerminal;
//!
//! #[test]
//! fn test_header_renders_project_name() {
//!     let mut term = TestTerminal::new();
//!     let header = MainHeader::new(Some("my_project"));
//!
//!     term.render_widget(&header, term.area());
//!
//!     assert!(term.buffer_contains("my_project"));
//! }
//! ```

use crate::daemon::Device;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::Frame;
use ratatui::Terminal;

/// Standard test terminal size (matches common terminal dimensions)
pub const TEST_WIDTH: u16 = 80;
pub const TEST_HEIGHT: u16 = 24;

/// Compact terminal for testing responsive layouts
pub const COMPACT_WIDTH: u16 = 40;
pub const COMPACT_HEIGHT: u16 = 12;

/// Test utility wrapper around ratatui's TestBackend terminal.
///
/// Provides ergonomic methods for widget testing while maintaining
/// access to the underlying terminal for advanced use cases.
///
/// # Usage
///
/// For simple widget testing, use the wrapper methods:
/// ```ignore
/// let mut term = TestTerminal::new();
/// term.render_widget(my_widget, term.area());
/// assert!(term.buffer_contains("expected text"));
/// ```
///
/// For full-frame rendering (like `tui::view`), use `draw_with()`:
/// ```ignore
/// let mut term = TestTerminal::new();
/// term.draw_with(|frame| view(frame, &state))?;
/// ```
pub struct TestTerminal {
    /// The underlying ratatui terminal with TestBackend.
    ///
    /// This field is public to allow direct access for advanced terminal
    /// operations not covered by wrapper methods.
    ///
    /// Prefer using wrapper methods (`render_widget`, `draw_with`,
    /// `buffer_contains`, etc.) for most test scenarios.
    pub terminal: Terminal<TestBackend>,
}

impl TestTerminal {
    /// Create a new test terminal with standard dimensions (80x24)
    pub fn new() -> Self {
        Self::with_size(TEST_WIDTH, TEST_HEIGHT)
    }

    /// Create a new test terminal with compact dimensions (40x12)
    pub fn compact() -> Self {
        Self::with_size(COMPACT_WIDTH, COMPACT_HEIGHT)
    }

    /// Create a new test terminal with custom dimensions
    pub fn with_size(width: u16, height: u16) -> Self {
        let backend = TestBackend::new(width, height);
        let terminal = Terminal::new(backend).expect("Failed to create test terminal");
        Self { terminal }
    }

    /// Get the full terminal area
    pub fn area(&self) -> Rect {
        let size = self.terminal.size().expect("Failed to get terminal size");
        Rect::new(0, 0, size.width, size.height)
    }

    /// Render a widget to the terminal
    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        self.terminal
            .draw(|frame| frame.render_widget(widget, area))
            .expect("Failed to render widget");
    }

    /// Render a stateful widget to the terminal
    pub fn render_stateful_widget<W, S>(&mut self, widget: W, area: Rect, state: &mut S)
    where
        W: ratatui::widgets::StatefulWidget<State = S>,
    {
        self.terminal
            .draw(|frame| frame.render_stateful_widget(widget, area, state))
            .expect("Failed to render stateful widget");
    }

    /// Draws a frame using a custom rendering function.
    ///
    /// This is useful for testing full-screen rendering (like `tui::view`)
    /// rather than individual widgets.
    ///
    /// # Arguments
    /// * `f` - A closure that receives a mutable Frame reference
    ///
    /// # Example
    /// ```ignore
    /// let mut term = TestTerminal::new();
    /// term.draw_with(|frame| view(frame, &state));
    /// assert!(term.buffer_contains("expected content"));
    /// ```
    pub fn draw_with<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f).expect("Failed to draw frame");
    }

    /// Get the underlying buffer for assertions
    pub fn buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    /// Check if the buffer contains a string anywhere
    pub fn buffer_contains(&self, text: &str) -> bool {
        let buffer = self.buffer();
        let content = buffer_to_string(buffer);
        content.contains(text)
    }

    /// Check if a specific line contains text
    pub fn line_contains(&self, line: u16, text: &str) -> bool {
        let buffer = self.buffer();
        let line_content = get_line_content(buffer, line);
        line_content.contains(text)
    }

    /// Get the content of a specific cell
    pub fn cell_at(&self, x: u16, y: u16) -> Option<&str> {
        let buffer = self.buffer();
        if x < buffer.area.width && y < buffer.area.height {
            Some(buffer[(x, y)].symbol())
        } else {
            None
        }
    }

    /// Get all content as a string (for debugging)
    pub fn content(&self) -> String {
        buffer_to_string(self.buffer())
    }

    /// Clear the terminal for a fresh render
    pub fn clear(&mut self) {
        self.terminal.clear().expect("Failed to clear terminal");
    }
}

impl Default for TestTerminal {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert buffer to string representation
fn buffer_to_string(buffer: &Buffer) -> String {
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer[(x, y)].symbol());
        }
        result.push('\n');
    }
    result
}

/// Get content of a specific line
fn get_line_content(buffer: &Buffer, line: u16) -> String {
    let mut result = String::new();
    if line < buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer[(x, line)].symbol());
        }
    }
    result
}

/// Create a minimal AppState for testing
pub fn create_test_state() -> crate::app::state::AppState {
    use crate::app::state::AppState;

    AppState::new()
}

/// Create AppState with custom project name
pub fn create_test_state_with_name(name: &str) -> crate::app::state::AppState {
    let mut state = create_test_state();
    state.project_name = Some(name.to_string());
    state
}

// Re-export device test utilities from daemon layer
pub use crate::daemon::test_utils::{test_device, test_device_full, test_device_with_platform};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_creation() {
        let term = TestTerminal::new();
        assert_eq!(term.area().width, TEST_WIDTH);
        assert_eq!(term.area().height, TEST_HEIGHT);
    }

    #[test]
    fn test_compact_terminal() {
        let term = TestTerminal::compact();
        assert_eq!(term.area().width, COMPACT_WIDTH);
        assert_eq!(term.area().height, COMPACT_HEIGHT);
    }

    #[test]
    fn test_custom_size() {
        let term = TestTerminal::with_size(100, 50);
        assert_eq!(term.area().width, 100);
        assert_eq!(term.area().height, 50);
    }

    #[test]
    fn test_buffer_to_string() {
        let backend = TestBackend::new(5, 2);
        let buffer = backend.buffer();
        let content = buffer_to_string(buffer);
        // Default buffer is filled with spaces
        assert_eq!(content.lines().count(), 2);
    }

    #[test]
    fn test_buffer_contains() {
        use ratatui::widgets::Paragraph;

        let mut term = TestTerminal::with_size(20, 5);
        let paragraph = Paragraph::new("Hello World");
        term.render_widget(paragraph, term.area());

        assert!(term.buffer_contains("Hello World"));
        assert!(!term.buffer_contains("Goodbye"));
    }

    #[test]
    fn test_line_contains() {
        use ratatui::widgets::Paragraph;

        let mut term = TestTerminal::with_size(20, 5);
        let paragraph = Paragraph::new("Hello\nWorld");
        term.render_widget(paragraph, term.area());

        assert!(term.line_contains(0, "Hello"));
        assert!(term.line_contains(1, "World"));
        assert!(!term.line_contains(0, "World"));
    }

    #[test]
    fn test_cell_at() {
        use ratatui::widgets::Paragraph;

        let mut term = TestTerminal::with_size(20, 5);
        let paragraph = Paragraph::new("AB");
        term.render_widget(paragraph, term.area());

        assert_eq!(term.cell_at(0, 0), Some("A"));
        assert_eq!(term.cell_at(1, 0), Some("B"));
        assert_eq!(term.cell_at(2, 0), Some(" "));
    }

    #[test]
    fn test_cell_at_out_of_bounds() {
        let term = TestTerminal::with_size(10, 5);
        assert_eq!(term.cell_at(100, 100), None);
    }

    #[test]
    fn test_content_full_dump() {
        let term = TestTerminal::with_size(5, 2);
        let content = term.content();
        // Should have 2 lines of 5 spaces each (with newlines)
        assert!(content.contains('\n'));
        assert_eq!(content.lines().count(), 2);
    }

    #[test]
    fn test_clear() {
        use ratatui::widgets::Paragraph;

        let mut term = TestTerminal::with_size(20, 5);
        let paragraph = Paragraph::new("Hello");
        term.render_widget(paragraph, term.area());

        assert!(term.buffer_contains("Hello"));

        term.clear();
        // After clear, the buffer should still exist but be empty
        let content = term.content();
        assert!(!content.contains("Hello"));
    }

    #[test]
    fn test_default_terminal() {
        let term = TestTerminal::default();
        assert_eq!(term.area().width, TEST_WIDTH);
        assert_eq!(term.area().height, TEST_HEIGHT);
    }

    #[test]
    fn test_create_test_state() {
        let state = create_test_state();
        // Should create a valid AppState
        assert_eq!(state.ui_mode, crate::app::state::UiMode::Normal);
    }

    #[test]
    fn test_create_test_state_with_name() {
        let state = create_test_state_with_name("test_project");
        assert_eq!(state.project_name, Some("test_project".to_string()));
    }
}
