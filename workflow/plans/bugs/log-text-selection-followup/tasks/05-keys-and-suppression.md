## Task: Keys + suppression refinements (Shift+Alt+m, NewSessionDialog field-focus, missing tests)

**Objective:** Three related fixes to the global `Alt+m` pre-dispatch in `handler/keys.rs`:

1. Widen the match to accept Shift+Alt+m as well (`'m' | 'M'`), so users holding Shift don't silently lose the toggle.
2. Refine the `Startup`/`NewSessionDialog` suppression to be field-focus-sensitive — currently the entire dialog is treated as a text-input mode, making `Alt+m` unreachable when the user is on the device-list pane.
3. Add the missing regression tests for Settings-editing and NewSessionDialog suppression branches.

**Depends on:** None

**Agent:** implementor

**Estimated time:** 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/keys.rs`: widen Alt+m match; refine `Startup | NewSessionDialog` suppression.
- `crates/fdemon-app/src/handler/tests.rs`: add Shift+Alt+m test + Settings-editing suppression test + NewSessionDialog field-focus suppression test (both pane states).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/types.rs`: `DialogPane` enum (`TargetSelector` | `LaunchContext`).
- `crates/fdemon-app/src/state.rs`: `NewSessionDialogState` and `SettingsViewState::editing`.
- `crates/fdemon-app/src/handler/keys.rs`: existing global Alt+m intercept.

### Details

#### 1. Shift+Alt+m widening

Current code at `crates/fdemon-app/src/handler/keys.rs:23` (approximately — search for `InputKey::CharAlt('m')`):

```rust
if matches!(key, InputKey::CharAlt('m')) {
```

Change to:

```rust
if matches!(key, InputKey::CharAlt('m' | 'M')) {
```

Per the codebase researcher's finding, `event.rs:117` already canonicalises both `Char('m')|ALT` and `Char('M')|ALT|SHIFT` to `CharAlt(c)` — so widening the match arm is the only change needed.

#### 2. NewSessionDialog field-focus refinement

Current code (approximately):

```rust
let in_text_input = matches!(state.ui_mode,
    UiMode::SearchInput |
    UiMode::Startup |
    UiMode::NewSessionDialog) ||
    (matches!(state.ui_mode, UiMode::Settings) && state.settings_view_state.editing);
```

Refine the `Startup | NewSessionDialog` clause so it suppresses only when a text field is actually focused. The dialog's text-field state is encoded as `DialogPane::LaunchContext` (text fields) vs `DialogPane::TargetSelector` (device picker, no text). Also account for sub-modals (`DartDefinesModalState`, `FuzzyModalState`) which are always text-input contexts when open.

Replace with:

```rust
let in_text_input = match state.ui_mode {
    UiMode::SearchInput => true,
    UiMode::Settings => state.settings_view_state.editing,
    UiMode::Startup | UiMode::NewSessionDialog => {
        let dlg = &state.new_session_dialog;
        // Sub-modals are always text-input contexts.
        if dlg.dart_defines_modal.is_some() || dlg.fuzzy_modal.is_some() {
            true
        } else {
            // Main dialog: text input only when LaunchContext pane is focused.
            matches!(dlg.active_pane, DialogPane::LaunchContext)
        }
    }
    _ => false,
};
```

(Field/path names are illustrative — verify the exact accessor names against `state.rs` and `new_session_dialog/types.rs`.)

If a fuzzy modal or dart-defines modal is open, suppress (those modals always have a text-input field focused). If neither modal is open, check the main dialog's `active_pane`: `TargetSelector` → no text field → don't suppress; `LaunchContext` → text fields → suppress.

#### 3. Missing suppression tests

Add three tests in `crates/fdemon-app/src/handler/tests.rs`:

```rust
#[test]
fn test_shift_alt_m_in_normal_mode_emits_toggle() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;

    let result = handle_key(&state, InputKey::CharAlt('M'));

    assert!(
        matches!(result, Some(Message::ToggleMouseCapture)),
        "Shift+Alt+m (CharAlt('M')) must emit ToggleMouseCapture; got {:?}",
        result
    );
}

#[test]
fn test_alt_m_in_settings_editing_mode_does_not_toggle() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Settings;
    state.settings_view_state.start_editing();  // sets editing = true

    let result = handle_key(&state, InputKey::CharAlt('m'));

    assert!(
        result.is_none(),
        "Alt+m must be suppressed while editing a Settings field; got {:?}",
        result
    );
}

#[test]
fn test_alt_m_in_new_session_dialog_target_selector_emits_toggle() {
    // Device picker pane has no text input; Alt+m should fire.
    let mut state = AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;
    state.new_session_dialog.active_pane = DialogPane::TargetSelector;

    let result = handle_key(&state, InputKey::CharAlt('m'));

    assert!(
        matches!(result, Some(Message::ToggleMouseCapture)),
        "Alt+m must fire when device picker pane is focused; got {:?}",
        result
    );
}

#[test]
fn test_alt_m_in_new_session_dialog_launch_context_does_not_toggle() {
    // Launch-context pane has text fields; suppress.
    let mut state = AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;
    state.new_session_dialog.active_pane = DialogPane::LaunchContext;

    let result = handle_key(&state, InputKey::CharAlt('m'));

    assert!(
        result.is_none(),
        "Alt+m must be suppressed when LaunchContext pane is focused; got {:?}",
        result
    );
}
```

Field names and constructors are illustrative — adapt to actual API.

### Acceptance Criteria

1. `Shift+Alt+m` (delivered as `InputKey::CharAlt('M')` per `event.rs:117`) emits `Message::ToggleMouseCapture`.
2. `Alt+m` is suppressed when `Settings` is in `editing = true`.
3. `Alt+m` fires when `NewSessionDialog` has `DialogPane::TargetSelector` focused (device picker, no text input).
4. `Alt+m` is suppressed when `NewSessionDialog` has `DialogPane::LaunchContext` focused (text input).
5. `Alt+m` is suppressed when `dart_defines_modal` or `fuzzy_modal` is open within the dialog.
6. All four new tests pass.
7. Existing `test_alt_m_*` tests still pass.
8. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all pass.

### Testing

Run `cargo test -p fdemon-app handler::tests::test_alt_m`. All existing + new tests in this group must pass.

### Notes

- This task touches `handler/tests.rs`. **Task 04 also touches `handler/tests.rs`.** The orchestrator will run these two sequentially on the current branch, not in parallel worktrees. Run task 04 first (it adds tests in distinct slots near the copy-message tests); then task 05 adds tests near the existing Alt+m tests.
- If `DialogPane` or `NewSessionDialogState` field accessors differ from the names above, adapt to actual API. The `codebase_researcher` confirmed the enum variants are `TargetSelector` and `LaunchContext` and live at `crates/fdemon-app/src/new_session_dialog/types.rs:7`.
- Do NOT change `event.rs` — the canonicalisation there already handles both `'m'` and `'M'` correctly.
