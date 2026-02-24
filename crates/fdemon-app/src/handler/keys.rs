//! Key event handlers for different UI modes

use crate::input_key::InputKey;
use crate::message::{InspectorNav, Message, NetworkNav};
use crate::session::NetworkDetailTab;
use crate::state::{AppState, DevToolsPanel, UiMode};

/// Convert key events to messages based on current UI mode
pub fn handle_key(state: &AppState, key: InputKey) -> Option<Message> {
    match state.ui_mode {
        UiMode::Startup | UiMode::NewSessionDialog => handle_key_new_session_dialog(key, state),
        UiMode::SearchInput => handle_key_search_input(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
        UiMode::LinkHighlight => handle_key_link_highlight(key),
        UiMode::Settings => handle_key_settings(state, key),
        UiMode::DevTools => handle_key_devtools(state, key),
    }
}

/// Handle key events in device selector mode
fn handle_key_confirm_dialog(key: InputKey) -> Option<Message> {
    match key {
        // Confirm quit
        // 'y', 'Y', or 'q' confirms the dialog action
        // Note: 'q' allows double-tap "qq" as quick quit shortcut
        InputKey::Char('y' | 'Y' | 'q') | InputKey::Enter => Some(Message::ConfirmQuit),
        // Cancel
        InputKey::Char('n' | 'N') | InputKey::Esc => Some(Message::CancelQuit),
        // Force quit with Ctrl+C even in dialog
        InputKey::CharCtrl('c') => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in emulator selector mode (placeholder)
fn handle_key_emulator_selector(key: InputKey) -> Option<Message> {
    match key {
        InputKey::Esc => Some(Message::OpenNewSessionDialog), // Go back to new session dialog
        InputKey::CharCtrl('c') => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in loading mode
fn handle_key_loading(key: InputKey) -> Option<Message> {
    match key {
        InputKey::Char('q') | InputKey::Esc => Some(Message::Quit),
        InputKey::CharCtrl('c') => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in search input mode
fn handle_key_search_input(state: &AppState, key: InputKey) -> Option<Message> {
    match key {
        // Cancel search input (return to normal mode)
        InputKey::Esc => Some(Message::CancelSearch),

        // Submit search and return to normal mode
        InputKey::Enter => Some(Message::CancelSearch), // Keep query, exit input mode

        // Delete character
        InputKey::Backspace => {
            if let Some(handle) = state.session_manager.selected() {
                let mut query = handle.session.search_state.query.clone();
                query.pop();
                Some(Message::SearchInput { text: query })
            } else {
                None
            }
        }

        // Clear all input
        InputKey::CharCtrl('u') => Some(Message::SearchInput {
            text: String::new(),
        }),

        // Type character (regular chars)
        InputKey::Char(c) => {
            if let Some(handle) = state.session_manager.selected() {
                let mut query = handle.session.search_state.query.clone();
                query.push(c);
                Some(Message::SearchInput { text: query })
            } else {
                None
            }
        }

        // Force quit even in search mode
        InputKey::CharCtrl('c') => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events in normal mode
fn handle_key_normal(state: &AppState, key: InputKey) -> Option<Message> {
    // Check if any session is busy (reloading)
    let is_busy = state.session_manager.any_session_busy();

    match key {
        // Request quit (may show confirmation dialog if sessions running)
        InputKey::Char('q') | InputKey::Esc => Some(Message::RequestQuit),

        // Force quit (bypass confirmation) - Ctrl+C for emergency exit
        InputKey::CharCtrl('c') => Some(Message::Quit),

        // ─────────────────────────────────────────────────────────
        // Session Navigation (Task 10)
        // ─────────────────────────────────────────────────────────
        // Number keys 1-9 select session by index
        InputKey::Char('1') => Some(Message::SelectSessionByIndex(0)),
        InputKey::Char('2') => Some(Message::SelectSessionByIndex(1)),
        InputKey::Char('3') => Some(Message::SelectSessionByIndex(2)),
        InputKey::Char('4') => Some(Message::SelectSessionByIndex(3)),
        InputKey::Char('5') => Some(Message::SelectSessionByIndex(4)),
        InputKey::Char('6') => Some(Message::SelectSessionByIndex(5)),
        InputKey::Char('7') => Some(Message::SelectSessionByIndex(6)),
        InputKey::Char('8') => Some(Message::SelectSessionByIndex(7)),
        InputKey::Char('9') => Some(Message::SelectSessionByIndex(8)),

        // Tab navigation
        InputKey::Tab => Some(Message::NextSession),
        InputKey::BackTab => Some(Message::PreviousSession),

        // Close current session
        InputKey::Char('x') => Some(Message::CloseCurrentSession),
        InputKey::CharCtrl('w') => Some(Message::CloseCurrentSession),

        // Clear logs
        InputKey::Char('c') => Some(Message::ClearLogs),

        // ─────────────────────────────────────────────────────────
        // App Control
        // ─────────────────────────────────────────────────────────
        // Hot reload (lowercase 'r') - only when not busy
        InputKey::Char('r') if !is_busy => Some(Message::HotReload),

        // Hot restart (uppercase 'R') - only when not busy
        InputKey::Char('R') if !is_busy => Some(Message::HotRestart),

        // Stop app (lowercase 's') - only when not busy
        InputKey::Char('s') if !is_busy => Some(Message::StopApp),

        // ─────────────────────────────────────────────────────────
        // Session Management
        // ─────────────────────────────────────────────────────────
        // '+' - Start new session (unified handler)
        // Always opens NewSessionDialog, regardless of existing sessions
        // Don't show dialogs while loading (auto-launch in progress)
        InputKey::Char('+') => {
            if state.ui_mode == UiMode::Loading {
                None
            } else {
                Some(Message::OpenNewSessionDialog)
            }
        }

        // 'd' for DevTools mode — available when any session exists.
        // Individual panels handle disconnected VM gracefully.
        InputKey::Char('d') => {
            if state.session_manager.selected().is_some() {
                Some(Message::EnterDevToolsMode)
            } else {
                None
            }
        }

        // ─────────────────────────────────────────────────────────
        // Log Filtering (Phase 1 - Task 4)
        // ─────────────────────────────────────────────────────────
        // 'f' - Cycle log level filter
        InputKey::Char('f') => Some(Message::CycleLevelFilter),

        // 'F' - Cycle log source filter
        InputKey::Char('F') => Some(Message::CycleSourceFilter),

        // Ctrl+f - Reset all filters
        InputKey::CharCtrl('f') => Some(Message::ResetFilters),

        // ─────────────────────────────────────────────────────────
        // Log Search (Phase 1 - Task 5)
        // ─────────────────────────────────────────────────────────
        // '/' - Enter search mode (vim-style)
        InputKey::Char('/') => Some(Message::StartSearch),

        // 'n' - Next search match (vim-style, only when search active)
        // Note: This is ONLY for search navigation, NOT for session management
        // Only works when there's an active search query
        InputKey::Char('n') => {
            if let Some(handle) = state.session_manager.selected() {
                if !handle.session.search_state.query.is_empty() {
                    return Some(Message::NextSearchMatch);
                }
            }
            None // No action when no search query
        }

        // 'N' - Previous search match
        InputKey::Char('N') => Some(Message::PrevSearchMatch),

        // ─────────────────────────────────────────────────────────
        // Error Navigation (Phase 1 - Task 7)
        // ─────────────────────────────────────────────────────────
        // 'e' - Jump to next error
        InputKey::Char('e') => Some(Message::NextError),

        // 'E' - Jump to previous error
        InputKey::Char('E') => Some(Message::PrevError),

        // ─────────────────────────────────────────────────────────
        // Stack Trace Collapse (Phase 2 - Task 6)
        // ─────────────────────────────────────────────────────────
        // Enter - Toggle stack trace expand/collapse on focused entry
        InputKey::Enter => {
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
        InputKey::Char('j') | InputKey::Down => Some(Message::ScrollDown),
        InputKey::Char('k') | InputKey::Up => Some(Message::ScrollUp),
        InputKey::Char('g') => Some(Message::ScrollToTop),
        InputKey::Char('G') => Some(Message::ScrollToBottom),
        InputKey::PageUp => Some(Message::PageUp),
        InputKey::PageDown => Some(Message::PageDown),
        InputKey::Home => Some(Message::ScrollToTop),
        InputKey::End => Some(Message::ScrollToBottom),

        // ─────────────────────────────────────────────────────────
        // Horizontal Scrolling (Phase 2 Task 12)
        // ─────────────────────────────────────────────────────────
        InputKey::Char('h') | InputKey::Left => Some(Message::ScrollLeft(10)),
        InputKey::Char('l') | InputKey::Right => Some(Message::ScrollRight(10)),
        InputKey::Char('0') => Some(Message::ScrollToLineStart),
        InputKey::Char('$') => Some(Message::ScrollToLineEnd),

        // ─────────────────────────────────────────────────────────
        // Wrap Mode (v1-refinements Phase 1)
        // ─────────────────────────────────────────────────────────
        // 'w' - Toggle line wrap mode
        InputKey::Char('w') => Some(Message::ToggleWrapMode),

        // ─────────────────────────────────────────────────────────
        // Link Highlight Mode (Phase 3.1)
        // ─────────────────────────────────────────────────────────
        // 'L' - Enter link highlight mode
        InputKey::Char('L') => Some(Message::EnterLinkMode),

        // ─────────────────────────────────────────────────────────
        // Settings (Phase 4)
        // ─────────────────────────────────────────────────────────
        // ',' - Open settings panel
        InputKey::Char(',') => Some(Message::ShowSettings),

        _ => None,
    }
}

/// Handle key events in link highlight mode (Phase 3.1)
///
/// In this mode, the viewport shows file references with shortcut keys.
/// User can press 1-9 or a-z to select and open a file.
fn handle_key_link_highlight(key: InputKey) -> Option<Message> {
    match key {
        // Exit link mode
        InputKey::Esc | InputKey::Char('L') => Some(Message::ExitLinkMode),

        // Force quit with Ctrl+C (must be before a-z pattern)
        InputKey::CharCtrl('c') => Some(Message::Quit),

        // Allow scrolling while in link mode (must be before a-z pattern)
        InputKey::Char('j') | InputKey::Down => Some(Message::ScrollDown),
        InputKey::Char('k') | InputKey::Up => Some(Message::ScrollUp),
        InputKey::PageUp => Some(Message::PageUp),
        InputKey::PageDown => Some(Message::PageDown),

        // Number keys 1-9 select links
        InputKey::Char(c @ '1'..='9') => Some(Message::SelectLink(c)),

        // Letter keys a-z select links 10-35 (excluding j, k which are for scrolling)
        InputKey::Char(c @ 'a'..='z') => Some(Message::SelectLink(c)),

        _ => None,
    }
}

/// Handle key events in DevTools mode (Phase 4, Task 02).
///
/// Key bindings:
/// - `Esc` — exit DevTools mode (or deselect frame when Performance panel has one selected)
/// - `i` — switch to Inspector panel
/// - `p` — switch to Performance panel
/// - `b` — open Flutter DevTools in system browser
/// - `Ctrl+r` — toggle repaint rainbow overlay
/// - `Ctrl+p` — toggle performance overlay
/// - `Ctrl+d` — toggle debug paint overlay
/// - `j`/Down — scroll/navigate down (in Inspector: move selection down)
/// - `k`/Up — scroll/navigate up (in Inspector: move selection up)
/// - `h`/Left — in Inspector: collapse node; in Performance: previous frame
/// - `Right`/`Enter` — in Inspector: expand node; in Performance (Right): next frame
/// - `r` — in Inspector: refresh widget tree
/// - `q` — request quit
fn handle_key_devtools(state: &AppState, key: InputKey) -> Option<Message> {
    let in_inspector = state.devtools_view_state.active_panel == DevToolsPanel::Inspector;
    let in_performance = state.devtools_view_state.active_panel == DevToolsPanel::Performance;
    let in_network = state.devtools_view_state.active_panel == DevToolsPanel::Network;
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    // ── Network filter input mode ─────────────────────────────────────────────
    // When filter input is active, route keys to the filter buffer before any
    // other Network panel binding so no regular network key leaks through.
    if in_network {
        let filter_active = state
            .session_manager
            .selected()
            .map(|h| h.session.network.filter_input_active)
            .unwrap_or(false);

        if filter_active {
            return match key {
                InputKey::Esc => Some(Message::NetworkExitFilterMode),
                InputKey::Enter => Some(Message::NetworkCommitFilter),
                InputKey::Backspace => Some(Message::NetworkFilterBackspace),
                InputKey::Char(c) if !c.is_control() => Some(Message::NetworkFilterInput(c)),
                _ => None,
            };
        }
    }

    match key {
        // ── Exit DevTools / deselect frame ────────────────────────────────────
        //
        // When the Performance panel is active and a frame is selected, Esc
        // "unwinds" one level: it deselects the frame instead of exiting. This
        // matches common TUI conventions where Esc dismisses the innermost
        // selection before navigating outward.
        //
        // When the Network panel is active, Esc deselects the current request.
        InputKey::Esc => {
            if in_performance {
                let frame_selected = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.performance.selected_frame.is_some())
                    .unwrap_or(false);
                if frame_selected {
                    return Some(Message::SelectPerformanceFrame { index: None });
                }
            }
            if in_network {
                let has_selection = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.network.selected_index.is_some())
                    .unwrap_or(false);
                if has_selection {
                    return Some(Message::NetworkSelectRequest { index: None });
                }
            }
            Some(Message::ExitDevToolsMode)
        }

        // ── Sub-panel switching ───────────────────────────────────────────────
        InputKey::Char('i') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector)),

        // 'p' always switches to Performance panel.
        InputKey::Char('p') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),

        // 'n' always switches to Network panel.
        InputKey::Char('n') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Network)),

        // ── Browser DevTools ──────────────────────────────────────────────────
        InputKey::Char('b') => Some(Message::OpenBrowserDevTools),

        // ── Debug overlay toggles ─────────────────────────────────────────────
        InputKey::CharCtrl('r') => Some(Message::ToggleDebugOverlay {
            extension: crate::message::DebugOverlayKind::RepaintRainbow,
        }),
        InputKey::CharCtrl('p') => Some(Message::ToggleDebugOverlay {
            extension: crate::message::DebugOverlayKind::PerformanceOverlay,
        }),
        InputKey::CharCtrl('d') => Some(Message::ToggleDebugOverlay {
            extension: crate::message::DebugOverlayKind::DebugPaint,
        }),

        // ── Network panel — list navigation ───────────────────────────────────
        InputKey::Up | InputKey::Char('k') if in_network => {
            Some(Message::NetworkNavigate(NetworkNav::Up))
        }
        InputKey::Down | InputKey::Char('j') if in_network => {
            Some(Message::NetworkNavigate(NetworkNav::Down))
        }
        InputKey::PageUp if in_network => Some(Message::NetworkNavigate(NetworkNav::PageUp)),
        InputKey::PageDown if in_network => Some(Message::NetworkNavigate(NetworkNav::PageDown)),

        // ── Network panel — request selection ────────────────────────────────
        InputKey::Enter if in_network => {
            // Re-fetch detail for the currently selected request (if any).
            if let Some(handle) = state.session_manager.selected() {
                if handle.session.network.selected_index.is_some() {
                    return Some(Message::NetworkSelectRequest {
                        index: handle.session.network.selected_index,
                    });
                }
            }
            None
        }

        // ── Network panel — detail sub-tab switching ──────────────────────────
        InputKey::Char('g') if in_network => {
            Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::General))
        }
        InputKey::Char('h') if in_network => {
            Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::Headers))
        }
        InputKey::Char('q') if in_network => Some(Message::NetworkSwitchDetailTab(
            NetworkDetailTab::RequestBody,
        )),
        InputKey::Char('s') if in_network => Some(Message::NetworkSwitchDetailTab(
            NetworkDetailTab::ResponseBody,
        )),
        InputKey::Char('t') if in_network => {
            Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::Timing))
        }

        // ── Network panel — recording toggle ─────────────────────────────────
        InputKey::Char(' ') if in_network => Some(Message::ToggleNetworkRecording),

        // ── Network panel — clear history ─────────────────────────────────────
        InputKey::CharCtrl('x') if in_network => state
            .session_manager
            .selected_id()
            .map(|session_id| Message::ClearNetworkProfile { session_id }),

        // ── Network panel — enter filter input mode ───────────────────────────
        InputKey::Char('/') if in_network => Some(Message::NetworkEnterFilterMode),

        // ── Inspector navigation (only active in Inspector panel) ─────────────
        InputKey::Up | InputKey::Char('k') if in_inspector => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Up))
        }
        InputKey::Down | InputKey::Char('j') if in_inspector => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Down))
        }
        InputKey::Enter | InputKey::Right if in_inspector => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Expand))
        }
        InputKey::Left | InputKey::Char('h') if in_inspector => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Collapse))
        }
        // 'r' in Inspector panel refreshes the widget tree.
        InputKey::Char('r') if in_inspector => {
            active_id.map(|session_id| Message::RequestWidgetTree { session_id })
        }

        // ── Performance panel — allocation table sort ─────────────────────────
        //
        // 's' toggles the allocation table sort column between BySize and
        // ByInstances. This binding is only active in the Performance panel;
        // in the Network panel 's' switches to the ResponseBody sub-tab (handled
        // above with the `in_network` guard), so there is no conflict.
        InputKey::Char('s') if in_performance => Some(Message::ToggleAllocationSort),

        // ── Performance panel frame navigation ────────────────────────────────
        //
        // Left and Right navigate between frames in the bar chart. The guards
        // are exclusive with the Inspector panel guards above, so there is no
        // conflict: Inspector uses Left/Right for tree collapse/expand, and
        // Performance uses them for frame prev/next.
        InputKey::Left if in_performance => Some(Message::SelectPerformanceFrame {
            index: state
                .session_manager
                .selected()
                .and_then(|h| h.session.performance.compute_prev_frame_index()),
        }),
        InputKey::Right if in_performance => Some(Message::SelectPerformanceFrame {
            index: state
                .session_manager
                .selected()
                .and_then(|h| h.session.performance.compute_next_frame_index()),
        }),

        // ── Quit still works from DevTools mode ───────────────────────────────
        // Guard: 'q' is also used as RequestBody sub-tab in Network panel
        // (handled above by the in_network guard). At this point in the match
        // we are NOT in the Network panel, so this is a safe global quit.
        InputKey::Char('q') => Some(Message::RequestQuit),

        // Force quit
        InputKey::CharCtrl('c') => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events in settings mode (Phase 4)
fn handle_key_settings(state: &AppState, key: InputKey) -> Option<Message> {
    // If dart defines modal is open, route all keys to it
    if state.settings_view_state.dart_defines_modal.is_some() {
        return handle_key_settings_dart_defines(state, key);
    }

    // If extra args modal is open, route all keys to it
    if state.settings_view_state.extra_args_modal.is_some() {
        return handle_key_settings_extra_args(key);
    }

    // If editing, handle text input
    if state.settings_view_state.editing {
        return handle_key_settings_edit(state, key);
    }

    match key {
        // Close settings
        InputKey::Esc | InputKey::Char('q') => Some(Message::HideSettings),

        // Tab navigation
        InputKey::Tab => Some(Message::SettingsNextTab),
        InputKey::BackTab => Some(Message::SettingsPrevTab),

        // Number keys for direct tab access
        InputKey::Char('1') => Some(Message::SettingsGotoTab(0)),
        InputKey::Char('2') => Some(Message::SettingsGotoTab(1)),
        InputKey::Char('3') => Some(Message::SettingsGotoTab(2)),
        InputKey::Char('4') => Some(Message::SettingsGotoTab(3)),

        // Item navigation
        InputKey::Char('j') | InputKey::Down => Some(Message::SettingsNextItem),
        InputKey::Char('k') | InputKey::Up => Some(Message::SettingsPrevItem),

        // Toggle/edit
        InputKey::Enter | InputKey::Char(' ') => Some(Message::SettingsToggleEdit),

        // Save
        InputKey::CharCtrl('s') => Some(Message::SettingsSave),

        // Create new launch config ('n' on Launch Config tab)
        InputKey::Char('n')
            if state.settings_view_state.active_tab == crate::config::SettingsTab::LaunchConfig =>
        {
            Some(Message::LaunchConfigCreate)
        }

        // Force quit with Ctrl+C
        InputKey::CharCtrl('c') => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events while editing a setting value
fn handle_key_settings_edit(state: &AppState, key: InputKey) -> Option<Message> {
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
            match key {
                InputKey::Enter | InputKey::Char(' ') => Some(Message::SettingsToggleBool),
                InputKey::Esc => Some(Message::SettingsCancelEdit),
                _ => None,
            }
        }
        SettingValue::Number(_) => match key {
            InputKey::Esc => Some(Message::SettingsCancelEdit),
            InputKey::Enter => Some(Message::SettingsCommitEdit),
            InputKey::Char('+' | '=') => Some(Message::SettingsIncrement(1)),
            InputKey::Char('-') => {
                if state.settings_view_state.edit_buffer.is_empty() {
                    Some(Message::SettingsCharInput('-'))
                } else {
                    Some(Message::SettingsIncrement(-1))
                }
            }
            InputKey::Char(c) if c.is_ascii_digit() => Some(Message::SettingsCharInput(c)),
            InputKey::Backspace => Some(Message::SettingsBackspace),
            _ => None,
        },
        SettingValue::Float(_) => match key {
            InputKey::Esc => Some(Message::SettingsCancelEdit),
            InputKey::Enter => Some(Message::SettingsCommitEdit),
            InputKey::Char(c) if c.is_ascii_digit() || c == '.' => {
                Some(Message::SettingsCharInput(c))
            }
            InputKey::Char('-') if state.settings_view_state.edit_buffer.is_empty() => {
                Some(Message::SettingsCharInput('-'))
            }
            InputKey::Backspace => Some(Message::SettingsBackspace),
            _ => None,
        },
        SettingValue::String(_) => match key {
            InputKey::Esc => Some(Message::SettingsCancelEdit),
            InputKey::Enter => Some(Message::SettingsCommitEdit),
            InputKey::Char(c) => Some(Message::SettingsCharInput(c)),
            InputKey::Backspace => Some(Message::SettingsBackspace),
            InputKey::Delete => Some(Message::SettingsClearBuffer),
            _ => None,
        },
        SettingValue::Enum { .. } => {
            // Enums don't use traditional edit mode - cycle directly
            match key {
                InputKey::Enter | InputKey::Char(' ') | InputKey::Right => {
                    Some(Message::SettingsCycleEnumNext)
                }
                InputKey::Left => Some(Message::SettingsCycleEnumPrev),
                InputKey::Esc => Some(Message::SettingsCancelEdit),
                _ => None,
            }
        }
        SettingValue::List(_) => match key {
            InputKey::Esc => Some(Message::SettingsCancelEdit),
            InputKey::Enter => Some(Message::SettingsCommitEdit), // Add item
            InputKey::Char('d') if !state.settings_view_state.editing => {
                Some(Message::SettingsRemoveListItem)
            }
            InputKey::Char(c) => Some(Message::SettingsCharInput(c)),
            InputKey::Backspace => Some(Message::SettingsBackspace),
            _ => None,
        },
    }
}

/// Handle key events when the dart defines modal is open in settings mode.
///
/// Routes keys to the modal overlay messages.  The active pane (List vs Edit)
/// and focused field determine which messages are emitted.
fn handle_key_settings_dart_defines(state: &AppState, key: InputKey) -> Option<Message> {
    use crate::new_session_dialog::{DartDefinesEditField, DartDefinesPane};

    let modal = state.settings_view_state.dart_defines_modal.as_ref()?;

    match modal.active_pane {
        DartDefinesPane::List => match key {
            InputKey::Up | InputKey::Char('k') => Some(Message::SettingsDartDefinesUp),
            InputKey::Down | InputKey::Char('j') => Some(Message::SettingsDartDefinesDown),
            InputKey::Enter => Some(Message::SettingsDartDefinesConfirm),
            InputKey::Tab => Some(Message::SettingsDartDefinesSwitchPane),
            InputKey::Esc => Some(Message::SettingsDartDefinesCancel),
            _ => None,
        },
        DartDefinesPane::Edit => match modal.edit_field {
            DartDefinesEditField::Key | DartDefinesEditField::Value => match key {
                InputKey::Char(c) => Some(Message::SettingsDartDefinesInput { c }),
                InputKey::Backspace => Some(Message::SettingsDartDefinesBackspace),
                InputKey::Tab => Some(Message::SettingsDartDefinesNextField),
                InputKey::Enter => Some(Message::SettingsDartDefinesConfirm),
                InputKey::Esc => Some(Message::SettingsDartDefinesSwitchPane),
                _ => None,
            },
            DartDefinesEditField::Save => match key {
                InputKey::Enter => Some(Message::SettingsDartDefinesSave),
                InputKey::Tab => Some(Message::SettingsDartDefinesNextField),
                InputKey::Esc => Some(Message::SettingsDartDefinesSwitchPane),
                _ => None,
            },
            DartDefinesEditField::Delete => match key {
                InputKey::Enter => Some(Message::SettingsDartDefinesDelete),
                InputKey::Tab => Some(Message::SettingsDartDefinesNextField),
                InputKey::Esc => Some(Message::SettingsDartDefinesSwitchPane),
                _ => None,
            },
        },
    }
}

/// Handle key events when the extra args fuzzy modal is open in settings mode.
///
/// Routes keys to the fuzzy modal overlay messages.
fn handle_key_settings_extra_args(key: InputKey) -> Option<Message> {
    match key {
        InputKey::Char(c) => Some(Message::SettingsExtraArgsInput { c }),
        InputKey::Backspace => Some(Message::SettingsExtraArgsBackspace),
        InputKey::Up => Some(Message::SettingsExtraArgsUp),
        InputKey::Down => Some(Message::SettingsExtraArgsDown),
        InputKey::Enter => Some(Message::SettingsExtraArgsConfirm),
        InputKey::Esc => Some(Message::SettingsExtraArgsClose),
        InputKey::CharCtrl('u') => Some(Message::SettingsExtraArgsClear),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Startup Dialog Key Handling (Phase 5)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle key events in startup dialog mode
fn handle_key_new_session_dialog(key: InputKey, state: &AppState) -> Option<Message> {
    use crate::new_session_dialog::{DialogPane, TargetTab};

    let dialog = &state.new_session_dialog_state;

    // Check highest priority keys and modals first
    if dialog.is_fuzzy_modal_open() {
        return handle_fuzzy_modal_key(key);
    }
    if dialog.is_dart_defines_modal_open() {
        return handle_dart_defines_modal_key(key, dialog);
    }

    match key {
        // Ctrl+C to quit (highest priority)
        InputKey::CharCtrl('c') => Some(Message::Quit),

        // Settings accessible from startup dialog (comma key)
        InputKey::Char(',') => Some(Message::ShowSettings),

        // Main dialog keys
        InputKey::Esc => Some(Message::NewSessionDialogEscape),
        InputKey::Tab => Some(Message::NewSessionDialogSwitchPane),
        InputKey::Char('1') => Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
        InputKey::Char('2') => Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),

        // Route based on focused pane
        _ => match dialog.focused_pane {
            DialogPane::TargetSelector => handle_target_selector_key(key),
            DialogPane::LaunchContext => handle_launch_context_key(key, dialog),
        },
    }
}

fn handle_fuzzy_modal_key(key: InputKey) -> Option<Message> {
    match key {
        InputKey::Up => Some(Message::NewSessionDialogFuzzyUp),
        InputKey::Down => Some(Message::NewSessionDialogFuzzyDown),
        InputKey::Enter => Some(Message::NewSessionDialogFuzzyConfirm),
        InputKey::Esc => Some(Message::NewSessionDialogCloseFuzzyModal),
        InputKey::Backspace => Some(Message::NewSessionDialogFuzzyBackspace),
        InputKey::Char(c) => Some(Message::NewSessionDialogFuzzyInput { c }),
        _ => None,
    }
}

fn handle_dart_defines_modal_key(
    key: InputKey,
    dialog: &crate::new_session_dialog::NewSessionDialogState,
) -> Option<Message> {
    use crate::new_session_dialog::DartDefinesPane;

    let active_pane = dialog
        .dart_defines_modal
        .as_ref()
        .map(|m| m.active_pane)
        .unwrap_or(DartDefinesPane::List);

    match key {
        InputKey::Tab => Some(Message::NewSessionDialogDartDefinesSwitchPane),
        InputKey::Up => Some(Message::NewSessionDialogDartDefinesUp),
        InputKey::Down => Some(Message::NewSessionDialogDartDefinesDown),
        InputKey::Enter => Some(Message::NewSessionDialogDartDefinesConfirm),
        InputKey::Esc => match active_pane {
            // Esc in List pane → cancel (discard changes, close modal)
            DartDefinesPane::List => Some(Message::NewSessionDialogCancelDartDefinesModal),
            // Esc in Edit pane → switch back to List pane (don't close)
            DartDefinesPane::Edit => Some(Message::NewSessionDialogDartDefinesSwitchPane),
        },
        InputKey::Backspace => Some(Message::NewSessionDialogDartDefinesBackspace),
        InputKey::Char(c) => Some(Message::NewSessionDialogDartDefinesInput { c }),
        _ => None,
    }
}

fn handle_target_selector_key(key: InputKey) -> Option<Message> {
    match key {
        InputKey::Up => Some(Message::NewSessionDialogDeviceUp),
        InputKey::Down => Some(Message::NewSessionDialogDeviceDown),
        InputKey::Enter => Some(Message::NewSessionDialogDeviceSelect),
        InputKey::Char('r') => Some(Message::NewSessionDialogRefreshDevices),
        _ => None,
    }
}

fn handle_launch_context_key(
    key: InputKey,
    dialog: &crate::new_session_dialog::NewSessionDialogState,
) -> Option<Message> {
    use crate::new_session_dialog::LaunchContextField;

    match key {
        InputKey::Up => Some(Message::NewSessionDialogFieldPrev),
        InputKey::Down => Some(Message::NewSessionDialogFieldNext),
        InputKey::Enter => Some(Message::NewSessionDialogFieldActivate),
        InputKey::Left if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModePrev)
        }
        InputKey::Right if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModeNext)
        }
        _ => None,
    }
}

#[cfg(test)]
mod link_mode_key_tests {
    use super::*;

    #[test]
    fn test_escape_exits_link_mode() {
        let msg = handle_key_link_highlight(InputKey::Esc);
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
    }

    #[test]
    fn test_l_toggles_link_mode() {
        let msg = handle_key_link_highlight(InputKey::Char('L'));
        assert!(matches!(msg, Some(Message::ExitLinkMode)));
    }

    #[test]
    fn test_number_selects_link() {
        let msg = handle_key_link_highlight(InputKey::Char('1'));
        assert!(matches!(msg, Some(Message::SelectLink('1'))));

        let msg = handle_key_link_highlight(InputKey::Char('5'));
        assert!(matches!(msg, Some(Message::SelectLink('5'))));

        let msg = handle_key_link_highlight(InputKey::Char('9'));
        assert!(matches!(msg, Some(Message::SelectLink('9'))));
    }

    #[test]
    fn test_letter_selects_link() {
        let msg = handle_key_link_highlight(InputKey::Char('a'));
        assert!(matches!(msg, Some(Message::SelectLink('a'))));

        let msg = handle_key_link_highlight(InputKey::Char('z'));
        assert!(matches!(msg, Some(Message::SelectLink('z'))));
    }

    #[test]
    fn test_scroll_allowed_in_link_mode() {
        // j/k scroll
        let msg = handle_key_link_highlight(InputKey::Char('j'));
        assert!(matches!(msg, Some(Message::ScrollDown)));

        let msg = handle_key_link_highlight(InputKey::Char('k'));
        assert!(matches!(msg, Some(Message::ScrollUp)));

        // Arrow keys
        let msg = handle_key_link_highlight(InputKey::Down);
        assert!(matches!(msg, Some(Message::ScrollDown)));

        let msg = handle_key_link_highlight(InputKey::Up);
        assert!(matches!(msg, Some(Message::ScrollUp)));

        // Page up/down
        let msg = handle_key_link_highlight(InputKey::PageUp);
        assert!(matches!(msg, Some(Message::PageUp)));

        let msg = handle_key_link_highlight(InputKey::PageDown);
        assert!(matches!(msg, Some(Message::PageDown)));
    }

    #[test]
    fn test_ctrl_c_quits_in_link_mode() {
        let msg = handle_key_link_highlight(InputKey::CharCtrl('c'));
        assert!(matches!(msg, Some(Message::Quit)));
    }

    #[test]
    fn test_unknown_key_returns_none() {
        // Keys that should not do anything in link mode
        let msg = handle_key_link_highlight(InputKey::Char('!'));
        assert!(msg.is_none());

        let msg = handle_key_link_highlight(InputKey::Tab);
        assert!(msg.is_none());

        let msg = handle_key_link_highlight(InputKey::Enter);
        assert!(msg.is_none());
    }

    #[test]
    fn test_j_k_are_scroll_not_select() {
        // Even though j and k are in a-z range, they should scroll, not select
        let msg = handle_key_link_highlight(InputKey::Char('j'));
        assert!(
            matches!(msg, Some(Message::ScrollDown)),
            "j should scroll down, not select link"
        );

        let msg = handle_key_link_highlight(InputKey::Char('k'));
        assert!(
            matches!(msg, Some(Message::ScrollUp)),
            "k should scroll up, not select link"
        );
    }
}

#[cfg(test)]
mod device_selector_key_tests {
    use super::*;

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
    fn test_d_key_with_session_emits_enter_devtools() {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();

        let msg = handle_key_normal(&state, InputKey::Char('d'));

        assert!(matches!(msg, Some(Message::EnterDevToolsMode)));
    }

    #[test]
    fn test_d_key_without_sessions_returns_none() {
        let state = AppState::new();
        // No sessions at all

        let msg = handle_key_normal(&state, InputKey::Char('d'));

        assert!(msg.is_none());
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

        let msg = handle_key_normal(&state, InputKey::Char('n'));

        // 'n' should do nothing when no search query is active
        assert!(msg.is_none());
    }

    #[test]
    fn test_n_key_without_sessions() {
        let state = AppState::new();
        // No running sessions

        let msg = handle_key_normal(&state, InputKey::Char('n'));

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

        let msg = handle_key_normal(&state, InputKey::Char('n'));

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

        let msg = handle_key_normal(&state, InputKey::Char('+'));

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_plus_key_without_sessions() {
        let state = AppState::new();
        // No running sessions

        let msg = handle_key_normal(&state, InputKey::Char('+'));

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

        // InputKey doesn't distinguish between Char('+') with SHIFT vs NONE,
        // both become Char('+'), so this test is the same as without shift
        let msg = handle_key_normal(&state, InputKey::Char('+'));

        assert!(matches!(msg, Some(Message::OpenNewSessionDialog)));
    }

    #[test]
    fn test_plus_key_ignored_during_loading() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Loading;

        let msg = handle_key_normal(&state, InputKey::Char('+'));

        assert!(msg.is_none());
        assert_eq!(state.ui_mode, UiMode::Loading); // Still loading, no dialog
    }

    #[test]
    fn test_d_key_ignored_during_loading() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Loading;

        let msg = handle_key_normal(&state, InputKey::Char('d'));

        assert!(msg.is_none());
        assert_eq!(state.ui_mode, UiMode::Loading); // Still loading, no dialog
    }

    // test_comma_opens_settings_from_device_selector removed - DeviceSelector no longer exists
}

#[cfg(test)]
mod settings_key_tests {
    use super::*;

    #[test]
    fn test_comma_opens_settings() {
        let state = AppState::new();
        let msg = handle_key_normal(&state, InputKey::Char(','));
        assert!(matches!(msg, Some(Message::ShowSettings)));
    }

    #[test]
    fn test_escape_closes_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, InputKey::Esc);
        assert!(matches!(msg, Some(Message::HideSettings)));
    }

    #[test]
    fn test_q_closes_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, InputKey::Char('q'));
        assert!(matches!(msg, Some(Message::HideSettings)));
    }

    #[test]
    fn test_tab_navigation() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, InputKey::Tab);
        assert!(matches!(msg, Some(Message::SettingsNextTab)));

        let msg = handle_key_settings(&state, InputKey::BackTab);
        assert!(matches!(msg, Some(Message::SettingsPrevTab)));
    }

    #[test]
    fn test_number_keys_jump_to_tab() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, InputKey::Char('1'));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(0))));

        let msg = handle_key_settings(&state, InputKey::Char('2'));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(1))));

        let msg = handle_key_settings(&state, InputKey::Char('3'));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(2))));

        let msg = handle_key_settings(&state, InputKey::Char('4'));
        assert!(matches!(msg, Some(Message::SettingsGotoTab(3))));
    }

    #[test]
    fn test_item_navigation() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        // j/Down for next
        let msg = handle_key_settings(&state, InputKey::Char('j'));
        assert!(matches!(msg, Some(Message::SettingsNextItem)));

        let msg = handle_key_settings(&state, InputKey::Down);
        assert!(matches!(msg, Some(Message::SettingsNextItem)));

        // k/Up for previous
        let msg = handle_key_settings(&state, InputKey::Char('k'));
        assert!(matches!(msg, Some(Message::SettingsPrevItem)));

        let msg = handle_key_settings(&state, InputKey::Up);
        assert!(matches!(msg, Some(Message::SettingsPrevItem)));
    }

    #[test]
    fn test_toggle_edit() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        // Enter toggles edit
        let msg = handle_key_settings(&state, InputKey::Enter);
        assert!(matches!(msg, Some(Message::SettingsToggleEdit)));

        // Space toggles edit
        let msg = handle_key_settings(&state, InputKey::Char(' '));
        assert!(matches!(msg, Some(Message::SettingsToggleEdit)));
    }

    #[test]
    fn test_ctrl_s_saves() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, InputKey::CharCtrl('s'));
        assert!(matches!(msg, Some(Message::SettingsSave)));
    }

    #[test]
    fn test_ctrl_c_quits_in_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, InputKey::CharCtrl('c'));
        assert!(matches!(msg, Some(Message::Quit)));
    }

    #[test]
    fn test_comma_opens_settings_from_startup_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Startup;

        let msg = handle_key(&state, InputKey::Char(','));
        assert!(
            matches!(msg, Some(Message::ShowSettings)),
            "Comma should open settings from Startup mode"
        );
    }

    #[test]
    fn test_comma_opens_settings_from_new_session_dialog_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::NewSessionDialog;

        let msg = handle_key(&state, InputKey::Char(','));
        assert!(
            matches!(msg, Some(Message::ShowSettings)),
            "Comma should open settings from NewSessionDialog mode"
        );
    }

    #[test]
    fn test_edit_mode_escape_exits() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.editing = true;

        let msg = handle_key_settings(&state, InputKey::Esc);
        // Now returns SettingsCancelEdit in edit mode
        assert!(matches!(msg, Some(Message::SettingsCancelEdit)));
    }

    #[test]
    fn test_edit_mode_enter_confirms() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.editing = true;

        let msg = handle_key_settings(&state, InputKey::Enter);
        // Now returns SettingsCommitEdit or value-specific message
        // This depends on the value type, so just verify it returns something
        assert!(msg.is_some());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration: key routing with modals open (Phase 2, Task 06)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod settings_modal_key_routing_tests {
    use super::*;

    // ── Dart defines modal intercepts keys ──────────────────────────────────

    /// When the dart defines modal is open, Esc closes the modal (not settings).
    #[test]
    fn test_key_routing_dart_defines_modal_esc_in_list_cancels() {
        use crate::new_session_dialog::{DartDefine, DartDefinesModalState};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.dart_defines_modal =
            Some(DartDefinesModalState::new(vec![DartDefine::new("K", "V")]));

        let msg = handle_key_settings(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::SettingsDartDefinesCancel)),
            "Esc in List pane should emit SettingsDartDefinesCancel, not Close or HideSettings"
        );
    }

    /// With no modal open, Esc closes the settings panel.
    #[test]
    fn test_key_routing_settings_normal_esc_closes_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        // No modal open

        let msg = handle_key_settings(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::HideSettings)),
            "Esc without any modal should emit HideSettings"
        );
    }

    /// Typed characters are routed to the dart defines modal, not to edit mode.
    #[test]
    fn test_key_routing_dart_defines_modal_intercepts_char_input() {
        use crate::new_session_dialog::{DartDefinesModalState, DartDefinesPane};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        let mut modal = DartDefinesModalState::new(vec![]);
        modal.active_pane = DartDefinesPane::Edit;
        state.settings_view_state.dart_defines_modal = Some(modal);

        let msg = handle_key_settings(&state, InputKey::Char('x'));
        assert!(
            matches!(msg, Some(Message::SettingsDartDefinesInput { c: 'x' })),
            "Char with dart defines modal open in Edit pane should emit SettingsDartDefinesInput"
        );
    }

    /// In the List pane, j/Down navigates in the dart defines list.
    #[test]
    fn test_key_routing_dart_defines_modal_list_pane_nav() {
        use crate::new_session_dialog::DartDefinesModalState;

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        // Default active_pane is List
        state.settings_view_state.dart_defines_modal = Some(DartDefinesModalState::new(vec![]));

        let msg_j = handle_key_settings(&state, InputKey::Char('j'));
        assert!(
            matches!(msg_j, Some(Message::SettingsDartDefinesDown)),
            "'j' in List pane should emit SettingsDartDefinesDown"
        );

        let msg_k = handle_key_settings(&state, InputKey::Char('k'));
        assert!(
            matches!(msg_k, Some(Message::SettingsDartDefinesUp)),
            "'k' in List pane should emit SettingsDartDefinesUp"
        );
    }

    // ── Extra args modal intercepts keys ────────────────────────────────────

    /// When the extra args modal is open, Esc closes it (not settings).
    #[test]
    fn test_key_routing_extra_args_modal_esc_closes_modal() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.extra_args_modal = Some(FuzzyModalState::new(
            FuzzyModalType::ExtraArgs,
            vec!["--verbose".to_string()],
        ));

        let msg = handle_key_settings(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::SettingsExtraArgsClose)),
            "Esc with extra args modal open should emit SettingsExtraArgsClose, not HideSettings"
        );
    }

    /// Typed characters are routed to the extra args modal query.
    #[test]
    fn test_key_routing_extra_args_modal_intercepts_char_input() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![]));

        let msg = handle_key_settings(&state, InputKey::Char('a'));
        assert!(
            matches!(msg, Some(Message::SettingsExtraArgsInput { c: 'a' })),
            "Char with extra args modal open should emit SettingsExtraArgsInput"
        );
    }

    /// Enter confirms the selection in the extra args modal.
    #[test]
    fn test_key_routing_extra_args_modal_enter_confirms() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.extra_args_modal = Some(FuzzyModalState::new(
            FuzzyModalType::ExtraArgs,
            vec!["--verbose".to_string()],
        ));

        let msg = handle_key_settings(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::SettingsExtraArgsConfirm)),
            "Enter with extra args modal open should emit SettingsExtraArgsConfirm"
        );
    }

    /// Up/Down navigate in the extra args modal.
    #[test]
    fn test_key_routing_extra_args_modal_nav() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.extra_args_modal = Some(FuzzyModalState::new(
            FuzzyModalType::ExtraArgs,
            vec!["--verbose".to_string(), "--trace-startup".to_string()],
        ));

        let msg_down = handle_key_settings(&state, InputKey::Down);
        assert!(
            matches!(msg_down, Some(Message::SettingsExtraArgsDown)),
            "Down with extra args modal open should emit SettingsExtraArgsDown"
        );

        let msg_up = handle_key_settings(&state, InputKey::Up);
        assert!(
            matches!(msg_up, Some(Message::SettingsExtraArgsUp)),
            "Up with extra args modal open should emit SettingsExtraArgsUp"
        );
    }

    /// Ctrl+U clears the extra args modal query.
    #[test]
    fn test_key_routing_extra_args_modal_ctrl_u_clears_query() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![]));

        let msg = handle_key_settings(&state, InputKey::CharCtrl('u'));
        assert!(
            matches!(msg, Some(Message::SettingsExtraArgsClear)),
            "Ctrl+U with extra args modal open should emit SettingsExtraArgsClear"
        );
    }

    // ── Modal priority over edit mode ────────────────────────────────────────

    /// When both editing=true and a modal is open, the modal takes priority.
    #[test]
    fn test_modal_takes_priority_over_edit_mode() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.editing = true; // edit mode is active
        state.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![]));

        // Char input should go to modal, not edit buffer
        let msg = handle_key_settings(&state, InputKey::Char('z'));
        assert!(
            matches!(msg, Some(Message::SettingsExtraArgsInput { c: 'z' })),
            "When modal is open, char input must route to modal even if editing=true"
        );
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

#[cfg(test)]
mod performance_sort_key_tests {
    use super::*;

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    /// Create a state with one session in DevTools / Performance panel.
    fn make_state_in_performance_panel() -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        state
    }

    /// Create a state with one session in DevTools / Network panel.
    fn make_state_in_network_panel() -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Network;
        state
    }

    #[test]
    fn test_s_in_performance_panel_emits_toggle_allocation_sort() {
        let state = make_state_in_performance_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('s'));
        assert!(
            matches!(msg, Some(Message::ToggleAllocationSort)),
            "'s' in Performance panel should emit ToggleAllocationSort"
        );
    }

    #[test]
    fn test_s_in_network_panel_emits_response_body_tab() {
        let state = make_state_in_network_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('s'));
        // In the Network panel 's' maps to NetworkSwitchDetailTab(ResponseBody), not ToggleAllocationSort.
        assert!(
            matches!(
                msg,
                Some(Message::NetworkSwitchDetailTab(
                    crate::session::NetworkDetailTab::ResponseBody
                ))
            ),
            "'s' in Network panel should still emit NetworkSwitchDetailTab(ResponseBody)"
        );
    }

    #[test]
    fn test_s_in_inspector_panel_returns_none() {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

        let msg = handle_key_devtools(&state, InputKey::Char('s'));
        // 's' has no binding in the Inspector panel.
        assert!(msg.is_none(), "'s' in Inspector panel should return None");
    }
}

#[cfg(test)]
mod network_filter_key_tests {
    use super::*;

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    fn make_state_in_network_panel() -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Network;
        state
    }

    fn make_state_in_network_filter_mode() -> AppState {
        let mut state = make_state_in_network_panel();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .filter_input_active = true;
        state
    }

    #[test]
    fn test_slash_in_network_panel_enters_filter_mode() {
        let state = make_state_in_network_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('/'));
        assert!(
            matches!(msg, Some(Message::NetworkEnterFilterMode)),
            "'/' in Network panel should emit NetworkEnterFilterMode"
        );
    }

    #[test]
    fn test_filter_mode_escape_exits() {
        let state = make_state_in_network_filter_mode();
        let msg = handle_key_devtools(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::NetworkExitFilterMode)),
            "Esc in filter mode should emit NetworkExitFilterMode"
        );
    }

    #[test]
    fn test_filter_mode_enter_commits() {
        let state = make_state_in_network_filter_mode();
        let msg = handle_key_devtools(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::NetworkCommitFilter)),
            "Enter in filter mode should emit NetworkCommitFilter"
        );
    }

    #[test]
    fn test_filter_mode_backspace_removes_char() {
        let state = make_state_in_network_filter_mode();
        let msg = handle_key_devtools(&state, InputKey::Backspace);
        assert!(
            matches!(msg, Some(Message::NetworkFilterBackspace)),
            "Backspace in filter mode should emit NetworkFilterBackspace"
        );
    }

    #[test]
    fn test_filter_mode_char_appends() {
        let state = make_state_in_network_filter_mode();
        let msg = handle_key_devtools(&state, InputKey::Char('a'));
        assert!(
            matches!(msg, Some(Message::NetworkFilterInput('a'))),
            "Char in filter mode should emit NetworkFilterInput"
        );
    }

    #[test]
    fn test_filter_mode_keys_do_not_conflict_with_panel_bindings() {
        // In filter mode, 'j'/'k' should emit NetworkFilterInput, not NetworkNavigate.
        let state = make_state_in_network_filter_mode();
        let msg_j = handle_key_devtools(&state, InputKey::Char('j'));
        assert!(
            matches!(msg_j, Some(Message::NetworkFilterInput('j'))),
            "'j' in filter mode should be treated as text input, not navigation"
        );
        let msg_k = handle_key_devtools(&state, InputKey::Char('k'));
        assert!(
            matches!(msg_k, Some(Message::NetworkFilterInput('k'))),
            "'k' in filter mode should be treated as text input, not navigation"
        );
    }

    #[test]
    fn test_slash_does_not_trigger_filter_mode_in_inspector() {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        let msg = handle_key_devtools(&state, InputKey::Char('/'));
        // '/' has no binding in the Inspector panel.
        assert!(
            msg.is_none(),
            "'/' in Inspector panel should not emit NetworkEnterFilterMode"
        );
    }

    #[test]
    fn test_filter_mode_unknown_key_returns_none() {
        let state = make_state_in_network_filter_mode();
        let msg = handle_key_devtools(&state, InputKey::Tab);
        assert!(
            msg.is_none(),
            "Unknown key in filter mode should return None"
        );
    }
}
