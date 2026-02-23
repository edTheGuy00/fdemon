## Task: Add Settings Modal State and Message Variants

**Objective**: Add the state fields and message variants needed to support dart defines and extra args modal editing in the settings panel. This is the foundational types-only task — no handler implementations or rendering.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/state.rs`: Add modal fields to `SettingsViewState`
- `crates/fdemon-app/src/new_session_dialog/types.rs`: Add `ExtraArgs` variant to `FuzzyModalType`
- `crates/fdemon-app/src/message.rs`: Add settings modal message variants
- `crates/fdemon-app/src/lib.rs`: Ensure new types are re-exported if needed

### Details

#### 1. Add modal state fields to `SettingsViewState`

Add two new optional fields to `SettingsViewState` at `state.rs:482-503`:

```rust
pub struct SettingsViewState {
    // ... existing 7 fields ...
    pub dart_defines_modal: Option<DartDefinesModalState>,
    pub extra_args_modal: Option<FuzzyModalState>,
}
```

Add a helper method:

```rust
impl SettingsViewState {
    /// Returns true if any modal overlay is currently open
    pub fn has_modal_open(&self) -> bool {
        self.dart_defines_modal.is_some() || self.extra_args_modal.is_some()
    }
}
```

Import `DartDefinesModalState` from `crate::new_session_dialog::state` and `FuzzyModalState` from the same module. Both are already `pub` and accessible from the `fdemon-app` crate.

**Default initialization:** Both fields default to `None` — add them to the `Default` impl or `new()` constructor for `SettingsViewState`.

#### 2. Add `ExtraArgs` variant to `FuzzyModalType`

In `new_session_dialog/types.rs:112-140`, add a new variant:

```rust
pub enum FuzzyModalType {
    Config,
    Flavor,
    EntryPoint,
    ExtraArgs,  // NEW — for settings panel extra args picker
}

impl FuzzyModalType {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config     => "Select Configuration",
            Self::Flavor     => "Select Flavor",
            Self::EntryPoint => "Select Entry Point",
            Self::ExtraArgs  => "Edit Extra Args",
        }
    }

    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config     => false,
            Self::Flavor     => true,
            Self::EntryPoint => true,
            Self::ExtraArgs  => true,  // Users can type arbitrary args
        }
    }
}
```

The `FuzzyModal` TUI widget calls `state.modal_type.title()` and `state.modal_type.allows_custom()` — both will work automatically with the new variant. No TUI widget changes needed.

#### 3. Add settings dart defines modal message variants

Add to `Message` enum in `message.rs`. Mirror the `NewSessionDialogDartDefines*` variants with a `Settings` prefix:

```rust
// Settings — Dart Defines Modal
SettingsDartDefinesOpen { config_idx: usize },
SettingsDartDefinesClose,
SettingsDartDefinesSwitchPane,
SettingsDartDefinesUp,
SettingsDartDefinesDown,
SettingsDartDefinesConfirm,
SettingsDartDefinesNextField,
SettingsDartDefinesInput { c: char },
SettingsDartDefinesBackspace,
SettingsDartDefinesSave,
SettingsDartDefinesDelete,
```

The `config_idx: usize` on `Open` identifies which launch config's dart defines to edit. This is extracted from the `SettingItem.id` pattern `"launch.{idx}.dart_defines"`.

#### 4. Add settings extra args fuzzy modal message variants

```rust
// Settings — Extra Args Fuzzy Modal
SettingsExtraArgsOpen { config_idx: usize },
SettingsExtraArgsClose,
SettingsExtraArgsInput { c: char },
SettingsExtraArgsBackspace,
SettingsExtraArgsClear,
SettingsExtraArgsUp,
SettingsExtraArgsDown,
SettingsExtraArgsConfirm,
```

The `config_idx: usize` on `Open` identifies which launch config's extra args to edit.

#### 5. Add placeholder match arms in `update()`

In the main `update()` function (`handler/update.rs`), add match arms for all new message variants that return `UpdateResult::none()` (no-op). This prevents compilation errors while the actual handlers are implemented in subsequent tasks:

```rust
Message::SettingsDartDefinesOpen { .. }
| Message::SettingsDartDefinesClose
| Message::SettingsDartDefinesSwitchPane
// ... etc
=> UpdateResult::none(),

Message::SettingsExtraArgsOpen { .. }
| Message::SettingsExtraArgsClose
// ... etc
=> UpdateResult::none(),
```

### Acceptance Criteria

1. `SettingsViewState` has `dart_defines_modal: Option<DartDefinesModalState>` and `extra_args_modal: Option<FuzzyModalState>` fields, both defaulting to `None`
2. `SettingsViewState::has_modal_open()` returns `true` when either modal is `Some`
3. `FuzzyModalType::ExtraArgs` variant exists with `title() = "Edit Extra Args"` and `allows_custom() = true`
4. All 19 new message variants compile and are handled (with no-op) in `update()`
5. `cargo check --workspace` passes — no compilation errors across all crates
6. `cargo test --workspace` passes — no regressions
7. `cargo clippy --workspace` passes — no new warnings

### Testing

```rust
#[test]
fn test_settings_view_state_has_modal_open() {
    let mut state = SettingsViewState::new();
    assert!(!state.has_modal_open());

    state.extra_args_modal = Some(FuzzyModalState::new(
        FuzzyModalType::ExtraArgs,
        vec!["--verbose".to_string()],
    ));
    assert!(state.has_modal_open());
}

#[test]
fn test_extra_args_fuzzy_modal_type() {
    assert_eq!(FuzzyModalType::ExtraArgs.title(), "Edit Extra Args");
    assert!(FuzzyModalType::ExtraArgs.allows_custom());
}
```

### Notes

- `DartDefinesModalState` and `FuzzyModalState` are in `crate::new_session_dialog::state` — they are already `pub` and available within the `fdemon-app` crate
- `DartDefine` struct (used by `DartDefinesModalState`) is in `crate::new_session_dialog::types` — also already `pub`
- The placeholder no-op match arms in `update()` are intentional — they will be replaced with real handler calls in tasks 03 and 04
- The `config_idx` on `Open` messages allows the handlers to load the correct config from disk and populate the modal with the right data
- Keep the `ExtraArgs` variant on `FuzzyModalType` rather than creating a separate enum — this allows full reuse of the `FuzzyModal` TUI widget without any changes to it
