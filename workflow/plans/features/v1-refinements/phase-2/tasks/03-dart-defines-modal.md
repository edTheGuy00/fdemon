## Task: Dart Defines Modal Handlers and Key Routing

**Objective**: Wire up the dart defines modal for the settings panel so that pressing Enter on a `dart_defines` setting item opens the `DartDefinesModal` overlay, allows full CRUD editing of key-value pairs, and persists changes on close.

**Depends on**: 02-settings-modal-state

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-app/src/handler/settings_handlers.rs`: Route Enter on `dart_defines` to open modal; add handler functions for all `SettingsDartDefines*` messages
- `crates/fdemon-app/src/handler/settings.rs`: Add `dart_defines` arm to `apply_launch_config_change()`
- `crates/fdemon-app/src/handler/keys.rs`: Intercept keys when dart defines modal is open in settings mode
- `crates/fdemon-app/src/handler/update.rs`: Replace no-op match arms with real handler calls

### Details

#### 1. Route Enter on `dart_defines` to open modal

In `handle_settings_toggle_edit()` at `settings_handlers.rs:74-112`, add a check before the `SettingValue` match:

```rust
// When user presses Enter on a dart_defines item, open the modal instead of inline edit
if item.id.ends_with(".dart_defines") {
    // Extract config_idx from ID: "launch.{idx}.dart_defines"
    let parts: Vec<&str> = item.id.split('.').collect();
    if let Some(idx_str) = parts.get(1) {
        if let Ok(config_idx) = idx_str.parse::<usize>() {
            return update(state, Message::SettingsDartDefinesOpen { config_idx });
        }
    }
    return UpdateResult::none();
}
```

#### 2. Implement `SettingsDartDefinesOpen` handler

In `settings_handlers.rs` (or a new `handler/settings_dart_defines.rs` submodule if the file grows too large):

```rust
pub fn handle_settings_dart_defines_open(state: &mut AppState, config_idx: usize) -> UpdateResult {
    // Load configs from disk
    let configs = load_launch_configs(&state.project_path);
    if let Some(resolved) = configs.get(config_idx) {
        // Convert HashMap<String,String> → Vec<DartDefine>
        let defines: Vec<DartDefine> = resolved.config.dart_defines.iter()
            .map(|(k, v)| DartDefine { key: k.clone(), value: v.clone() })
            .collect();
        state.settings_view_state.dart_defines_modal =
            Some(DartDefinesModalState::new(defines));
        // Store config_idx somewhere for use on close (see Notes)
    }
    UpdateResult::none()
}
```

**Important:** The `config_idx` needs to be available when the modal closes so we know which config to update. Options:
- Add a `editing_config_idx: Option<usize>` field to `SettingsViewState`
- Or encode it in the `DartDefinesModalState` (would require a new field on that struct)

The cleanest approach is adding `editing_config_idx: Option<usize>` to `SettingsViewState` and setting it on open.

#### 3. Implement remaining handler functions

Mirror the pattern from `handler/new_session/dart_defines_modal.rs`. Each handler is ~5-15 lines:

| Message | Handler Logic |
|---------|--------------|
| `SettingsDartDefinesClose` | Extract `modal.defines`, convert to `HashMap`, load config from disk, update `dart_defines`, save via `save_launch_configs()`, clear modal |
| `SettingsDartDefinesSwitchPane` | `modal.switch_pane()` |
| `SettingsDartDefinesUp` | `modal.navigate_up()` |
| `SettingsDartDefinesDown` | `modal.navigate_down()` |
| `SettingsDartDefinesConfirm` | In List pane: `modal.load_selected_into_edit()`. In Edit pane on Save: `modal.save_edit()`. On Delete: `modal.delete_selected()` |
| `SettingsDartDefinesNextField` | `modal.next_field()` |
| `SettingsDartDefinesInput { c }` | `modal.input_char(c)` |
| `SettingsDartDefinesBackspace` | `modal.backspace()` |
| `SettingsDartDefinesSave` | `modal.save_edit()` |
| `SettingsDartDefinesDelete` | `modal.delete_selected()` |

The close handler is the most complex — it must persist:

```rust
pub fn handle_settings_dart_defines_close(state: &mut AppState) -> UpdateResult {
    if let Some(modal) = state.settings_view_state.dart_defines_modal.take() {
        if let Some(config_idx) = state.settings_view_state.editing_config_idx.take() {
            let mut configs = load_launch_configs(&state.project_path);
            if let Some(resolved) = configs.get_mut(config_idx) {
                // Convert Vec<DartDefine> → HashMap<String, String>
                resolved.config.dart_defines = modal.defines.iter()
                    .map(|d| (d.key.clone(), d.value.clone()))
                    .collect();
                // Save all configs back to disk
                let config_vec: Vec<LaunchConfig> = configs.iter()
                    .map(|r| r.config.clone())
                    .collect();
                if let Err(e) = save_launch_configs(&state.project_path, &config_vec) {
                    state.settings_view_state.error = Some(format!("Failed to save: {}", e));
                }
            }
        }
    }
    UpdateResult::none()
}
```

#### 4. Add key routing for settings dart defines modal

In `handle_key_settings()` at `keys.rs:510-544`, add an early check:

```rust
pub fn handle_key_settings(state: &AppState, key: InputKey) -> Option<Message> {
    // If dart defines modal is open, route keys to it
    if state.settings_view_state.dart_defines_modal.is_some() {
        return handle_key_settings_dart_defines(state, key);
    }
    // ... existing code ...
}

fn handle_key_settings_dart_defines(state: &AppState, key: InputKey) -> Option<Message> {
    let modal = state.settings_view_state.dart_defines_modal.as_ref().unwrap();
    match modal.active_pane {
        DartDefinesPane::List => match key {
            InputKey::Up | InputKey::Char('k') => Some(Message::SettingsDartDefinesUp),
            InputKey::Down | InputKey::Char('j') => Some(Message::SettingsDartDefinesDown),
            InputKey::Enter => Some(Message::SettingsDartDefinesConfirm),
            InputKey::Tab => Some(Message::SettingsDartDefinesSwitchPane),
            InputKey::Esc => Some(Message::SettingsDartDefinesClose),
            _ => None,
        },
        DartDefinesPane::Edit => match modal.edit_field {
            DartDefinesEditField::Key | DartDefinesEditField::Value => match key {
                InputKey::Char(c) => Some(Message::SettingsDartDefinesInput { c }),
                InputKey::Backspace => Some(Message::SettingsDartDefinesBackspace),
                InputKey::Tab => Some(Message::SettingsDartDefinesNextField),
                InputKey::Enter => Some(Message::SettingsDartDefinesConfirm),
                InputKey::Esc => Some(Message::SettingsDartDefinesSwitchPane),
                _ => None,
            },
            DartDefinesEditField::Save => match key {
                InputKey::Enter => Some(Message::SettingsDartDefinesSave),
                InputKey::Tab => Some(Message::SettingsDartDefinesNextField),
                InputKey::Esc => Some(Message::SettingsDartDefinesSwitchPane),
                _ => None,
            },
            DartDefinesEditField::Delete => match key {
                InputKey::Enter => Some(Message::SettingsDartDefinesDelete),
                InputKey::Tab => Some(Message::SettingsDartDefinesNextField),
                InputKey::Esc => Some(Message::SettingsDartDefinesSwitchPane),
                _ => None,
            },
        },
    }
}
```

Reference the existing key routing in `handler/new_session/keys.rs` for the dart defines modal — the pattern should match.

#### 5. Add `dart_defines` arm to `apply_launch_config_change()`

In `handler/settings.rs:160-204`, add a match arm for `dart_defines`. This is used by the inline `SettingValue::List` edit path (separate from the modal path):

```rust
"dart_defines" => {
    if let SettingValue::List(items) = &item.value {
        config.dart_defines = items.iter()
            .filter_map(|s| {
                let parts: Vec<&str> = s.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect();
    }
}
```

#### 6. Replace no-op match arms in `update()`

Replace the placeholder `UpdateResult::none()` arms from task 02 with real handler calls.

### Acceptance Criteria

1. Pressing Enter on a `dart_defines` setting item opens the `DartDefinesModal` (state: `dart_defines_modal.is_some()`)
2. All key inputs while the modal is open are routed to the dart defines modal handlers (not the regular settings handlers)
3. Adding/editing/deleting dart defines in the modal works correctly (via `DartDefinesModalState` methods)
4. Closing the modal (Esc) persists changes: `dart_defines` HashMap is updated on the `LaunchConfig` and saved to `.fdemon/launch.toml`
5. `apply_launch_config_change()` handles the `dart_defines` field (for inline list edit fallback)
6. `cargo test -p fdemon-app` passes with all new and existing tests
7. `cargo clippy -p fdemon-app` passes

### Testing

```rust
#[test]
fn test_enter_on_dart_defines_opens_modal() {
    // Create a state with selected_index pointing to a dart_defines item
    // Call handle_settings_toggle_edit
    // Verify it returns SettingsDartDefinesOpen message
}

#[test]
fn test_dart_defines_modal_close_persists() {
    // Create state with dart_defines_modal = Some(...)
    // Add defines to the modal
    // Call handle_settings_dart_defines_close
    // Verify launch config on disk has updated dart_defines
}

#[test]
fn test_apply_launch_config_change_dart_defines() {
    let mut config = LaunchConfig::default();
    let item = SettingItem::new("launch.0.dart_defines", "Dart Defines")
        .value(SettingValue::List(vec!["KEY=VALUE".to_string(), "FOO=BAR".to_string()]));
    apply_launch_config_change(&mut config, &item);
    assert_eq!(config.dart_defines.get("KEY"), Some(&"VALUE".to_string()));
    assert_eq!(config.dart_defines.get("FOO"), Some(&"BAR".to_string()));
}

#[test]
fn test_key_routing_when_dart_defines_modal_open() {
    // Verify that key events are routed to dart defines handlers
    // when the modal is open, not to regular settings handlers
}
```

### Notes

- The `DartDefinesModalState` and its methods are already fully implemented in `new_session_dialog/state.rs:184-399` — we are reusing them as-is
- `DartDefine { key, value }` is defined in `new_session_dialog/types.rs:143-161`
- The key routing pattern should closely follow `handler/new_session/keys.rs` for dart defines modal keys — reference lines 670-720+ (or wherever the dart defines key handler is)
- Consider extracting the dart defines handler functions into a dedicated `handler/settings_dart_defines.rs` module if `settings_handlers.rs` exceeds 500 lines (per CODE_STANDARDS.md)
- The `editing_config_idx` field on `SettingsViewState` is needed to track which config is being edited — set on open, cleared on close
- The `HashMap` → `Vec<DartDefine>` conversion on open and the reverse on close must handle the case where keys have `=` characters in values (use `splitn(2, '=')`)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `editing_config_idx: Option<usize>` field to `SettingsViewState` struct and its `Default` impl |
| `crates/fdemon-app/src/handler/mod.rs` | Registered new `settings_dart_defines` submodule; updated doc comment |
| `crates/fdemon-app/src/handler/settings_dart_defines.rs` | **New file** — 10 handler functions + 10 unit tests covering open/close/persist/CRUD/key-routing |
| `crates/fdemon-app/src/handler/settings_handlers.rs` | Added early-exit branch in `handle_settings_toggle_edit()` to route dart_defines items to `SettingsDartDefinesOpen` message |
| `crates/fdemon-app/src/handler/settings.rs` | Added `"dart_defines"` arm to `apply_launch_config_change()` using `splitn(2, '=')` for safe parsing; added 3 new tests |
| `crates/fdemon-app/src/handler/keys.rs` | Added early-exit guard in `handle_key_settings()` to route keys to `handle_key_settings_dart_defines()`; added new `handle_key_settings_dart_defines()` function with pane-aware key routing |
| `crates/fdemon-app/src/handler/update.rs` | Replaced 11 no-op placeholder match arms with real calls into `settings_dart_defines`; added `settings_dart_defines` to imports |

### Notable Decisions/Tradeoffs

1. **Dedicated submodule**: Per CODE_STANDARDS.md guidance and task notes, handlers were placed in `handler/settings_dart_defines.rs` rather than growing `settings_handlers.rs`. This keeps all files under 500 lines.
2. **editing_config_idx placement**: Added to `SettingsViewState` rather than `DartDefinesModalState` to keep the modal state reusable and consistent with the `NewSessionDialog` pattern.
3. **HashMap ordering**: The open handler iterates the `HashMap` — defines may appear in an arbitrary order in the modal list. This is acceptable since dart defines are key→value pairs without semantic ordering.
4. **`splitn(2, '=')` in apply_launch_config_change**: Values containing `=` (e.g. URLs) are preserved correctly. Entries without `=` are silently skipped.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app` — **1082 tests pass** (10 new in `settings_dart_defines`)
- `cargo test --workspace` — All crates pass (2689+ tests total, 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **HashMap ordering in modal list**: Dart defines loaded from disk appear in HashMap iteration order (arbitrary). If users care about ordering, a future `IndexMap` migration would be needed — but this matches the existing `NewSessionDialog` behaviour.
2. **No modal dirty-tracking**: Closing the modal always attempts a save even if no changes were made. The save is idempotent (writes the same TOML), so this is safe but slightly inefficient.
