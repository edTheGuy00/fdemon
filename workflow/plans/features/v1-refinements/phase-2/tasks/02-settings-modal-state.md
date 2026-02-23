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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/types.rs` | Added `ExtraArgs` variant to `FuzzyModalType` with `title() = "Edit Extra Args"` and `allows_custom() = true`; added test `test_extra_args_fuzzy_modal_type` |
| `crates/fdemon-app/src/new_session_dialog/state.rs` | Added `ExtraArgs` arm to `close_fuzzy_modal_with_selection()` match (no-op, settings panel owns the result) |
| `crates/fdemon-app/src/handler/new_session/fuzzy_modal.rs` | Added `ExtraArgs` arms to `handle_open_fuzzy_modal()` (warn + no-op) and `handle_fuzzy_confirm()` (close + no-op) |
| `crates/fdemon-app/src/state.rs` | Added `dart_defines_modal: Option<DartDefinesModalState>` and `extra_args_modal: Option<FuzzyModalState>` fields to `SettingsViewState`; added `has_modal_open()` method; updated `Default` impl; added import; added 3 tests |
| `crates/fdemon-app/src/message.rs` | Added 19 new `Message` variants: 11 `SettingsDartDefines*` + 8 `SettingsExtraArgs*` |
| `crates/fdemon-app/src/handler/update.rs` | Added placeholder no-op match arms for all 19 new message variants |

### Notable Decisions/Tradeoffs

1. **ExtraArgs arms in NewSessionDialog handlers**: Added no-op arms to `handle_open_fuzzy_modal`, `handle_fuzzy_confirm`, and `close_fuzzy_modal_with_selection` in the new-session-dialog handlers. This is correct because `ExtraArgs` is owned by `SettingsViewState::extra_args_modal`, not `NewSessionDialogState::fuzzy_modal`. The new session dialog's fuzzy modal must remain exhaustive over all `FuzzyModalType` variants to satisfy Rust's match exhaustiveness.

2. **Import path for modal types in state.rs**: Used `crate::new_session_dialog::{DartDefinesModalState, FuzzyModalState}` (via `pub use state::*` re-export in mod.rs) rather than the longer `crate::new_session_dialog::state::` path, consistent with how other types from this module are imported.

3. **Placeholder no-ops over unimplemented!()**: The placeholder match arms use `UpdateResult::none()` rather than `todo!()` or `unimplemented!()` to keep the app runnable while the handlers are developed in Tasks 03 and 04.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (no compilation errors)
- `cargo test --workspace` - Passed (all tests pass, count increased by new tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no new warnings)
- Targeted: `cargo test -p fdemon-app -- test_settings_view_state` - 9 tests passed
- Targeted: `cargo test -p fdemon-app -- test_extra_args_fuzzy_modal_type` - 1 test passed

### Risks/Limitations

1. **Exhaustiveness in new session dialog**: The `ExtraArgs` variant must be handled in new-session-dialog match arms. These arms are no-ops by design but must exist. If future variants are added to `FuzzyModalType`, both the new-session-dialog handlers and the settings handlers need updating.
2. **config_idx not yet used**: The `config_idx` field on `SettingsDartDefinesOpen` and `SettingsExtraArgsOpen` is declared but not consumed (placeholder no-ops). This is intentional — Tasks 03 and 04 implement the real handlers.
