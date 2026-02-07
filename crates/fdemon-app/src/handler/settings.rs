//! Settings change application helpers

use crate::config::{
    EditorSettings, LaunchConfig, SettingItem, SettingValue, Settings, UserPreferences,
};

/// Apply a setting item change to the Settings struct
pub fn apply_project_setting(settings: &mut Settings, item: &SettingItem) {
    match item.id.as_str() {
        // Behavior
        "behavior.auto_start" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.behavior.auto_start = *v;
            }
        }
        "behavior.confirm_quit" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.behavior.confirm_quit = *v;
            }
        }

        // Watcher
        "watcher.paths" => {
            if let SettingValue::List(v) = &item.value {
                settings.watcher.paths = v.clone();
            }
        }
        "watcher.debounce_ms" => {
            if let SettingValue::Number(v) = &item.value {
                settings.watcher.debounce_ms = *v as u64;
            }
        }
        "watcher.auto_reload" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.watcher.auto_reload = *v;
            }
        }
        "watcher.extensions" => {
            if let SettingValue::List(v) = &item.value {
                settings.watcher.extensions = v.clone();
            }
        }

        // UI
        "ui.log_buffer_size" => {
            if let SettingValue::Number(v) = &item.value {
                settings.ui.log_buffer_size = *v as usize;
            }
        }
        "ui.show_timestamps" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.ui.show_timestamps = *v;
            }
        }
        "ui.compact_logs" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.ui.compact_logs = *v;
            }
        }
        "ui.theme" => {
            if let SettingValue::Enum { value, .. } = &item.value {
                settings.ui.theme = value.clone();
            }
        }
        "ui.stack_trace_collapsed" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.ui.stack_trace_collapsed = *v;
            }
        }
        "ui.stack_trace_max_frames" => {
            if let SettingValue::Number(v) = &item.value {
                settings.ui.stack_trace_max_frames = *v as usize;
            }
        }

        // DevTools
        "devtools.auto_open" => {
            if let SettingValue::Bool(v) = &item.value {
                settings.devtools.auto_open = *v;
            }
        }
        "devtools.browser" => {
            if let SettingValue::String(v) = &item.value {
                settings.devtools.browser = v.clone();
            }
        }

        // Editor
        "editor.command" => {
            if let SettingValue::String(v) = &item.value {
                settings.editor.command = v.clone();
            }
        }
        "editor.open_pattern" => {
            if let SettingValue::String(v) = &item.value {
                settings.editor.open_pattern = v.clone();
            }
        }

        _ => {
            tracing::warn!("Unknown project setting id: {}", item.id);
        }
    }
}

/// Apply a user preference item change to UserPreferences struct
pub fn apply_user_preference(prefs: &mut UserPreferences, item: &SettingItem) {
    match item.id.as_str() {
        "editor.command" => {
            if let SettingValue::String(v) = &item.value {
                if prefs.editor.is_none() {
                    prefs.editor = Some(EditorSettings::default());
                }
                if let Some(ref mut editor) = prefs.editor {
                    editor.command = v.clone();
                }
            }
        }
        "editor.open_pattern" => {
            if let SettingValue::String(v) = &item.value {
                if prefs.editor.is_none() {
                    prefs.editor = Some(EditorSettings::default());
                }
                if let Some(ref mut editor) = prefs.editor {
                    editor.open_pattern = v.clone();
                }
            }
        }
        "theme" => {
            if let SettingValue::Enum { value, .. } = &item.value {
                prefs.theme = if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                };
            }
        }
        _ => {
            tracing::warn!("Unknown user preference id: {}", item.id);
        }
    }
}

/// Apply a launch config item change to a LaunchConfig struct
pub fn apply_launch_config_change(config: &mut LaunchConfig, item: &SettingItem) {
    // Extract config index from ID (format: "launch.{idx}.field")
    let parts: Vec<&str> = item.id.split('.').collect();
    if parts.len() < 3 || parts[0] != "launch" {
        return;
    }

    let field = parts[2];
    match field {
        "name" => {
            if let SettingValue::String(v) = &item.value {
                config.name = v.clone();
            }
        }
        "device" => {
            if let SettingValue::String(v) = &item.value {
                config.device = v.clone();
            }
        }
        "mode" => {
            if let SettingValue::Enum { value, .. } = &item.value {
                use crate::config::FlutterMode;
                config.mode = match value.as_str() {
                    "debug" => FlutterMode::Debug,
                    "profile" => FlutterMode::Profile,
                    "release" => FlutterMode::Release,
                    _ => FlutterMode::Debug,
                };
            }
        }
        "flavor" => {
            if let SettingValue::String(v) = &item.value {
                config.flavor = if v.is_empty() { None } else { Some(v.clone()) };
            }
        }
        "auto_start" => {
            if let SettingValue::Bool(v) = &item.value {
                config.auto_start = *v;
            }
        }
        _ => {
            tracing::warn!("Unknown launch config field: {}", field);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_project_setting_bool() {
        let mut settings = Settings::default();
        assert!(!settings.behavior.auto_start);

        let item =
            SettingItem::new("behavior.auto_start", "Auto Start").value(SettingValue::Bool(true));

        apply_project_setting(&mut settings, &item);
        assert!(settings.behavior.auto_start);
    }

    #[test]
    fn test_apply_project_setting_number() {
        let mut settings = Settings::default();
        assert_eq!(settings.watcher.debounce_ms, 500);

        let item =
            SettingItem::new("watcher.debounce_ms", "Debounce").value(SettingValue::Number(1000));

        apply_project_setting(&mut settings, &item);
        assert_eq!(settings.watcher.debounce_ms, 1000);
    }

    #[test]
    fn test_apply_project_setting_string() {
        let mut settings = Settings::default();
        assert!(settings.editor.command.is_empty());

        let item = SettingItem::new("editor.command", "Command")
            .value(SettingValue::String("code".to_string()));

        apply_project_setting(&mut settings, &item);
        assert_eq!(settings.editor.command, "code");
    }

    #[test]
    fn test_apply_project_setting_list() {
        let mut settings = Settings::default();
        assert_eq!(settings.watcher.paths, vec!["lib"]);

        let item = SettingItem::new("watcher.paths", "Paths")
            .value(SettingValue::List(vec!["lib".into(), "test".into()]));

        apply_project_setting(&mut settings, &item);
        assert_eq!(settings.watcher.paths, vec!["lib", "test"]);
    }

    #[test]
    fn test_apply_project_setting_enum() {
        let mut settings = Settings::default();
        assert_eq!(settings.ui.theme, "default");

        let item = SettingItem::new("ui.theme", "Theme").value(SettingValue::Enum {
            value: "dark".to_string(),
            options: vec!["default".into(), "dark".into()],
        });

        apply_project_setting(&mut settings, &item);
        assert_eq!(settings.ui.theme, "dark");
    }

    #[test]
    fn test_apply_user_preference_editor_command() {
        let mut prefs = UserPreferences::default();
        assert!(prefs.editor.is_none());

        let item = SettingItem::new("editor.command", "Command")
            .value(SettingValue::String("nvim".to_string()));

        apply_user_preference(&mut prefs, &item);
        assert!(prefs.editor.is_some());
        assert_eq!(prefs.editor.as_ref().unwrap().command, "nvim");
    }

    #[test]
    fn test_apply_user_preference_theme() {
        let mut prefs = UserPreferences::default();
        assert!(prefs.theme.is_none());

        let item = SettingItem::new("theme", "Theme").value(SettingValue::Enum {
            value: "dark".to_string(),
            options: vec!["".into(), "default".into(), "dark".into()],
        });

        apply_user_preference(&mut prefs, &item);
        assert_eq!(prefs.theme, Some("dark".to_string()));
    }

    #[test]
    fn test_apply_user_preference_theme_empty() {
        let mut prefs = UserPreferences {
            theme: Some("dark".to_string()),
            ..Default::default()
        };

        let item = SettingItem::new("theme", "Theme").value(SettingValue::Enum {
            value: "".to_string(), // Empty means use project default
            options: vec!["".into(), "default".into(), "dark".into()],
        });

        apply_user_preference(&mut prefs, &item);
        assert!(prefs.theme.is_none());
    }

    #[test]
    fn test_apply_launch_config_name() {
        use crate::config::FlutterMode;

        let mut config = LaunchConfig {
            name: "Old".to_string(),
            device: "auto".to_string(),
            mode: FlutterMode::Debug,
            ..Default::default()
        };

        let item = SettingItem::new("launch.0.name", "Name")
            .value(SettingValue::String("New".to_string()));

        apply_launch_config_change(&mut config, &item);
        assert_eq!(config.name, "New");
    }

    #[test]
    fn test_apply_launch_config_mode() {
        use crate::config::FlutterMode;

        let mut config = LaunchConfig::default();
        assert_eq!(config.mode, FlutterMode::Debug);

        let item = SettingItem::new("launch.0.mode", "Mode").value(SettingValue::Enum {
            value: "release".to_string(),
            options: vec!["debug".into(), "profile".into(), "release".into()],
        });

        apply_launch_config_change(&mut config, &item);
        assert_eq!(config.mode, FlutterMode::Release);
    }

    #[test]
    fn test_apply_launch_config_flavor_empty() {
        let mut config = LaunchConfig {
            flavor: Some("prod".to_string()),
            ..Default::default()
        };

        let item = SettingItem::new("launch.0.flavor", "Flavor")
            .value(SettingValue::String("".to_string()));

        apply_launch_config_change(&mut config, &item);
        assert!(config.flavor.is_none());
    }
}
