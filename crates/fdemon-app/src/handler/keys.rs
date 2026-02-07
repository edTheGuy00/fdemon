//! Key event handlers for different UI modes

use crate::message::Message;
use crate::state::{AppState, UiMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Convert key events to messages based on current UI mode
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::Startup | UiMode::NewSessionDialog => handle_key_new_session_dialog(key, state),
        UiMode::SearchInput => handle_key_search_input(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
        UiMode::LinkHighlight => handle_key_link_highlight(key),
        UiMode::Settings => handle_key_settings(state, key),
    }
}

/// Handle key events in device selector mode
fn handle_key_confirm_dialog(key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Confirm quit
        // 'y', 'Y', or 'q' confirms the dialog action
        // Note: 'q' allows double-tap "qq" as quick quit shortcut
        (KeyCode::Char('y'), _)
        | (KeyCode::Char('Y'), _)
        | (KeyCode::Char('q'), _)
        | (KeyCode::Enter, _) => Some(Message::ConfirmQuit),
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
        KeyCode::Esc => Some(Message::OpenNewSessionDialog), // Go back to new session dialog
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

        // ─────────────────────────────────────────────────────────
        // Session Management
        // ─────────────────────────────────────────────────────────
        // '+' - Start new session (unified handler)
        // Always opens NewSessionDialog, regardless of existing sessions
        // Don't show dialogs while loading (auto-launch in progress)
        (KeyCode::Char('+'), KeyModifiers::NONE) | (KeyCode::Char('+'), KeyModifiers::SHIFT) => {
            if state.ui_mode == UiMode::Loading {
                None
            } else {
                Some(Message::OpenNewSessionDialog)
            }
        }

        // 'd' for adding device/session (alternative to '+')
        // Always opens NewSessionDialog, regardless of existing sessions
        // Don't show dialogs while loading (auto-launch in progress)
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            if state.ui_mode == UiMode::Loading {
                None
            } else {
                Some(Message::OpenNewSessionDialog)
            }
        }

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

        // 'n' - Next search match (vim-style, only when search active)
        // Note: This is ONLY for search navigation, NOT for session management
        // Only works when there's an active search query
        (KeyCode::Char('n'), KeyModifiers::NONE) => {
            if let Some(handle) = state.session_manager.selected() {
                if !handle.session.search_state.query.is_empty() {
                    return Some(Message::NextSearchMatch);
                }
            }
            None // No action when no search query
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

        // ─────────────────────────────────────────────────────────
        // Settings (Phase 4)
        // ─────────────────────────────────────────────────────────
        // ',' - Open settings panel
        (KeyCode::Char(','), KeyModifiers::NONE) => Some(Message::ShowSettings),

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

/// Handle key events in settings mode (Phase 4)
fn handle_key_settings(state: &AppState, key: KeyEvent) -> Option<Message> {
    // If editing, handle text input
    if state.settings_view_state.editing {
        return handle_key_settings_edit(state, key);
    }

    match key.code {
        // Close settings
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::HideSettings),

        // Tab navigation
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some(Message::SettingsPrevTab)
            } else {
                Some(Message::SettingsNextTab)
            }
        }

        // Number keys for direct tab access
        KeyCode::Char('1') => Some(Message::SettingsGotoTab(0)),
        KeyCode::Char('2') => Some(Message::SettingsGotoTab(1)),
        KeyCode::Char('3') => Some(Message::SettingsGotoTab(2)),
        KeyCode::Char('4') => Some(Message::SettingsGotoTab(3)),

        // Item navigation
        KeyCode::Char('j') | KeyCode::Down => Some(Message::SettingsNextItem),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::SettingsPrevItem),

        // Toggle/edit
        KeyCode::Enter | KeyCode::Char(' ') => Some(Message::SettingsToggleEdit),

        // Save
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::SettingsSave)
        }

        // Force quit with Ctrl+C
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events while editing a setting value
fn handle_key_settings_edit(state: &AppState, key: KeyEvent) -> Option<Message> {
    // Get the current item type to determine appropriate key handling
    use crate::config::SettingValue;
    use crate::settings_items::get_selected_item;

    let item = get_selected_item(
        &state.settings,
        &state.project_path,
        &state.settings_view_state,
    )?;

    match &item.value {
        SettingValue::Bool(_) => {
            // Booleans don't use traditional edit mode - toggle directly
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => Some(Message::SettingsToggleBool),
                KeyCode::Esc => Some(Message::SettingsCancelEdit),
                _ => None,
            }
        }
        SettingValue::Number(_) => match key.code {
            KeyCode::Esc => Some(Message::SettingsCancelEdit),
            KeyCode::Enter => Some(Message::SettingsCommitEdit),
            KeyCode::Char('+') | KeyCode::Char('=') => Some(Message::SettingsIncrement(1)),
            KeyCode::Char('-') if key.modifiers.is_empty() => Some(Message::SettingsIncrement(-1)),
            KeyCode::Char(c) if c.is_ascii_digit() => Some(Message::SettingsCharInput(c)),
            KeyCode::Char('-') if state.settings_view_state.edit_buffer.is_empty() => {
                Some(Message::SettingsCharInput('-'))
            }
            KeyCode::Backspace => Some(Message::SettingsBackspace),
            _ => None,
        },
        SettingValue::Float(_) => match key.code {
            KeyCode::Esc => Some(Message::SettingsCancelEdit),
            KeyCode::Enter => Some(Message::SettingsCommitEdit),
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                Some(Message::SettingsCharInput(c))
            }
            KeyCode::Char('-') if state.settings_view_state.edit_buffer.is_empty() => {
                Some(Message::SettingsCharInput('-'))
            }
            KeyCode::Backspace => Some(Message::SettingsBackspace),
            _ => None,
        },
        SettingValue::String(_) => match key.code {
            KeyCode::Esc => Some(Message::SettingsCancelEdit),
            KeyCode::Enter => Some(Message::SettingsCommitEdit),
            KeyCode::Char(c) => Some(Message::SettingsCharInput(c)),
            KeyCode::Backspace => Some(Message::SettingsBackspace),
            KeyCode::Delete => Some(Message::SettingsClearBuffer),
            _ => None,
        },
        SettingValue::Enum { .. } => {
            // Enums don't use traditional edit mode - cycle directly
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right => {
                    Some(Message::SettingsCycleEnumNext)
                }
                KeyCode::Left => Some(Message::SettingsCycleEnumPrev),
                KeyCode::Esc => Some(Message::SettingsCancelEdit),
                _ => None,
            }
        }
        SettingValue::List(_) => {
            match key.code {
                KeyCode::Esc => Some(Message::SettingsCancelEdit),
                KeyCode::Enter => Some(Message::SettingsCommitEdit), // Add item
                KeyCode::Char('d') if !state.settings_view_state.editing => {
                    Some(Message::SettingsRemoveListItem)
                }
                KeyCode::Char(c) => Some(Message::SettingsCharInput(c)),
                KeyCode::Backspace => Some(Message::SettingsBackspace),
                _ => None,
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Startup Dialog Key Handling (Phase 5)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle key events in startup dialog mode
fn handle_key_new_session_dialog(key: KeyEvent, state: &AppState) -> Option<Message> {
    use crate::new_session_dialog::{DialogPane, TargetTab};

    let dialog = &state.new_session_dialog_state;

    match (key.code, key.modifiers) {
        // Ctrl+C to quit (highest priority)
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Check if modal is open first
        _ if dialog.is_fuzzy_modal_open() => handle_fuzzy_modal_key(key),
        _ if dialog.is_dart_defines_modal_open() => handle_dart_defines_modal_key(key),

        // Main dialog keys
        (KeyCode::Esc, _) => Some(Message::NewSessionDialogEscape),
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchPane),
        (KeyCode::Char('1'), KeyModifiers::NONE) => {
            Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected))
        }
        (KeyCode::Char('2'), KeyModifiers::NONE) => {
            Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable))
        }

        // Route based on focused pane
        _ => match dialog.focused_pane {
            DialogPane::TargetSelector => handle_target_selector_key(key),
            DialogPane::LaunchContext => handle_launch_context_key(key, dialog),
        },
    }
}

fn handle_fuzzy_modal_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFuzzyUp),
        KeyCode::Down => Some(Message::NewSessionDialogFuzzyDown),
        KeyCode::Enter => Some(Message::NewSessionDialogFuzzyConfirm),
        KeyCode::Esc => Some(Message::NewSessionDialogCloseFuzzyModal),
        KeyCode::Backspace => Some(Message::NewSessionDialogFuzzyBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogFuzzyInput { c }),
        _ => None,
    }
}

fn handle_dart_defines_modal_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Tab => Some(Message::NewSessionDialogDartDefinesSwitchPane),
        KeyCode::Up => Some(Message::NewSessionDialogDartDefinesUp),
        KeyCode::Down => Some(Message::NewSessionDialogDartDefinesDown),
        KeyCode::Enter => Some(Message::NewSessionDialogDartDefinesConfirm),
        KeyCode::Esc => Some(Message::NewSessionDialogCloseDartDefinesModal),
        KeyCode::Backspace => Some(Message::NewSessionDialogDartDefinesBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogDartDefinesInput { c }),
        _ => None,
    }
}

fn handle_target_selector_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogDeviceUp),
        KeyCode::Down => Some(Message::NewSessionDialogDeviceDown),
        KeyCode::Enter => Some(Message::NewSessionDialogDeviceSelect),
        KeyCode::Char('r') => Some(Message::NewSessionDialogRefreshDevices),
        _ => None,
    }
}

fn handle_launch_context_key(
    key: KeyEvent,
    dialog: &crate::new_session_dialog::NewSessionDialogState,
) -> Option<Message> {
    use crate::new_session_dialog::LaunchContextField;

    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFieldPrev),
        KeyCode::Down => Some(Message::NewSessionDialogFieldNext),
        KeyCode::Enter => Some(Message::NewSessionDialogFieldActivate),
        KeyCode::Left if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModePrev)
        }
        KeyCode::Right if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModeNext)
        }
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

#[cfg(test)]
mod device_selector_key_tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    #[test]
    fn test_d_key_with_running_sessions() {
        use fdemon_core::AppPhase;

        let mut state = AppState::new();
        // Simulate running session
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        // Mark session as running (newly created sessions aren't in Running phase)
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.phase = AppPhase::Running;
        }

        let msg = handle_key_normal(&state, key(KeyCode::Char('d')));

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_d_key_without_sessions() {
        let state = AppState::new();
        // No running sessions

        let msg = handle_key_normal(&state, key(KeyCode::Char('d')));

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_n_key_with_running_sessions_no_search() {
        use fdemon_core::AppPhase;

        let mut state = AppState::new();
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        // Mark session as running
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.phase = AppPhase::Running;
        }

        let msg = handle_key_normal(&state, key(KeyCode::Char('n')));

        // 'n' should do nothing when no search query is active
        assert!(msg.is_none());
    }

    #[test]
    fn test_n_key_without_sessions() {
        let state = AppState::new();
        // No running sessions

        let msg = handle_key_normal(&state, key(KeyCode::Char('n')));

        // 'n' should do nothing when no search query is active
        assert!(msg.is_none());
    }

    #[test]
    fn test_n_key_with_search_query() {
        let mut state = AppState::new();
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();

        // Set search query
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.search_state.query = "test query".to_string();
        }

        // Select the session
        state.session_manager.select_by_id(session_id);

        let msg = handle_key_normal(&state, key(KeyCode::Char('n')));

        // Should trigger NextSearchMatch when search query is active
        assert!(matches!(msg, Some(Message::NextSearchMatch)));
    }

    #[test]
    fn test_plus_key_with_running_sessions() {
        use fdemon_core::AppPhase;

        let mut state = AppState::new();
        // Simulate running session
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        // Mark session as running (newly created sessions aren't in Running phase)
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.phase = AppPhase::Running;
        }

        let msg = handle_key_normal(&state, key(KeyCode::Char('+')));

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_plus_key_without_sessions() {
        let state = AppState::new();
        // No running sessions

        let msg = handle_key_normal(&state, key(KeyCode::Char('+')));

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_plus_key_with_shift_modifier() {
        use fdemon_core::AppPhase;

        let mut state = AppState::new();
        // Simulate running session
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.phase = AppPhase::Running;
        }

        let msg = handle_key_normal(
            &state,
            KeyEvent::new(KeyCode::Char('+'), KeyModifiers::SHIFT),
        );

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_plus_key_ignored_during_loading() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Loading;

        let msg = handle_key_normal(&state, key(KeyCode::Char('+')));

        assert!(msg.is_none());
        assert_eq!(state.ui_mode, UiMode::Loading); // Still loading, no dialog
    }

    #[test]
    fn test_d_key_ignored_during_loading() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Loading;

        let msg = handle_key_normal(&state, key(KeyCode::Char('d')));

        assert!(msg.is_none());
        assert_eq!(state.ui_mode, UiMode::Loading); // Still loading, no dialog
    }

    // test_comma_opens_settings_from_device_selector removed - DeviceSelector no longer exists
}

#[cfg(test)]
mod settings_key_tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn test_comma_opens_settings() {
        let state = AppState::new();
        let msg = handle_key_normal(&state, key(KeyCode::Char(',')));
        assert!(matches!(msg, Some(Message::ShowSettings)));
    }

    #[test]
    fn test_escape_closes_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, key(KeyCode::Esc));
        assert!(matches!(msg, Some(Message::HideSettings)));
    }

    #[test]
    fn test_q_closes_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, key(KeyCode::Char('q')));
        assert!(matches!(msg, Some(Message::HideSettings)));
    }

    #[test]
    fn test_tab_navigation() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, key(KeyCode::Tab));
        assert!(matches!(msg, Some(Message::SettingsNextTab)));

        let msg = handle_key_settings(&state, key_with_mod(KeyCode::Tab, KeyModifiers::SHIFT));
        assert!(matches!(msg, Some(Message::SettingsPrevTab)));
    }

    #[test]
    fn test_number_keys_jump_to_tab() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, key(KeyCode::Char('1')));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(0))));

        let msg = handle_key_settings(&state, key(KeyCode::Char('2')));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(1))));

        let msg = handle_key_settings(&state, key(KeyCode::Char('3')));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(2))));

        let msg = handle_key_settings(&state, key(KeyCode::Char('4')));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(3))));
    }

    #[test]
    fn test_item_navigation() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        // j/Down for next
        let msg = handle_key_settings(&state, key(KeyCode::Char('j')));
        assert!(matches!(msg, Some(Message::SettingsNextItem)));

        let msg = handle_key_settings(&state, key(KeyCode::Down));
        assert!(matches!(msg, Some(Message::SettingsNextItem)));

        // k/Up for previous
        let msg = handle_key_settings(&state, key(KeyCode::Char('k')));
        assert!(matches!(msg, Some(Message::SettingsPrevItem)));

        let msg = handle_key_settings(&state, key(KeyCode::Up));
        assert!(matches!(msg, Some(Message::SettingsPrevItem)));
    }

    #[test]
    fn test_toggle_edit() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        // Enter toggles edit
        let msg = handle_key_settings(&state, key(KeyCode::Enter));
        assert!(matches!(msg, Some(Message::SettingsToggleEdit)));

        // Space toggles edit
        let msg = handle_key_settings(&state, key(KeyCode::Char(' ')));
        assert!(matches!(msg, Some(Message::SettingsToggleEdit)));
    }

    #[test]
    fn test_ctrl_s_saves() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(
            &state,
            key_with_mod(KeyCode::Char('s'), KeyModifiers::CONTROL),
        );
        assert!(matches!(msg, Some(Message::SettingsSave)));
    }

    #[test]
    fn test_ctrl_c_quits_in_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(
            &state,
            key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(matches!(msg, Some(Message::Quit)));
    }

    #[test]
    fn test_edit_mode_escape_exits() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.editing = true;

        let msg = handle_key_settings(&state, key(KeyCode::Esc));
        // Now returns SettingsCancelEdit in edit mode
        assert!(matches!(msg, Some(Message::SettingsCancelEdit)));
    }

    #[test]
    fn test_edit_mode_enter_confirms() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.editing = true;

        let msg = handle_key_settings(&state, key(KeyCode::Enter));
        // Now returns SettingsCommitEdit or value-specific message
        // This depends on the value type, so just verify it returns something
        assert!(msg.is_some());
    }
}

#[cfg(test)]
mod settings_view_state_tests {
    use crate::config::SettingsTab;
    use crate::state::SettingsViewState;

    #[test]
    fn test_settings_view_state_default() {
        let state = SettingsViewState::default();
        assert_eq!(state.active_tab, SettingsTab::Project);
        assert_eq!(state.selected_index, 0);
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
        assert!(!state.dirty);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_settings_view_state_tab_navigation() {
        let mut state = SettingsViewState::new();
        assert_eq!(state.active_tab, SettingsTab::Project);

        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::UserPrefs);

        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::LaunchConfig);

        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);

        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::Project); // Wraps around

        state.prev_tab();
        assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);

        state.prev_tab();
        assert_eq!(state.active_tab, SettingsTab::LaunchConfig);
    }

    #[test]
    fn test_settings_view_state_goto_tab() {
        let mut state = SettingsViewState::new();

        state.goto_tab(SettingsTab::LaunchConfig);
        assert_eq!(state.active_tab, SettingsTab::LaunchConfig);
        assert_eq!(state.selected_index, 0); // Reset on tab change

        state.goto_tab(SettingsTab::UserPrefs);
        assert_eq!(state.active_tab, SettingsTab::UserPrefs);
    }

    #[test]
    fn test_settings_view_state_item_selection() {
        let mut state = SettingsViewState::new();
        assert_eq!(state.selected_index, 0);

        state.select_next(5);
        assert_eq!(state.selected_index, 1);

        state.select_next(5);
        assert_eq!(state.selected_index, 2);

        state.select_previous(5);
        assert_eq!(state.selected_index, 1);

        state.select_previous(5);
        assert_eq!(state.selected_index, 0);

        // Wrap around
        state.select_previous(5);
        assert_eq!(state.selected_index, 4);

        state.select_next(5);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_settings_view_state_editing() {
        let mut state = SettingsViewState::new();
        assert!(!state.editing);

        state.start_editing("test value");
        assert!(state.editing);
        assert_eq!(state.edit_buffer, "test value");

        state.stop_editing();
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
    }

    #[test]
    fn test_settings_view_state_dirty_flag() {
        let mut state = SettingsViewState::new();
        assert!(!state.dirty);

        state.mark_dirty();
        assert!(state.dirty);

        state.clear_dirty();
        assert!(!state.dirty);
    }

    #[test]
    fn test_tab_change_resets_selection_and_editing() {
        let mut state = SettingsViewState::new();
        state.selected_index = 5;
        state.editing = true;
        state.edit_buffer = "test".to_string();

        state.next_tab();
        assert_eq!(state.selected_index, 0);
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
    }
}
