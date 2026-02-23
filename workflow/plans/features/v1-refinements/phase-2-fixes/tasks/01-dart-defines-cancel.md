## Task: Add dart defines cancel path (Esc discards changes)

**Objective**: Add a `SettingsDartDefinesCancel` message that closes the dart defines modal without persisting changes. Map Esc in the List pane to Cancel instead of Close. Fix the inaccurate doc comment, add a `tracing::warn` for the missing config_idx edge case, and sort HashMap entries when loading.

**Depends on**: None

**Estimated Time**: 2-3 hours

**Review Issues**: Critical #1, Major #4, Major #5, Minor #9

### Scope

- `crates/fdemon-app/src/message.rs`: Add `SettingsDartDefinesCancel` variant, fix doc comments on Close
- `crates/fdemon-app/src/handler/settings_dart_defines.rs`: Add `handle_settings_dart_defines_cancel()`, add `tracing::warn` in close handler, sort defines on open
- `crates/fdemon-app/src/handler/keys.rs`: Map Esc in `DartDefinesPane::List` to Cancel instead of Close
- `crates/fdemon-app/src/handler/update.rs`: Route new Cancel message to handler

### Details

#### 1. Add `SettingsDartDefinesCancel` message variant

In `message.rs`, add a new variant near the existing `SettingsDartDefinesClose`:

```rust
/// Close the dart defines modal and persist all changes to disk.
SettingsDartDefinesClose,

/// Cancel the dart defines modal, discarding any unsaved changes.
SettingsDartDefinesCancel,
```

Also fix the existing doc comment on `SettingsDartDefinesClose` — it currently says "without saving changes" but the handler saves. After this change, Close = save, Cancel = discard.

#### 2. Add `handle_settings_dart_defines_cancel()` handler

In `settings_dart_defines.rs`, add a cancel handler that clears modal state without persisting:

```rust
/// Cancel the dart defines modal without persisting changes.
pub fn handle_settings_dart_defines_cancel(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.dart_defines_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}
```

This mirrors the extra args close handler pattern (`settings_extra_args.rs:50-54`).

#### 3. Add `tracing::warn` for missing config_idx in close handler

In `handle_settings_dart_defines_close()`, when `editing_config_idx` is `None` but `dart_defines_modal` is `Some`, the modal is consumed via `.take()` without persisting. Add a warning:

```rust
pub fn handle_settings_dart_defines_close(state: &mut AppState) -> UpdateResult {
    if let Some(modal) = state.settings_view_state.dart_defines_modal.take() {
        if let Some(config_idx) = state.settings_view_state.editing_config_idx.take() {
            // ... existing persist logic ...
        } else {
            tracing::warn!("dart defines modal closed with no editing_config_idx — changes discarded");
        }
    }
    UpdateResult::none()
}
```

#### 4. Map Esc to Cancel in keys.rs

In `keys.rs` around line 652, change the Esc mapping in `DartDefinesPane::List`:

```rust
// Before:
KeyCode::Esc => Message::SettingsDartDefinesClose,
// After:
KeyCode::Esc => Message::SettingsDartDefinesCancel,
```

Leave the Esc in `DartDefinesPane::Edit` unchanged — it correctly switches back to the List pane.

#### 5. Route Cancel in update.rs

In `update.rs`, add routing for the new message variant:

```rust
Message::SettingsDartDefinesCancel => {
    settings_dart_defines::handle_settings_dart_defines_cancel(state)
}
```

#### 6. Sort dart defines alphabetically on open

In `handle_settings_dart_defines_open()`, sort the defines by key after collecting from the HashMap:

```rust
let mut defines: Vec<DartDefine> = resolved
    .config
    .dart_defines
    .iter()
    .map(|(k, v)| DartDefine::new(k.clone(), v.clone()))
    .collect();
defines.sort_by(|a, b| a.key.cmp(&b.key));
state.settings_view_state.dart_defines_modal = Some(DartDefinesModalState::new(defines));
```

### Acceptance Criteria

1. Pressing Esc in the dart defines modal List pane discards changes (does not write to `.fdemon/launch.toml`)
2. Pressing Esc in the dart defines modal Edit pane still switches to List pane (unchanged)
3. Doc comment on `SettingsDartDefinesClose` accurately says it persists changes
4. Doc comment on `SettingsDartDefinesCancel` says it discards changes
5. `tracing::warn` emitted when close handler encounters missing config_idx
6. Dart defines appear in alphabetical order when modal opens
7. Extra args modal Esc behavior is unchanged (already correct)

### Testing

Add tests in `settings_dart_defines.rs` tests module:

```rust
#[test]
fn test_cancel_does_not_persist_changes() {
    // Open dart defines modal, make edits, send Cancel, verify disk state unchanged
}

#[test]
fn test_close_persists_changes() {
    // Open dart defines modal, make edits, send Close, verify disk state updated
}

#[test]
fn test_defines_sorted_alphabetically_on_open() {
    // Create config with unsorted defines, open modal, verify sorted order
}
```

### Notes

- The existing `SettingsDartDefinesClose` handler remains the "save and exit" path — it should be triggered by an explicit save action (e.g., a "Save" button or Ctrl+S if added later)
- The extra args modal already has correct Esc-as-cancel semantics via `SettingsExtraArgsClose` — this task brings dart defines in line
- After this change, both modals have consistent behavior: Esc = discard, explicit action = save
