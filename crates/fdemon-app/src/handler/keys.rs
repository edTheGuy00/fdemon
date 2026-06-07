//! Key event handlers for different UI modes

use crate::input_key::InputKey;
use crate::install_wizard::WizardOrigin;
use crate::message::{InspectorNav, Message, NetworkNav};
use crate::session::performance::{PerfSection, SelectionDirection};
use crate::session::NetworkDetailTab;
use crate::state::{AppState, DevToolsPanel, PerfDetailsTab, UiMode};

/// Convert key events to messages based on current UI mode
pub fn handle_key(state: &AppState, key: InputKey) -> Option<Message> {
    // ── Global: Alt+m → ToggleMouseCapture ───────────────────────────────────
    //
    // This binding is mode-independent (works from Normal, DevTools, Loading,
    // LinkHighlight, ConfirmDialog, FlutterVersion, etc.) so users can always
    // recover native text selection regardless of where they are.
    //
    // Suppressed when a text-input field has focus, so Alt+m does not steal a
    // keystroke intended for that field:
    //   - SearchInput mode: the entire mode is a text field.
    //   - Settings when `editing` is true: an inline text/number field is open.
    //   - NewSessionDialog / Startup:
    //       * Sub-modals (dart-defines, fuzzy search) always host text inputs.
    //       * Main dialog: only the `LaunchContext` pane has text fields, so
    //         suppress there. The `TargetSelector` pane is a device-picker
    //         list with no text input — Alt+m still toggles capture there.
    //
    // The toggle is NOT gated on `is_busy` — it is a UI affordance, not an app
    // action, and must be reachable even during a hot-reload.
    if matches!(key, InputKey::CharAlt('m' | 'M')) {
        let in_text_input = match state.ui_mode {
            UiMode::SearchInput => true,
            UiMode::Settings => state.settings_view_state.editing,
            UiMode::Startup | UiMode::NewSessionDialog => {
                use crate::new_session_dialog::DialogPane;
                let dlg = &state.new_session_dialog_state;
                // Sub-modals (dart defines, fuzzy search) always contain text inputs.
                if dlg.dart_defines_modal.is_some() || dlg.fuzzy_modal.is_some() {
                    true
                } else {
                    // Main dialog: only LaunchContext pane has text fields.
                    // TargetSelector is a device-picker list — no text input.
                    matches!(dlg.focused_pane, DialogPane::LaunchContext)
                }
            }
            _ => false,
        };
        if !in_text_input {
            return Some(Message::ToggleMouseCapture);
        }
        // In text-input contexts, fall through to the mode handler (which has
        // no arm for CharAlt and will return None — correct behaviour).
    }

    match state.ui_mode {
        UiMode::Startup | UiMode::NewSessionDialog => handle_key_new_session_dialog(key, state),
        UiMode::SearchInput => handle_key_search_input(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
        UiMode::LinkHighlight => handle_key_link_highlight(key),
        UiMode::Settings => handle_key_settings(state, key),
        UiMode::FlutterVersion => handle_key_flutter_version(key, state),
        UiMode::DevTools => handle_key_devtools(state, key),
        UiMode::InstallWizard => handle_key_install_wizard(key, state),
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
    // ─────────────────────────────────────────────────────────────────────────
    // Tag filter overlay intercepts ALL keys when visible (Phase 2, Task 09)
    // ─────────────────────────────────────────────────────────────────────────
    if state.tag_filter_visible {
        return match key {
            // Close the overlay
            InputKey::Esc | InputKey::Char('T') | InputKey::Char('t') => {
                Some(Message::HideTagFilter)
            }
            // Navigate up
            InputKey::Up | InputKey::Char('k') => Some(Message::TagFilterMoveUp),
            // Navigate down
            InputKey::Down | InputKey::Char('j') => Some(Message::TagFilterMoveDown),
            // Toggle selected tag
            InputKey::Char(' ') | InputKey::Enter => Some(Message::TagFilterToggleSelected),
            // Show all tags
            InputKey::Char('a') => Some(Message::ShowAllNativeTags),
            // Hide all tags
            InputKey::Char('n') => Some(Message::HideAllNativeTags),
            // Force quit even while overlay is open (consistent with all other overlays)
            InputKey::CharCtrl('c') => Some(Message::Quit),
            // Consume all other keys while overlay is open
            _ => None,
        };
    }

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

        // 'D' for DAP server toggle — available regardless of session state.
        // The DAP server is a global service; it can start before any Flutter
        // session is running (IDE connects first, user starts session later).
        InputKey::Char('D') => Some(Message::ToggleDap),

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

        // ─────────────────────────────────────────────────────────
        // Native Tag Filter (Phase 2, Task 09)
        // ─────────────────────────────────────────────────────────
        // 'T' - Open tag filter overlay (mnemonic: Tag filter)
        InputKey::Char('T') | InputKey::Char('t') => Some(Message::ShowTagFilter),

        // ─────────────────────────────────────────────────────────
        // Flutter Version Panel
        // ─────────────────────────────────────────────────────────
        // 'V' - Open Flutter version panel (uppercase to avoid conflict with
        // future vim-style visual mode that might use lowercase 'v')
        InputKey::Char('V') => Some(Message::ShowFlutterVersion),

        // ─────────────────────────────────────────────────────────
        // Install Wizard
        // ─────────────────────────────────────────────────────────
        // 'I' - Open Install Wizard panel (uppercase; lowercase 'i' is used
        // in FlutterVersion for Install)
        InputKey::Char('I') => Some(Message::ShowInstallWizard {
            origin: WizardOrigin::UserInvoked,
        }),

        _ => None,
    }
}

/// Handle key events in Flutter version panel mode.
///
/// Key bindings:
/// - `Ctrl+C` — force quit (always active)
/// - `Esc` — close the panel (`FlutterVersionEscape`)
/// - `Tab` — switch between panes (`FlutterVersionSwitchPane`)
/// - `k`/`Up` — navigate up in the list (`FlutterVersionUp`)
/// - `j`/`Down` — navigate down in the list (`FlutterVersionDown`)
/// - `Enter` — switch to selected Flutter version (`FlutterVersionSwitch`)
/// - `d` — remove selected Flutter version (`FlutterVersionRemove`)
/// - `i` — install a Flutter version (`FlutterVersionInstall`)
/// - `u` — update the selected Flutter version (`FlutterVersionUpdate`)
///
/// Action keys (`Enter`, `d`, `i`, `u`) emit messages unconditionally;
/// the update handlers gate on the focused pane to decide whether to act.
fn handle_key_flutter_version(key: InputKey, _state: &AppState) -> Option<Message> {
    match key {
        // ── Global keys ───────────────────────────────────────────────────────
        InputKey::CharCtrl('c') => Some(Message::Quit),

        // ── Panel lifecycle ───────────────────────────────────────────────────
        InputKey::Esc => Some(Message::FlutterVersionEscape),

        // ── Pane switching ────────────────────────────────────────────────────
        InputKey::Tab => Some(Message::FlutterVersionSwitchPane),

        // ── Navigation ───────────────────────────────────────────────────────
        InputKey::Char('k') | InputKey::Up => Some(Message::FlutterVersionUp),
        InputKey::Char('j') | InputKey::Down => Some(Message::FlutterVersionDown),

        // ── Actions ───────────────────────────────────────────────────────────
        InputKey::Enter => Some(Message::FlutterVersionSwitch),
        InputKey::Char('d') => Some(Message::FlutterVersionRemove),
        InputKey::Char('i') => Some(Message::FlutterVersionInstall),
        InputKey::Char('u') => Some(Message::FlutterVersionUpdate),

        _ => None,
    }
}

/// Handle key events in Install Wizard panel mode.
///
/// Key bindings:
/// - `Ctrl+C` — force quit (always active)
/// - `Esc` — cancel the running step if one is in flight (`InstallWizardCancelStep`),
///   or close the panel when idle (`InstallWizardEscape`).
/// - `Tab` — switch between panes (`InstallWizardSwitchPane`)
/// - `k`/`Up` — navigate up in the step list or scroll detail up
/// - `j`/`Down` — navigate down in the step list or scroll detail down
/// - `Enter` — run (or retry) the selected wizard step (`InstallWizardRunSelectedStep`)
/// - `r` — re-run the preflight check (`InstallWizardRerunPreflight`)
/// - `c` — copy the selected guided command to the clipboard (`InstallWizardCopyCommand`)
/// - `[` — select the previous guided command (`InstallWizardPrevCommand`)
/// - `]` — select the next guided command (`InstallWizardNextCommand`)
fn handle_key_install_wizard(key: InputKey, state: &AppState) -> Option<Message> {
    match key {
        // ── Global keys ───────────────────────────────────────────────────────
        InputKey::CharCtrl('c') => Some(Message::Quit),

        // ── Panel lifecycle ───────────────────────────────────────────────────
        // Esc is overloaded: cancel the running step if one is in flight,
        // otherwise close the wizard (existing behaviour).
        InputKey::Esc => {
            if state.install_wizard_state.is_step_running() {
                Some(Message::InstallWizardCancelStep)
            } else {
                Some(Message::InstallWizardEscape)
            }
        }

        // ── Pane switching ────────────────────────────────────────────────────
        InputKey::Tab => Some(Message::InstallWizardSwitchPane),

        // ── Navigation ───────────────────────────────────────────────────────
        InputKey::Char('k') | InputKey::Up => Some(Message::InstallWizardUp),
        InputKey::Char('j') | InputKey::Down => Some(Message::InstallWizardDown),

        // ── Actions ───────────────────────────────────────────────────────────
        // Run (or retry) the currently selected wizard step (Phase 2, Task 05).
        InputKey::Enter => Some(Message::InstallWizardRunSelectedStep),
        InputKey::Char('r') => Some(Message::InstallWizardRerunPreflight),
        // Copy the selected guided command to the clipboard (Phase 3, Task 07).
        InputKey::Char('c') => Some(Message::InstallWizardCopyCommand),
        // Cycle through multiple guided commands on a step (Phase 4, Task 04).
        InputKey::Char('[') => Some(Message::InstallWizardPrevCommand),
        InputKey::Char(']') => Some(Message::InstallWizardNextCommand),

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
/// - `h`/Left — in Inspector tree mode: collapse node; in Performance: previous frame
/// - `Right` — in Inspector tree mode: expand node; in Inspector details mode: next tab; in Performance: next frame
/// - `Left` — in Inspector details mode: previous tab
/// - `Enter` — in Inspector tree mode: open details view
/// - `H` — in Inspector: toggle hide-implementation-widgets
/// - `Tab` — in Inspector details mode: cycle tabs forward
/// - `Shift+Tab` — in Inspector details mode: cycle tabs backward
/// - `r` — in Inspector: refresh widget tree
/// - `q` — request quit
fn handle_key_devtools(state: &AppState, key: InputKey) -> Option<Message> {
    let in_inspector = state.devtools_view_state.active_panel == DevToolsPanel::Inspector;
    let in_performance = state.devtools_view_state.active_panel == DevToolsPanel::Performance;
    let in_memory = state.devtools_view_state.active_panel == DevToolsPanel::Memory;
    let in_network = state.devtools_view_state.active_panel == DevToolsPanel::Network;
    let details_open = in_inspector && state.devtools_view_state.inspector.details_open;
    let active_id = state.session_manager.selected().map(|h| h.session.id);
    let is_busy = state.session_manager.any_session_busy();

    // ── Phase 5 T03: Timeline Events tab selection context ────────────────────
    //
    // Pre-computed at function entry so they are available in both the
    // `if in_performance` early-return block AND the main `match key` block.
    // Guards in the performance block AND in the final match reference these.
    let on_timeline_tab = in_performance
        && state
            .session_manager
            .selected()
            .filter(|h| h.session.performance.focused_section == PerfSection::Details)
            .map(|h| h.session.performance.details_tab)
            .is_some_and(|t| t == PerfDetailsTab::TimelineEvents);
    let has_selection = on_timeline_tab
        && state
            .session_manager
            .selected()
            .map(|h| h.session.performance.timeline_selected_event.is_some())
            .unwrap_or(false);
    let popup_open = on_timeline_tab
        && state
            .session_manager
            .selected()
            .map(|h| h.session.performance.timeline_details_popup_open)
            .unwrap_or(false);
    // Phase 5 T04: Timeline search — pre-computed for the input-intercept and
    // the `n`/`N` arms below.
    let has_query = on_timeline_tab
        && state
            .session_manager
            .selected()
            .map(|h| h.session.performance.timeline_search_query.is_some())
            .unwrap_or(false);
    let search_input_active = on_timeline_tab
        && state
            .session_manager
            .selected()
            .map(|h| h.session.performance.timeline_search_input_active)
            .unwrap_or(false);

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

    // ── Performance panel — section navigation and scroll ────────────────────
    //
    // These bindings MUST be evaluated before the generic `match key` block so
    // that Tab/Shift+Tab, j/k, Up/Down, PageUp/Down, Home/End are intercepted
    // when the Performance panel is active instead of falling through to the
    // generic DevTools handlers (which have no bindings for those keys here).
    //
    // Left/Right (frame selection) and `s` (sort toggle) remain in the main
    // match below with their `in_performance` guards — they are not moved here.
    if in_performance {
        // ── Phase 5 T04: Timeline search input mode ───────────────────────────
        //
        // When the search input is active, ALL keys are intercepted here so that
        // regular character keys cannot dispatch unintended actions (e.g. `j`
        // scrolling while typing). Mirrors the Network filter input pattern above.
        if search_input_active {
            if let Some(session_id) = active_id {
                return match key {
                    InputKey::Char(c) => {
                        Some(Message::TimelineSearchInputChar { session_id, ch: c })
                    }
                    InputKey::Backspace => {
                        Some(Message::TimelineSearchInputBackspace { session_id })
                    }
                    InputKey::Enter => Some(Message::TimelineSearchInputCommit { session_id }),
                    InputKey::Esc => Some(Message::TimelineSearchInputCancel { session_id }),
                    _ => None,
                };
            }
        }

        // ── Timeline popup-first Esc handling ─────────────────────────────────
        //
        // Intercept Esc when the popup is open so the popup closes before the
        // outer Esc handler (deselect frame / DevToolsEscape) fires.
        if matches!(key, InputKey::Esc) {
            if popup_open {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineClosePopup { session_id });
                }
            } else if has_selection {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineClearSelection { session_id });
                }
            }
            // Falls through to the outer Esc handler (deselect frame / DevToolsEscape).
        }

        // ── Timeline Enter handling ────────────────────────────────────────────
        //
        // When on the Timeline Events tab:
        //   - With selection and popup closed → open popup.
        //   - Without selection → select first visible event.
        // Falls through to the outer Enter handler when not on TimelineEvents tab.
        if matches!(key, InputKey::Enter) && on_timeline_tab {
            if let Some(session_id) = active_id {
                if has_selection && !popup_open {
                    return Some(Message::TimelineOpenPopup { session_id });
                } else if !has_selection {
                    return Some(Message::TimelineSelectFirstVisible { session_id });
                }
            }
        }

        // Ctrl+C and Esc must NOT be intercepted here — they are global and
        // handled by the main match below.
        match key {
            // ── Section focus cycling ─────────────────────────────────────────
            InputKey::Tab => {
                let next = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.performance.focused_section.next())
                    .unwrap_or_default();
                return Some(Message::PerfFocusSection(next));
            }
            InputKey::BackTab => {
                let prev = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.performance.focused_section.prev())
                    .unwrap_or_default();
                return Some(Message::PerfFocusSection(prev));
            }

            // ── Selection depth/thread nav (Drift #6: MUST appear before PerfScrollUp/Down) ──
            //
            // When on the Timeline Events tab with an event selected, ↑/↓/j/k move
            // the selection cursor (parent/child/thread). Without a selection, these
            // fall through to the PerfScrollUp/PerfScrollDown arms below.
            InputKey::Up | InputKey::Char('k') if has_selection => {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineMoveSelection {
                        session_id,
                        dir: SelectionDirection::ParentOrUpThread,
                    });
                }
            }
            InputKey::Down | InputKey::Char('j') if has_selection => {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineMoveSelection {
                        session_id,
                        dir: SelectionDirection::FirstChildOrDownThread,
                    });
                }
            }

            // ── Row / bar scroll ──────────────────────────────────────────────
            InputKey::Up | InputKey::Char('k') => return Some(Message::PerfScrollUp),
            InputKey::Down | InputKey::Char('j') => return Some(Message::PerfScrollDown),

            // ── Page scroll ───────────────────────────────────────────────────
            InputKey::PageUp => return Some(Message::PerfPageUp),
            InputKey::PageDown => return Some(Message::PerfPageDown),

            // ── Phase 5: Timeline pan/zoom — inserted BEFORE Home/End (Drift #4) ──
            //
            // `+`/`=` zoom in, `-`/`_` zoom out, `g` follow-latest (primary).
            // `End` is a tab-guarded alias for follow-latest; it must appear here
            // BEFORE the unconditional `InputKey::End => PerfJumpToEnd` arm so that
            // pressing End on the TimelineEvents tab emits `TimelineFollowLatest`
            // instead of `PerfJumpToEnd`. The fall-through `_ => {}` lets the End
            // key reach the `PerfJumpToEnd` arm when on other tabs. (Drift #4)
            InputKey::Char('+') | InputKey::Char('=') => {
                let is_timeline_tab = state
                    .session_manager
                    .selected()
                    .filter(|h| h.session.performance.focused_section == PerfSection::Details)
                    .map(|h| h.session.performance.details_tab)
                    .is_some_and(|t| t == PerfDetailsTab::TimelineEvents);
                if is_timeline_tab {
                    if let Some(session_id) = active_id {
                        return Some(Message::TimelineZoomIn { session_id });
                    }
                }
            }
            InputKey::Char('-') | InputKey::Char('_') => {
                let is_timeline_tab = state
                    .session_manager
                    .selected()
                    .filter(|h| h.session.performance.focused_section == PerfSection::Details)
                    .map(|h| h.session.performance.details_tab)
                    .is_some_and(|t| t == PerfDetailsTab::TimelineEvents);
                if is_timeline_tab {
                    if let Some(session_id) = active_id {
                        return Some(Message::TimelineZoomOut { session_id });
                    }
                }
            }
            InputKey::Char('g') => {
                let is_timeline_tab = state
                    .session_manager
                    .selected()
                    .filter(|h| h.session.performance.focused_section == PerfSection::Details)
                    .map(|h| h.session.performance.details_tab)
                    .is_some_and(|t| t == PerfDetailsTab::TimelineEvents);
                if is_timeline_tab {
                    if let Some(session_id) = active_id {
                        return Some(Message::TimelineFollowLatest { session_id });
                    }
                }
            }
            InputKey::End => {
                // Tab-guarded `End` alias for follow-latest (Drift #4).
                // On the TimelineEvents tab: emit TimelineFollowLatest.
                // On other tabs: fall through to the PerfJumpToEnd arm below.
                let is_timeline_tab = state
                    .session_manager
                    .selected()
                    .filter(|h| h.session.performance.focused_section == PerfSection::Details)
                    .map(|h| h.session.performance.details_tab)
                    .is_some_and(|t| t == PerfDetailsTab::TimelineEvents);
                if is_timeline_tab {
                    if let Some(session_id) = active_id {
                        return Some(Message::TimelineFollowLatest { session_id });
                    }
                }
                // Not on TimelineEvents tab — fall through to PerfJumpToEnd.
                return Some(Message::PerfJumpToEnd);
            }

            // ── Jump to oldest / live edge ────────────────────────────────────
            InputKey::Home => return Some(Message::PerfJumpToStart),
            // Note: InputKey::End is handled above with the TimelineEvents guard.

            // ── Phase-3 contextual bindings (Details section only) ───────────
            //
            // `f` cycles the Timeline Events filter (All → UI → Raster → All).
            // `R` (Shift+r) toggles `ext.flutter.profileWidgetBuilds` for Rebuild Stats.
            //
            // IMPORTANT — `R` precedence: these early-returns MUST fire before the
            // main `match key` block where `InputKey::Char('R') if !is_busy` maps
            // to `Message::HotRestart`. The early-return here ensures that pressing
            // `R` on the RebuildStats tab toggles tracking rather than hot-restarting.
            // The regression tests in `performance_sort_key_tests` pin this ordering.
            InputKey::Char('f') => {
                let details_tab = state
                    .session_manager
                    .selected()
                    .filter(|h| h.session.performance.focused_section == PerfSection::Details)
                    .map(|h| h.session.performance.details_tab);
                if let Some(PerfDetailsTab::TimelineEvents) = details_tab {
                    if let Some(session_id) = active_id {
                        return Some(Message::TimelineEventsCycleFilter { session_id });
                    }
                }
            }
            InputKey::Char('R') => {
                let details_tab = state
                    .session_manager
                    .selected()
                    .filter(|h| h.session.performance.focused_section == PerfSection::Details)
                    .map(|h| h.session.performance.details_tab);
                if let Some(PerfDetailsTab::RebuildStats) = details_tab {
                    if let Some(session_id) = active_id {
                        return Some(Message::ToggleRebuildStats { session_id });
                    }
                }
            }

            // ── Details tab cycling (Phase 2) ─────────────────────────────────
            // Only active when the Details section is focused.
            InputKey::Char(']') => {
                let in_details = state
                    .session_manager
                    .selected()
                    .is_some_and(|h| h.session.performance.focused_section == PerfSection::Details);
                if in_details {
                    return Some(Message::PerfCycleDetailsTab { forward: true });
                }
            }
            InputKey::Char('[') => {
                let in_details = state
                    .session_manager
                    .selected()
                    .is_some_and(|h| h.session.performance.focused_section == PerfSection::Details);
                if in_details {
                    return Some(Message::PerfCycleDetailsTab { forward: false });
                }
            }

            // ── Phase 5 T04: Timeline search open (`/`) ──────────────────────
            //
            // On the TimelineEvents tab, `/` opens the search input rather than
            // falling through to the Network panel's `/` → NetworkEnterFilterMode.
            // Placed here (inside `if in_performance`) so it fires before the
            // main match arms.
            InputKey::Char('/') if on_timeline_tab => {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineSearchOpen { session_id });
                }
            }

            // ── Phase 5 T04: Timeline search navigation (`n` / `N`) ──────────
            //
            // Drift #5 — these arms MUST appear before the global `n` →
            // SwitchDevToolsPanel(Network) arm in the main match below.
            // The guards ensure that when no query is active OR the user is not
            // on the TimelineEvents tab, the `n` key falls through to the global
            // Network arm (no regression).
            InputKey::Char('n') if has_query && on_timeline_tab => {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineSearchNextMatch { session_id });
                }
            }
            InputKey::Char('N') if has_query && on_timeline_tab => {
                if let Some(session_id) = active_id {
                    return Some(Message::TimelineSearchPrevMatch { session_id });
                }
            }

            // All other keys fall through to the main match.
            _ => {}
        }
    }

    // ── Memory panel — section navigation and scroll ─────────────────────────
    //
    // These bindings MUST be evaluated before the generic `match key` block so
    // that Tab/Shift+Tab, j/k, Up/Down, PageUp/Down, Home/End are intercepted
    // when the Memory panel is active.
    if in_memory {
        match key {
            InputKey::Tab => {
                let next = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.memory.focused_section.next())
                    .unwrap_or_default();
                return Some(Message::MemFocusSection(next));
            }
            InputKey::BackTab => {
                let prev = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.memory.focused_section.prev())
                    .unwrap_or_default();
                return Some(Message::MemFocusSection(prev));
            }
            InputKey::Up | InputKey::Char('k') => return Some(Message::MemScrollUp),
            InputKey::Down | InputKey::Char('j') => return Some(Message::MemScrollDown),
            InputKey::PageUp => return Some(Message::MemPageUp),
            InputKey::PageDown => return Some(Message::MemPageDown),
            InputKey::Home => return Some(Message::MemJumpToStart),
            InputKey::End => return Some(Message::MemJumpToEnd),
            _ => {}
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
            if in_memory {
                let row_selected = state
                    .session_manager
                    .selected()
                    .map(|h| h.session.memory.alloc_table_selected_row.is_some())
                    .unwrap_or(false);
                if row_selected {
                    return Some(Message::MemSelectAllocRow { index: None });
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
            Some(Message::DevToolsEscape)
        }

        // ── Sub-panel switching ───────────────────────────────────────────────
        InputKey::Char('i') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector)),

        // 'p' always switches to Performance panel.
        InputKey::Char('p') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),

        // 'm' always switches to Memory panel.
        InputKey::Char('m') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Memory)),

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
        //
        // Navigation keys (Up/Down/j/k) are emitted in both tree and details
        // modes; the handler returns no-op when `details_open == true` (selection
        // frozen). See handler/devtools/inspector.rs::handle_inspector_navigate
        // for the guard.
        InputKey::Up | InputKey::Char('k') if in_inspector => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Up))
        }
        InputKey::Down | InputKey::Char('j') if in_inspector => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Down))
        }

        // Enter opens the details view only when in tree mode.  In details
        // mode Enter has no binding so it falls through to None.
        InputKey::Enter if in_inspector && !details_open => {
            Some(Message::DevToolsInspectorOpenDetails)
        }

        // Right expands a tree node in tree mode; in details mode it cycles
        // tabs forward so the arrow keys can navigate tabs without the keyboard.
        InputKey::Right if in_inspector && !details_open => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Expand))
        }
        InputKey::Right if in_inspector && details_open => {
            Some(Message::DevToolsInspectorCycleTab { forward: true })
        }

        // Left cycles tabs backward when details are open; otherwise it
        // collapses the currently selected tree node (same as 'h').
        InputKey::Left if in_inspector && details_open => {
            Some(Message::DevToolsInspectorCycleTab { forward: false })
        }
        InputKey::Left | InputKey::Char('h') if in_inspector && !details_open => {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Collapse))
        }

        // Tab / Shift+Tab cycle Details tabs; only active when Details is open.
        InputKey::Tab if in_inspector && details_open => {
            Some(Message::DevToolsInspectorCycleTab { forward: true })
        }
        InputKey::BackTab if in_inspector && details_open => {
            Some(Message::DevToolsInspectorCycleTab { forward: false })
        }

        // 'H' (Shift+H) toggles the "hide implementation widgets" filter.
        // 'h' (lowercase) is already bound above to tree collapse so there is
        // no conflict; the project convention is case-sensitive `Char('H')`.
        InputKey::Char('H') if in_inspector => {
            Some(Message::DevToolsInspectorToggleHideImplementation)
        }

        // 'r' in Inspector panel refreshes the widget tree.
        InputKey::Char('r') if in_inspector => {
            active_id.map(|session_id| Message::RequestWidgetTree { session_id })
        }

        // ── Memory panel — allocation table sort ──────────────────────────────
        //
        // 's' toggles the allocation table sort column between BySize and
        // ByInstances. This binding is only active in the Memory panel;
        // in the Network panel 's' switches to the ResponseBody sub-tab (handled
        // above with the `in_network` guard), so there is no conflict.
        InputKey::Char('s') if in_memory => Some(Message::MemToggleSort),

        // ── Performance panel frame navigation ────────────────────────────────
        //
        // Left and Right navigate the Timeline Events Gantt depending on selection
        // state (T03), pan the Gantt without selection (T01), or navigate frames
        // in the bar chart when not on the TimelineEvents tab. (Drift #3 + T03)
        //
        // Priority order (all arms share the `in_performance` guard, evaluated
        // in order by Rust):
        //   1. `has_selection && on_timeline_tab` → sibling selection nav (T03).
        //   2. `!has_selection && on_timeline_tab` → pan the Gantt (T01).
        //   3. Otherwise → navigate frame selection (SelectPerformanceFrame).
        //
        // `has_selection` and `on_timeline_tab` were computed before this match
        // block and are captured by the arm guards.

        // 1a. Left with selection → PrevSibling.
        InputKey::Left if in_performance && has_selection => {
            active_id.map(|session_id| Message::TimelineMoveSelection {
                session_id,
                dir: SelectionDirection::PrevSibling,
            })
        }
        // 1b. Right with selection → NextSibling.
        InputKey::Right if in_performance && has_selection => {
            active_id.map(|session_id| Message::TimelineMoveSelection {
                session_id,
                dir: SelectionDirection::NextSibling,
            })
        }
        // 2a. Left without selection on TimelineEvents tab → pan left.
        InputKey::Left if in_performance && on_timeline_tab => {
            active_id.map(|session_id| Message::TimelinePanLeft { session_id })
        }
        // 2b. Right without selection on TimelineEvents tab → pan right.
        InputKey::Right if in_performance && on_timeline_tab => {
            active_id.map(|session_id| Message::TimelinePanRight { session_id })
        }
        // 3. Left/Right on other Performance tabs → frame selection.
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

        // ── Hot restart fallthrough ───────────────────────────────────────────
        //
        // `R` (Shift+r) triggers hot restart in all DevTools contexts EXCEPT
        // Performance/Details/RebuildStats, which is intercepted earlier by the
        // `in_performance` early-return block to emit `ToggleRebuildStats`.
        //
        // This arm preserves muscle-memory: pressing `R` in Inspector, Memory,
        // Network, or Performance-with-FrameChart/FrameAnalysis/TimelineEvents
        // focused all behave the same as in Normal mode.
        InputKey::Char('R') if !is_busy => Some(Message::HotRestart),

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
        InputKey::Char(' ') => Some(Message::NewSessionDialogToggleDeviceSelection),
        InputKey::Char('a') => Some(Message::NewSessionDialogSelectAllDevices),
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
            is_supported: true,
            capabilities: None,
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
            is_supported: true,
            capabilities: None,
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

    /// Create a state with one session in DevTools / Memory panel.
    fn make_state_in_memory_panel() -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Memory;
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
    fn test_s_in_memory_panel_emits_mem_toggle_sort() {
        let state = make_state_in_memory_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('s'));
        assert!(
            matches!(msg, Some(Message::MemToggleSort)),
            "'s' in Memory panel should emit MemToggleSort"
        );
    }

    #[test]
    fn test_s_in_performance_panel_does_not_emit_sort() {
        let state = make_state_in_performance_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('s'));
        // 's' is no longer bound in the Performance panel.
        assert!(
            !matches!(msg, Some(Message::MemToggleSort)),
            "'s' in Performance panel should NOT emit MemToggleSort"
        );
    }

    #[test]
    fn test_s_in_network_panel_emits_response_body_tab() {
        let state = make_state_in_network_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('s'));
        // In the Network panel 's' maps to NetworkSwitchDetailTab(ResponseBody), not MemToggleSort.
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

    #[test]
    fn memory_panel_tab_cycles_memory_section() {
        let state = make_state_in_memory_panel();
        let msg = handle_key_devtools(&state, InputKey::Tab);
        assert!(
            matches!(msg, Some(Message::MemFocusSection(_))),
            "Tab in Memory panel should emit MemFocusSection"
        );
    }

    #[test]
    fn memory_panel_j_emits_mem_scroll_down() {
        let state = make_state_in_memory_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('j'));
        assert!(
            matches!(msg, Some(Message::MemScrollDown)),
            "'j' in Memory panel should emit MemScrollDown"
        );
    }

    #[test]
    fn memory_panel_esc_without_selection_exits() {
        let state = make_state_in_memory_panel();
        let msg = handle_key_devtools(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::DevToolsEscape)),
            "Esc in Memory panel without selection should emit DevToolsEscape"
        );
    }

    #[test]
    fn memory_panel_esc_with_selection_deselects_first() {
        let mut state = make_state_in_memory_panel();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .memory
            .alloc_table_selected_row = Some(3);
        let msg = handle_key_devtools(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::MemSelectAllocRow { index: None })),
            "Esc in Memory panel with row selected should emit MemSelectAllocRow{{index: None}}"
        );
    }

    #[test]
    fn bracket_close_when_details_focused_emits_cycle_forward() {
        let mut state = make_state_in_performance_panel();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section =
                crate::session::performance::PerfSection::Details;
        }
        let msg = handle_key_devtools(&state, InputKey::Char(']'));
        assert!(
            matches!(msg, Some(Message::PerfCycleDetailsTab { forward: true })),
            "']' when Details focused should emit PerfCycleDetailsTab{{forward: true}}"
        );
    }

    #[test]
    fn bracket_open_when_details_focused_emits_cycle_backward() {
        let mut state = make_state_in_performance_panel();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section =
                crate::session::performance::PerfSection::Details;
        }
        let msg = handle_key_devtools(&state, InputKey::Char('['));
        assert!(
            matches!(msg, Some(Message::PerfCycleDetailsTab { forward: false })),
            "'[' when Details focused should emit PerfCycleDetailsTab{{forward: false}}"
        );
    }

    #[test]
    fn bracket_close_when_frame_chart_focused_is_noop() {
        let state = make_state_in_performance_panel();
        // focused_section defaults to FrameChart
        let msg = handle_key_devtools(&state, InputKey::Char(']'));
        // Falls through to the outer match, which has no binding for ']' — None.
        assert!(
            msg.is_none(),
            "']' when FrameChart focused should produce no message"
        );
    }

    #[test]
    fn bracket_open_when_frame_chart_focused_is_noop() {
        let state = make_state_in_performance_panel();
        // focused_section defaults to FrameChart
        let msg = handle_key_devtools(&state, InputKey::Char('['));
        assert!(
            msg.is_none(),
            "'[' when FrameChart focused should produce no message"
        );
    }

    // ── Phase-3 contextual `f` and `R` tests ────────────────────────────────

    /// Helper: state in Performance/Details with a given details_tab.
    fn make_state_in_details(tab: crate::state::PerfDetailsTab) -> AppState {
        let mut state = make_state_in_performance_panel();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section =
                crate::session::performance::PerfSection::Details;
            h.session.performance.details_tab = tab;
        }
        state
    }

    #[test]
    fn test_f_on_timeline_events_tab_emits_filter_cycle() {
        let state = make_state_in_details(crate::state::PerfDetailsTab::TimelineEvents);
        let msg = handle_key_devtools(&state, InputKey::Char('f'));
        assert!(
            matches!(msg, Some(Message::TimelineEventsCycleFilter { .. })),
            "'f' on TimelineEvents tab should emit TimelineEventsCycleFilter; got {msg:?}"
        );
    }

    #[test]
    fn test_f_on_frame_analysis_tab_does_not_emit_filter_cycle() {
        let state = make_state_in_details(crate::state::PerfDetailsTab::FrameAnalysis);
        let msg = handle_key_devtools(&state, InputKey::Char('f'));
        assert!(
            !matches!(msg, Some(Message::TimelineEventsCycleFilter { .. })),
            "'f' on FrameAnalysis tab should NOT emit TimelineEventsCycleFilter; got {msg:?}"
        );
    }

    #[test]
    fn test_f_on_logs_panel_does_not_emit_filter_cycle() {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        // active_panel defaults to Inspector (not Performance)
        let msg = handle_key_devtools(&state, InputKey::Char('f'));
        assert!(
            !matches!(msg, Some(Message::TimelineEventsCycleFilter { .. })),
            "'f' outside Performance panel should NOT emit TimelineEventsCycleFilter; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_rebuild_stats_tab_emits_toggle() {
        let state = make_state_in_details(crate::state::PerfDetailsTab::RebuildStats);
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::ToggleRebuildStats { .. })),
            "'R' on RebuildStats tab should emit ToggleRebuildStats; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_rebuild_stats_tab_does_not_trigger_hot_restart() {
        let state = make_state_in_details(crate::state::PerfDetailsTab::RebuildStats);
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            !matches!(msg, Some(Message::HotRestart)),
            "'R' on RebuildStats tab must NOT emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_frame_analysis_tab_triggers_hot_restart() {
        // `R` with FrameAnalysis tab (not RebuildStats) must emit HotRestart — it falls
        // through the early-return block (which only fires on RebuildStats tab) and hits
        // the global `Char('R') if !is_busy` arm in the main DevTools match.
        let state = make_state_in_details(crate::state::PerfDetailsTab::FrameAnalysis);
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' on FrameAnalysis tab should emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_logs_panel_triggers_hot_restart() {
        // In Normal (logs) mode, `R` must still trigger hot restart.
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::Normal;
        let msg = handle_key(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' in Normal (logs) mode should emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_memory_panel_triggers_hot_restart() {
        // `R` in DevTools/Memory panel must emit HotRestart — falls through the
        // `in_performance` early-return block and hits the global `Char('R') if !is_busy`
        // arm in the main DevTools match.
        let state = make_state_in_memory_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' on Memory panel should emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_inspector_panel_triggers_hot_restart() {
        // `R` in DevTools/Inspector panel must emit HotRestart (fallthrough to global arm).
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' on Inspector panel should emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_network_panel_triggers_hot_restart() {
        // `R` in DevTools/Network panel must emit HotRestart (fallthrough to global arm).
        let state = make_state_in_network_panel();
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' on Network panel should emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_frame_chart_focused_triggers_hot_restart() {
        // `R` in Performance panel with FrameChart focused must emit HotRestart.
        // The `in_performance` early-return only fires when Details is focused AND
        // on the RebuildStats tab; FrameChart focus falls through to the global arm.
        let state = make_state_in_performance_panel();
        // focused_section defaults to FrameChart
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' in Performance/FrameChart should emit HotRestart; got {msg:?}"
        );
    }

    #[test]
    fn test_capital_r_on_timeline_events_tab_triggers_hot_restart() {
        // `R` in Performance/Details/TimelineEvents must emit HotRestart.
        let state = make_state_in_details(crate::state::PerfDetailsTab::TimelineEvents);
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::HotRestart)),
            "'R' on TimelineEvents tab should emit HotRestart; got {msg:?}"
        );
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
            is_supported: true,
            capabilities: None,
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

#[cfg(test)]
mod dap_key_tests {
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
            is_supported: true,
            capabilities: None,
        }
    }

    #[test]
    fn test_d_key_sends_toggle_dap() {
        let state = AppState::new();
        let result = handle_key_normal(&state, InputKey::Char('D'));
        assert!(
            matches!(result, Some(Message::ToggleDap)),
            "'D' in Normal mode should emit Message::ToggleDap"
        );
    }

    #[test]
    fn test_d_key_works_without_active_session() {
        // No sessions created — session_manager is empty
        let state = AppState::new();
        assert!(
            state.session_manager.selected().is_none(),
            "Test precondition: no active session"
        );
        let result = handle_key_normal(&state, InputKey::Char('D'));
        assert!(
            matches!(result, Some(Message::ToggleDap)),
            "'D' should emit ToggleDap regardless of session state"
        );
    }

    #[test]
    fn test_d_key_works_with_active_session() {
        let mut state = AppState::new();
        let device = test_device();
        state.session_manager.create_session(&device).unwrap();
        let result = handle_key_normal(&state, InputKey::Char('D'));
        assert!(
            matches!(result, Some(Message::ToggleDap)),
            "'D' should emit ToggleDap even when a session is active"
        );
    }

    #[test]
    fn test_lowercase_d_requires_session() {
        // Lowercase 'd' (DevTools) still requires an active session
        let state = AppState::new();
        let result = handle_key_normal(&state, InputKey::Char('d'));
        assert!(
            result.is_none(),
            "'d' (DevTools) should return None when no session is active"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tag filter overlay key handling (Phase 2, Task 09 + fix Task 04)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tag_filter_key_tests {
    use super::*;

    #[test]
    fn test_tag_filter_ctrl_c_quits() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::CharCtrl('c'));
        assert!(
            matches!(result, Some(Message::Quit)),
            "Ctrl+C while tag filter overlay is open should emit Message::Quit"
        );
    }

    #[test]
    fn test_tag_filter_esc_hides_overlay() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Esc);
        assert!(
            matches!(result, Some(Message::HideTagFilter)),
            "Esc while tag filter overlay is open should emit Message::HideTagFilter"
        );
    }

    #[test]
    fn test_tag_filter_t_hides_overlay() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Char('t'));
        assert!(
            matches!(result, Some(Message::HideTagFilter)),
            "'t' while tag filter overlay is open should emit Message::HideTagFilter"
        );
    }

    #[test]
    fn test_tag_filter_j_moves_down() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Char('j'));
        assert!(
            matches!(result, Some(Message::TagFilterMoveDown)),
            "'j' while tag filter overlay is open should emit Message::TagFilterMoveDown"
        );
    }

    #[test]
    fn test_tag_filter_k_moves_up() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Char('k'));
        assert!(
            matches!(result, Some(Message::TagFilterMoveUp)),
            "'k' while tag filter overlay is open should emit Message::TagFilterMoveUp"
        );
    }

    #[test]
    fn test_tag_filter_space_toggles_selected() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Char(' '));
        assert!(
            matches!(result, Some(Message::TagFilterToggleSelected)),
            "Space while tag filter overlay is open should emit Message::TagFilterToggleSelected"
        );
    }

    #[test]
    fn test_tag_filter_a_shows_all() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Char('a'));
        assert!(
            matches!(result, Some(Message::ShowAllNativeTags)),
            "'a' while tag filter overlay is open should emit Message::ShowAllNativeTags"
        );
    }

    #[test]
    fn test_tag_filter_n_hides_all() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let result = handle_key_normal(&state, InputKey::Char('n'));
        assert!(
            matches!(result, Some(Message::HideAllNativeTags)),
            "'n' while tag filter overlay is open should emit Message::HideAllNativeTags"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Flutter version panel key handling tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod flutter_version_key_tests {
    use super::*;

    fn fv_state() -> AppState {
        let mut state = AppState::new();
        state.ui_mode = UiMode::FlutterVersion;
        state
    }

    #[test]
    fn test_v_key_in_normal_opens_panel() {
        let state = AppState::new();
        let msg = handle_key(&state, InputKey::Char('V'));
        assert!(matches!(msg, Some(Message::ShowFlutterVersion)));
    }

    #[test]
    fn test_escape_closes_panel() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Esc);
        assert!(matches!(msg, Some(Message::FlutterVersionEscape)));
    }

    #[test]
    fn test_tab_switches_pane() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Tab);
        assert!(matches!(msg, Some(Message::FlutterVersionSwitchPane)));
    }

    #[test]
    fn test_j_navigates_down() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('j'));
        assert!(matches!(msg, Some(Message::FlutterVersionDown)));
    }

    #[test]
    fn test_down_arrow_navigates_down() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Down);
        assert!(matches!(msg, Some(Message::FlutterVersionDown)));
    }

    #[test]
    fn test_k_navigates_up() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('k'));
        assert!(matches!(msg, Some(Message::FlutterVersionUp)));
    }

    #[test]
    fn test_up_arrow_navigates_up() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Up);
        assert!(matches!(msg, Some(Message::FlutterVersionUp)));
    }

    #[test]
    fn test_enter_switches_version() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Enter);
        assert!(matches!(msg, Some(Message::FlutterVersionSwitch)));
    }

    #[test]
    fn test_d_removes_version() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('d'));
        assert!(matches!(msg, Some(Message::FlutterVersionRemove)));
    }

    #[test]
    fn test_i_installs_version() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('i'));
        assert!(matches!(msg, Some(Message::FlutterVersionInstall)));
    }

    #[test]
    fn test_u_updates_version() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('u'));
        assert!(matches!(msg, Some(Message::FlutterVersionUpdate)));
    }

    #[test]
    fn test_ctrl_c_quits() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::CharCtrl('c'));
        assert!(matches!(msg, Some(Message::Quit)));
    }

    #[test]
    fn test_unmapped_key_returns_none() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('z'));
        assert!(msg.is_none());
    }

    #[test]
    fn test_unmapped_backtab_returns_none() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::BackTab);
        assert!(msg.is_none());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inspector Phase-1 key binding tests (task 06)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod inspector_phase1_key_tests {
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
            is_supported: true,
            capabilities: None,
        }
    }

    /// Build a state that is in DevTools / Inspector panel.
    ///
    /// `details_open` controls whether the Details pane is currently visible.
    fn make_state_in_inspector_tab(details_open: bool) -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
        state.devtools_view_state.inspector.details_open = details_open;
        state
    }

    // ── Enter ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_enter_in_inspector_tree_mode_emits_open_details() {
        let state = make_state_in_inspector_tab(false);
        let msg = handle_key_devtools(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::DevToolsInspectorOpenDetails)),
            "Enter in tree mode should emit DevToolsInspectorOpenDetails"
        );
    }

    #[test]
    fn test_enter_in_inspector_details_mode_is_unbound() {
        let state = make_state_in_inspector_tab(true);
        let msg = handle_key_devtools(&state, InputKey::Enter);
        assert!(
            msg.is_none(),
            "Enter in details mode should return None (no binding)"
        );
    }

    // ── H / h ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_uppercase_h_in_inspector_emits_toggle_hide_implementation() {
        let state = make_state_in_inspector_tab(false);
        let msg = handle_key_devtools(&state, InputKey::Char('H'));
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorToggleHideImplementation)
            ),
            "Uppercase H in Inspector should emit DevToolsInspectorToggleHideImplementation"
        );
    }

    #[test]
    fn test_lowercase_h_in_inspector_still_emits_collapse() {
        // Regression guard: 'h' must remain bound to Collapse even after adding 'H'.
        let state = make_state_in_inspector_tab(false);
        let msg = handle_key_devtools(&state, InputKey::Char('h'));
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorNavigate(InspectorNav::Collapse))
            ),
            "'h' (lowercase) in Inspector tree mode should still emit Collapse"
        );
    }

    // ── Tab / BackTab ─────────────────────────────────────────────────────────

    #[test]
    fn test_tab_in_inspector_details_mode_emits_cycle_tab_forward() {
        let state = make_state_in_inspector_tab(true);
        let msg = handle_key_devtools(&state, InputKey::Tab);
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorCycleTab { forward: true })
            ),
            "Tab in details mode should emit CycleTab {{ forward: true }}"
        );
    }

    #[test]
    fn test_back_tab_in_inspector_details_mode_emits_cycle_tab_backward() {
        let state = make_state_in_inspector_tab(true);
        let msg = handle_key_devtools(&state, InputKey::BackTab);
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorCycleTab { forward: false })
            ),
            "BackTab in details mode should emit CycleTab {{ forward: false }}"
        );
    }

    #[test]
    fn test_tab_in_inspector_tree_mode_is_unbound() {
        let state = make_state_in_inspector_tab(false);
        let msg = handle_key_devtools(&state, InputKey::Tab);
        assert!(
            msg.is_none(),
            "Tab in tree mode (details closed) should return None"
        );
    }

    // ── Left / Right in details mode ──────────────────────────────────────────

    #[test]
    fn test_left_in_inspector_details_mode_emits_cycle_tab_backward() {
        let state = make_state_in_inspector_tab(true);
        let msg = handle_key_devtools(&state, InputKey::Left);
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorCycleTab { forward: false })
            ),
            "Left arrow in details mode should emit CycleTab {{ forward: false }}"
        );
    }

    #[test]
    fn test_right_in_inspector_details_mode_emits_cycle_tab_forward() {
        let state = make_state_in_inspector_tab(true);
        let msg = handle_key_devtools(&state, InputKey::Right);
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorCycleTab { forward: true })
            ),
            "Right arrow in details mode should emit CycleTab {{ forward: true }}"
        );
    }

    // ── Right in tree mode still expands ─────────────────────────────────────

    #[test]
    fn test_right_in_inspector_tree_mode_emits_expand() {
        let state = make_state_in_inspector_tab(false);
        let msg = handle_key_devtools(&state, InputKey::Right);
        assert!(
            matches!(
                msg,
                Some(Message::DevToolsInspectorNavigate(InspectorNav::Expand))
            ),
            "Right arrow in tree mode should still emit Expand"
        );
    }
}

#[cfg(test)]
mod memory_panel_key_tests {
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
            is_supported: true,
            capabilities: None,
        }
    }

    #[test]
    fn key_m_switches_to_memory_panel() {
        let mut state = AppState::new();
        let device = test_device();
        let _session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

        let msg = handle_key_devtools(&state, InputKey::Char('m'));
        assert!(
            matches!(
                msg,
                Some(Message::SwitchDevToolsPanel(DevToolsPanel::Memory))
            ),
            "'m' in DevTools mode should emit SwitchDevToolsPanel(Memory), got: {msg:?}"
        );
    }
}

// ── Phase 5: Timeline pan/zoom keybinding conflict guard tests ────────────────
//
// Validates Drift #3 and #4: ←/→ pan only on TimelineEvents tab; Left/Right
// still emit SelectPerformanceFrame on other tabs/sections. `End` emits
// TimelineFollowLatest on TimelineEvents tab and PerfJumpToEnd elsewhere.

#[cfg(test)]
mod timeline_pan_zoom_key_tests {
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
            is_supported: true,
            capabilities: None,
        }
    }

    /// Performance panel state with FrameChart section focused (default).
    fn make_perf_frame_chart() -> AppState {
        let mut state = AppState::new();
        let _id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        // Default: focused_section = FrameChart
        state
    }

    /// Performance panel state with Details/TimelineEvents focused.
    fn make_perf_timeline_events() -> AppState {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        if let Some(h) = state.session_manager.get_mut(id) {
            h.session.performance.focused_section =
                crate::session::performance::PerfSection::Details;
            h.session.performance.details_tab = PerfDetailsTab::TimelineEvents;
        }
        state
    }

    /// Performance panel state with Details/FrameAnalysis focused.
    fn make_perf_frame_analysis() -> AppState {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        if let Some(h) = state.session_manager.get_mut(id) {
            h.session.performance.focused_section =
                crate::session::performance::PerfSection::Details;
            h.session.performance.details_tab = PerfDetailsTab::FrameAnalysis;
        }
        state
    }

    /// Drift #3: Left on FrameChart section → SelectPerformanceFrame (not pan).
    #[test]
    fn test_left_on_frame_chart_still_selects_frame() {
        let state = make_perf_frame_chart();
        let msg = handle_key_devtools(&state, InputKey::Left);
        assert!(
            matches!(msg, Some(Message::SelectPerformanceFrame { .. })),
            "Left on FrameChart should emit SelectPerformanceFrame, got: {msg:?}"
        );
    }

    /// Drift #3: Left on Details/FrameAnalysis tab → SelectPerformanceFrame (not pan).
    #[test]
    fn test_left_on_frame_analysis_tab_still_selects_frame() {
        let state = make_perf_frame_analysis();
        let msg = handle_key_devtools(&state, InputKey::Left);
        assert!(
            matches!(msg, Some(Message::SelectPerformanceFrame { .. })),
            "Left on Details/FrameAnalysis should emit SelectPerformanceFrame, got: {msg:?}"
        );
    }

    /// Drift #3: Left on Details/TimelineEvents tab → TimelinePanLeft (not frame select).
    #[test]
    fn test_left_on_timeline_events_tab_pans() {
        let state = make_perf_timeline_events();
        let msg = handle_key_devtools(&state, InputKey::Left);
        assert!(
            matches!(msg, Some(Message::TimelinePanLeft { .. })),
            "Left on TimelineEvents should emit TimelinePanLeft, got: {msg:?}"
        );
    }

    /// Drift #4: End on FrameChart section → PerfJumpToEnd (not follow-latest).
    #[test]
    fn test_end_on_frame_chart_jumps_to_end() {
        let state = make_perf_frame_chart();
        let msg = handle_key_devtools(&state, InputKey::End);
        assert!(
            matches!(msg, Some(Message::PerfJumpToEnd)),
            "End on FrameChart should emit PerfJumpToEnd, got: {msg:?}"
        );
    }

    /// Drift #4: End on Details/TimelineEvents tab → TimelineFollowLatest (not jump).
    #[test]
    fn test_end_on_timeline_events_follow_latest() {
        let state = make_perf_timeline_events();
        let msg = handle_key_devtools(&state, InputKey::End);
        assert!(
            matches!(msg, Some(Message::TimelineFollowLatest { .. })),
            "End on TimelineEvents should emit TimelineFollowLatest, got: {msg:?}"
        );
    }

    // ── Phase 5 T03: Key ordering tests (Drift #6) ───────────────────────────

    /// Build a Timeline Events state where `timeline_selected_event` is set.
    fn make_perf_timeline_events_with_selection() -> AppState {
        use crate::session::TimelineEventCursor;
        let mut state = make_perf_timeline_events();
        let id = state
            .session_manager
            .selected_id()
            .expect("should have a session");
        if let Some(h) = state.session_manager.get_mut(id) {
            h.session.performance.timeline_selected_event = Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000_000,
            });
        }
        state
    }

    /// Drift #6: Down on Details/TimelineEvents WITHOUT selection → PerfScrollDown.
    #[test]
    fn test_down_on_timeline_events_without_selection_scrolls() {
        let state = make_perf_timeline_events();
        // No selection — Down should scroll.
        let msg = handle_key_devtools(&state, InputKey::Down);
        assert!(
            matches!(msg, Some(Message::PerfScrollDown)),
            "Down on TimelineEvents without selection should emit PerfScrollDown, got: {msg:?}"
        );
    }

    /// Drift #6: Down on Details/TimelineEvents WITH selection → TimelineMoveSelection.
    #[test]
    fn test_down_on_timeline_events_with_selection_moves_cursor() {
        use crate::session::performance::SelectionDirection;
        let state = make_perf_timeline_events_with_selection();
        let msg = handle_key_devtools(&state, InputKey::Down);
        assert!(
            matches!(
                msg,
                Some(Message::TimelineMoveSelection {
                    dir: SelectionDirection::FirstChildOrDownThread,
                    ..
                })
            ),
            "Down on TimelineEvents with selection should emit TimelineMoveSelection(FirstChildOrDownThread), got: {msg:?}"
        );
    }

    /// T03: Left on TimelineEvents WITH selection → TimelineMoveSelection(PrevSibling).
    #[test]
    fn test_left_on_timeline_events_with_selection_moves_prev_sibling() {
        use crate::session::performance::SelectionDirection;
        let state = make_perf_timeline_events_with_selection();
        let msg = handle_key_devtools(&state, InputKey::Left);
        assert!(
            matches!(
                msg,
                Some(Message::TimelineMoveSelection {
                    dir: SelectionDirection::PrevSibling,
                    ..
                })
            ),
            "Left on TimelineEvents with selection should emit TimelineMoveSelection(PrevSibling), got: {msg:?}"
        );
    }

    /// T03: Left on TimelineEvents WITHOUT selection → TimelinePanLeft.
    #[test]
    fn test_left_on_timeline_events_without_selection_pans() {
        let state = make_perf_timeline_events();
        let msg = handle_key_devtools(&state, InputKey::Left);
        assert!(
            matches!(msg, Some(Message::TimelinePanLeft { .. })),
            "Left on TimelineEvents without selection should emit TimelinePanLeft, got: {msg:?}"
        );
    }

    /// T03: Enter on TimelineEvents WITHOUT selection → TimelineSelectFirstVisible.
    #[test]
    fn test_enter_on_timeline_events_without_selection_selects_first() {
        let state = make_perf_timeline_events();
        let msg = handle_key_devtools(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::TimelineSelectFirstVisible { .. })),
            "Enter on TimelineEvents without selection should emit TimelineSelectFirstVisible, got: {msg:?}"
        );
    }

    /// T03: Enter on TimelineEvents WITH selection (popup closed) → TimelineOpenPopup.
    #[test]
    fn test_enter_on_timeline_events_with_selection_opens_popup() {
        let state = make_perf_timeline_events_with_selection();
        let msg = handle_key_devtools(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::TimelineOpenPopup { .. })),
            "Enter on TimelineEvents with selection should emit TimelineOpenPopup, got: {msg:?}"
        );
    }

    // ── Phase 5 T04: Timeline search key binding tests (Drift #5) ────────────

    /// Build a Timeline Events state with a search query set (committed, not active).
    fn make_perf_timeline_events_with_query(query: &str) -> AppState {
        let mut state = make_perf_timeline_events();
        let id = state
            .session_manager
            .selected_id()
            .expect("should have a session");
        if let Some(h) = state.session_manager.get_mut(id) {
            h.session.performance.timeline_search_query = Some(query.to_string());
            h.session.performance.timeline_search_input_active = false;
        }
        state
    }

    /// Build a Timeline Events state with search input active.
    fn make_perf_timeline_events_search_input_active() -> AppState {
        let mut state = make_perf_timeline_events();
        let id = state
            .session_manager
            .selected_id()
            .expect("should have a session");
        if let Some(h) = state.session_manager.get_mut(id) {
            h.session.performance.timeline_search_query = Some(String::new());
            h.session.performance.timeline_search_input_active = true;
        }
        state
    }

    /// AC2: `/` on TimelineEvents tab → TimelineSearchOpen.
    #[test]
    fn test_slash_on_timeline_events_opens_search() {
        let state = make_perf_timeline_events();
        let msg = handle_key_devtools(&state, InputKey::Char('/'));
        assert!(
            matches!(msg, Some(Message::TimelineSearchOpen { .. })),
            "'/' on TimelineEvents tab should emit TimelineSearchOpen, got: {msg:?}"
        );
    }

    /// AC8 / Drift #5: `n` with no query on TimelineEvents tab → SwitchDevToolsPanel(Network).
    #[test]
    fn test_n_with_no_query_on_timeline_tab_switches_to_network() {
        let state = make_perf_timeline_events(); // no query
        let msg = handle_key_devtools(&state, InputKey::Char('n'));
        assert!(
            matches!(
                msg,
                Some(Message::SwitchDevToolsPanel(DevToolsPanel::Network))
            ),
            "'n' with no query on TimelineEvents should switch to Network, got: {msg:?}"
        );
    }

    /// AC6 / Drift #5: `n` with query on TimelineEvents tab → TimelineSearchNextMatch.
    #[test]
    fn test_n_with_query_on_timeline_tab_next_match() {
        let state = make_perf_timeline_events_with_query("foo");
        let msg = handle_key_devtools(&state, InputKey::Char('n'));
        assert!(
            matches!(msg, Some(Message::TimelineSearchNextMatch { .. })),
            "'n' with query on TimelineEvents should emit TimelineSearchNextMatch, got: {msg:?}"
        );
    }

    /// AC8 / Drift #5: `n` with query but on FrameChart section → SwitchDevToolsPanel(Network).
    /// The `on_timeline_tab` guard must defeat the search arm when not on TimelineEvents.
    #[test]
    fn test_n_with_query_on_frame_chart_switches_to_network() {
        let mut state = make_perf_frame_chart();
        let id = state
            .session_manager
            .selected_id()
            .expect("should have a session");
        if let Some(h) = state.session_manager.get_mut(id) {
            // Set a query, but the user is on FrameChart (not TimelineEvents).
            h.session.performance.timeline_search_query = Some("foo".to_string());
            h.session.performance.timeline_search_input_active = false;
        }
        let msg = handle_key_devtools(&state, InputKey::Char('n'));
        assert!(
            matches!(
                msg,
                Some(Message::SwitchDevToolsPanel(DevToolsPanel::Network))
            ),
            "'n' with query on FrameChart should still switch to Network, got: {msg:?}"
        );
    }

    /// AC7: `N` with query on TimelineEvents tab → TimelineSearchPrevMatch.
    #[test]
    fn test_shift_n_with_query_on_timeline_tab_prev_match() {
        let state = make_perf_timeline_events_with_query("foo");
        let msg = handle_key_devtools(&state, InputKey::Char('N'));
        assert!(
            matches!(msg, Some(Message::TimelineSearchPrevMatch { .. })),
            "'N' with query on TimelineEvents should emit TimelineSearchPrevMatch, got: {msg:?}"
        );
    }

    /// AC3: Char keys while search input active → TimelineSearchInputChar.
    #[test]
    fn test_char_while_search_input_active_appends() {
        let state = make_perf_timeline_events_search_input_active();
        let msg = handle_key_devtools(&state, InputKey::Char('R'));
        assert!(
            matches!(msg, Some(Message::TimelineSearchInputChar { ch: 'R', .. })),
            "Char while search input active should emit TimelineSearchInputChar, got: {msg:?}"
        );
    }

    /// AC3: Backspace while search input active → TimelineSearchInputBackspace.
    #[test]
    fn test_backspace_while_search_input_active_deletes() {
        let state = make_perf_timeline_events_search_input_active();
        let msg = handle_key_devtools(&state, InputKey::Backspace);
        assert!(
            matches!(msg, Some(Message::TimelineSearchInputBackspace { .. })),
            "Backspace while search input active should emit TimelineSearchInputBackspace, got: {msg:?}"
        );
    }

    /// AC4: Enter while search input active → TimelineSearchInputCommit.
    #[test]
    fn test_enter_while_search_input_active_commits() {
        let state = make_perf_timeline_events_search_input_active();
        let msg = handle_key_devtools(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::TimelineSearchInputCommit { .. })),
            "Enter while search input active should emit TimelineSearchInputCommit, got: {msg:?}"
        );
    }

    /// AC5: Esc while search input active → TimelineSearchInputCancel.
    #[test]
    fn test_esc_while_search_input_active_cancels() {
        let state = make_perf_timeline_events_search_input_active();
        let msg = handle_key_devtools(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::TimelineSearchInputCancel { .. })),
            "Esc while search input active should emit TimelineSearchInputCancel, got: {msg:?}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Multi-select key routing — TargetSelector pane (Phase 1, Task 02)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod target_selector_multiselect_key_tests {
    use super::*;
    use crate::config::LoadedConfigs;
    use crate::new_session_dialog::DialogPane;
    use crate::state::{AppState, UiMode};
    use std::path::PathBuf;

    fn make_new_session_dialog_target_selector() -> AppState {
        let mut state = AppState::with_settings(
            PathBuf::from("/test/project"),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());
        // Ensure TargetSelector pane is focused
        state.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;
        state
    }

    #[test]
    fn space_key_maps_to_toggle_selection() {
        let state = make_new_session_dialog_target_selector();
        let msg = handle_key(&state, InputKey::Char(' '));
        assert!(
            matches!(msg, Some(Message::NewSessionDialogToggleDeviceSelection)),
            "Space in TargetSelector should emit NewSessionDialogToggleDeviceSelection, got: {msg:?}"
        );
    }

    #[test]
    fn a_key_maps_to_select_all() {
        let state = make_new_session_dialog_target_selector();
        let msg = handle_key(&state, InputKey::Char('a'));
        assert!(
            matches!(msg, Some(Message::NewSessionDialogSelectAllDevices)),
            "'a' in TargetSelector should emit NewSessionDialogSelectAllDevices, got: {msg:?}"
        );
    }

    #[test]
    fn up_down_enter_r_unchanged() {
        let state = make_new_session_dialog_target_selector();
        assert!(matches!(
            handle_key(&state, InputKey::Up),
            Some(Message::NewSessionDialogDeviceUp)
        ));
        assert!(matches!(
            handle_key(&state, InputKey::Down),
            Some(Message::NewSessionDialogDeviceDown)
        ));
        assert!(matches!(
            handle_key(&state, InputKey::Enter),
            Some(Message::NewSessionDialogDeviceSelect)
        ));
        assert!(matches!(
            handle_key(&state, InputKey::Char('r')),
            Some(Message::NewSessionDialogRefreshDevices)
        ));
    }
}

#[cfg(test)]
mod install_wizard_key_tests {
    use super::*;
    use crate::state::{AppState, UiMode};
    use std::path::PathBuf;

    fn make_install_wizard_state() -> AppState {
        let mut state = AppState::with_settings(
            PathBuf::from("/test/project"),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::InstallWizard;
        state
    }

    /// Acceptance criterion (Task 05): `Enter` in `UiMode::InstallWizard`
    /// produces `Message::InstallWizardRunSelectedStep`.
    #[test]
    fn test_enter_in_install_wizard_runs_selected_step() {
        let state = make_install_wizard_state();
        let msg = handle_key(&state, InputKey::Enter);
        assert!(
            matches!(msg, Some(Message::InstallWizardRunSelectedStep)),
            "Enter in InstallWizard should emit InstallWizardRunSelectedStep, got: {msg:?}"
        );
    }

    /// Esc while idle (no step running) must close the wizard.
    #[test]
    fn test_esc_while_idle_closes_wizard() {
        let state = make_install_wizard_state();
        // No step started → is_step_running() == false → InstallWizardEscape.
        assert!(
            !state.install_wizard_state.is_step_running(),
            "precondition: no step running"
        );
        let msg = handle_key(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::InstallWizardEscape)),
            "Esc while idle must emit InstallWizardEscape, got: {msg:?}"
        );
    }

    /// Esc while a step is running must cancel, not close.
    #[test]
    fn esc_while_running_cancels_not_closes() {
        let mut state = make_install_wizard_state();
        // Simulate a running step.
        state
            .install_wizard_state
            .begin_step(crate::install_wizard::WizardStepKind::FlutterSdk);
        assert!(
            state.install_wizard_state.is_step_running(),
            "precondition: step must be running"
        );
        let msg = handle_key(&state, InputKey::Esc);
        assert!(
            matches!(msg, Some(Message::InstallWizardCancelStep)),
            "Esc while running must emit InstallWizardCancelStep, got: {msg:?}"
        );
        // Wizard must remain open (the message closes nothing — cancel handler
        // resets execution to Idle without changing UiMode).
        assert_eq!(
            state.ui_mode,
            crate::state::UiMode::InstallWizard,
            "ui_mode must still be InstallWizard after Esc-cancel"
        );
    }

    #[test]
    fn test_tab_in_install_wizard_switches_pane() {
        let state = make_install_wizard_state();
        let msg = handle_key(&state, InputKey::Tab);
        assert!(
            matches!(msg, Some(Message::InstallWizardSwitchPane)),
            "Tab in InstallWizard should emit InstallWizardSwitchPane, got: {msg:?}"
        );
    }

    #[test]
    fn test_r_in_install_wizard_reruns_preflight() {
        let state = make_install_wizard_state();
        let msg = handle_key(&state, InputKey::Char('r'));
        assert!(
            matches!(msg, Some(Message::InstallWizardRerunPreflight)),
            "'r' in InstallWizard should emit InstallWizardRerunPreflight, got: {msg:?}"
        );
    }

    #[test]
    fn test_ctrl_c_in_install_wizard_quits() {
        let state = make_install_wizard_state();
        let msg = handle_key(&state, InputKey::CharCtrl('c'));
        assert!(
            matches!(msg, Some(Message::Quit)),
            "Ctrl+C in InstallWizard should emit Quit, got: {msg:?}"
        );
    }

    /// Acceptance criterion (Task 04): `c` in `UiMode::InstallWizard`
    /// produces `Message::InstallWizardCopyCommand`.
    #[test]
    fn test_c_in_install_wizard_emits_copy_command() {
        let mut state = AppState::with_settings(
            PathBuf::from("/test/project"),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::InstallWizard;
        let msg = handle_key(&state, InputKey::Char('c'));
        assert!(
            matches!(msg, Some(Message::InstallWizardCopyCommand)),
            "'c' in InstallWizard should emit InstallWizardCopyCommand, got: {msg:?}"
        );
    }

    #[test]
    fn test_bracket_open_in_install_wizard_emits_prev_command() {
        let state = make_install_wizard_state();
        let msg = handle_key(&state, InputKey::Char('['));
        assert!(
            matches!(msg, Some(Message::InstallWizardPrevCommand)),
            "'[' in InstallWizard should emit InstallWizardPrevCommand, got: {msg:?}"
        );
    }

    #[test]
    fn test_bracket_close_in_install_wizard_emits_next_command() {
        let state = make_install_wizard_state();
        let msg = handle_key(&state, InputKey::Char(']'));
        assert!(
            matches!(msg, Some(Message::InstallWizardNextCommand)),
            "']' in InstallWizard should emit InstallWizardNextCommand, got: {msg:?}"
        );
    }
}
