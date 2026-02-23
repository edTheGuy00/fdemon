## Task: Add mutual exclusion guard to modal open handlers

**Objective**: Add an early-return guard using `has_modal_open()` at the top of both `handle_settings_dart_defines_open()` and `handle_settings_extra_args_open()` to prevent two modals from being open simultaneously.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

**Review Issues**: Critical #2

### Scope

- `crates/fdemon-app/src/handler/settings_dart_defines.rs`: Add guard to `handle_settings_dart_defines_open()`
- `crates/fdemon-app/src/handler/settings_extra_args.rs`: Add guard to `handle_settings_extra_args_open()`

### Details

#### 1. Guard in `handle_settings_dart_defines_open()`

Add at the very top of the function (before `load_launch_configs`):

```rust
pub fn handle_settings_dart_defines_open(state: &mut AppState, config_idx: usize) -> UpdateResult {
    if state.settings_view_state.has_modal_open() {
        return UpdateResult::none();
    }
    // ... existing logic ...
}
```

This prevents opening the dart defines modal when the extra args modal (or another dart defines instance) is already active. It also avoids the unnecessary `load_launch_configs()` disk I/O when the guard triggers.

#### 2. Guard in `handle_settings_extra_args_open()`

Same pattern:

```rust
pub fn handle_settings_extra_args_open(state: &mut AppState, config_idx: usize) -> UpdateResult {
    if state.settings_view_state.has_modal_open() {
        return UpdateResult::none();
    }
    // ... existing logic ...
}
```

### Acceptance Criteria

1. `handle_settings_dart_defines_open()` returns `UpdateResult::none()` when any modal is already open
2. `handle_settings_extra_args_open()` returns `UpdateResult::none()` when any modal is already open
3. The guard fires before `load_launch_configs()` (no unnecessary disk I/O)
4. Existing behavior when no modal is open is unchanged

### Testing

Add tests in the respective test modules:

```rust
#[test]
fn test_dart_defines_open_blocked_when_extra_args_modal_active() {
    // Set extra_args_modal to Some, dispatch DartDefinesOpen, verify dart_defines_modal is still None
}

#[test]
fn test_extra_args_open_blocked_when_dart_defines_modal_active() {
    // Set dart_defines_modal to Some, dispatch ExtraArgsOpen, verify extra_args_modal is still None
}

#[test]
fn test_dart_defines_open_blocked_when_already_open() {
    // Set dart_defines_modal to Some, dispatch DartDefinesOpen again, verify state unchanged
}
```

### Notes

- `has_modal_open()` is already defined on `SettingsViewState` at `state.rs:546-548`
- While key routing currently prevents simultaneous modals in practice (modal keys are consumed before reaching the other modal's open dispatch), this guard protects against programmatic `Message` dispatch
- The shared `editing_config_idx` field is the reason this matters — without the guard, a second open would overwrite the index used by the first modal's close handler
