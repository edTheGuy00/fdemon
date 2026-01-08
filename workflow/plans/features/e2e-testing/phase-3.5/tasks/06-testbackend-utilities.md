## Task: Create TestBackend Test Utilities

**Objective**: Create a test utilities module that provides easy setup for TestBackend-based widget and rendering tests.

**Depends on**: 05-validate-pty-tests

### Scope

- `src/tui/test_utils.rs`: **NEW** - Test utilities module
- `src/tui/mod.rs`: Export test utilities

### Details

#### 1. Create Test Utilities Module

Create `src/tui/test_utils.rs`:

```rust
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
//!     let mut term = TestTerminal::new(80, 24);
//!     let header = MainHeader::new(Some("my_project"));
//!
//!     term.render_widget(&header, term.area());
//!
//!     assert!(term.buffer_contains("my_project"));
//! }
//! ```

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use ratatui::widgets::Widget;

/// Standard test terminal size (matches common terminal dimensions)
pub const TEST_WIDTH: u16 = 80;
pub const TEST_HEIGHT: u16 = 24;

/// Compact terminal for testing responsive layouts
pub const COMPACT_WIDTH: u16 = 40;
pub const COMPACT_HEIGHT: u16 = 12;

/// A test terminal wrapper for easy widget testing
pub struct TestTerminal {
    terminal: Terminal<TestBackend>,
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
        self.terminal.size().expect("Failed to get terminal size")
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
            Some(buffer.get(x, y).symbol())
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
            result.push_str(buffer.get(x, y).symbol());
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
            result.push_str(buffer.get(x, line).symbol());
        }
    }
    result
}

/// Create a minimal AppState for testing
#[cfg(test)]
pub fn create_test_state() -> crate::app::state::AppState {
    use crate::app::state::AppState;
    use std::path::PathBuf;

    AppState::new(PathBuf::from("/test/project"))
}

/// Create AppState with custom project name
#[cfg(test)]
pub fn create_test_state_with_name(name: &str) -> crate::app::state::AppState {
    let mut state = create_test_state();
    state.project_name = Some(name.to_string());
    state
}

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
}
```

#### 2. Export from mod.rs

Add to `src/tui/mod.rs`:

```rust
#[cfg(test)]
pub mod test_utils;
```

#### 3. Re-export for Easy Access

Consider adding to `src/lib.rs` for test visibility:

```rust
#[cfg(test)]
pub use tui::test_utils;
```

### Acceptance Criteria

1. `TestTerminal` struct provides easy widget testing
2. Helper methods for common assertions:
   - `buffer_contains(text)` - Check if text appears anywhere
   - `line_contains(line, text)` - Check specific line
   - `cell_at(x, y)` - Get specific cell content
3. Standard and compact terminal sizes available
4. `create_test_state()` helper for AppState creation
5. All utility tests pass

### Testing

```bash
# Run test utility tests
cargo test tui::test_utils --lib

# Verify module compiles
cargo check --lib
```

### Notes

- These utilities are `#[cfg(test)]` only - not in release builds
- Keep utilities simple - complexity belongs in widget tests
- Consider adding insta integration in Task 11

---

## Completion Summary

**Status:** Not Started
