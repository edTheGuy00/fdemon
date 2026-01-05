//! Key event handlers for different UI modes

use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Convert key events to messages based on current UI mode
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::SearchInput => handle_key_search_input(state, key),
        UiMode::DeviceSelector => handle_key_device_selector(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
        UiMode::LinkHighlight => handle_key_link_highlight(key),
    }
}

/// Handle key events in device selector mode
fn handle_key_device_selector(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Message::DeviceSelectorUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::DeviceSelectorDown),

        // Selection
        KeyCode::Enter => {
            if state.device_selector.is_device_selected() {
                if let Some(device) = state.device_selector.selected_device() {
                    return Some(Message::DeviceSelected {
                        device: device.clone(),
                    });
                }
            } else if state.device_selector.is_android_emulator_selected() {
                return Some(Message::LaunchAndroidEmulator);
            } else if state.device_selector.is_ios_simulator_selected() {
                return Some(Message::LaunchIOSSimulator);
            }
            None
        }

        // Refresh
        KeyCode::Char('r') => Some(Message::RefreshDevices),

        // Cancel/close - only if there are running sessions
        KeyCode::Esc => Some(Message::HideDeviceSelector),

        // Quit with Ctrl+C
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Char('q') => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events in confirm dialog mode
fn handle_key_confirm_dialog(key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Confirm quit
        (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) | (KeyCode::Enter, _) => {
            Some(Message::ConfirmQuit)
        }
        // Cancel
        (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
            Some(Message::CancelQuit)
        }
        // Force quit with Ctrl+C even in dialog
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in emulator selector mode (placeholder)
fn handle_key_emulator_selector(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc => Some(Message::ShowDeviceSelector), // Go back to device selector
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in loading mode
fn handle_key_loading(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in search input mode
fn handle_key_search_input(state: &AppState, key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Cancel search input (return to normal mode)
        (KeyCode::Esc, _) => Some(Message::CancelSearch),

        // Submit search and return to normal mode
        (KeyCode::Enter, _) => Some(Message::CancelSearch), // Keep query, exit input mode

        // Delete character
        (KeyCode::Backspace, _) => {
            if let Some(handle) = state.session_manager.selected() {
                let mut query = handle.session.search_state.query.clone();
                query.pop();
                Some(Message::SearchInput { text: query })
            } else {
                None
            }
        }

        // Clear all input
        (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
            Some(Message::SearchInput {
                text: String::new(),
            })
        }

        // Type character
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            if let Some(handle) = state.session_manager.selected() {
                let mut query = handle.session.search_state.query.clone();
                query.push(c);
                Some(Message::SearchInput { text: query })
            } else {
                None
            }
        }

        // Force quit even in search mode
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events in normal mode
fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    // Check if any session is busy (reloading)
    let is_busy = state.session_manager.any_session_busy();

    match (key.code, key.modifiers) {
        // Request quit (may show confirmation dialog if sessions running)
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Message::RequestQuit),
        (KeyCode::Esc, _) => Some(Message::RequestQuit),

        // Force quit (bypass confirmation) - Ctrl+C for emergency exit
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // ─────────────────────────────────────────────────────────
        // Session Navigation (Task 10)
        // ─────────────────────────────────────────────────────────
        // Number keys 1-9 select session by index
        (KeyCode::Char('1'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(0)),
        (KeyCode::Char('2'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(1)),
        (KeyCode::Char('3'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(2)),
        (KeyCode::Char('4'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(3)),
        (KeyCode::Char('5'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(4)),
        (KeyCode::Char('6'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(5)),
        (KeyCode::Char('7'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(6)),
        (KeyCode::Char('8'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(7)),
        (KeyCode::Char('9'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(8)),

        // Tab navigation
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Message::NextSession),
        (KeyCode::BackTab, _) => Some(Message::PreviousSession),
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => Some(Message::PreviousSession),

        // Close current session
        (KeyCode::Char('x'), KeyModifiers::NONE) => Some(Message::CloseCurrentSession),
        (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
            Some(Message::CloseCurrentSession)
        }

        // Clear logs
        (KeyCode::Char('c'), KeyModifiers::NONE) => Some(Message::ClearLogs),

        // ─────────────────────────────────────────────────────────
        // App Control
        // ─────────────────────────────────────────────────────────
        // Hot reload (lowercase 'r') - only when not busy
        (KeyCode::Char('r'), KeyModifiers::NONE) if !is_busy => Some(Message::HotReload),

        // Hot restart (uppercase 'R') - only when not busy
        (KeyCode::Char('R'), KeyModifiers::NONE) if !is_busy => Some(Message::HotRestart),
        (KeyCode::Char('R'), m) if m.contains(KeyModifiers::SHIFT) && !is_busy => {
            Some(Message::HotRestart)
        }

        // Stop app (lowercase 's') - only when not busy
        (KeyCode::Char('s'), KeyModifiers::NONE) if !is_busy => Some(Message::StopApp),

        // 'd' for device selector (as shown in header)
        // Note: 'n' also triggers device selector but is overloaded with search navigation
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Message::ShowDeviceSelector),

        // ─────────────────────────────────────────────────────────
        // Log Filtering (Phase 1 - Task 4)
        // ─────────────────────────────────────────────────────────
        // 'f' - Cycle log level filter
        (KeyCode::Char('f'), KeyModifiers::NONE) => Some(Message::CycleLevelFilter),

        // 'F' - Cycle log source filter
        (KeyCode::Char('F'), KeyModifiers::NONE) => Some(Message::CycleSourceFilter),
        (KeyCode::Char('F'), m) if m.contains(KeyModifiers::SHIFT) => {
            Some(Message::CycleSourceFilter)
        }

        // Ctrl+f - Reset all filters
        (KeyCode::Char('f'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::ResetFilters),

        // ─────────────────────────────────────────────────────────
        // Log Search (Phase 1 - Task 5)
        // ─────────────────────────────────────────────────────────
        // '/' - Enter search mode (vim-style)
        (KeyCode::Char('/'), KeyModifiers::NONE) => Some(Message::StartSearch),

        // 'n' - Next search match (only when search has query)
        // Note: 'n' is overloaded - it's also used for "New session"
        // If there's an active search query, use it for next match
        (KeyCode::Char('n'), KeyModifiers::NONE) => {
            if let Some(handle) = state.session_manager.selected() {
                if !handle.session.search_state.query.is_empty() {
                    return Some(Message::NextSearchMatch);
                }
            }
            Some(Message::ShowDeviceSelector)
        }

        // 'N' - Previous search match
        (KeyCode::Char('N'), KeyModifiers::NONE) => Some(Message::PrevSearchMatch),
        (KeyCode::Char('N'), m) if m.contains(KeyModifiers::SHIFT) => {
            Some(Message::PrevSearchMatch)
        }

        // ─────────────────────────────────────────────────────────
        // Error Navigation (Phase 1 - Task 7)
        // ─────────────────────────────────────────────────────────
        // 'e' - Jump to next error
        (KeyCode::Char('e'), KeyModifiers::NONE) => Some(Message::NextError),

        // 'E' - Jump to previous error
        (KeyCode::Char('E'), KeyModifiers::NONE) => Some(Message::PrevError),
        (KeyCode::Char('E'), m) if m.contains(KeyModifiers::SHIFT) => Some(Message::PrevError),

        // ─────────────────────────────────────────────────────────
        // Stack Trace Collapse (Phase 2 - Task 6)
        // ─────────────────────────────────────────────────────────
        // Enter - Toggle stack trace expand/collapse on focused entry
        (KeyCode::Enter, KeyModifiers::NONE) => {
            // Check if current focused entry has a stack trace
            if let Some(handle) = state.session_manager.selected() {
                if let Some(entry) = handle.session.focused_entry() {
                    if entry.has_stack_trace() {
                        return Some(Message::ToggleStackTrace);
                    }
                }
            }
            None
        }

        // ─────────────────────────────────────────────────────────
        // Vertical Scrolling - always allowed
        // ─────────────────────────────────────────────────────────
        (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Message::ScrollDown),
        (KeyCode::Down, _) => Some(Message::ScrollDown),
        (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Message::ScrollUp),
        (KeyCode::Up, _) => Some(Message::ScrollUp),
        (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Message::ScrollToTop),
        (KeyCode::Char('G'), KeyModifiers::NONE) => Some(Message::ScrollToBottom),
        (KeyCode::Char('G'), m) if m.contains(KeyModifiers::SHIFT) => Some(Message::ScrollToBottom),
        (KeyCode::PageUp, _) => Some(Message::PageUp),
        (KeyCode::PageDown, _) => Some(Message::PageDown),
        (KeyCode::Home, _) => Some(Message::ScrollToTop),
        (KeyCode::End, _) => Some(Message::ScrollToBottom),

        // ─────────────────────────────────────────────────────────
        // Horizontal Scrolling (Phase 2 Task 12)
        // ─────────────────────────────────────────────────────────
        (KeyCode::Char('h'), KeyModifiers::NONE) => Some(Message::ScrollLeft(10)),
        (KeyCode::Left, KeyModifiers::NONE) => Some(Message::ScrollLeft(10)),
        (KeyCode::Char('l'), KeyModifiers::NONE) => Some(Message::ScrollRight(10)),
        (KeyCode::Right, KeyModifiers::NONE) => Some(Message::ScrollRight(10)),
        (KeyCode::Char('0'), KeyModifiers::NONE) => Some(Message::ScrollToLineStart),
        (KeyCode::Char('$'), KeyModifiers::NONE) => Some(Message::ScrollToLineEnd),
        (KeyCode::Char('$'), m) if m.contains(KeyModifiers::SHIFT) => {
            Some(Message::ScrollToLineEnd)
        }

        // ─────────────────────────────────────────────────────────
        // Link Highlight Mode (Phase 3.1)
        // ─────────────────────────────────────────────────────────
        // 'L' - Enter link highlight mode
        (KeyCode::Char('L'), KeyModifiers::NONE) => Some(Message::EnterLinkMode),
        (KeyCode::Char('L'), m) if m.contains(KeyModifiers::SHIFT) => Some(Message::EnterLinkMode),

        _ => None,
    }
}

/// Handle key events in link highlight mode (Phase 3.1)
///
/// In this mode, the viewport shows file references with shortcut keys.
/// User can press 1-9 or a-z to select and open a file.
fn handle_key_link_highlight(key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Exit link mode
        (KeyCode::Esc, _) => Some(Message::ExitLinkMode),
        (KeyCode::Char('L'), _) => Some(Message::ExitLinkMode),

        // Force quit with Ctrl+C (must be before a-z pattern)
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Allow scrolling while in link mode (must be before a-z pattern)
        (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Message::ScrollDown),
        (KeyCode::Down, _) => Some(Message::ScrollDown),
        (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Message::ScrollUp),
        (KeyCode::Up, _) => Some(Message::ScrollUp),
        (KeyCode::PageUp, _) => Some(Message::PageUp),
        (KeyCode::PageDown, _) => Some(Message::PageDown),

        // Number keys 1-9 select links
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::NONE) => Some(Message::SelectLink(c)),

        // Letter keys a-z select links 10-35 (excluding j, k which are for scrolling)
        (KeyCode::Char(c @ 'a'..='z'), KeyModifiers::NONE) => Some(Message::SelectLink(c)),

        _ => None,
    }
}

#[cfg(test)]
mod link_mode_key_tests {
    use super::*;

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_escape_exits_link_mode() {
        let msg = handle_key_link_highlight(key_event(KeyCode::Esc));
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
    }

    #[test]
    fn test_l_toggles_link_mode() {
        let msg = handle_key_link_highlight(key_event(KeyCode::Char('L')));
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
    }

    #[test]
    fn test_number_selects_link() {
        let msg = handle_key_link_highlight(key_event(KeyCode::Char('1')));
        assert!(matches!(msg, Some(Message::SelectLink('1'))));

        let msg = handle_key_link_highlight(key_event(KeyCode::Char('5')));
        assert!(matches!(msg, Some(Message::SelectLink('5'))));

        let msg = handle_key_link_highlight(key_event(KeyCode::Char('9')));
        assert!(matches!(msg, Some(Message::SelectLink('9'))));
    }

    #[test]
    fn test_letter_selects_link() {
        let msg = handle_key_link_highlight(key_event(KeyCode::Char('a')));
        assert!(matches!(msg, Some(Message::SelectLink('a'))));

        let msg = handle_key_link_highlight(key_event(KeyCode::Char('z')));
        assert!(matches!(msg, Some(Message::SelectLink('z'))));
    }

    #[test]
    fn test_scroll_allowed_in_link_mode() {
        // j/k scroll
        let msg = handle_key_link_highlight(key_event(KeyCode::Char('j')));
        assert!(matches!(msg, Some(Message::ScrollDown)));

        let msg = handle_key_link_highlight(key_event(KeyCode::Char('k')));
        assert!(matches!(msg, Some(Message::ScrollUp)));

        // Arrow keys
        let msg = handle_key_link_highlight(key_event(KeyCode::Down));
        assert!(matches!(msg, Some(Message::ScrollDown)));

        let msg = handle_key_link_highlight(key_event(KeyCode::Up));
        assert!(matches!(msg, Some(Message::ScrollUp)));

        // Page up/down
        let msg = handle_key_link_highlight(key_event(KeyCode::PageUp));
        assert!(matches!(msg, Some(Message::PageUp)));

        let msg = handle_key_link_highlight(key_event(KeyCode::PageDown));
        assert!(matches!(msg, Some(Message::PageDown)));
    }

    #[test]
    fn test_ctrl_c_quits_in_link_mode() {
        let msg = handle_key_link_highlight(key_event_with_modifiers(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert!(matches!(msg, Some(Message::Quit)));
    }

    #[test]
    fn test_unknown_key_returns_none() {
        // Keys that should not do anything in link mode
        let msg = handle_key_link_highlight(key_event(KeyCode::Char('!')));
        assert!(msg.is_none());

        let msg = handle_key_link_highlight(key_event(KeyCode::Tab));
        assert!(msg.is_none());

        let msg = handle_key_link_highlight(key_event(KeyCode::Enter));
        assert!(msg.is_none());
    }

    #[test]
    fn test_j_k_are_scroll_not_select() {
        // Even though j and k are in a-z range, they should scroll, not select
        let msg = handle_key_link_highlight(key_event(KeyCode::Char('j')));
        assert!(
            matches!(msg, Some(Message::ScrollDown)),
            "j should scroll down, not select link"
        );

        let msg = handle_key_link_highlight(key_event(KeyCode::Char('k')));
        assert!(
            matches!(msg, Some(Message::ScrollUp)),
            "k should scroll up, not select link"
        );
    }
}
