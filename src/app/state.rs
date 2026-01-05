//! Application state (Model in TEA pattern)

use std::path::PathBuf;

use crate::config::{Settings, SettingsTab, UserPreferences};
use crate::core::AppPhase;
use crate::tui::widgets::{ConfirmDialogState, DeviceSelectorState};

use super::session_manager::SessionManager;

/// Current UI mode/screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// Normal TUI with log view and status bar
    #[default]
    Normal,

    /// Device selector modal is active
    DeviceSelector,

    /// Emulator selector (after choosing "Launch Android Emulator")
    EmulatorSelector,

    /// Confirmation dialog (e.g., quit confirmation)
    ConfirmDialog,

    /// Initial loading screen (discovering devices)
    Loading,

    /// Search input mode - capturing text for log search
    SearchInput,

    /// Link highlight mode - showing clickable file references
    /// User can press 1-9 or a-z to open a file in their editor
    LinkHighlight,

    /// Settings panel - full-screen settings UI
    Settings,
}

/// State for the settings panel view
#[derive(Debug, Clone)]
pub struct SettingsViewState {
    /// Currently active tab
    pub active_tab: SettingsTab,

    /// Currently selected item index within the active tab
    pub selected_index: usize,

    /// Whether we're in edit mode for the current item
    pub editing: bool,

    /// Text buffer for string editing
    pub edit_buffer: String,

    /// Dirty flag - have settings been modified?
    pub dirty: bool,

    /// Loaded user preferences (for User tab)
    pub user_prefs: UserPreferences,

    /// Error message to display (if any)
    pub error: Option<String>,
}

impl Default for SettingsViewState {
    fn default() -> Self {
        Self {
            active_tab: SettingsTab::Project,
            selected_index: 0,
            editing: false,
            edit_buffer: String::new(),
            dirty: false,
            user_prefs: UserPreferences::default(),
            error: None,
        }
    }
}

impl SettingsViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load user preferences from disk
    pub fn load_user_prefs(&mut self, project_path: &std::path::Path) {
        if let Some(prefs) = crate::config::load_user_preferences(project_path) {
            self.user_prefs = prefs;
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Jump to specific tab
    pub fn goto_tab(&mut self, tab: SettingsTab) {
        self.active_tab = tab;
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Select next item
    pub fn select_next(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = (self.selected_index + 1) % item_count;
        }
    }

    /// Select previous item
    pub fn select_previous(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = if self.selected_index == 0 {
                item_count - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Enter edit mode
    pub fn start_editing(&mut self, initial_value: &str) {
        self.editing = true;
        self.edit_buffer = initial_value.to_string();
    }

    /// Exit edit mode
    pub fn stop_editing(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Mark settings as modified
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear dirty flag (after save)
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Complete application state (the Model in TEA)
#[derive(Debug)]
pub struct AppState {
    /// Current UI mode/screen
    pub ui_mode: UiMode,

    /// Session manager for multi-instance support
    pub session_manager: SessionManager,

    /// Device selector state
    pub device_selector: DeviceSelectorState,

    /// Application settings from config file
    pub settings: Settings,

    /// Confirmation dialog state
    pub confirm_dialog_state: Option<ConfirmDialogState>,

    /// Project path
    pub project_path: PathBuf,

    /// Project name from pubspec.yaml (cached at startup)
    pub project_name: Option<String>,

    /// Current application phase (used for app-level quitting state)
    pub phase: AppPhase,

    /// Settings view state (for Settings UI mode)
    pub settings_view_state: SettingsViewState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new AppState with default settings (for backward compatibility)
    pub fn new() -> Self {
        Self::with_settings(PathBuf::new(), Settings::default())
    }

    /// Create a new AppState with project path and settings
    pub fn with_settings(project_path: PathBuf, settings: Settings) -> Self {
        // Parse project name from pubspec.yaml
        let project_name = crate::core::get_project_name(&project_path);

        Self {
            ui_mode: UiMode::Normal,
            session_manager: SessionManager::new(),
            device_selector: DeviceSelectorState::new(),
            settings,
            confirm_dialog_state: None,
            project_path,
            project_name,
            phase: AppPhase::Initializing,
            settings_view_state: SettingsViewState::new(),
        }
    }

    // ─────────────────────────────────────────────────────────
    // UI Mode Helpers
    // ─────────────────────────────────────────────────────────

    /// Show device selector modal
    pub fn show_device_selector(&mut self) {
        self.ui_mode = UiMode::DeviceSelector;
        self.device_selector.show_loading();
    }

    /// Hide device selector modal
    pub fn hide_device_selector(&mut self) {
        self.device_selector.hide();
        self.ui_mode = UiMode::Normal;
    }

    /// Show settings panel
    pub fn show_settings(&mut self) {
        self.settings_view_state = SettingsViewState::new();
        self.settings_view_state.load_user_prefs(&self.project_path);
        self.ui_mode = UiMode::Settings;
    }

    /// Hide settings panel
    pub fn hide_settings(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    /// Check if any session should prevent immediate quit
    pub fn has_running_sessions(&self) -> bool {
        self.session_manager.has_running_sessions()
    }

    /// Request application quit
    pub fn request_quit(&mut self) {
        if self.has_running_sessions() && self.settings.behavior.confirm_quit {
            // Create dialog state with session count
            let session_count = self.session_manager.running_sessions().len();
            self.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(session_count));
            self.ui_mode = UiMode::ConfirmDialog;
        } else {
            self.phase = AppPhase::Quitting;
        }
    }

    /// Force quit without confirmation
    pub fn force_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Confirm quit (from confirmation dialog)
    pub fn confirm_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Cancel quit (from confirmation dialog)
    pub fn cancel_quit(&mut self) {
        self.confirm_dialog_state = None;
        self.ui_mode = UiMode::Normal;
    }

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.phase == AppPhase::Quitting
    }
}
