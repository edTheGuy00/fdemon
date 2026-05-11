//! Terminal event handling for the TUI.
//!
//! This module has three responsibilities:
//!
//! * **Key conversion** — maps crossterm [`KeyEvent`]s to the abstract
//!   [`InputKey`] type defined in `fdemon-app`, filtering out key kinds other
//!   than `Press` and key codes not represented in [`InputKey`].
//! * **Mouse conversion** — maps crossterm [`MouseEvent`]s to the abstract
//!   [`MouseInput`] type. `Moved` events are dropped at this boundary (high
//!   volume, no consumer); all other event kinds are exhaustively mapped.
//! * **Polling** — wraps `crossterm::event::poll` with a fixed 50 ms timeout,
//!   translates the accepted event into a [`Message`] for the TEA bus, and
//!   emits a [`Message::Tick`] on every timeout so the engine can drive
//!   time-based updates (animations, debounce expiry, etc.).

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
        MouseEventKind::Down(button) => Some(MouseInput::Press {
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

/// Convert a crossterm [`KeyEvent`] into the abstract [`InputKey`] used by
/// the TEA handler layer.
///
/// Returns `None` for key codes not represented in [`InputKey`] (e.g. Insert,
/// F13+, and any future crossterm variants). Callers should pass only
/// `KeyEventKind::Press` events; key repeats and releases are filtered earlier
/// in [`poll`].
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

/// Drain pending terminal events for up to `timeout`, discarding them.
///
/// Used during exit to consume any mouse SGR reports that the terminal
/// emitted before `DisableMouseCapture` took effect — without draining,
/// those reports remain in the kernel TTY queue and leak to the shell
/// after fdemon exits.
///
/// Returns when no event is available within a single poll slice or when
/// the cumulative elapsed time exceeds `timeout`. Errors from
/// `crossterm::event::poll` / `read` are silently swallowed — this is
/// best-effort cleanup; we must not block exit indefinitely.
pub fn drain_input(timeout: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        let remaining = timeout.saturating_sub(start.elapsed());
        // Cap each poll slice so a stuck terminal cannot block the full timeout.
        let slice = remaining.min(Duration::from_millis(10));
        match event::poll(slice) {
            Ok(true) => {
                let _ = event::read();
            }
            Ok(false) => return,
            Err(_) => return,
        }
    }
}

/// Poll the terminal for the next available event with a short timeout.
///
/// Returns:
/// * `Ok(Some(Message))` — a translated key or mouse event, or a
///   [`Message::Tick`] when the 50 ms timeout elapses (used to drive
///   animations and debounce expiry).
/// * `Ok(None)` — the event was filtered out (e.g. `KeyEventKind::Repeat`,
///   `MouseEventKind::Moved`, resize events, or an unmapped key code).
/// * `Err(_)` — an I/O error from crossterm; callers should treat this as
///   fatal and shut down the event loop.
///
/// This is the single integration point between crossterm and the TEA loop.
/// All event filtering happens here so the engine never sees raw terminal
/// events.
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
    fn test_mouse_down_left_converts_to_press() {
        let ev = MouseEvent {
            kind: MouseEventKind::Down(CtMouseButton::Left),
            column: 5,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        let input = mouse_event_to_input(ev).expect("must convert");
        assert_eq!(
            input,
            MouseInput::Press {
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

    /// Verify that `drain_input` returns quickly when no events are pending.
    ///
    /// In non-TTY CI environments `crossterm::event::poll` returns immediately
    /// with `Ok(false)`, so the function should return well within the given
    /// timeout. The test is marked `#[ignore]` so it is skipped in environments
    /// where stdin is not a real TTY and poll behaviour is unpredictable.
    #[test]
    #[ignore]
    fn test_drain_input_returns_quickly_with_no_pending_events() {
        let start = std::time::Instant::now();
        drain_input(Duration::from_millis(100));
        // Should return in under 50ms when no events are pending (first poll
        // slice returns Ok(false) immediately in a quiet terminal).
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "drain_input took too long: {:?}",
            start.elapsed()
        );
    }
}
