//! Tests for LaunchContextState

use super::super::*;
use crate::config::{ConfigSource, FlutterMode, LaunchConfig, LoadedConfigs, SourcedConfig};

#[test]
fn test_field_navigation() {
    let field = LaunchContextField::Config;
    assert_eq!(field.next(), LaunchContextField::Mode);
    assert_eq!(field.prev(), LaunchContextField::Launch);
}

#[test]
fn test_field_navigation_wraps() {
    let field = LaunchContextField::Launch;
    assert_eq!(field.next(), LaunchContextField::Config);
}

#[test]
fn test_editability_no_config() {
    let state = LaunchContextState::new(LoadedConfigs::default());

    assert!(state.is_field_editable(LaunchContextField::Config));
    assert!(state.is_field_editable(LaunchContextField::Mode));
    assert!(state.is_field_editable(LaunchContextField::Flavor));
    assert!(state.is_field_editable(LaunchContextField::DartDefines));
    assert!(state.is_field_editable(LaunchContextField::Launch));
}

#[test]
fn test_editability_vscode_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "Test".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config(Some(0));

    assert!(state.is_field_editable(LaunchContextField::Config)); // Always editable
    assert!(!state.is_field_editable(LaunchContextField::Mode));
    assert!(!state.is_field_editable(LaunchContextField::Flavor));
    assert!(!state.is_field_editable(LaunchContextField::DartDefines));
    assert!(state.is_field_editable(LaunchContextField::Launch)); // Always editable
}

#[test]
fn test_editability_fdemon_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "Test".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config(Some(0));

    assert!(state.is_field_editable(LaunchContextField::Config));
    assert!(state.is_field_editable(LaunchContextField::Mode));
    assert!(state.is_field_editable(LaunchContextField::Flavor));
    assert!(state.is_field_editable(LaunchContextField::DartDefines));
    assert!(state.is_field_editable(LaunchContextField::Launch));
}

#[test]
fn test_dart_define_to_arg() {
    let define = DartDefine::new("API_KEY", "secret123");
    assert_eq!(define.to_arg(), "API_KEY=secret123");
}

#[test]
fn test_focus_navigation() {
    let state = LaunchContextState::new(LoadedConfigs::default());

    // With no config selected, all fields are editable
    assert_eq!(state.focused_field, LaunchContextField::Config);
}

#[test]
fn test_set_flavor() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());

    state.set_flavor(Some("production".to_string()));
    assert_eq!(state.flavor, Some("production".to_string()));
}

#[test]
fn test_set_flavor_disabled_when_vscode() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig {
            flavor: Some("dev".to_string()),
            ..Default::default()
        },
        source: ConfigSource::VSCode,
        display_name: "Test".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config(Some(0));

    // Flavor should be "dev" from config
    assert_eq!(state.flavor, Some("dev".to_string()));

    // Setting should have no effect because VSCode configs are read-only
    state.set_flavor(Some("production".to_string()));
    assert_eq!(state.flavor, Some("dev".to_string()));
}

#[test]
fn test_set_dart_defines() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());

    let defines = vec![DartDefine::new("KEY", "value")];
    state.set_dart_defines(defines.clone());
    assert_eq!(state.dart_defines, defines);
}

#[test]
fn test_set_dart_defines_disabled_when_vscode() {
    let mut configs = LoadedConfigs::default();
    let mut config = LaunchConfig::default();
    config
        .dart_defines
        .insert("ORIGINAL".to_string(), "value".to_string());

    configs.configs.push(SourcedConfig {
        config,
        source: ConfigSource::VSCode,
        display_name: "Test".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config(Some(0));

    // Should have original define from config
    assert_eq!(state.dart_defines.len(), 1);
    assert_eq!(state.dart_defines[0].key, "ORIGINAL");

    // Setting should have no effect because VSCode configs are read-only
    let new_defines = vec![DartDefine::new("NEW", "value")];
    state.set_dart_defines(new_defines);

    assert_eq!(state.dart_defines.len(), 1);
    assert_eq!(state.dart_defines[0].key, "ORIGINAL");
}

#[test]
fn test_flavor_display() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());

    assert_eq!(state.flavor_display(), "(none)");

    state.flavor = Some("production".to_string());
    assert_eq!(state.flavor_display(), "production");
}

#[test]
fn test_dart_defines_display() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());

    assert_eq!(state.dart_defines_display(), "(none)");

    state.dart_defines = vec![DartDefine::new("KEY", "value")];
    assert_eq!(state.dart_defines_display(), "1 item");

    state.dart_defines.push(DartDefine::new("KEY2", "value2"));
    assert_eq!(state.dart_defines_display(), "2 items");
}

#[test]
fn test_config_display() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "My Config".to_string(),
    });

    let mut state = LaunchContextState::new(configs);

    assert_eq!(state.config_display(), "(none)");

    state.select_config(Some(0));
    assert_eq!(state.config_display(), "My Config");
}

#[test]
fn test_select_config_applies_values() {
    let mut configs = LoadedConfigs::default();
    let mut config = LaunchConfig::default();
    config.mode = FlutterMode::Release;
    config.flavor = Some("production".to_string());
    config
        .dart_defines
        .insert("API_URL".to_string(), "https://api.com".to_string());

    configs.configs.push(SourcedConfig {
        config,
        source: ConfigSource::FDemon,
        display_name: "Production".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config(Some(0));

    assert_eq!(state.mode, FlutterMode::Release);
    assert_eq!(state.flavor, Some("production".to_string()));
    assert_eq!(state.dart_defines.len(), 1);
    assert_eq!(state.dart_defines[0].key, "API_URL");
    assert_eq!(state.dart_defines[0].value, "https://api.com");
}

#[test]
fn test_select_config_by_name() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "Debug Config".to_string(),
    });
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "Release Config".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config_by_name("Release Config");

    assert_eq!(state.selected_config_index, Some(1));
}

#[test]
fn test_select_config_by_name_not_found() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "Debug Config".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config_by_name("Nonexistent");

    assert_eq!(state.selected_config_index, None);
}
