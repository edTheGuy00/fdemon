## Task: Phase 2 Integration Tests and Verification

**Objective**: Add comprehensive integration tests covering all Phase 2 changes end-to-end, verify no regressions, and run the full quality gate.

**Depends on**: 01-fix-add-config-bug, 03-dart-defines-modal, 04-extra-args-modal, 05-render-settings-modals

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/settings_handlers.rs`: Integration tests for full add-config + modal flows
- `crates/fdemon-app/src/handler/settings.rs`: Complete `apply_launch_config_change` coverage
- `crates/fdemon-app/src/handler/keys.rs`: Key routing tests with modal-open state
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Rendering integration tests
- Workspace-wide: Full quality gate verification

### Details

#### 1. Integration test: "Add New Configuration" end-to-end

Test the full flow from navigation to creation:

```rust
#[test]
fn test_add_new_config_end_to_end() {
    let temp = tempdir().unwrap();
    let mut state = AppState::new();
    state.project_path = temp.path().to_path_buf();
    state.ui_mode = UiMode::Settings;
    state.settings_view_state.active_tab = SettingsTab::LaunchConfig;

    // Init with one config
    init_launch_file(temp.path()).unwrap();

    // Navigate to add-new button (index 7 = 7 items for 1 config)
    let item_count = get_item_count_for_tab(&state);
    state.settings_view_state.selected_index = item_count - 1; // last = add-new

    // Verify selected item is the add-new sentinel
    let item = get_selected_item(&state.settings, &state.project_path, &state.settings_view_state);
    assert!(item.is_some());
    assert_eq!(item.unwrap().id, "launch.__add_new__");

    // Toggle edit should dispatch LaunchConfigCreate
    let result = handle_settings_toggle_edit(&mut state);
    // Verify a new config was created (2 configs now on disk)
    let configs = load_launch_configs(temp.path());
    assert_eq!(configs.len(), 2);
}
```

#### 2. Integration test: Dart defines modal lifecycle

```rust
#[test]
fn test_dart_defines_modal_full_lifecycle() {
    let temp = tempdir().unwrap();
    let mut state = AppState::new();
    state.project_path = temp.path().to_path_buf();
    init_launch_file(temp.path()).unwrap();

    // Open dart defines modal for config 0
    handle_settings_dart_defines_open(&mut state, 0);
    assert!(state.settings_view_state.dart_defines_modal.is_some());

    // Add a define via the modal
    let modal = state.settings_view_state.dart_defines_modal.as_mut().unwrap();
    // Navigate to Add New, load into edit, type key/value, save
    // ... (exercise DartDefinesModalState methods)

    // Close modal — should persist
    handle_settings_dart_defines_close(&mut state);
    assert!(state.settings_view_state.dart_defines_modal.is_none());

    // Verify persistence
    let configs = load_launch_configs(temp.path());
    // Assert dart_defines contains the new entry
}
```

#### 3. Integration test: Extra args modal lifecycle

```rust
#[test]
fn test_extra_args_modal_full_lifecycle() {
    let temp = tempdir().unwrap();
    let mut state = AppState::new();
    state.project_path = temp.path().to_path_buf();
    init_launch_file(temp.path()).unwrap();

    // Open extra args modal for config 0
    handle_settings_extra_args_open(&mut state, 0);
    assert!(state.settings_view_state.extra_args_modal.is_some());

    // Type a custom arg
    let modal = state.settings_view_state.extra_args_modal.as_mut().unwrap();
    modal.input_char('-');
    modal.input_char('-');
    modal.input_char('v');
    // ... apply fuzzy filter

    // Confirm — should add arg and close
    handle_settings_extra_args_confirm(&mut state);
    assert!(state.settings_view_state.extra_args_modal.is_none());

    // Verify persistence
    let configs = load_launch_configs(temp.path());
    assert!(configs[0].config.extra_args.contains(&"--v".to_string())
        || configs[0].config.extra_args.contains(&"--verbose".to_string()));
}
```

#### 4. Key routing integration tests

```rust
#[test]
fn test_key_routing_settings_normal_vs_modal() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Settings;

    // Normal settings mode: Esc closes settings
    let msg = handle_key_settings(&state, InputKey::Esc);
    assert!(matches!(msg, Some(Message::HideSettings)));

    // With dart defines modal open: Esc closes modal (not settings)
    state.settings_view_state.dart_defines_modal = Some(
        DartDefinesModalState::new(vec![])
    );
    let msg = handle_key_settings(&state, InputKey::Esc);
    assert!(matches!(msg, Some(Message::SettingsDartDefinesClose)));
}

#[test]
fn test_key_routing_extra_args_modal_intercepts() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Settings;
    state.settings_view_state.extra_args_modal = Some(
        FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![])
    );

    // Typed chars go to modal, not settings edit
    let msg = handle_key_settings(&state, InputKey::Char('a'));
    assert!(matches!(msg, Some(Message::SettingsExtraArgsInput { c: 'a' })));
}
```

#### 5. `apply_launch_config_change` complete field coverage

```rust
#[test]
fn test_apply_launch_config_change_all_fields() {
    let mut config = LaunchConfig::default();

    // Test dart_defines
    let item = SettingItem::new("launch.0.dart_defines", "Dart Defines")
        .value(SettingValue::List(vec!["KEY=VALUE".to_string()]));
    apply_launch_config_change(&mut config, &item);
    assert_eq!(config.dart_defines.get("KEY"), Some(&"VALUE".to_string()));

    // Test extra_args
    let item = SettingItem::new("launch.0.extra_args", "Extra Args")
        .value(SettingValue::List(vec!["--verbose".to_string()]));
    apply_launch_config_change(&mut config, &item);
    assert_eq!(config.extra_args, vec!["--verbose"]);

    // Test existing fields still work (regression)
    let item = SettingItem::new("launch.0.name", "Name")
        .value(SettingValue::String("Test".to_string()));
    apply_launch_config_change(&mut config, &item);
    assert_eq!(config.name, "Test");
}

#[test]
fn test_apply_launch_config_change_dart_defines_with_equals_in_value() {
    let mut config = LaunchConfig::default();
    let item = SettingItem::new("launch.0.dart_defines", "Dart Defines")
        .value(SettingValue::List(vec!["API_URL=https://api.example.com/v1?key=abc".to_string()]));
    apply_launch_config_change(&mut config, &item);
    assert_eq!(
        config.dart_defines.get("API_URL"),
        Some(&"https://api.example.com/v1?key=abc".to_string())
    );
}

#[test]
fn test_apply_launch_config_change_dart_defines_empty_list() {
    let mut config = LaunchConfig::default();
    config.dart_defines.insert("OLD".to_string(), "value".to_string());
    let item = SettingItem::new("launch.0.dart_defines", "Dart Defines")
        .value(SettingValue::List(vec![]));
    apply_launch_config_change(&mut config, &item);
    assert!(config.dart_defines.is_empty());
}
```

#### 6. Rendering integration tests

```rust
#[test]
fn test_render_add_config_button_selected() {
    let settings = Settings::default();
    let temp = tempdir().unwrap();
    init_launch_file(temp.path()).unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    // Set selected_index to the add-new button position
    let configs = load_launch_configs(temp.path());
    let item_count: usize = configs.iter().enumerate()
        .map(|(idx, r)| launch_config_items(&r.config, idx).len())
        .sum();
    state.selected_index = item_count; // add-new button

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        let panel = SettingsPanel::new(&settings, temp.path());
        frame.render_stateful_widget(panel, frame.area(), &mut state);
    }).unwrap();

    let content: String = terminal.backend().buffer().content()
        .iter().map(|c| c.symbol()).collect();
    // Should show the selection indicator on the Add New button
    assert!(content.contains("Add New Configuration"));
}
```

#### 7. Full quality gate

Run the complete verification suite:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

### Acceptance Criteria

1. All integration tests pass covering:
   - Add-new config navigation + creation end-to-end
   - Dart defines modal open → edit → close → persist
   - Extra args modal open → type → confirm → persist
   - Key routing with modals open vs closed
   - `apply_launch_config_change` for all 7 fields
   - Edge cases: empty lists, equals in values, no configs
2. All existing settings tests still pass (no regressions)
3. `cargo fmt --all` — formatted
4. `cargo check --workspace` — compiles
5. `cargo test --workspace` — all tests pass
6. `cargo clippy --workspace -- -D warnings` — no warnings

### Testing

This task IS the testing task. The tests listed above are the deliverables.

### Notes

- Use `tempdir()` from `tempfile` crate for all tests that involve disk I/O (launch config read/write)
- Use `init_launch_file()` to seed a default config for tests
- The `DartDefinesModalState` methods (`navigate_up/down`, `load_selected_into_edit`, `save_edit`, etc.) are already tested in the new session dialog test suite — focus integration tests on the settings-specific wiring, not on re-testing the modal state machine
- For rendering tests, use the existing `TestBackend` + `Terminal` pattern from `settings_panel/tests.rs`
- If any test failures are found during this task, fix the root cause in the relevant task's code (01-05), not by working around it in tests
