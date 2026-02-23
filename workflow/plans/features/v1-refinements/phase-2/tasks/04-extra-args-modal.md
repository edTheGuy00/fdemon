## Task: Extra Args Fuzzy Modal Handlers and Key Routing

**Objective**: Wire up the extra args fuzzy modal for the settings panel so that pressing Enter on an `extra_args` setting item opens a `FuzzyModal` overlay with `allows_custom: true`, allowing users to select common args or type custom ones, and persists changes on confirm.

**Depends on**: 02-settings-modal-state

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/settings_handlers.rs`: Route Enter on `extra_args` to open modal; add handler functions for all `SettingsExtraArgs*` messages
- `crates/fdemon-app/src/handler/settings.rs`: Add `extra_args` arm to `apply_launch_config_change()`
- `crates/fdemon-app/src/handler/keys.rs`: Intercept keys when extra args modal is open in settings mode
- `crates/fdemon-app/src/handler/update.rs`: Replace no-op match arms with real handler calls

### Details

#### 1. Route Enter on `extra_args` to open modal

In `handle_settings_toggle_edit()` at `settings_handlers.rs`, add a check before the `SettingValue` match (similar to dart_defines routing in task 03):

```rust
if item.id.ends_with(".extra_args") {
    let parts: Vec<&str> = item.id.split('.').collect();
    if let Some(idx_str) = parts.get(1) {
        if let Ok(config_idx) = idx_str.parse::<usize>() {
            return update(state, Message::SettingsExtraArgsOpen { config_idx });
        }
    }
    return UpdateResult::none();
}
```

#### 2. Implement `SettingsExtraArgsOpen` handler

```rust
pub fn handle_settings_extra_args_open(state: &mut AppState, config_idx: usize) -> UpdateResult {
    let configs = load_launch_configs(&state.project_path);
    if let Some(resolved) = configs.get(config_idx) {
        let items = resolved.config.extra_args.clone();
        state.settings_view_state.extra_args_modal = Some(
            FuzzyModalState::new(FuzzyModalType::ExtraArgs, items)
        );
        state.settings_view_state.editing_config_idx = Some(config_idx);
    }
    UpdateResult::none()
}
```

The modal opens with the current `extra_args` as the item list. Since `allows_custom` is `true`, users can type arbitrary arguments and press Enter to add them.

#### 3. Implement remaining handler functions

Mirror the pattern from `handler/new_session/fuzzy_modal.rs`. Each handler is ~5-10 lines:

| Message | Handler Logic |
|---------|--------------|
| `SettingsExtraArgsClose` | Clear `extra_args_modal` and `editing_config_idx` (cancel, no save) |
| `SettingsExtraArgsInput { c }` | `modal.input_char(c)` + re-run `fuzzy_filter` |
| `SettingsExtraArgsBackspace` | `modal.backspace()` + re-run `fuzzy_filter` |
| `SettingsExtraArgsClear` | `modal.clear_query()` + re-run `fuzzy_filter` |
| `SettingsExtraArgsUp` | `modal.navigate_up()` |
| `SettingsExtraArgsDown` | `modal.navigate_down()` |
| `SettingsExtraArgsConfirm` | See below |

**Confirm handler — the key decision:**

The extra args modal is used to **manage a list** of args, not just pick one. The UX should work as follows:

1. **Select existing arg** → remove it from the list (toggle off)
2. **Type custom arg + Enter** → add it to the list
3. On close (Esc), the current list state is persisted

Alternative simpler approach: Use the fuzzy modal as an "add one arg" flow:
1. Open modal with existing args as display-only context
2. User types a new arg or selects from common presets
3. Confirm adds the arg to the list; cancel discards

The simpler approach is recommended. The confirm handler:

```rust
pub fn handle_settings_extra_args_confirm(state: &mut AppState) -> UpdateResult {
    if let Some(modal) = &state.settings_view_state.extra_args_modal {
        if let Some(selected) = modal.selected_value() {
            if let Some(config_idx) = state.settings_view_state.editing_config_idx {
                let mut configs = load_launch_configs(&state.project_path);
                if let Some(resolved) = configs.get_mut(config_idx) {
                    // Add the arg if not already present
                    if !resolved.config.extra_args.contains(&selected) {
                        resolved.config.extra_args.push(selected);
                    }
                    let config_vec: Vec<LaunchConfig> = configs.iter()
                        .map(|r| r.config.clone())
                        .collect();
                    if let Err(e) = save_launch_configs(&state.project_path, &config_vec) {
                        state.settings_view_state.error = Some(format!("Failed to save: {}", e));
                    }
                }
            }
        }
    }
    // Close the modal after confirm
    state.settings_view_state.extra_args_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}
```

#### 4. Add key routing for settings extra args modal

In `handle_key_settings()` at `keys.rs`, add after the dart defines modal check:

```rust
// If extra args modal is open, route keys to it
if state.settings_view_state.extra_args_modal.is_some() {
    return handle_key_settings_extra_args(state, key);
}
```

```rust
fn handle_key_settings_extra_args(_state: &AppState, key: InputKey) -> Option<Message> {
    match key {
        InputKey::Char(c) => Some(Message::SettingsExtraArgsInput { c }),
        InputKey::Backspace => Some(Message::SettingsExtraArgsBackspace),
        InputKey::Up => Some(Message::SettingsExtraArgsUp),
        InputKey::Down => Some(Message::SettingsExtraArgsDown),
        InputKey::Enter => Some(Message::SettingsExtraArgsConfirm),
        InputKey::Esc => Some(Message::SettingsExtraArgsClose),
        InputKey::CtrlChar('u') => Some(Message::SettingsExtraArgsClear),
        _ => None,
    }
}
```

#### 5. Add `extra_args` arm to `apply_launch_config_change()`

In `handler/settings.rs:160-204`:

```rust
"extra_args" => {
    if let SettingValue::List(items) = &item.value {
        config.extra_args = items.clone();
    }
}
```

This handles the inline list edit fallback path (separate from the modal path).

#### 6. Apply `fuzzy_filter` after input mutations

After `input_char`, `backspace`, and `clear_query`, re-run the fuzzy filter:

```rust
fn apply_settings_extra_args_filter(state: &mut AppState) {
    if let Some(modal) = &mut state.settings_view_state.extra_args_modal {
        let filtered = fuzzy_filter(&modal.query, &modal.items);
        modal.update_filter(filtered);
    }
}
```

Call this in the `Input`, `Backspace`, and `Clear` handlers after mutating the query.

#### 7. Replace no-op match arms in `update()`

Replace the placeholder arms from task 02 with real handler calls.

### Acceptance Criteria

1. Pressing Enter on an `extra_args` setting item opens the `FuzzyModal` overlay (state: `extra_args_modal.is_some()`)
2. The modal shows existing args as items, with fuzzy filtering
3. Typing a custom arg and pressing Enter adds it to the config's `extra_args` list
4. Selecting an existing arg from the filtered list and pressing Enter adds it (or allows editing)
5. Esc closes the modal without changes
6. Changes are persisted to `.fdemon/launch.toml`
7. `apply_launch_config_change()` handles the `extra_args` field
8. `cargo test -p fdemon-app` passes with all new and existing tests
9. `cargo clippy -p fdemon-app` passes

### Testing

```rust
#[test]
fn test_enter_on_extra_args_opens_modal() {
    // Create state with selected_index on extra_args item
    // Call handle_settings_toggle_edit
    // Verify it returns SettingsExtraArgsOpen message
}

#[test]
fn test_extra_args_confirm_adds_arg() {
    // Create state with extra_args_modal open, query = "--verbose"
    // Call handle_settings_extra_args_confirm
    // Verify config.extra_args now contains "--verbose"
}

#[test]
fn test_extra_args_close_does_not_persist() {
    // Create state with extra_args_modal open
    // Call handle_settings_extra_args_close
    // Verify modal is cleared, no disk write
}

#[test]
fn test_apply_launch_config_change_extra_args() {
    let mut config = LaunchConfig::default();
    let item = SettingItem::new("launch.0.extra_args", "Extra Args")
        .value(SettingValue::List(vec!["--verbose".to_string(), "--trace-startup".to_string()]));
    apply_launch_config_change(&mut config, &item);
    assert_eq!(config.extra_args, vec!["--verbose", "--trace-startup"]);
}
```

### Notes

- `FuzzyModalState::new(FuzzyModalType::ExtraArgs, items)` initializes with all items visible (empty query shows all)
- `fuzzy_filter()` is in `crate::new_session_dialog::fuzzy` — import it
- `FuzzyModalState::selected_value()` returns raw query text when `allows_custom` is true and no items match — this is how custom args are captured
- The `editing_config_idx` field (added in task 03 or here) must be shared between dart defines and extra args — only one modal can be open at a time
- Consider common Flutter args as preset items in the modal: `["--verbose", "--trace-startup", "--trace-skia", "--enable-software-rendering", "--dart-entrypoint-args"]` — or just use the current args list as items
- The extra args flow is simpler than dart defines because it's a flat `Vec<String>` with no key-value structure

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/settings_extra_args.rs` | New module: all extra args modal handlers (`open`, `close`, `input`, `backspace`, `clear`, `up`, `down`, `confirm`) plus inline tests |
| `crates/fdemon-app/src/handler/mod.rs` | Declared new `settings_extra_args` module |
| `crates/fdemon-app/src/handler/settings_handlers.rs` | Added routing for `.extra_args` items in `handle_settings_toggle_edit()` to dispatch `SettingsExtraArgsOpen` |
| `crates/fdemon-app/src/handler/keys.rs` | Added `extra_args_modal` check in `handle_key_settings()` and new `handle_key_settings_extra_args()` function |
| `crates/fdemon-app/src/handler/update.rs` | Replaced placeholder no-op arms with real `settings_extra_args::*` handler calls; added `settings_extra_args` to imports |
| `crates/fdemon-app/src/handler/settings.rs` | Added `extra_args` arm in `apply_launch_config_change()` plus two new tests |

### Notable Decisions/Tradeoffs

1. **Preset args when list is empty**: When the config's `extra_args` is empty, the modal shows common Flutter flag presets (`--verbose`, `--trace-startup`, etc.) so the user has something to pick from. When args already exist, only the existing args are shown as items (user can still type custom args via the query).
2. **Confirm adds, close cancels**: Confirm appends the selected (or typed) value to the config's `extra_args` without duplicating. Esc/close discards without persisting. This matches the "add one arg" simplified approach from the task spec.
3. **Shared `editing_config_idx`**: `editing_config_idx` is shared between the dart defines modal and the extra args modal — only one modal is ever open at a time, so this is safe.
4. **Key routing order in `handle_key_settings()`**: Extra args modal check is placed after dart defines modal check, matching the task spec and ensuring only one modal intercepts keys at a time.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app` — Passed (1098 tests, 0 failed)
- `cargo clippy --workspace -- -D warnings` — Passed

### Risks/Limitations

1. **Remove-from-list UX not implemented**: The task spec noted a simpler "add one arg" flow was recommended. Removing an arg from the list requires re-opening the modal and is not yet supported via a dedicated delete action. This is an acceptable scope boundary — the task spec explicitly recommends the simpler approach.
