//! Abstract input key event, independent of terminal library.
//!
//! This module defines the `InputKey` enum which abstracts keyboard input
//! from the underlying terminal library (crossterm). This allows fdemon-app
//! to remain independent of terminal-specific types, enabling non-TUI
//! consumers (future MCP server, GUI frontend, etc.) to use the engine
//! without depending on crossterm.

/// Abstract input key event, independent of terminal library.
/// Converted from crossterm::event::KeyEvent at the TUI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKey {
    // Character keys
    /// Regular character key (a-z, 0-9, symbols)
    Char(char),
    /// Character with Ctrl modifier (Ctrl+a, Ctrl+c, etc.)
    CharCtrl(char),

    // Navigation
    /// Up arrow key
    Up,
    /// Down arrow key
    Down,
    /// Left arrow key
    Left,
    /// Right arrow key
    Right,
    /// Home key
    Home,
    /// End key
    End,
    /// Page Up key
    PageUp,
    /// Page Down key
    PageDown,

    // Action keys
    /// Enter/Return key
    Enter,
    /// Escape key
    Esc,
    /// Tab key
    Tab,
    /// Shift+Tab (BackTab)
    BackTab,
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,

    // Function keys
    /// Function key (F1-F12)
    F(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_key_equality() {
        assert_eq!(InputKey::Char('a'), InputKey::Char('a'));
        assert_ne!(InputKey::Char('a'), InputKey::Char('b'));
        assert_eq!(InputKey::CharCtrl('c'), InputKey::CharCtrl('c'));
        assert_ne!(InputKey::CharCtrl('c'), InputKey::Char('c'));
    }

    #[test]
    fn test_input_key_clone() {
        let key = InputKey::Char('x');
        let cloned = key.clone();
        assert_eq!(key, cloned);
    }

    #[test]
    fn test_input_key_debug() {
        let key = InputKey::CharCtrl('c');
        let debug_str = format!("{:?}", key);
        assert!(debug_str.contains("CharCtrl"));
    }
}
