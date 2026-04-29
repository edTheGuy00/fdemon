//! Startup functions for the TUI runner
//!
//! Contains initialization logic for the TUI:
//! - `startup_flutter`: Detects auto-start or shows NewSessionDialog at startup

use std::path::Path;

use fdemon_app::config::{
    self, get_first_auto_start, has_cached_last_device, load_all_configs, LoadedConfigs,
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

/// Initialize startup state.
///
/// The auto-start gate fires when **either** of these conditions holds:
///
/// 1. **Explicit config:** any launch config in `launch.toml` has `auto_start = true`.
/// 2. **Cached last device:** `settings.local.toml` exists and contains a
///    non-empty `last_device` field (written by Task 02's symmetric persistence),
///    **and** `settings.behavior.auto_launch == true` (opt-in gate).
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
    settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    // Load configs upfront
    let configs = load_all_configs(project_path);

    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let has_cache = has_cached_last_device(project_path);
    let cache_opt_in = settings.behavior.auto_launch;

    // Cache-trigger requires explicit opt-in via [behavior] auto_launch = true
    let cache_trigger = !has_auto_start_config && cache_opt_in && has_cache;

    // Migration nudge: user has a cached device but didn't opt in. Tell them
    // this once so they understand why fdemon didn't auto-launch like it used to.
    if !has_auto_start_config && has_cache && !cache_opt_in {
        tracing::info!(
            "settings.local.toml has a cached last_device but [behavior] auto_launch \
             is not set in config.toml. Auto-launch via cache is now opt-in. \
             Set `[behavior] auto_launch = true` to restore the previous behavior."
        );
    }

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

    // ── Cache-gate tests (G1 / G2 / G3 / G4 / G5) ───────────────────────────

    /// G1: Cache with last_device = "foo", no auto_start configs, auto_launch = false
    /// (default) → Ready. Cache alone must NOT fire the gate without opt-in.
    #[test]
    fn cache_alone_does_not_trigger_auto_start() {
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
        let settings = Settings::default(); // auto_launch = false

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Cache without opt-in → NewSessionDialog shown, Ready returned
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(
            matches!(result, StartupAction::Ready),
            "Expected Ready when last_device is set but auto_launch = false, got AutoStart"
        );
    }

    /// G2: Cache with last_device = "foo", no auto_start configs, auto_launch = true
    /// → AutoStart fires. Opt-in + cache = auto-launch.
    #[test]
    fn cache_with_auto_launch_triggers_auto_start() {
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
        let mut settings = Settings::default();
        settings.behavior.auto_launch = true; // opt in

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Cache + opt-in → AutoStart fires
        assert!(
            matches!(result, StartupAction::AutoStart { .. }),
            "Expected AutoStart when last_device is set and auto_launch = true"
        );
        // NewSessionDialog was NOT shown
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    /// G3: Cache present with last_device = "foo" AND an auto_start config,
    /// auto_launch = false → AutoStart fires (auto_start config path; cache flag irrelevant).
    #[test]
    fn auto_start_config_beats_cache_regardless_of_flag() {
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
        let settings = Settings::default(); // auto_launch = false

        let result = startup_flutter(&mut state, &settings, temp.path());

        // auto_start config takes priority regardless of auto_launch flag
        assert!(
            matches!(result, StartupAction::AutoStart { .. }),
            "Expected AutoStart when auto_start config is present"
        );
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    /// G4: Cache present AND auto_start config AND auto_launch = true → AutoStart fires.
    #[test]
    fn auto_start_config_beats_cache_with_flag_set() {
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
        let mut settings = Settings::default();
        settings.behavior.auto_launch = true;

        let result = startup_flutter(&mut state, &settings, temp.path());

        // auto_start config present → AutoStart regardless
        assert!(
            matches!(result, StartupAction::AutoStart { .. }),
            "Expected AutoStart when auto_start config and auto_launch = true"
        );
        assert_ne!(state.ui_mode, UiMode::Startup);
    }

    /// G5: No cache, auto_launch = false, no auto_start configs → Ready (dialog shown).
    #[test]
    fn nothing_set_shows_dialog() {
        let temp = tempdir().unwrap();
        // No .fdemon dir at all

        let mut state = AppState::new();
        let settings = Settings::default(); // auto_launch = false

        let result = startup_flutter(&mut state, &settings, temp.path());

        // Nothing configured → NewSessionDialog
        assert_eq!(state.ui_mode, UiMode::Startup);
        assert!(
            matches!(result, StartupAction::Ready),
            "Expected Ready when nothing is configured"
        );
    }

    /// Cache file present but last_device = "", no auto_start configs → Ready.
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
}
