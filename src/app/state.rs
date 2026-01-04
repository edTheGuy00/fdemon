//! Application state (Model in TEA pattern)

use std::path::PathBuf;

use crate::config::Settings;
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
