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
