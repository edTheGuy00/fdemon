//! Settings item enumeration.
//!
//! Builds the list of configurable setting items per tab,
//! used by both the settings handler (for editing) and the
//! settings panel widget (for rendering).

use std::path::Path;

use crate::config::{
    launch::load_launch_configs, load_vscode_configs, LaunchConfig, SettingItem, SettingValue,
    Settings, SettingsTab, UserPreferences,
};
use crate::state::SettingsViewState;

/// Get the currently selected setting item for editing
///
/// This function builds the list of settings items for the active tab
/// and returns the one at the selected index.
///
/// # Arguments
/// * `settings` - Global settings
/// * `project_path` - Project root path for loading launch configurations
/// * `view_state` - Current view state (which tab, which item selected)
///
/// # Returns
/// The selected `SettingItem`, or `None` if the index is out of bounds.
pub fn get_selected_item(
    settings: &Settings,
    project_path: &Path,
    view_state: &SettingsViewState,
) -> Option<SettingItem> {
    let items = match view_state.active_tab {
        SettingsTab::Project => project_settings_items(settings),
        SettingsTab::UserPrefs => user_prefs_items(&view_state.user_prefs, settings),
        SettingsTab::LaunchConfig => {
            let configs = load_launch_configs(project_path);
            let mut all_items = Vec::new();
            for (idx, resolved) in configs.iter().enumerate() {
                all_items.extend(launch_config_items(&resolved.config, idx));
            }
            all_items
        }
        SettingsTab::VSCodeConfig => {
            let configs = load_vscode_configs(project_path);
            let mut all_items = Vec::new();
            for (idx, resolved) in configs.iter().enumerate() {
                all_items.extend(vscode_config_items(&resolved.config, idx));
            }
            all_items
        }
    };

    items.get(view_state.selected_index).cloned()
}

/// Generate settings items for the Project tab from Settings struct
pub fn project_settings_items(settings: &Settings) -> Vec<SettingItem> {
    vec![
        // ─────────────────────────────────────────────────────────
        // Behavior Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("behavior.auto_start", "Auto Start")
            .description("Skip device selector and start immediately")
            .value(SettingValue::Bool(settings.behavior.auto_start))
            .default(SettingValue::Bool(false))
            .section("Behavior"),
        SettingItem::new("behavior.confirm_quit", "Confirm Quit")
            .description("Ask before quitting with running apps")
            .value(SettingValue::Bool(settings.behavior.confirm_quit))
            .default(SettingValue::Bool(true))
            .section("Behavior"),
        // ─────────────────────────────────────────────────────────
        // Watcher Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("watcher.paths", "Watch Paths")
            .description("Directories to watch for changes")
            .value(SettingValue::List(settings.watcher.paths.clone()))
            .default(SettingValue::List(vec!["lib".to_string()]))
            .section("Watcher"),
        SettingItem::new("watcher.debounce_ms", "Debounce (ms)")
            .description("Delay before triggering reload")
            .value(SettingValue::Number(settings.watcher.debounce_ms as i64))
            .default(SettingValue::Number(500))
            .section("Watcher"),
        SettingItem::new("watcher.auto_reload", "Auto Reload")
            .description("Hot reload on file changes")
            .value(SettingValue::Bool(settings.watcher.auto_reload))
            .default(SettingValue::Bool(true))
            .section("Watcher"),
        SettingItem::new("watcher.extensions", "Extensions")
            .description("File extensions to watch")
            .value(SettingValue::List(settings.watcher.extensions.clone()))
            .default(SettingValue::List(vec!["dart".to_string()]))
            .section("Watcher"),
        // ─────────────────────────────────────────────────────────
        // UI Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("ui.log_buffer_size", "Log Buffer Size")
            .description("Maximum log entries to keep")
            .value(SettingValue::Number(settings.ui.log_buffer_size as i64))
            .default(SettingValue::Number(10000))
            .section("UI"),
        SettingItem::new("ui.show_timestamps", "Show Timestamps")
            .description("Display timestamps in logs")
            .value(SettingValue::Bool(settings.ui.show_timestamps))
            .default(SettingValue::Bool(true))
            .section("UI"),
        SettingItem::new("ui.compact_logs", "Compact Logs")
            .description("Collapse similar consecutive logs")
            .value(SettingValue::Bool(settings.ui.compact_logs))
            .default(SettingValue::Bool(false))
            .section("UI"),
        SettingItem::new("ui.theme", "Theme")
            .description("Color theme")
            .value(SettingValue::Enum {
                value: settings.ui.theme.clone(),
                options: vec![
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .default(SettingValue::Enum {
                value: "default".to_string(),
                options: vec![
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .section("UI"),
        SettingItem::new("ui.icons", "Icon Style")
            .description(
                "Icon rendering: unicode (all terminals) or nerd_fonts (requires Nerd Font)",
            )
            .value(SettingValue::Enum {
                value: settings.ui.icons.to_string(),
                options: vec!["unicode".to_string(), "nerd_fonts".to_string()],
            })
            .default(SettingValue::Enum {
                value: "nerd_fonts".to_string(),
                options: vec!["unicode".to_string(), "nerd_fonts".to_string()],
            })
            .section("UI"),
        SettingItem::new("ui.stack_trace_collapsed", "Collapse Stack Traces")
            .description("Start stack traces collapsed")
            .value(SettingValue::Bool(settings.ui.stack_trace_collapsed))
            .default(SettingValue::Bool(true))
            .section("UI"),
        SettingItem::new("ui.stack_trace_max_frames", "Max Frames")
            .description("Frames shown when collapsed")
            .value(SettingValue::Number(
                settings.ui.stack_trace_max_frames as i64,
            ))
            .default(SettingValue::Number(3))
            .section("UI"),
        // ─────────────────────────────────────────────────────────
        // DevTools Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("devtools.auto_open", "Auto Open DevTools")
            .description("Open DevTools when app starts")
            .value(SettingValue::Bool(settings.devtools.auto_open))
            .default(SettingValue::Bool(false))
            .section("DevTools"),
        SettingItem::new("devtools.browser", "Browser")
            .description("Browser for DevTools (empty = default)")
            .value(SettingValue::String(settings.devtools.browser.clone()))
            .default(SettingValue::String(String::new()))
            .section("DevTools"),
        SettingItem::new("devtools.default_panel", "Default Panel")
            .description("Panel shown when entering DevTools mode")
            .value(SettingValue::Enum {
                value: settings.devtools.default_panel.clone(),
                options: vec![
                    "inspector".to_string(),
                    "performance".to_string(),
                    "network".to_string(),
                ],
            })
            .default(SettingValue::Enum {
                value: "inspector".to_string(),
                options: vec![
                    "inspector".to_string(),
                    "performance".to_string(),
                    "network".to_string(),
                ],
            })
            .section("DevTools"),
        SettingItem::new("devtools.performance_refresh_ms", "Perf Refresh (ms)")
            .description("Memory polling interval in milliseconds (min 500)")
            .value(SettingValue::Number(
                settings.devtools.performance_refresh_ms as i64,
            ))
            .default(SettingValue::Number(2000))
            .section("DevTools"),
        SettingItem::new("devtools.memory_history_size", "Memory History Size")
            .description("Number of memory snapshots to retain")
            .value(SettingValue::Number(
                settings.devtools.memory_history_size as i64,
            ))
            .default(SettingValue::Number(60))
            .section("DevTools"),
        SettingItem::new("devtools.tree_max_depth", "Widget Tree Max Depth")
            .description("Max fetch depth for widget tree (0 = unlimited)")
            .value(SettingValue::Number(
                settings.devtools.tree_max_depth as i64,
            ))
            .default(SettingValue::Number(0))
            .section("DevTools"),
        SettingItem::new(
            "devtools.inspector_fetch_timeout_secs",
            "Inspector Fetch Timeout (s)",
        )
        .description("Widget tree fetch timeout with retries (min 5s)")
        .value(SettingValue::Number(
            settings.devtools.inspector_fetch_timeout_secs as i64,
        ))
        .default(SettingValue::Number(30))
        .section("DevTools"),
        SettingItem::new("devtools.auto_repaint_rainbow", "Auto Repaint Rainbow")
            .description("Enable repaint rainbow overlay on VM connect")
            .value(SettingValue::Bool(settings.devtools.auto_repaint_rainbow))
            .default(SettingValue::Bool(false))
            .section("DevTools"),
        SettingItem::new("devtools.auto_performance_overlay", "Auto Perf Overlay")
            .description("Enable performance overlay on VM connect")
            .value(SettingValue::Bool(
                settings.devtools.auto_performance_overlay,
            ))
            .default(SettingValue::Bool(false))
            .section("DevTools"),
        // ─────────────────────────────────────────────────────────
        // DevTools Logging Sub-Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("devtools.logging.hybrid_enabled", "Hybrid Logging")
            .description("Enable hybrid logging (VM Service + daemon)")
            .value(SettingValue::Bool(settings.devtools.logging.hybrid_enabled))
            .default(SettingValue::Bool(true))
            .section("DevTools Logging"),
        SettingItem::new("devtools.logging.prefer_vm_level", "Prefer VM Log Level")
            .description("Use VM Service log level when available")
            .value(SettingValue::Bool(
                settings.devtools.logging.prefer_vm_level,
            ))
            .default(SettingValue::Bool(true))
            .section("DevTools Logging"),
        SettingItem::new(
            "devtools.logging.show_source_indicator",
            "Show Source Indicator",
        )
        .description("Show [VM] / [daemon] tag on log entries")
        .value(SettingValue::Bool(
            settings.devtools.logging.show_source_indicator,
        ))
        .default(SettingValue::Bool(false))
        .section("DevTools Logging"),
        SettingItem::new(
            "devtools.logging.dedupe_threshold_ms",
            "Dedupe Threshold (ms)",
        )
        .description("Suppress duplicate logs within N milliseconds")
        .value(SettingValue::Number(
            settings.devtools.logging.dedupe_threshold_ms as i64,
        ))
        .default(SettingValue::Number(100))
        .section("DevTools Logging"),
        // ─────────────────────────────────────────────────────────
        // Editor Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("editor.command", "Editor Command")
            .description("Editor to open files (empty = auto-detect)")
            .value(SettingValue::String(settings.editor.command.clone()))
            .default(SettingValue::String(String::new()))
            .section("Editor"),
        SettingItem::new("editor.open_pattern", "Open Pattern")
            .description("Pattern for opening files ($FILE, $LINE, $COLUMN)")
            .value(SettingValue::String(settings.editor.open_pattern.clone()))
            .default(SettingValue::String("$EDITOR $FILE:$LINE".to_string()))
            .section("Editor"),
    ]
}

/// Generate settings items for the User Preferences tab
pub fn user_prefs_items(prefs: &UserPreferences, settings: &Settings) -> Vec<SettingItem> {
    vec![
        // ─────────────────────────────────────────────────────────
        // Editor Override
        // ─────────────────────────────────────────────────────────
        SettingItem::new("editor.command", "Editor Command")
            .description("Override project editor setting")
            .value(SettingValue::String(
                prefs
                    .editor
                    .as_ref()
                    .map(|e| e.command.clone())
                    .unwrap_or_default(),
            ))
            .default(SettingValue::String(settings.editor.command.clone()))
            .section("Editor Override"),
        SettingItem::new("editor.open_pattern", "Open Pattern")
            .description("Override project open pattern")
            .value(SettingValue::String(
                prefs
                    .editor
                    .as_ref()
                    .map(|e| e.open_pattern.clone())
                    .unwrap_or_default(),
            ))
            .default(SettingValue::String(settings.editor.open_pattern.clone()))
            .section("Editor Override"),
        // ─────────────────────────────────────────────────────────
        // UI Preferences
        // ─────────────────────────────────────────────────────────
        SettingItem::new("theme", "Theme Override")
            .description("Personal theme preference")
            .value(SettingValue::Enum {
                value: prefs.theme.clone().unwrap_or_default(),
                options: vec![
                    "".to_string(), // Use project default
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .default(SettingValue::String(String::new()))
            .section("UI Preferences"),
        // ─────────────────────────────────────────────────────────
        // Session Memory
        // ─────────────────────────────────────────────────────────
        SettingItem::new("last_device", "Last Device")
            .description("Device from last session (auto-saved)")
            .value(SettingValue::String(
                prefs.last_device.clone().unwrap_or_default(),
            ))
            .default(SettingValue::String(String::new()))
            .section("Session Memory")
            .readonly(),
        SettingItem::new("last_config", "Last Config")
            .description("Launch config from last session")
            .value(SettingValue::String(
                prefs.last_config.clone().unwrap_or_default(),
            ))
            .default(SettingValue::String(String::new()))
            .section("Session Memory")
            .readonly(),
    ]
}

/// Generate settings items for a single launch configuration
pub fn launch_config_items(config: &LaunchConfig, idx: usize) -> Vec<SettingItem> {
    let prefix = format!("launch.{}", idx);

    vec![
        SettingItem::new(format!("{}.name", prefix), "Name")
            .description("Configuration display name")
            .value(SettingValue::String(config.name.clone()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.device", prefix), "Device")
            .description("Target device ID or 'auto'")
            .value(SettingValue::String(config.device.clone()))
            .default(SettingValue::String("auto".to_string()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.mode", prefix), "Mode")
            .description("Flutter build mode")
            .value(SettingValue::Enum {
                value: config.mode.to_string(),
                options: vec![
                    "debug".to_string(),
                    "profile".to_string(),
                    "release".to_string(),
                ],
            })
            .default(SettingValue::String("debug".to_string()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.flavor", prefix), "Flavor")
            .description("Build flavor (optional)")
            .value(SettingValue::String(
                config.flavor.clone().unwrap_or_default(),
            ))
            .default(SettingValue::String(String::new()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.auto_start", prefix), "Auto Start")
            .description("Start this config automatically")
            .value(SettingValue::Bool(config.auto_start))
            .default(SettingValue::Bool(false))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.dart_defines", prefix), "Dart Defines")
            .description("--dart-define values")
            .value(SettingValue::List(
                config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect(),
            ))
            .default(SettingValue::List(vec![]))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.extra_args", prefix), "Extra Args")
            .description("Additional flutter run arguments")
            .value(SettingValue::List(config.extra_args.clone()))
            .default(SettingValue::List(vec![]))
            .section(format!("Configuration {}", idx + 1)),
    ]
}

/// Generate read-only settings items for VSCode launch configuration
pub fn vscode_config_items(config: &LaunchConfig, idx: usize) -> Vec<SettingItem> {
    let prefix = format!("vscode.{}", idx);

    vec![
        SettingItem::new(format!("{}.name", prefix), "Name")
            .description("Configuration name")
            .value(SettingValue::String(config.name.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.device", prefix), "Device ID")
            .description("Target device")
            .value(SettingValue::String(config.device.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.mode", prefix), "Flutter Mode")
            .description("Build mode")
            .value(SettingValue::String(config.mode.to_string()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.flavor", prefix), "Flavor")
            .description("Build flavor")
            .value(SettingValue::String(
                config.flavor.clone().unwrap_or_else(|| "-".to_string()),
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.entry_point", prefix), "Entry Point")
            .description("Program entry point")
            .value(SettingValue::String(
                config
                    .entry_point
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "lib/main.dart".to_string()),
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.extra_args", prefix), "Arguments")
            .description("Additional arguments")
            .value(SettingValue::List(config.extra_args.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn test_default_panel_options_match_enum_variants() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);
        let panel_item = items
            .iter()
            .find(|i| i.id == "devtools.default_panel")
            .unwrap();
        if let SettingValue::Enum { options, .. } = &panel_item.value {
            assert!(options.contains(&"inspector".to_string()));
            assert!(options.contains(&"performance".to_string()));
            assert!(options.contains(&"network".to_string()));
            assert!(!options.contains(&"layout".to_string()));
        } else {
            panic!("devtools.default_panel value should be SettingValue::Enum");
        }
    }

    #[test]
    fn test_default_panel_default_options_match_enum_variants() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);
        let panel_item = items
            .iter()
            .find(|i| i.id == "devtools.default_panel")
            .unwrap();
        if let SettingValue::Enum { options, .. } = &panel_item.default {
            assert!(options.contains(&"inspector".to_string()));
            assert!(options.contains(&"performance".to_string()));
            assert!(options.contains(&"network".to_string()));
            assert!(!options.contains(&"layout".to_string()));
        } else {
            panic!("devtools.default_panel default should be SettingValue::Enum");
        }
    }
}
