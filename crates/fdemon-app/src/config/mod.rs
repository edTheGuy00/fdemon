//! Configuration file parsing for Flutter Demon
//!
//! Supports:
//! - `.fdemon/config.toml` - Global settings
//! - `.fdemon/launch.toml` - Launch configurations
//! - `.vscode/launch.json` - VSCode compatibility

pub mod launch;
pub mod priority;
pub mod settings;
pub mod types;
pub mod vscode;
pub mod writer;

pub use launch::{
    add_launch_config, create_default_launch_config, delete_launch_config, find_config_by_name,
    get_auto_start_configs, init_launch_file, load_launch_configs, parse_dart_defines,
    save_launch_configs, update_launch_config_dart_defines, update_launch_config_field,
};
pub use priority::{
    find_config, get_first_auto_start, get_first_config, load_all_configs, LoadedConfigs,
    SourcedConfig,
};
pub use settings::{
    clear_last_selection, detect_editor, detect_parent_ide, editor_config_for_ide,
    find_editor_config, init_config_dir, init_fdemon_directory, load_last_selection, load_settings,
    load_user_preferences, merge_preferences, save_last_selection, save_settings,
    save_user_preferences, should_auto_start_dap, validate_last_selection, EditorConfig,
    LastSelection, ValidatedSelection, KNOWN_EDITORS,
};
// Re-export public config types used by TUI and other crates
pub use types::{
    BehaviorSettings, ConfigSource, CustomSourceConfig, DapSettings, DevToolsLoggingSettings,
    DevToolsSettings, EditorSettings, FlutterMode, IconMode, LaunchConfig, LaunchFile,
    NativeLogsSettings, ParentIde, ReadyCheck, ResolvedLaunchConfig, SettingItem, SettingValue,
    Settings, SettingsTab, TagConfig, UiSettings, UserPreferences, WatcherSettings, WindowPrefs,
};
pub use vscode::load_vscode_configs;
pub use writer::{
    save_fdemon_configs, update_config_dart_defines, update_config_flavor, update_config_mode,
    ConfigAutoSaver,
};

/// Returns `true` when `settings.local.toml` exists in `project_path` and
/// contains a non-empty `last_device` value.
///
/// A missing file, a parse failure, or an empty string all return `false`.
pub fn has_cached_last_device(project_path: &std::path::Path) -> bool {
    load_last_selection(project_path)
        .and_then(|s| s.device_id)
        .is_some_and(|d| !d.is_empty())
}

/// Identifies the calling context so the migration nudge can produce
/// mode-appropriate remediation text.
pub enum NudgeMode {
    Tui,
    Headless,
}

/// Emit a one-time-per-process migration nudge if a cached `last_device`
/// is present but the user has not opted into cache-driven auto-launch.
///
/// Returns `true` if the nudge condition applies (cache present, no
/// auto_start config, `auto_launch` flag unset) — useful for callers
/// that want to drive secondary UI (e.g., a TUI banner). The actual
/// `tracing::info!` emission is gated by a process-level `OnceLock`,
/// so the log line appears at most once per process.
///
/// The returned `bool` reflects the condition itself, not whether
/// the `OnceLock` fired — i.e., this returns `true` on every call
/// when conditions are met, so callers can render UI consistently
/// even if the log was already emitted earlier this process.
pub fn emit_migration_nudge(
    mode: NudgeMode,
    project_path: &std::path::Path,
    settings: &Settings,
) -> bool {
    use std::sync::OnceLock;
    static EMITTED: OnceLock<()> = OnceLock::new();

    let configs = load_all_configs(project_path);
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let has_cache = has_cached_last_device(project_path);
    let cache_opt_in = settings.behavior.auto_launch;

    let applies = !has_auto_start_config && has_cache && !cache_opt_in;
    if !applies {
        return false;
    }

    EMITTED.get_or_init(|| match mode {
        NudgeMode::Tui => tracing::info!(
            "settings.local.toml has a cached last_device but [behavior] auto_launch \
             is not set in config.toml. Auto-launch via cache is now opt-in. \
             Set `[behavior] auto_launch = true` to restore the previous behavior."
        ),
        NudgeMode::Headless => tracing::info!(
            "settings.local.toml has a cached last_device. Headless mode is intentionally \
             cache-blind — it picks the first available device or honors per-config \
             `auto_start = true` in launch.toml. The `[behavior] auto_launch` flag \
             does NOT apply in headless."
        ),
    });

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Verify that emit_migration_nudge returns `true` when the nudge condition
    /// is met: cache present, no auto_start config, auto_launch = false.
    ///
    /// Note: we test the *return value* rather than whether the OnceLock fired.
    /// The OnceLock is a static — it can only fire once across the entire test
    /// binary, and the order of test execution is not guaranteed. Testing the
    /// log emission directly would require a tracing subscriber harness, which
    /// the CODE_STANDARDS.md explicitly says is not required for this case.
    #[test]
    fn emit_migration_nudge_returns_true_when_condition_met() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Write a settings.local.toml with a non-empty last_device
        std::fs::write(
            fdemon_dir.join("settings.local.toml"),
            r#"last_device = "some-device""#,
        )
        .unwrap();

        let settings = Settings::default(); // auto_launch = false

        // Condition: cache present, no auto_start config, auto_launch = false → true
        let result = emit_migration_nudge(NudgeMode::Tui, temp.path(), &settings);
        assert!(
            result,
            "expected true when cache exists and auto_launch is not set"
        );
    }

    /// Verify that emit_migration_nudge returns `false` when auto_launch is set.
    #[test]
    fn emit_migration_nudge_returns_false_when_opted_in() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        std::fs::write(
            fdemon_dir.join("settings.local.toml"),
            r#"last_device = "some-device""#,
        )
        .unwrap();

        let mut settings = Settings::default();
        settings.behavior.auto_launch = true; // opted in → no nudge

        let result = emit_migration_nudge(NudgeMode::Headless, temp.path(), &settings);
        assert!(
            !result,
            "expected false when auto_launch = true (user has opted in)"
        );
    }

    /// Verify that emit_migration_nudge returns `false` when no cache exists.
    #[test]
    fn emit_migration_nudge_returns_false_when_no_cache() {
        let temp = tempdir().unwrap();
        // No .fdemon dir, no cache

        let settings = Settings::default();

        let result = emit_migration_nudge(NudgeMode::Tui, temp.path(), &settings);
        assert!(!result, "expected false when no cache is present");
    }

    /// Verify that emit_migration_nudge returns `false` when an auto_start
    /// config is present (cache is superseded by explicit config).
    #[test]
    fn emit_migration_nudge_returns_false_when_auto_start_config_present() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Cache exists
        std::fs::write(
            fdemon_dir.join("settings.local.toml"),
            r#"last_device = "some-device""#,
        )
        .unwrap();

        // auto_start config also exists → nudge does not apply
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

        let settings = Settings::default(); // auto_launch = false

        let result = emit_migration_nudge(NudgeMode::Tui, temp.path(), &settings);
        assert!(
            !result,
            "expected false when an auto_start config is present"
        );
    }
}
