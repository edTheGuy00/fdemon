//! Startup functions for the TUI runner
//!
//! Contains initialization logic for the TUI:
//! - `startup_flutter`: Shows NewSessionDialog at startup

use std::path::Path;

use fdemon_app::config::{self, load_all_configs};
use fdemon_app::state::{AppState, UiMode};

/// Result of startup initialization
#[derive(Debug)]
pub enum StartupAction {
    /// Enter normal mode, no auto-start
    Ready,
}

/// Initialize startup state
///
/// Shows NewSessionDialog at startup. Device discovery and tool availability
/// checks will be triggered by the runner after the first render.
pub fn startup_flutter(
    state: &mut AppState,
    _settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    // Load configs upfront
    let configs = load_all_configs(project_path);

    // Show NewSessionDialog at startup (Startup mode)
    state.show_new_session_dialog(configs.clone());
    state.ui_mode = UiMode::Startup; // Override to Startup mode

    // Return Ready - the runner will trigger tool availability and device discovery
    StartupAction::Ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::config::Settings;

    #[test]
    fn test_startup_flutter_shows_new_session_dialog() {
        let mut state = AppState::new();
        let settings = Settings::default();
        let project_path = Path::new("/tmp/test");

        let result = startup_flutter(&mut state, &settings, project_path);

        // Should always show NewSessionDialog in Startup mode
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(matches!(result, StartupAction::Ready));
    }

    #[test]
    fn test_startup_flutter_ignores_auto_start_setting() {
        let mut state = AppState::new();
        let mut settings = Settings::default();
        settings.behavior.auto_start = true;
        let project_path = Path::new("/tmp/test");

        let result = startup_flutter(&mut state, &settings, project_path);

        // auto_start setting is ignored - always show NewSessionDialog
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(matches!(result, StartupAction::Ready));
    }
}
