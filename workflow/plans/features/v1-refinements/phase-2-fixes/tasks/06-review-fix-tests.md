## Task: Add verification tests for all review fixes

**Objective**: Add focused tests verifying each critical and major fix from the phase-2 review. These tests serve as regression anchors ensuring the fixes are not accidentally reverted.

**Depends on**: 01-dart-defines-cancel, 02-modal-open-guard, 03-magic-string-constants, 04-extra-args-empty-confirm, 05-modal-state-cleanup

**Estimated Time**: 1.5-2 hours

**Review Issues**: Cross-cutting verification for Critical #1, Critical #2, Major #6

### Scope

- `crates/fdemon-app/src/handler/settings_dart_defines.rs`: Tests for cancel behavior, sort order
- `crates/fdemon-app/src/handler/settings_extra_args.rs`: Tests for open guard, empty confirm
- `crates/fdemon-app/src/state.rs`: Tests for hide_settings modal cleanup
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Optional rendering test for cancel

### Details

#### Test Group 1: Dart defines cancel (Critical #1)

```rust
#[test]
fn test_esc_in_dart_defines_list_sends_cancel_not_close() {
    // Setup: state with dart defines modal open, List pane active
    // Action: simulate Esc keypress
    // Assert: Message produced is SettingsDartDefinesCancel (not Close)
}

#[test]
fn test_dart_defines_cancel_does_not_persist() {
    // Setup: open dart defines modal, add a new define via handler
    // Action: send SettingsDartDefinesCancel
    // Assert: modal is None, editing_config_idx is None
    // Assert: re-load launch configs from disk shows original (unmodified) defines
}

#[test]
fn test_dart_defines_close_still_persists() {
    // Setup: open dart defines modal, modify defines
    // Action: send SettingsDartDefinesClose
    // Assert: launch configs on disk reflect the modifications
}
```

#### Test Group 2: Modal open guard (Critical #2)

```rust
#[test]
fn test_dart_defines_open_noop_when_extra_args_modal_active() {
    // Setup: open extra args modal (state.extra_args_modal = Some)
    // Action: send SettingsDartDefinesOpen
    // Assert: dart_defines_modal is still None
    // Assert: editing_config_idx unchanged (still points to extra args config)
}

#[test]
fn test_extra_args_open_noop_when_dart_defines_modal_active() {
    // Setup: open dart defines modal (state.dart_defines_modal = Some)
    // Action: send SettingsExtraArgsOpen
    // Assert: extra_args_modal is still None
}
```

#### Test Group 3: Extra args empty confirm (Major #6)

```rust
#[test]
fn test_extra_args_confirm_with_no_selection_keeps_modal_open() {
    // Setup: open extra args modal, set query to filter out all items
    // Action: send SettingsExtraArgsConfirm
    // Assert: extra_args_modal is still Some (modal stays open)
}
```

#### Test Group 4: Modal state cleanup (Minor #7, #8)

```rust
#[test]
fn test_hide_settings_clears_all_modal_state() {
    // Setup: show_settings, open dart defines modal
    // Action: hide_settings
    // Assert: dart_defines_modal is None
    // Assert: editing_config_idx is None
    // Assert: has_modal_open() returns false
}
```

#### Test Group 5: Alphabetical sort (Minor #9)

```rust
#[test]
fn test_dart_defines_sorted_alphabetically_on_open() {
    // Setup: create launch config with defines {"zebra": "1", "apple": "2", "mango": "3"}
    // Action: send SettingsDartDefinesOpen
    // Assert: modal defines order is ["apple", "mango", "zebra"]
}
```

### Acceptance Criteria

1. All new tests pass
2. Tests cover every critical and major fix
3. Tests use descriptive names following project conventions
4. No existing tests broken
5. `cargo test --workspace` passes

### Testing

This task IS the testing task. Verify with:

```bash
cargo test -p fdemon-app -- settings_dart_defines
cargo test -p fdemon-app -- settings_extra_args
cargo test -p fdemon-app -- settings
cargo test --workspace
```

### Notes

- Tests should use the existing test helper patterns from the codebase (`AppState::test_default()`, tempdir-based config files, etc.)
- Each test should be minimal — test one behavior, not an entire flow
- Tests in this task may overlap with tests written inline by tasks 01-05. Deduplicate if needed — the goal is coverage, not duplication.
- The magic string constants task (03) is pure refactoring with no behavioral change, so it does not need dedicated new tests — existing tests serve as regression anchors.
