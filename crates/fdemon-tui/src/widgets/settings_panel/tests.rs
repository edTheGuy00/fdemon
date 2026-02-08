//! Tests for settings_panel widget module

use super::*;
use fdemon_app::config::{FlutterMode, LaunchConfig, SettingValue};
use ratatui::{backend::TestBackend, Terminal};
use tempfile::tempdir;

#[test]
fn test_settings_panel_renders() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    assert!(content.contains("Settings"));
    assert!(content.contains("Project"));
    assert!(content.contains("User"));
    assert!(content.contains("Launch"));
    assert!(content.contains("VSCode"));
}

#[test]
fn test_settings_panel_shows_active_tab() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    // Verify Launch tab content is shown (empty state in this case)
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("No launch configurations"));
}

#[test]
fn test_settings_panel_dirty_indicator() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.dirty = true;
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("unsaved"));
}

#[test]
fn test_tab_navigation_wraps() {
    let mut state = SettingsViewState::new();
    assert_eq!(state.active_tab, SettingsTab::Project);

    // Forward through all tabs
    state.next_tab();
    assert_eq!(state.active_tab, SettingsTab::UserPrefs);
    state.next_tab();
    assert_eq!(state.active_tab, SettingsTab::LaunchConfig);
    state.next_tab();
    assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);
    state.next_tab(); // Wrap
    assert_eq!(state.active_tab, SettingsTab::Project);
}

#[test]
fn test_tab_switch_resets_selection() {
    let mut state = SettingsViewState::new();
    state.selected_index = 5;

    state.next_tab();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_tab_switch_exits_edit_mode() {
    let mut state = SettingsViewState::new();
    state.editing = true;
    state.edit_buffer = "test".to_string();

    state.next_tab();
    assert!(!state.editing);
    assert!(state.edit_buffer.is_empty());
}

#[test]
fn test_goto_tab() {
    let mut state = SettingsViewState::new();

    state.goto_tab(SettingsTab::VSCodeConfig);
    assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_tab_readonly() {
    assert!(!SettingsTab::Project.is_readonly());
    assert!(!SettingsTab::UserPrefs.is_readonly());
    assert!(!SettingsTab::LaunchConfig.is_readonly());
    assert!(SettingsTab::VSCodeConfig.is_readonly());
}

#[test]
fn test_render_shows_all_tabs() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    assert!(content.contains("1.Project"));
    assert!(content.contains("2.User"));
    assert!(content.contains("3.Launch"));
    assert!(content.contains("4.VSCode"));
}

#[test]
fn test_tab_icons() {
    assert_eq!(SettingsTab::Project.icon(), "⚙");
    assert_eq!(SettingsTab::UserPrefs.icon(), "👤");
    assert_eq!(SettingsTab::LaunchConfig.icon(), "▶");
    assert_eq!(SettingsTab::VSCodeConfig.icon(), "📁");
}

#[test]
fn test_project_settings_items_count() {
    let settings = Settings::default();
    let items = project_settings_items(&settings);

    // Should have 17 items across 5 sections (includes ui.icons from Phase 1)
    assert_eq!(items.len(), 17);
}

#[test]
fn test_project_settings_sections() {
    let settings = Settings::default();
    let items = project_settings_items(&settings);

    let sections: Vec<&str> = items.iter().map(|i| i.section.as_str()).collect();
    assert!(sections.contains(&"Behavior"));
    assert!(sections.contains(&"Watcher"));
    assert!(sections.contains(&"UI"));
    assert!(sections.contains(&"DevTools"));
    assert!(sections.contains(&"Editor"));
}

#[test]
fn test_setting_is_modified() {
    let settings = Settings::default();
    let items = project_settings_items(&settings);

    // Default values should not be modified
    for item in &items {
        assert!(
            !item.is_modified(),
            "Item {} should not be modified",
            item.id
        );
    }
}

#[test]
fn test_truncate_str() {
    use styles::truncate_str;

    assert_eq!(truncate_str("short", 10), "short");
    assert_eq!(truncate_str("this is long", 8), "this is...");
    assert_eq!(truncate_str("ab", 2), "ab");
    assert_eq!(truncate_str("abc", 2), "a...");
}

#[test]
fn test_render_project_tab() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::Project;
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    // Check sections are rendered
    assert!(content.contains("[Behavior]"));
    assert!(content.contains("[Watcher]"));
    assert!(content.contains("[UI]"));

    // Check some settings are rendered
    assert!(content.contains("Auto Start"));
    assert!(content.contains("Debounce"));
    assert!(content.contains("Log Buffer"));
}

#[test]
fn test_launch_config_items() {
    let config = LaunchConfig {
        name: "Development".to_string(),
        device: "iphone".to_string(),
        mode: FlutterMode::Debug,
        flavor: Some("dev".to_string()),
        auto_start: true,
        dart_defines: [("API_URL".to_string(), "https://dev.api.com".to_string())]
            .into_iter()
            .collect(),
        extra_args: vec!["--verbose".to_string()],
        entry_point: None,
    };

    let items = launch_config_items(&config, 0);

    assert_eq!(items.len(), 7);
    assert!(items.iter().any(|i| i.id == "launch.0.name"));
    assert!(items.iter().any(|i| i.id == "launch.0.mode"));
}

#[test]
fn test_render_launch_tab_empty() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    assert!(content.contains("No launch configurations"));
}

#[test]
fn test_render_launch_tab_with_configs() {
    use fdemon_app::config::launch::init_launch_file;

    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    let temp = tempdir().unwrap();

    // Create a launch.toml file
    init_launch_file(temp.path()).unwrap();

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    // Should show configuration header
    assert!(content.contains("Configuration 1"));
    // Should show setting fields
    assert!(content.contains("Name"));
    assert!(content.contains("Device"));
    assert!(content.contains("Mode"));
    // Should show "+ Add New Configuration" option
    assert!(content.contains("Add New Configuration"));
}

// ─────────────────────────────────────────────────────────
// Style Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_value_style_bool_true() {
    use ratatui::style::Color;
    let style = styles::value_style(&SettingValue::Bool(true), false);
    assert_eq!(style.fg, Some(Color::Green));
}

#[test]
fn test_value_style_bool_false() {
    use ratatui::style::Color;
    let style = styles::value_style(&SettingValue::Bool(false), false);
    assert_eq!(style.fg, Some(Color::Red));
}

#[test]
fn test_value_style_number() {
    use ratatui::style::Color;
    let style = styles::value_style(&SettingValue::Number(42), false);
    assert_eq!(style.fg, Some(Color::Cyan));
}

#[test]
fn test_value_style_string_empty() {
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::String(String::new()), false);
    assert_eq!(style.fg, Some(palette::TEXT_MUTED));
}

#[test]
fn test_value_style_string_non_empty() {
    use ratatui::style::Color;
    let style = styles::value_style(&SettingValue::String("test".to_string()), false);
    assert_eq!(style.fg, Some(Color::White));
}

#[test]
fn test_value_style_enum() {
    use ratatui::style::Color;
    let style = styles::value_style(
        &SettingValue::Enum {
            value: "option".to_string(),
            options: vec!["option".to_string()],
        },
        false,
    );
    assert_eq!(style.fg, Some(Color::Magenta));
}

#[test]
fn test_value_style_list() {
    use ratatui::style::Color;
    let style = styles::value_style(&SettingValue::List(vec!["item".to_string()]), false);
    assert_eq!(style.fg, Some(Color::Blue));
}

#[test]
fn test_value_style_selected_adds_bold() {
    use ratatui::style::Modifier;
    let style = styles::value_style(&SettingValue::Bool(true), true);
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

// ─────────────────────────────────────────────────────────
// User Preferences Tab Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_user_prefs_items_count() {
    let prefs = UserPreferences::default();
    let settings = Settings::default();
    let items = user_prefs_items(&prefs, &settings);

    // Should have 5 items: 2 editor overrides, 1 theme, 2 session memory
    assert_eq!(items.len(), 5);
}

#[test]
fn test_user_prefs_items_sections() {
    let prefs = UserPreferences::default();
    let settings = Settings::default();
    let items = user_prefs_items(&prefs, &settings);

    let sections: Vec<&str> = items.iter().map(|i| i.section.as_str()).collect();
    assert!(sections.contains(&"Editor Override"));
    assert!(sections.contains(&"UI Preferences"));
    assert!(sections.contains(&"Session Memory"));
}

#[test]
fn test_user_prefs_session_memory_readonly() {
    let prefs = UserPreferences::default();
    let settings = Settings::default();
    let items = user_prefs_items(&prefs, &settings);

    // Session memory items should be readonly
    let readonly_items: Vec<_> = items.iter().filter(|i| i.readonly).collect();
    assert_eq!(readonly_items.len(), 2);
    assert!(readonly_items.iter().any(|i| i.id == "last_device"));
    assert!(readonly_items.iter().any(|i| i.id == "last_config"));
}

// ─────────────────────────────────────────────────────────
// VSCode Config Items Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_vscode_config_items_count() {
    let config = LaunchConfig {
        name: "Test".to_string(),
        device: "auto".to_string(),
        mode: FlutterMode::Debug,
        flavor: None,
        auto_start: false,
        dart_defines: Default::default(),
        extra_args: vec![],
        entry_point: None,
    };

    let items = vscode_config_items(&config, 0);

    // Should have 6 items per config
    assert_eq!(items.len(), 6);
}

#[test]
fn test_vscode_config_items_all_readonly() {
    let config = LaunchConfig {
        name: "Test".to_string(),
        device: "auto".to_string(),
        mode: FlutterMode::Debug,
        flavor: None,
        auto_start: false,
        dart_defines: Default::default(),
        extra_args: vec![],
        entry_point: None,
    };

    let items = vscode_config_items(&config, 0);

    // All VSCode items should be readonly
    for item in &items {
        assert!(item.readonly, "Item {} should be readonly", item.id);
    }
}

// ─────────────────────────────────────────────────────────
// Editor Tests (Phase 4, Task 10)
// ─────────────────────────────────────────────────────────

#[test]
fn test_toggle_bool() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Bool(false));

    // Simulate toggle
    if let SettingValue::Bool(ref mut val) = item.value {
        *val = !*val;
    }

    assert!(matches!(item.value, SettingValue::Bool(true)));
}

#[test]
fn test_toggle_bool_twice() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Bool(true));

    // Toggle twice should return to original
    if let SettingValue::Bool(ref mut val) = item.value {
        *val = !*val;
        assert_eq!(*val, false);
        *val = !*val;
        assert_eq!(*val, true);
    }
}

#[test]
fn test_cycle_enum_next() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Enum {
        value: "debug".to_string(),
        options: vec![
            "debug".to_string(),
            "profile".to_string(),
            "release".to_string(),
        ],
    });

    // Simulate cycle next
    if let SettingValue::Enum {
        ref mut value,
        ref options,
    } = item.value
    {
        let idx = options.iter().position(|o| o == value).unwrap_or(0);
        *value = options[(idx + 1) % options.len()].clone();
    }

    assert!(matches!(
        item.value,
        SettingValue::Enum { ref value, .. } if value == "profile"
    ));
}

#[test]
fn test_cycle_enum_prev() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Enum {
        value: "profile".to_string(),
        options: vec![
            "debug".to_string(),
            "profile".to_string(),
            "release".to_string(),
        ],
    });

    // Simulate cycle prev
    if let SettingValue::Enum {
        ref mut value,
        ref options,
    } = item.value
    {
        let idx = options.iter().position(|o| o == value).unwrap_or(0);
        let next_idx = if idx == 0 { options.len() - 1 } else { idx - 1 };
        *value = options[next_idx].clone();
    }

    assert!(matches!(
        item.value,
        SettingValue::Enum { ref value, .. } if value == "debug"
    ));
}

#[test]
fn test_cycle_enum_wraps_around() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Enum {
        value: "release".to_string(),
        options: vec![
            "debug".to_string(),
            "profile".to_string(),
            "release".to_string(),
        ],
    });

    // Cycle from last to first
    if let SettingValue::Enum {
        ref mut value,
        ref options,
    } = item.value
    {
        let idx = options.iter().position(|o| o == value).unwrap_or(0);
        *value = options[(idx + 1) % options.len()].clone();
    }

    assert!(matches!(
        item.value,
        SettingValue::Enum { ref value, .. } if value == "debug"
    ));
}

#[test]
fn test_add_list_item() {
    let mut item =
        SettingItem::new("test", "Test").value(SettingValue::List(vec!["lib".to_string()]));

    // Simulate add
    if let SettingValue::List(ref mut items) = item.value {
        items.push("test".to_string());
    }

    assert!(matches!(
        item.value,
        SettingValue::List(ref items) if items.len() == 2 && items[1] == "test"
    ));
}

#[test]
fn test_remove_list_item() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::List(vec![
        "lib".to_string(),
        "test".to_string(),
    ]));

    // Simulate remove last
    if let SettingValue::List(ref mut items) = item.value {
        items.pop();
    }

    assert!(matches!(
        item.value,
        SettingValue::List(ref items) if items.len() == 1 && items[0] == "lib"
    ));
}

#[test]
fn test_list_no_duplicates() {
    let mut item =
        SettingItem::new("test", "Test").value(SettingValue::List(vec!["lib".to_string()]));

    // Simulate add with duplicate check
    if let SettingValue::List(ref mut items) = item.value {
        let new_item = "lib".to_string();
        if !new_item.is_empty() && !items.contains(&new_item) {
            items.push(new_item);
        }
    }

    // Should not add duplicate
    assert!(matches!(
        item.value,
        SettingValue::List(ref items) if items.len() == 1
    ));
}

#[test]
fn test_number_edit_buffer() {
    let mut state = SettingsViewState::new();
    state.start_editing("500");

    assert!(state.editing);
    assert_eq!(state.edit_buffer, "500");

    // Simulate backspace
    state.edit_buffer.pop();
    assert_eq!(state.edit_buffer, "50");

    // Simulate adding digit
    state.edit_buffer.push('0');
    assert_eq!(state.edit_buffer, "500");
}

#[test]
fn test_number_edit_parse() {
    let buffer = "42";
    let result: Result<i64, _> = buffer.parse();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);

    let buffer = "-100";
    let result: Result<i64, _> = buffer.parse();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), -100);

    let buffer = "invalid";
    let result: Result<i64, _> = buffer.parse();
    assert!(result.is_err());
}

#[test]
fn test_string_edit() {
    let mut state = SettingsViewState::new();
    state.start_editing("hello");

    state.edit_buffer.push_str(" world");
    assert_eq!(state.edit_buffer, "hello world");

    // Simulate backspace
    state.edit_buffer.pop();
    assert_eq!(state.edit_buffer, "hello worl");

    // Clear
    state.edit_buffer.clear();
    assert_eq!(state.edit_buffer, "");
}

#[test]
fn test_increment_number() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Number(5));

    // Simulate increment
    if let SettingValue::Number(ref mut val) = item.value {
        *val = val.saturating_add(1);
    }

    assert!(matches!(item.value, SettingValue::Number(6)));
}

#[test]
fn test_decrement_number() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Number(5));

    // Simulate decrement
    if let SettingValue::Number(ref mut val) = item.value {
        *val = val.saturating_add(-1);
    }

    assert!(matches!(item.value, SettingValue::Number(4)));
}

#[test]
fn test_number_saturating() {
    let mut item = SettingItem::new("test", "Test").value(SettingValue::Number(i64::MAX));

    // Saturating add won't overflow
    if let SettingValue::Number(ref mut val) = item.value {
        *val = val.saturating_add(1);
    }

    assert!(matches!(item.value, SettingValue::Number(v) if v == i64::MAX));
}

#[test]
fn test_edit_mode_state_transitions() {
    let mut state = SettingsViewState::new();
    assert!(!state.editing);
    assert!(state.edit_buffer.is_empty());

    // Enter edit mode
    state.start_editing("initial");
    assert!(state.editing);
    assert_eq!(state.edit_buffer, "initial");

    // Exit edit mode
    state.stop_editing();
    assert!(!state.editing);
    assert!(state.edit_buffer.is_empty());
}

#[test]
fn test_dirty_flag_on_edit() {
    let mut state = SettingsViewState::new();
    assert!(!state.dirty);

    // Mark dirty after edit
    state.mark_dirty();
    assert!(state.dirty);

    // Clear after save
    state.clear_dirty();
    assert!(!state.dirty);
}
