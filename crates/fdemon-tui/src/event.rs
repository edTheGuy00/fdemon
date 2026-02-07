//! Terminal event polling

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use fdemon_app::message::Message;
use fdemon_app::InputKey;
use fdemon_core::prelude::*;
use std::time::Duration;

/// Convert crossterm KeyEvent to InputKey
pub fn key_event_to_input(key: crossterm::event::KeyEvent) -> Option<InputKey> {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputKey::CharCtrl(c))
        }
        KeyCode::Char(c) => Some(InputKey::Char(c)),
        KeyCode::Enter => Some(InputKey::Enter),
        KeyCode::Esc => Some(InputKey::Esc),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => Some(InputKey::BackTab),
        KeyCode::Tab => Some(InputKey::Tab),
        KeyCode::BackTab => Some(InputKey::BackTab),
        KeyCode::Backspace => Some(InputKey::Backspace),
        KeyCode::Delete => Some(InputKey::Delete),
        KeyCode::Up => Some(InputKey::Up),
        KeyCode::Down => Some(InputKey::Down),
        KeyCode::Left => Some(InputKey::Left),
        KeyCode::Right => Some(InputKey::Right),
        KeyCode::Home => Some(InputKey::Home),
        KeyCode::End => Some(InputKey::End),
        KeyCode::PageUp => Some(InputKey::PageUp),
        KeyCode::PageDown => Some(InputKey::PageDown),
        KeyCode::F(n) => Some(InputKey::F(n)),
        _ => None, // Unsupported keys ignored
    }
}

/// Poll for terminal events with timeout
pub fn poll() -> Result<Option<Message>> {
    // Poll with 50ms timeout (20 FPS)
    if event::poll(Duration::from_millis(50))? {
        let event = event::read()?;

        match event {
            Event::Key(key) => {
                if key.kind == event::KeyEventKind::Press {
                    if let Some(input_key) = key_event_to_input(key) {
                        Ok(Some(Message::Key(input_key)))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    } else {
        // Generate tick on timeout for animations
        Ok(Some(Message::Tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn test_char_conversion() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_event_to_input(key), Some(InputKey::Char('a')));
    }

    #[test]
    fn test_char_with_ctrl_conversion() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_input(key), Some(InputKey::CharCtrl('c')));
    }

    #[test]
    fn test_navigation_keys() {
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Some(InputKey::Up)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(InputKey::Down)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Some(InputKey::Left)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            Some(InputKey::Right)
        );
    }

    #[test]
    fn test_action_keys() {
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(InputKey::Enter)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(InputKey::Esc)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Some(InputKey::Tab)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(InputKey::Backspace)
        );
    }

    #[test]
    fn test_backtab_with_shift() {
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
        assert_eq!(key_event_to_input(key), Some(InputKey::BackTab));
    }

    #[test]
    fn test_backtab_keycode() {
        let key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(key_event_to_input(key), Some(InputKey::BackTab));
    }

    #[test]
    fn test_function_keys() {
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            Some(InputKey::F(1))
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE)),
            Some(InputKey::F(12))
        );
    }

    #[test]
    fn test_page_keys() {
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            Some(InputKey::PageUp)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            Some(InputKey::PageDown)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
            Some(InputKey::Home)
        );
        assert_eq!(
            key_event_to_input(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            Some(InputKey::End)
        );
    }

    #[test]
    fn test_uppercase_letters() {
        let key = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT);
        assert_eq!(key_event_to_input(key), Some(InputKey::Char('R')));
    }

    #[test]
    fn test_special_chars_with_shift() {
        let key = KeyEvent::new(KeyCode::Char('!'), KeyModifiers::SHIFT);
        assert_eq!(key_event_to_input(key), Some(InputKey::Char('!')));
    }

    #[test]
    fn test_unsupported_key_returns_none() {
        // Example: Insert key, which is not in InputKey enum
        let key = KeyEvent::new(KeyCode::Insert, KeyModifiers::NONE);
        assert_eq!(key_event_to_input(key), None);
    }
}
