//! Startup functions for the TUI runner
//!
//! Contains initialization logic for the TUI:
//! - `startup_flutter`: Detects auto-start or shows NewSessionDialog at startup

use std::path::Path;

use fdemon_app::config::{self, get_first_auto_start, load_all_configs, LoadedConfigs};
use fdemon_app::state::{AppState, UiMode};

/// Result of startup initialization
#[derive(Debug)]
pub enum StartupAction {
    /// Enter normal mode, no auto-start — show NewSessionDialog
    Ready,
    /// Auto-start detected — runner will send StartAutoLaunch message
    AutoStart { configs: LoadedConfigs },
}

/// Initialize startup state
///
/// Checks for auto-start conditions (config `auto_start = true` or
/// `behavior.auto_start = true` in settings). If either is set, returns
/// `StartupAction::AutoStart` so the runner can send `Message::StartAutoLaunch`.
/// Otherwise shows the NewSessionDialog in Startup mode and returns `Ready`.
pub fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    // Load configs upfront
    let configs = load_all_configs(project_path);

    // Check if any launch config has auto_start = true, or behavior.auto_start is enabled
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let behavior_auto_start = settings.behavior.auto_start;

    if has_auto_start_config || behavior_auto_start {
        // Return AutoStart — runner will send StartAutoLaunch message
        return StartupAction::AutoStart { configs };
    }

    // Default: show NewSessionDialog at startup (Startup mode)
    state.show_new_session_dialog(configs);
    state.ui_mode = UiMode::Startup; // Override to Startup mode

    // Return Ready - the runner will trigger tool availability and device discovery
    StartupAction::Ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::config::Settings;
    use tempfile::tempdir;

    #[test]
    fn test_startup_flutter_shows_new_session_dialog_when_no_auto_start() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::new();
        let settings = Settings::default();
        let project_path = dir.path();

        let result = startup_flutter(&mut state, &settings, project_path);

        // Should show NewSessionDialog in Startup mode when no auto-start configured
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(matches!(result, StartupAction::Ready));
    }

    #[test]
    fn test_startup_flutter_returns_auto_start_when_behavior_auto_start_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::new();
        let mut settings = Settings::default();
        settings.behavior.auto_start = true;
        let project_path = dir.path();

        let result = startup_flutter(&mut state, &settings, project_path);

        // behavior.auto_start = true triggers AutoStart
        assert!(matches!(result, StartupAction::AutoStart { .. }));
        // UI mode should NOT be set to Startup when auto-starting
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    #[test]
    fn test_startup_flutter_returns_auto_start_when_config_auto_start_set() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "AutoDev"
device = "auto"
auto_start = true
"#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Config with auto_start = true triggers AutoStart
        assert!(matches!(result, StartupAction::AutoStart { .. }));
        // UI mode should NOT be set to Startup when auto-starting
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    #[test]
    fn test_startup_flutter_ready_when_config_auto_start_false() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "ManualDev"
device = "auto"
auto_start = false
"#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Config with auto_start = false → shows dialog
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(matches!(result, StartupAction::Ready));
    }

    #[test]
    fn test_startup_flutter_prefers_launch_config_auto_start() {
        // launch.toml has auto_start = true, but behavior.auto_start is false
        // The launch config auto_start should win
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "AutoDev"
device = "auto"
auto_start = true
"#,
        )
        .unwrap();

        let mut state = AppState::new();
        let mut settings = Settings::default();
        settings.behavior.auto_start = false;

        let result = startup_flutter(&mut state, &settings, temp.path());

        // launch.toml auto_start = true takes precedence even when behavior.auto_start = false
        assert!(matches!(result, StartupAction::AutoStart { .. }));
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    #[test]
    fn test_startup_flutter_auto_start_configs_passed_through() {
        // Verify that the AutoStart variant carries the loaded configs
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "AutoConfig"
device = "emulator-1"
auto_start = true
"#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Configs must be present in the AutoStart variant
        match result {
            StartupAction::AutoStart { configs } => {
                assert!(!configs.configs.is_empty());
                assert_eq!(configs.configs.len(), 1);
                assert_eq!(configs.configs[0].config.name, "AutoConfig");
            }
            StartupAction::Ready => panic!("Expected AutoStart, got Ready"),
        }
    }

    #[test]
    fn test_startup_flutter_multiple_configs_one_auto_start() {
        // Multiple configs in launch.toml, only one with auto_start = true
        // AutoStart should be triggered and all configs should be present
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "ManualDebug"
device = "auto"
auto_start = false

[[configurations]]
name = "AutoRelease"
device = "auto"
auto_start = true

[[configurations]]
name = "ManualProfile"
device = "auto"
auto_start = false
"#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // One auto_start config is enough to trigger AutoStart
        assert!(matches!(result, StartupAction::AutoStart { .. }));
        assert_ne!(state.ui_mode, UiMode::Startup);

        // All configs (not just the auto_start one) should be in the AutoStart variant
        if let StartupAction::AutoStart { configs } = result {
            assert_eq!(configs.configs.len(), 3);
            assert_eq!(configs.configs[0].config.name, "ManualDebug");
            assert_eq!(configs.configs[1].config.name, "AutoRelease");
            assert_eq!(configs.configs[2].config.name, "ManualProfile");
        }
    }
}
