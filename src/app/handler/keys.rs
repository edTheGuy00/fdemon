//! Key event handlers for different UI modes

use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Convert key events to messages based on current UI mode
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::DeviceSelector => handle_key_device_selector(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
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

/// Handle key events in normal mode
fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    // Check if we're busy (reloading)
    let is_busy = state.is_busy();

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

        // New session (lowercase 'n') - show device selector
        (KeyCode::Char('n'), KeyModifiers::NONE) => Some(Message::ShowDeviceSelector),
        // Also allow 'd' for device selector (as shown in header)
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Message::ShowDeviceSelector),

        // ─────────────────────────────────────────────────────────
        // Scrolling - always allowed
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

        _ => None,
    }
}
