## Task: Clear modal state on hide_settings and document shared field

**Objective**: Make `hide_settings()` clear modal state (dart_defines_modal, extra_args_modal, editing_config_idx) to prevent stale state from leaking. Add a prominent comment documenting the shared `editing_config_idx` invariant.

**Depends on**: None

**Estimated Time**: 1-2 hours

**Review Issues**: Minor #7 (shared editing_config_idx), Minor #8 (hide_settings modal leak)

### Scope

- `crates/fdemon-app/src/state.rs`: Modify `hide_settings()`, add documentation to `editing_config_idx`

### Details

#### 1. Clear modal state in `hide_settings()`

Currently `hide_settings()` (lines 867-870) only sets `ui_mode`:

```rust
pub fn hide_settings(&mut self) {
    self.ui_mode = UiMode::Normal;
}
```

While `show_settings()` replaces the entire `settings_view_state` with a fresh default, the gap between hide and re-open leaves stale modal data in memory. If any code checks `has_modal_open()` during this window, it gets a false positive.

**Fix**: Clear modal-related fields in `hide_settings()`:

```rust
pub fn hide_settings(&mut self) {
    self.settings_view_state.dart_defines_modal = None;
    self.settings_view_state.extra_args_modal = None;
    self.settings_view_state.editing_config_idx = None;
    self.ui_mode = UiMode::Normal;
}
```

#### 2. Document the shared `editing_config_idx` invariant

On the `editing_config_idx` field (around line 512), update the doc comment to explicitly document the sharing:

```rust
/// The 0-based index of the launch config currently being edited.
///
/// **SHARED** between `dart_defines_modal` and `extra_args_modal` —
/// only one modal may be open at a time. The `has_modal_open()` guard
/// in each open handler enforces this invariant.
///
/// Set on modal open, cleared on modal close/cancel.
pub editing_config_idx: Option<usize>,
```

This makes the runtime invariant visible to future developers. Splitting into separate fields (`dart_defines_config_idx` / `extra_args_config_idx`) was considered but deferred — the guard in task 02 provides sufficient safety, and separate fields would add redundancy without compile-time benefit (both would still be `Option<usize>` with the same invariant).

### Acceptance Criteria

1. `hide_settings()` clears `dart_defines_modal`, `extra_args_modal`, and `editing_config_idx`
2. `has_modal_open()` returns `false` after `hide_settings()` is called
3. `editing_config_idx` has a doc comment documenting the shared invariant and referencing the guard
4. Existing tests pass without modification

### Testing

```rust
#[test]
fn test_hide_settings_clears_modal_state() {
    let mut state = AppState::test_default();
    state.show_settings();
    state.settings_view_state.dart_defines_modal = Some(DartDefinesModalState::new(vec![]));
    state.settings_view_state.editing_config_idx = Some(0);
    state.hide_settings();
    assert!(state.settings_view_state.dart_defines_modal.is_none());
    assert!(state.settings_view_state.editing_config_idx.is_none());
    assert!(!state.settings_view_state.has_modal_open());
}
```

### Notes

- `show_settings()` already replaces the entire `SettingsViewState`, so this cleanup in `hide_settings()` is technically redundant for the hide → show cycle. However, it makes `hide_settings()` self-consistent and prevents any code that checks modal state between hide and the next show from getting confused.
- The `handle_force_hide_settings()` handler calls `state.hide_settings()`, so it will also benefit from this fix.
- Splitting `editing_config_idx` into per-modal fields is deferred as over-engineering given the guard added in task 02.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `dart_defines_modal`, `editing_config_idx`, `extra_args_modal` fields to `SettingsViewState`; added `has_modal_open()` method; updated `hide_settings()` to clear all three modal fields; added import for `DartDefinesModalState` and `FuzzyModalState`; added 4 tests |
| `crates/fdemon-app/src/new_session_dialog/types.rs` | Added `ExtraArgs` variant to `FuzzyModalType` with `title() = "Edit Extra Args"` and `allows_custom() = true` |
| `crates/fdemon-app/src/new_session_dialog/state.rs` | Added `ExtraArgs` no-op arm to `close_fuzzy_modal_with_selection()` match |
| `crates/fdemon-app/src/handler/new_session/fuzzy_modal.rs` | Added `ExtraArgs` no-op/warn arms to `handle_open_fuzzy_modal()` and `handle_fuzzy_confirm()` |

### Notable Decisions/Tradeoffs

1. **Prerequisite modal state added**: The worktree branch predated the phase-2 modal state work. The task assumed the modal fields already existed in `SettingsViewState`, but they did not. Added all prerequisite work (fields, `has_modal_open()`, `FuzzyModalType::ExtraArgs`) inline as part of this task to satisfy the acceptance criteria.

2. **`AppState::test_default()` replaced with `AppState::new()`**: The task test spec used `AppState::test_default()` which does not exist in the codebase. Used `AppState::new()` instead, which is consistent with other tests in `state.rs`.

3. **`editing_config_idx` doc comment**: Added the full SHARED invariant doc comment as specified, documenting that only one modal may be open at a time and that `has_modal_open()` guards enforce this.

4. **No-op ExtraArgs arms**: Added `ExtraArgs` arms to `NewSessionDialog`-owned fuzzy modal handlers. These are no-ops by design — `ExtraArgs` modals are owned by `SettingsViewState::extra_args_modal`, not `NewSessionDialogState::fuzzy_modal`. Needed to satisfy Rust match exhaustiveness.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed (no compilation errors)
- `cargo test -p fdemon-app -- hide_settings` - Passed (1 test)
- `cargo test -p fdemon-app -- settings` - Passed (108 tests)
- `cargo test -p fdemon-app` - Passed (910 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Worktree divergence**: The worktree was missing all phase-2 settings modal infrastructure (committed in `854a05a` on develop). Required adding prerequisite fields and `FuzzyModalType::ExtraArgs` as part of this task. When this branch is merged back to develop, these changes will need to be reconciled with the existing develop code to avoid duplication.
