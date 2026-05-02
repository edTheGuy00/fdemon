//! Terminal event polling

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton as CtMouseButton,
    MouseEvent, MouseEventKind,
};
use fdemon_app::message::Message;
use fdemon_app::{InputKey, KeyModSet, MouseButton, MouseInput, ScrollDir};
use fdemon_core::prelude::*;
use std::time::Duration;

/// Convert crossterm `KeyModifiers` (a bitflag) to our abstract `KeyModSet`.
///
/// Only Shift / Ctrl / Alt are propagated; other modifiers (Hyper, Meta,
/// Super) are dropped. Mouse handlers in Phase 2+ only consult Shift/Ctrl/Alt.
pub(crate) fn key_modifiers_to_set(m: KeyModifiers) -> KeyModSet {
    KeyModSet {
        shift: m.contains(KeyModifiers::SHIFT),
        ctrl: m.contains(KeyModifiers::CONTROL),
        alt: m.contains(KeyModifiers::ALT),
    }
}

fn ct_button_to_abstract(b: CtMouseButton) -> MouseButton {
    match b {
        CtMouseButton::Left => MouseButton::Left,
        CtMouseButton::Right => MouseButton::Right,
        CtMouseButton::Middle => MouseButton::Middle,
    }
}

/// Convert crossterm `MouseEvent` to abstract [`MouseInput`].
///
/// Returns `None` for `Moved` events (no consumer; high volume) and any
/// future `MouseEventKind` variants we have not explicitly mapped.
pub(crate) fn mouse_event_to_input(ev: MouseEvent) -> Option<MouseInput> {
    let modifiers = key_modifiers_to_set(ev.modifiers);
    let x = ev.column;
    let y = ev.row;

    match ev.kind {
        MouseEventKind::Down(button) => Some(MouseInput::Click {
            x,
            y,
            button: ct_button_to_abstract(button),
            modifiers,
        }),
        MouseEventKind::Up(button) => Some(MouseInput::Release {
            x,
            y,
            button: ct_button_to_abstract(button),
            modifiers,
        }),
        MouseEventKind::Drag(button) => Some(MouseInput::Drag {
            x,
            y,
            button: ct_button_to_abstract(button),
            modifiers,
        }),
        MouseEventKind::ScrollUp => Some(MouseInput::Scroll {
            x,
            y,
            direction: ScrollDir::Up,
            modifiers,
        }),
        MouseEventKind::ScrollDown => Some(MouseInput::Scroll {
            x,
            y,
            direction: ScrollDir::Down,
            modifiers,
        }),
        MouseEventKind::ScrollLeft => Some(MouseInput::Scroll {
            x,
            y,
            direction: ScrollDir::Left,
            modifiers,
        }),
        MouseEventKind::ScrollRight => Some(MouseInput::Scroll {
            x,
            y,
            direction: ScrollDir::Right,
            modifiers,
        }),
        MouseEventKind::Moved => None,
    }
}

/// Convert crossterm KeyEvent to InputKey
pub fn key_event_to_input(key: KeyEvent) -> Option<InputKey> {
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
                if key.kind == KeyEventKind::Press {
                    if let Some(input_key) = key_event_to_input(key) {
                        Ok(Some(Message::Key(input_key)))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            Event::Mouse(mouse) => {
                if let Some(input) = mouse_event_to_input(mouse) {
                    Ok(Some(Message::Mouse(input)))
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

    // --- Mouse conversion tests ---

    #[test]
    fn test_mouse_down_left_converts_to_click() {
        let ev = MouseEvent {
            kind: MouseEventKind::Down(CtMouseButton::Left),
            column: 5,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        let input = mouse_event_to_input(ev).expect("must convert");
        assert_eq!(
            input,
            MouseInput::Click {
                x: 5,
                y: 10,
                button: MouseButton::Left,
                modifiers: KeyModSet::NONE,
            }
        );
    }

    #[test]
    fn test_mouse_up_right_converts_to_release() {
        let ev = MouseEvent {
            kind: MouseEventKind::Up(CtMouseButton::Right),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            mouse_event_to_input(ev),
            Some(MouseInput::Release {
                button: MouseButton::Right,
                ..
            })
        ));
    }

    #[test]
    fn test_mouse_drag_middle_converts() {
        let ev = MouseEvent {
            kind: MouseEventKind::Drag(CtMouseButton::Middle),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            mouse_event_to_input(ev),
            Some(MouseInput::Drag {
                button: MouseButton::Middle,
                ..
            })
        ));
    }

    #[test]
    fn test_scroll_up_converts() {
        let ev = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            mouse_event_to_input(ev),
            Some(MouseInput::Scroll {
                direction: ScrollDir::Up,
                ..
            })
        ));
    }

    #[test]
    fn test_scroll_down_converts() {
        let ev = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            mouse_event_to_input(ev),
            Some(MouseInput::Scroll {
                direction: ScrollDir::Down,
                ..
            })
        ));
    }

    #[test]
    fn test_scroll_left_right_converts() {
        let left = MouseEvent {
            kind: MouseEventKind::ScrollLeft,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let right = MouseEvent {
            kind: MouseEventKind::ScrollRight,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            mouse_event_to_input(left),
            Some(MouseInput::Scroll {
                direction: ScrollDir::Left,
                ..
            })
        ));
        assert!(matches!(
            mouse_event_to_input(right),
            Some(MouseInput::Scroll {
                direction: ScrollDir::Right,
                ..
            })
        ));
    }

    #[test]
    fn test_moved_drops_to_none() {
        let ev = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert!(mouse_event_to_input(ev).is_none());
    }

    #[test]
    fn test_modifiers_round_trip_shift_ctrl_alt() {
        let m = KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT;
        let s = key_modifiers_to_set(m);
        assert!(s.shift && s.ctrl && s.alt);
    }

    #[test]
    fn test_modifiers_drops_unmapped() {
        // SUPER, HYPER, META should be ignored.
        let m = KeyModifiers::SUPER;
        let s = key_modifiers_to_set(m);
        assert_eq!(s, KeyModSet::NONE);
    }

    #[test]
    fn test_xy_coordinate_propagation() {
        let ev = MouseEvent {
            kind: MouseEventKind::Down(CtMouseButton::Left),
            column: 42,
            row: 7,
            modifiers: KeyModifiers::NONE,
        };
        let input = mouse_event_to_input(ev).unwrap();
        assert_eq!(input.position(), (42, 7));
    }
}
