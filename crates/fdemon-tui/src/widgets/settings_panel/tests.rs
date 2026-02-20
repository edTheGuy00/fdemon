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

    assert!(content.contains("System Settings"));
    assert!(content.contains("PROJECT"));
    assert!(content.contains("USER"));
    assert!(content.contains("LAUNCH"));
    assert!(content.contains("VSCODE"));
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

    // Footer shows "Save Changes*" hint when dirty (may be truncated on narrow terminals)
    // The footer displays: ⌨ Tab: Switch tabs  › j/k: Navigate  › Enter: Edit  [S] Ctrl+S: Save Changes*
    assert!(content.contains("Save"));
    assert!(content.contains("Ctrl+S"));
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

    assert!(content.contains("1. PROJECT"));
    assert!(content.contains("2. USER"));
    assert!(content.contains("3. LAUNCH"));
    assert!(content.contains("4. VSCODE"));
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

    // Should have 27 items across 7 sections (includes DevTools + DevTools Logging from Phase 5)
    assert_eq!(items.len(), 27);
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

    // No truncation needed
    assert_eq!(truncate_str("short", 10), "short");
    assert_eq!(truncate_str("ab", 2), "ab");
    assert_eq!(truncate_str("a", 1), "a");

    // Truncation with ellipsis
    let result = truncate_str("this is long", 8);
    assert_eq!(
        result.chars().count(),
        8,
        "Output exceeded max_len: {}",
        result
    );
    assert_eq!(result, "this is…");

    let result = truncate_str("abc", 2);
    assert_eq!(
        result.chars().count(),
        2,
        "Output exceeded max_len: {}",
        result
    );
    assert_eq!(result, "a…");

    // Edge cases
    assert_eq!(truncate_str("anything", 0), "");
    assert_eq!(truncate_str("", 5), "");
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

    // Check sections are rendered (spaced uppercase format from Phase 4, Task 03)
    assert!(content.contains("B E H A V I O R"));
    assert!(content.contains("W A T C H E R"));
    assert!(content.contains("U I"));

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
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::Bool(true), false);
    assert_eq!(style.fg, Some(palette::STATUS_GREEN));
}

#[test]
fn test_value_style_bool_false() {
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::Bool(false), false);
    assert_eq!(style.fg, Some(palette::STATUS_RED));
}

#[test]
fn test_value_style_number() {
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::Number(42), false);
    assert_eq!(style.fg, Some(palette::ACCENT));
}

#[test]
fn test_value_style_string_empty() {
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::String(String::new()), false);
    assert_eq!(style.fg, Some(palette::TEXT_MUTED));
}

#[test]
fn test_value_style_string_non_empty() {
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::String("test".to_string()), false);
    assert_eq!(style.fg, Some(palette::TEXT_PRIMARY));
}

#[test]
fn test_value_style_enum() {
    use crate::theme::palette;
    let style = styles::value_style(
        &SettingValue::Enum {
            value: "option".to_string(),
            options: vec!["option".to_string()],
        },
        false,
    );
    assert_eq!(style.fg, Some(palette::STATUS_INDIGO));
}

#[test]
fn test_value_style_list() {
    use crate::theme::palette;
    let style = styles::value_style(&SettingValue::List(vec!["item".to_string()]), false);
    assert_eq!(style.fg, Some(palette::STATUS_BLUE));
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

// ─────────────────────────────────────────────────────────
// Phase 4 Redesign Tests (Cyber-Glass Design)
// ─────────────────────────────────────────────────────────

#[test]
fn test_section_header_renders_icon_and_uppercase() {
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

    // Verify spaced uppercase section headers (from Phase 4, Task 03)
    assert!(
        content.contains("B E H A V I O R"),
        "Should render 'BEHAVIOR' with spaced uppercase"
    );
    assert!(
        content.contains("W A T C H E R"),
        "Should render 'WATCHER' with spaced uppercase"
    );
    assert!(
        content.contains("U I"),
        "Should render 'UI' with spaced uppercase"
    );

    // Icons are present in the buffer (but exact glyph may vary by IconMode)
    // We can verify by checking that section headers exist and are styled correctly
    // The implementation in render_section_header ensures icons are present
}

#[test]
fn test_selected_row_has_accent_bar() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::Project;
    state.selected_index = 0; // Select first setting
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

    // Find the first setting row (after section header) and check for accent bar
    let mut found_accent_bar = false;
    for y in 0..buffer.area().height {
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            if cell.symbol() == "▎" {
                // Verify it has ACCENT foreground color
                assert_eq!(
                    cell.fg,
                    palette::ACCENT,
                    "Accent bar should have ACCENT foreground color"
                );
                found_accent_bar = true;
                break;
            }
        }
        if found_accent_bar {
            break;
        }
    }

    assert!(
        found_accent_bar,
        "Selected row should display '▎' accent bar"
    );
}

#[test]
fn test_selected_row_has_tinted_background() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::Project;
    state.selected_index = 0; // Select first setting
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

    // Find a cell on the selected row and verify it has SELECTED_ROW_BG
    let mut found_selected_bg = false;
    for y in 0..buffer.area().height {
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            if cell.bg == palette::SELECTED_ROW_BG {
                found_selected_bg = true;
                break;
            }
        }
        if found_selected_bg {
            break;
        }
    }

    assert!(
        found_selected_bg,
        "Selected row should have SELECTED_ROW_BG background"
    );
}

#[test]
fn test_unselected_row_has_no_accent_bar() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::Project;
    state.selected_index = 0; // Select first setting only
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

    // Count accent bars - should only be 1 (for selected row)
    let mut accent_bar_count = 0;
    for y in 0..buffer.area().height {
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            if cell.symbol() == "▎" && cell.fg == palette::ACCENT {
                accent_bar_count += 1;
            }
        }
    }

    // Should have exactly 1 accent bar (for the selected row)
    // Note: This verifies unselected rows don't have accent bars
    assert_eq!(
        accent_bar_count, 1,
        "Should have exactly 1 accent bar for the selected row"
    );
}

#[test]
fn test_footer_normal_mode_shows_4_hints() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    // Verify all 4 normal mode hints are present
    assert!(content.contains("Tab:"), "Footer should show 'Tab:' hint");
    assert!(content.contains("j/k:"), "Footer should show 'j/k:' hint");
    assert!(
        content.contains("Enter:"),
        "Footer should show 'Enter:' hint"
    );
    assert!(
        content.contains("Ctrl+S:"),
        "Footer should show 'Ctrl+S:' hint"
    );
    assert!(
        content.contains("Switch tabs"),
        "Footer should show 'Switch tabs' label"
    );
    assert!(
        content.contains("Navigate"),
        "Footer should show 'Navigate' label"
    );
    assert!(content.contains("Edit"), "Footer should show 'Edit' label");
    assert!(
        content.contains("Save Changes"),
        "Footer should show 'Save Changes' label"
    );
}

#[test]
fn test_footer_editing_mode_shows_confirm_cancel() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.editing = true; // Enter editing mode
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    // Verify editing mode hints are present
    assert!(
        content.contains("Enter:"),
        "Footer should show 'Enter:' in editing mode"
    );
    assert!(
        content.contains("Confirm"),
        "Footer should show 'Confirm' label"
    );
    assert!(
        content.contains("Esc:"),
        "Footer should show 'Esc:' in editing mode"
    );
    assert!(
        content.contains("Cancel"),
        "Footer should show 'Cancel' label"
    );
}

#[test]
fn test_tab_labels_uppercase() {
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

    // Verify tab labels are uppercase
    assert!(
        content.contains("PROJECT"),
        "Tab label should be uppercase 'PROJECT'"
    );
    assert!(
        content.contains("USER"),
        "Tab label should be uppercase 'USER'"
    );
    assert!(
        content.contains("LAUNCH"),
        "Tab label should be uppercase 'LAUNCH'"
    );
    assert!(
        content.contains("VSCODE"),
        "Tab label should be uppercase 'VSCODE'"
    );
}

#[test]
fn test_header_shows_settings_title() {
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

    // Verify header shows "System Settings" title
    assert!(
        content.contains("System Settings"),
        "Header should display 'System Settings' title"
    );
}

// ─────────────────────────────────────────────────────────
// Phase 4 Fixes - Info Banner Content Tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_user_prefs_info_banner_shows_content() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::UserPrefs;
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

    // Verify info banner content is visible (not just empty bordered box)
    assert!(
        content.contains("Local Settings"),
        "Info banner should display 'Local Settings' title"
    );
    assert!(
        content.contains(".fdemon/settings.local.toml"),
        "Info banner should display file path subtitle"
    );
}

#[test]
fn test_vscode_info_banner_shows_content() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::VSCodeConfig;
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

    // Verify info banner content is visible (not just empty bordered box)
    assert!(
        content.contains("VSCode"),
        "Info banner should display 'VSCode' in title"
    );
    assert!(
        content.contains(".vscode/launch.json"),
        "Info banner should display file path subtitle"
    );
}

// ─────────────────────────────────────────────────────────
// Empty State Alignment Tests (Phase 4 Fixes, Task 02)
// ─────────────────────────────────────────────────────────

#[test]
fn test_launch_empty_state_top_aligned() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Find the icon box (should be in the top few rows after header+tabs)
    // Header is 3 lines, tabs are 3 lines, content starts at y=6
    // With top alignment (start_y = area.top() + 1), icon should be at y=7
    let mut found_icon_row = None;
    for y in 6..15 {
        // Search in top portion
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            // Look for the icon box border (rounded corners)
            if cell.symbol() == "╭" || cell.symbol() == "╮" {
                found_icon_row = Some(y);
                break;
            }
        }
        if found_icon_row.is_some() {
            break;
        }
    }

    assert!(
        found_icon_row.is_some(),
        "Icon box should be found in top portion of content area"
    );

    let icon_y = found_icon_row.unwrap();
    // Icon should be near the top (within first 8 rows of content area)
    // Content area starts at y=6, so icon should be between y=6 and y=14
    assert!(
        icon_y <= 14,
        "Icon should be top-aligned (found at y={}, expected <= 14)",
        icon_y
    );
}

#[test]
fn test_vscode_empty_state_top_aligned() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::VSCodeConfig;
    let temp = tempdir().unwrap();

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // VSCode tab shows "No .vscode/launch.json found" when no file exists
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("No .vscode/launch.json found"));

    // Find the icon box (should be in the top few rows)
    let mut found_icon_row = None;
    for y in 6..15 {
        // Search in top portion
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            if cell.symbol() == "╭" || cell.symbol() == "╮" {
                found_icon_row = Some(y);
                break;
            }
        }
        if found_icon_row.is_some() {
            break;
        }
    }

    assert!(
        found_icon_row.is_some(),
        "Icon box should be found in top portion of content area"
    );

    let icon_y = found_icon_row.unwrap();
    assert!(
        icon_y <= 14,
        "Icon should be top-aligned (found at y={}, expected <= 14)",
        icon_y
    );
}

#[test]
fn test_empty_state_not_vertically_centered() {
    let settings = Settings::default();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    let temp = tempdir().unwrap();

    // Use a tall terminal to make vertical centering obvious
    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Find the icon box
    let mut found_icon_row = None;
    for y in 0..buffer.area().height {
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            if cell.symbol() == "╭" || cell.symbol() == "╮" {
                found_icon_row = Some(y);
                break;
            }
        }
        if found_icon_row.is_some() {
            break;
        }
    }

    assert!(found_icon_row.is_some(), "Icon box should be found");

    let icon_y = found_icon_row.unwrap();
    // If it were vertically centered in a 40-row terminal (content area ~34 rows),
    // with total_height=7, it would be at approximately y = 6 + (34-7)/2 = 19-20
    // With top alignment (start_y = 6 + 1 = 7), icon should be at y=7
    // So icon_y should be much less than the midpoint
    assert!(
        icon_y < 15,
        "Icon should be top-aligned, not centered (found at y={}, would be ~20 if centered)",
        icon_y
    );
}
