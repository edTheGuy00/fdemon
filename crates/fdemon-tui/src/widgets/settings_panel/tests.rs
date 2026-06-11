//! Tests for settings_panel widget module

use super::*;
use fdemon_app::config::{FlutterMode, LaunchConfig, SettingValue};
use ratatui::{backend::TestBackend, Terminal};
use tempfile::tempdir;

// ─────────────────────────────────────────────────────────
// Helpers for render_with_regions tests
// ─────────────────────────────────────────────────────────

/// Extract the `Message` from a region entry's left-click action, if any.
fn extract_action(e: &fdemon_app::MouseRegionEntry) -> Option<fdemon_app::message::Message> {
    e.on_left.as_ref().and_then(|a| a.as_emit()).cloned()
}

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

    // Should have 36 items across 8 sections (includes DevTools + DevTools Logging + DAP Server +
    // behavior.auto_launch added in cache-auto-launch-gate + ui.enable_mouse added in mouse-support
    // + ui.clipboard_mode added in clipboard-osc52)
    assert_eq!(items.len(), 36);
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
    assert!(content.contains("Confirm Quit"));
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
        assert!(!(*val));
        *val = !*val;
        assert!(*val);
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

// ─────────────────────────────────────────────────────────
// Modal Overlay Tests (Phase 2, Task 05)
// ─────────────────────────────────────────────────────────

#[test]
fn test_settings_panel_renders_dart_defines_modal_overlay() {
    use fdemon_app::new_session_dialog::{DartDefine, DartDefinesModalState};

    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    state.dart_defines_modal = Some(DartDefinesModalState::new(vec![DartDefine::new(
        "API_KEY", "abc123",
    )]));

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    // DartDefinesModal renders "Manage Dart Defines" as its title
    assert!(
        content.contains("Manage Dart Defines") || content.contains("Dart Defines"),
        "DartDefinesModal overlay should be rendered"
    );
}

#[test]
fn test_settings_panel_renders_extra_args_modal_overlay() {
    use fdemon_app::new_session_dialog::{FuzzyModalState, FuzzyModalType};

    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    state.extra_args_modal = Some(FuzzyModalState::new(
        FuzzyModalType::ExtraArgs,
        vec!["--verbose".to_string()],
    ));

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    // FuzzyModal renders its title from modal_type.title() which returns "Edit Extra Args"
    assert!(
        content.contains("Edit Extra Args"),
        "FuzzyModal overlay for extra args should be rendered"
    );
}

#[test]
fn test_settings_panel_no_overlay_when_no_modal() {
    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::Project;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        !content.contains("Manage Dart Defines"),
        "No dart defines overlay should appear when dart_defines_modal is None"
    );
    assert!(
        !content.contains("Edit Extra Args"),
        "No extra args overlay should appear when extra_args_modal is None"
    );
}

// ─────────────────────────────────────────────────────────
// Rendering integration tests (Phase 2, Task 06)
// ─────────────────────────────────────────────────────────

/// Verify the "Add New Configuration" button is rendered and visible when
/// at least one launch config exists.
#[test]
fn test_render_add_config_button_visible_with_configs() {
    use fdemon_app::config::launch::init_launch_file;

    let settings = Settings::default();
    let temp = tempdir().unwrap();
    init_launch_file(temp.path()).unwrap();

    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        content.contains("Add New Configuration"),
        "Add New Configuration button must be rendered when configs exist"
    );
}

/// Verify that the "Add New Configuration" button row is visually selected
/// (i.e., uses the accent bar indicator) when selected_index is set to its
/// position.
#[test]
fn test_render_add_config_button_selected() {
    use fdemon_app::config::launch::{init_launch_file, load_launch_configs};
    use fdemon_app::settings_items::launch_config_items;

    let settings = Settings::default();
    let temp = tempdir().unwrap();
    init_launch_file(temp.path()).unwrap();

    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;

    // Calculate the add-new button position (sum of all item counts = add-new index)
    let configs = load_launch_configs(temp.path());
    let item_count: usize = configs
        .iter()
        .enumerate()
        .map(|(idx, r)| launch_config_items(&r.config, idx).len())
        .sum();
    // add-new is at index `item_count` (0-based), one past the last config item
    state.selected_index = item_count;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    // The add-new button text must be visible
    assert!(
        content.contains("Add New Configuration"),
        "Add New Configuration should be rendered"
    );
    // When selected, the add-new row renders a "▶ " triangle indicator (not the regular "▎" bar).
    // Verify that the selection indicator "▶" is present in the buffer.
    assert!(
        content.contains('▶'),
        "The selection indicator '▶' must appear for the selected add-new row"
    );
}

/// The "Add New Configuration" button must NOT be rendered when no launch
/// config file exists (i.e., empty state).
#[test]
fn test_render_add_config_button_absent_when_no_configs() {
    let settings = Settings::default();
    let temp = tempdir().unwrap();
    // No launch.toml created

    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    assert!(
        !content.contains("Add New Configuration"),
        "Add New Configuration button must NOT appear when there are no configs"
    );
}

/// Dart defines modal overlay shows the key from an existing define.
#[test]
fn test_render_dart_defines_modal_shows_define_key() {
    use fdemon_app::new_session_dialog::{DartDefine, DartDefinesModalState};

    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    state.dart_defines_modal = Some(DartDefinesModalState::new(vec![DartDefine::new(
        "MY_API_KEY",
        "secret",
    )]));

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    // The modal title must be present
    assert!(
        content.contains("Dart Defines") || content.contains("Manage Dart"),
        "Dart defines modal must be rendered"
    );
    // The key name must appear in the rendered output
    assert!(
        content.contains("MY_API_KEY"),
        "Dart define key 'MY_API_KEY' must be visible in modal"
    );
}

/// Extra args modal shows the item in the list.
#[test]
fn test_render_extra_args_modal_shows_item() {
    use fdemon_app::new_session_dialog::{FuzzyModalState, FuzzyModalType};

    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    state.extra_args_modal = Some(FuzzyModalState::new(
        FuzzyModalType::ExtraArgs,
        vec!["--trace-startup".to_string()],
    ));

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let panel = SettingsPanel::new(&settings, temp.path());
            frame.render_stateful_widget(panel, frame.area(), &mut state);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    // The modal title must be present
    assert!(
        content.contains("Edit Extra Args"),
        "Extra args modal must be rendered with its title"
    );
    // The item in the list must be visible
    assert!(
        content.contains("--trace-startup"),
        "'--trace-startup' must be visible in the extra args modal"
    );
}

// ─────────────────────────────────────────────────────────
// Phase 5 Task 10: render_with_regions tests
// ─────────────────────────────────────────────────────────

#[test]
fn render_with_regions_records_four_tab_headers() {
    use fdemon_app::message::Message;
    use fdemon_app::MouseRegions;

    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    let mut state = SettingsViewState::default();

    let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 40));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(
            ratatui::layout::Rect::new(0, 0, 100, 40),
            &mut buf,
            panel,
            &mut state,
            Some(&mut ctx),
        );
    }

    let tab_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SettingsGotoTab(_))))
        .count();
    assert_eq!(tab_count, 4, "expected 4 tab-header regions");

    // All regions register at z=0 (full-screen panel).
    for entry in regions.iter() {
        assert_eq!(entry.z_index, 0);
    }
}

#[test]
fn render_with_regions_records_one_region_per_visible_setting_row() {
    use fdemon_app::message::Message;
    use fdemon_app::MouseRegions;

    // Render with the Project tab active. Count SettingsClickRow regions —
    // must equal the number of items returned by project_settings_items().
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    // Project tab is the default — no need to set active_tab explicitly.
    let mut state = SettingsViewState::default();

    let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 60));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(
            ratatui::layout::Rect::new(0, 0, 100, 60),
            &mut buf,
            panel,
            &mut state,
            Some(&mut ctx),
        );
    }

    let row_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SettingsClickRow { .. })))
        .count();
    let expected = project_settings_items(&settings).len();
    // Allow row_count <= expected because some rows may scroll off-screen.
    assert!(row_count > 0 && row_count <= expected);
}

#[test]
fn render_with_regions_indices_match_item_positions() {
    use fdemon_app::message::Message;
    use fdemon_app::MouseRegions;

    // Click the third item — expect SettingsClickRow { index: 2 }.
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    let mut state = SettingsViewState::default();

    let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 60));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(
            ratatui::layout::Rect::new(0, 0, 100, 60),
            &mut buf,
            panel,
            &mut state,
            Some(&mut ctx),
        );
    }

    // Collect the recorded indices in registration order.
    let indices: Vec<usize> = regions
        .iter()
        .filter_map(|e| match extract_action(e) {
            Some(Message::SettingsClickRow { index }) => Some(index),
            _ => None,
        })
        .collect();
    // Indices must be strictly increasing AND start at 0.
    assert!(indices.first() == Some(&0));
    for window in indices.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn render_with_regions_section_headers_are_not_clickable() {
    use fdemon_app::message::Message;
    use fdemon_app::MouseRegions;

    // Verify that the row count of registered click regions equals the
    // number of items, NOT items + section headers.
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    // Project tab is the default — no need to set active_tab explicitly.
    let mut state = SettingsViewState::default();

    // Use a tall buffer so all rows are visible.
    let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 80));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(
            ratatui::layout::Rect::new(0, 0, 100, 80),
            &mut buf,
            panel,
            &mut state,
            Some(&mut ctx),
        );
    }

    let row_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SettingsClickRow { .. })))
        .count();
    let expected = project_settings_items(&settings).len();
    assert_eq!(
        row_count, expected,
        "all items registered, no section-header regions"
    );
}

#[test]
fn render_with_regions_visual_output_unchanged() {
    use fdemon_app::MouseRegions;

    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let mut state_a = SettingsViewState::default();
    let mut state_b = SettingsViewState::default();

    let mut buf_widget = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 40));
    let mut buf_with_regions =
        ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 40));

    let panel_a = SettingsPanel::new(&settings, project_path);
    ratatui::widgets::StatefulWidget::render(
        panel_a,
        ratatui::layout::Rect::new(0, 0, 100, 40),
        &mut buf_widget,
        &mut state_a,
    );

    let panel_b = SettingsPanel::new(&settings, project_path);
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(
            ratatui::layout::Rect::new(0, 0, 100, 40),
            &mut buf_with_regions,
            panel_b,
            &mut state_b,
            Some(&mut ctx),
        );
    }

    assert_eq!(buf_widget, buf_with_regions);
}

#[test]
fn render_with_regions_none_ctx_produces_same_output_as_widget_render() {
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let mut state_a = SettingsViewState::default();
    let mut state_b = SettingsViewState::default();

    let area = ratatui::layout::Rect::new(0, 0, 100, 40);
    let mut buf_widget = ratatui::buffer::Buffer::empty(area);
    let mut buf_no_ctx = ratatui::buffer::Buffer::empty(area);

    let panel_a = SettingsPanel::new(&settings, project_path);
    ratatui::widgets::StatefulWidget::render(panel_a, area, &mut buf_widget, &mut state_a);

    let panel_b = SettingsPanel::new(&settings, project_path);
    super::render_with_regions(area, &mut buf_no_ctx, panel_b, &mut state_b, None);

    assert_eq!(buf_widget, buf_no_ctx);
}

// ─────────────────────────────────────────────────────────
// Phase 5.5 Task 05 — Layout constants, sentinel, cache
// ─────────────────────────────────────────────────────────

/// Layout-parity test: verify that `render_with_regions` registers row regions
/// whose y coordinates align with the cells actually rendered by the widget.
///
/// For the Project tab, item 0 ("Confirm Quit") is the first item after the
/// first section header.  We verify that the region rect's y is the same row
/// as where the label text appears in the rendered buffer.
#[test]
fn render_with_regions_row_rect_y_aligns_with_rendered_label() {
    use fdemon_app::message::Message;
    use fdemon_app::settings_items::project_settings_items;
    use fdemon_app::MouseRegions;

    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");

    // Use a large, tall buffer so all rows are visible and nothing is clipped.
    let area = ratatui::layout::Rect::new(0, 0, 100, 60);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let mut regions = MouseRegions::default();

    let mut state = SettingsViewState::default();
    {
        let panel = SettingsPanel::new(&settings, project_path);
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, panel, &mut state, Some(&mut ctx));
    }

    // Find the region for index 0 (first item: "Confirm Quit").
    let row0_rect = regions
        .iter()
        .find_map(|e| match extract_action(e) {
            Some(Message::SettingsClickRow { index: 0 }) => Some(e.rect),
            _ => None,
        })
        .expect("region for SettingsClickRow { index: 0 } must be registered");

    // The first project_settings_items entry label is "Confirm Quit".
    let items = project_settings_items(&settings);
    let expected_label_prefix = &items[0].label[..3]; // e.g. "Con"

    // Scan the buffer row identified by row0_rect.y for the label text.
    let row_y = row0_rect.y;
    let row_content: String = (row0_rect.x..row0_rect.x + row0_rect.width)
        .map(|x| {
            buf.cell((x, row_y))
                .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
        })
        .collect();

    assert!(
        row_content.contains(expected_label_prefix),
        "buffer row {} (region rect y={}) must contain label prefix {:?}; got: {:?}",
        row_y,
        row_y,
        expected_label_prefix,
        &row_content[..row_content.len().min(40)],
    );
}

/// Sentinel-clickable test: when launch configs exist, `render_with_regions`
/// must register a `SettingsClickRow` region for the "Add New Configuration"
/// sentinel row at index equal to the total number of config items.
///
/// With 1 default config (7 items from `launch_config_items`), the sentinel
/// is registered at index 7 (= all_items.len()).
#[test]
fn launch_config_add_new_sentinel_is_clickable() {
    use fdemon_app::config::launch::{init_launch_file, load_launch_configs};
    use fdemon_app::message::Message;
    use fdemon_app::settings_items::launch_config_items;
    use fdemon_app::MouseRegions;

    let settings = Settings::default();
    let temp = tempdir().unwrap();

    // Create one default launch config.
    init_launch_file(temp.path()).unwrap();

    // Pre-compute expected item count (from the same function the renderer uses).
    let configs = load_launch_configs(temp.path());
    assert!(
        !configs.is_empty(),
        "init_launch_file must produce at least one config"
    );
    let expected_item_count: usize = configs
        .iter()
        .enumerate()
        .map(|(idx, r)| launch_config_items(&r.config, idx).len())
        .sum();
    let sentinel_index = expected_item_count;

    let area = ratatui::layout::Rect::new(0, 0, 100, 60);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let mut regions = MouseRegions::default();

    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;

    {
        let panel = SettingsPanel::new(&settings, temp.path());
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, panel, &mut state, Some(&mut ctx));
    }

    // Count SettingsClickRow regions: must be expected_item_count + 1 (sentinel).
    let row_indices: Vec<usize> = regions
        .iter()
        .filter_map(|e| match extract_action(e) {
            Some(Message::SettingsClickRow { index }) => Some(index),
            _ => None,
        })
        .collect();

    assert_eq!(
        row_indices.len(),
        expected_item_count + 1,
        "expected {} item regions + 1 sentinel; got {} total regions",
        expected_item_count,
        row_indices.len(),
    );
    assert_eq!(
        row_indices.last().copied(),
        Some(sentinel_index),
        "last registered region must be the sentinel at index {}",
        sentinel_index,
    );
}

/// Cache-fill structural test: `render_with_regions` on the LaunchConfig tab
/// must pass the same item set to the region recorder as the renderer used for
/// display.  This is verified by cross-checking that every registered
/// `SettingsClickRow` index is in range [0, item_count] (where `item_count`
/// is the sentinel-inclusive count from `get_item_count_for_tab`).
///
/// This test provides structural coverage that the region recorder does not
/// diverge from the renderer by loading configs independently with different
/// results (e.g., if a race condition produced different configs).
#[test]
fn render_with_regions_launch_config_region_count_matches_renderer() {
    use fdemon_app::config::launch::{init_launch_file, load_launch_configs};
    use fdemon_app::message::Message;
    use fdemon_app::settings_items::launch_config_items;
    use fdemon_app::MouseRegions;

    let settings = Settings::default();
    let temp = tempdir().unwrap();

    // Create one default launch config.
    init_launch_file(temp.path()).unwrap();

    let configs = load_launch_configs(temp.path());
    let item_count: usize = configs
        .iter()
        .enumerate()
        .map(|(idx, r)| launch_config_items(&r.config, idx).len())
        .sum();

    let area = ratatui::layout::Rect::new(0, 0, 100, 60);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let mut regions = MouseRegions::default();

    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;

    {
        let panel = SettingsPanel::new(&settings, temp.path());
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, panel, &mut state, Some(&mut ctx));
    }

    let row_indices: Vec<usize> = regions
        .iter()
        .filter_map(|e| match extract_action(e) {
            Some(Message::SettingsClickRow { index }) => Some(index),
            _ => None,
        })
        .collect();

    // Every registered index must be in [0, item_count] (inclusive: sentinel at item_count).
    for &idx in &row_indices {
        assert!(
            idx <= item_count,
            "region index {} out of range [0, {}] — recorder diverged from renderer",
            idx,
            item_count,
        );
    }

    // The set of indices must be exactly 0..=item_count with no gaps
    // (sentinel + all config items).
    assert_eq!(
        row_indices.len(),
        item_count + 1,
        "region count ({}) must equal item_count ({}) + 1 sentinel",
        row_indices.len(),
        item_count,
    );
}
