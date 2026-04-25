//! Startup functions for the TUI runner
//!
//! Contains initialization logic for the TUI:
//! - `startup_flutter`: Detects auto-start or shows NewSessionDialog at startup

use std::path::Path;

use fdemon_app::config::{
    self, get_first_auto_start, load_all_configs, load_last_selection, LoadedConfigs,
};
use fdemon_app::state::{AppState, UiMode};

/// Result of startup initialization
#[derive(Debug)]
pub enum StartupAction {
    /// Enter normal mode, no auto-start — show NewSessionDialog
    Ready,
    /// Auto-start detected — runner will send StartAutoLaunch message
    AutoStart { configs: LoadedConfigs },
}

/// Returns `true` when `settings.local.toml` exists in `project_path` and
/// contains a non-empty `last_device` value.
///
/// A missing file, a parse failure, or an empty string all return `false`.
fn has_cached_last_device(project_path: &Path) -> bool {
    load_last_selection(project_path)
        .and_then(|s| s.device_id)
        .is_some_and(|d| !d.is_empty())
}

/// Initialize startup state.
///
/// The auto-start gate fires when **either** of these conditions holds:
///
/// 1. **Explicit config:** any launch config in `launch.toml` has `auto_start = true`.
/// 2. **Cached last device:** `settings.local.toml` exists and contains a
///    non-empty `last_device` field (written by Task 02's symmetric persistence).
///
/// When the gate fires, returns `StartupAction::AutoStart` so the runner can
/// send `Message::StartAutoLaunch`. `find_auto_launch_target`'s 4-tier cascade
/// then resolves the actual target: Tier 1 wins when an explicit `auto_start`
/// config is present; Tier 2 consumes the cached selection otherwise. If the
/// cached device has since been disconnected, Tier 3 / Tier 4 handle the
/// fall-through as usual.
///
/// When neither condition holds, shows the NewSessionDialog in Startup mode
/// and returns `StartupAction::Ready`.
pub fn startup_flutter(
    state: &mut AppState,
    _settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    // Load configs upfront
    let configs = load_all_configs(project_path);

    // Gate: fire AutoStart when an explicit auto_start config exists OR a
    // cached last_device is present (makes find_auto_launch_target Tier 2 reachable).
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let cache_trigger = !has_auto_start_config && has_cached_last_device(project_path);

    if has_auto_start_config || cache_trigger {
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
    fn test_startup_flutter_auto_start_driven_by_launch_config() {
        // launch.toml has auto_start = true — the sole gate for auto-start
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

        // launch.toml auto_start = true triggers AutoStart
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

    // ── Cache-gate tests (G1 / G2 / G3) ─────────────────────────────────────

    /// G1: Cache with last_device = "foo", no auto_start configs → AutoStart fires.
    /// UI mode must NOT be Startup (we did not show the new-session dialog).
    #[test]
    fn test_startup_flutter_cache_last_device_triggers_auto_start() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Write a settings.local.toml with a non-empty last_device
        std::fs::write(
            fdemon_dir.join("settings.local.toml"),
            r#"last_device = "foo""#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Cache-gate fires → AutoStart
        assert!(
            matches!(result, StartupAction::AutoStart { .. }),
            "Expected AutoStart when last_device is set, got Ready"
        );
        // NewSessionDialog was NOT shown
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    /// G2: Cache file present but last_device = "", no auto_start configs → Ready.
    /// Empty string is treated as "no cache" — gate must NOT fire.
    #[test]
    fn test_startup_flutter_empty_cached_last_device_shows_dialog() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Write a settings.local.toml with an empty last_device
        std::fs::write(
            fdemon_dir.join("settings.local.toml"),
            r#"last_device = """#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Empty string → gate does not fire
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(
            matches!(result, StartupAction::Ready),
            "Expected Ready when last_device is empty string"
        );
    }

    /// G3: Cache present with last_device = "foo" AND an auto_start config →
    /// AutoStart still fires (auto_start path takes priority; cache doesn't matter).
    #[test]
    fn test_startup_flutter_auto_start_config_takes_priority_over_cache() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Write a launch.toml with auto_start = true
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

        // Also write a settings.local.toml with a non-empty last_device
        std::fs::write(
            fdemon_dir.join("settings.local.toml"),
            r#"last_device = "foo""#,
        )
        .unwrap();

        let mut state = AppState::new();
        let settings = Settings::default();

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Both conditions active → AutoStart fires (auto_start config path)
        assert!(
            matches!(result, StartupAction::AutoStart { .. }),
            "Expected AutoStart when auto_start config is present"
        );
        assert_ne!(state.ui_mode, UiMode::Startup);
    }
}
