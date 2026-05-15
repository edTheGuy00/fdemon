# Task 03 — New `Message` variants, `UpdateAction` variant, `AppState` field

**Agent:** implementor
**Wave:** 1
**Depends on:** — (parallel with 01, 02)
**Files written:**
- `crates/fdemon-app/src/message.rs`
- `crates/fdemon-app/src/state.rs`
- `crates/fdemon-app/src/update_action.rs` *(or the file where `UpdateAction` is defined — verify and document in the PR)*

---

## Goal

Wire the three new TEA primitives that the rest of the fix consumes:

1. `Message::CopyLogEntryToClipboard { entry_id: LogEntryId }` — emitted by right-click handler (Task 04).
2. `Message::ToggleMouseCapture` — emitted by the `Alt+m` key handler (Task 05).
3. `Message::MouseCaptureChanged { active: bool }` — sent by the runner after `terminal::set_mouse_capture` succeeds, so the model can update.
4. `UpdateAction::SetMouseCapture(bool)` — side-effect command the handler returns when capture should change.
5. `AppState::mouse_capture_active: bool` — initialized to `settings.ui.enable_mouse`.

This task touches only **declarations** — no behavior. Tasks 04–07 wire them up.

## Implementation

### `message.rs`

Locate the existing `Message` enum and add the three new variants. Keep the docstring conventions used by neighboring variants (e.g., the `Devtools*` cluster has detailed `///` blocks).

```text
/// Copy a specific log entry's rendered text to the system clipboard.
///
/// Emitted by the right-click handler in `handler/mouse.rs` when the user
/// right-clicks on a log row. The handler resolves `entry_id` to the entry's
/// rendered text and writes it via the `Clipboard` service; a confirmation
/// toast is pushed onto `AppState::toasts`.
///
/// Fix for log-text-selection bug — see
/// `workflow/plans/bugs/log-text-selection-broken/BUG.md`.
CopyLogEntryToClipboard { entry_id: LogEntryId },

/// Request a runtime toggle of terminal mouse capture.
///
/// Emitted by the `Alt+m` keybinding. The update handler returns
/// `UpdateAction::SetMouseCapture(!state.mouse_capture_active)`; the runner
/// performs the side effect and follows up with `MouseCaptureChanged` once
/// the terminal mode has changed.
ToggleMouseCapture,

/// Reflect a successful runtime change to terminal mouse capture.
///
/// Sent by the runner after `terminal::set_mouse_capture(...)` returns
/// `Ok(())`. Updates `AppState::mouse_capture_active` so the status-bar
/// indicator (Task 08) and the click hit-test gates render the correct
/// state.
MouseCaptureChanged { active: bool },
```

Use the `LogEntryId` type already in use elsewhere in the file (verify by grep — same crate path).

### `update_action.rs` (or wherever `UpdateAction` lives)

Add a new variant:

```text
/// Toggle terminal mouse capture at runtime. The runner calls
/// `terminal::set_mouse_capture(active)` and follows up with
/// `Message::MouseCaptureChanged { active }` on success.
SetMouseCapture(bool),
```

### `state.rs`

Add a new field on `AppState`:

```text
/// Whether terminal mouse capture is currently active.
///
/// Initialized from `settings.ui.enable_mouse` at construction. Mutated only
/// by the `MouseCaptureChanged` handler arm (Task 06) after the runner has
/// performed the corresponding `terminal::set_mouse_capture` call. The
/// indicator in the bottom metadata bar (Task 08) reads this field.
pub mouse_capture_active: bool,
```

Locate the existing `AppState::new(...)` constructor (or `Default` impl — verify in code) and initialize `mouse_capture_active: settings.ui.enable_mouse`. If multiple constructor paths exist (e.g., one for demo mode, one for full mode), initialize from the settings value in every path; in demo mode the value is `false` per the existing demo comment in `runner.rs:189`.

## Tests

- `test_appstate_initializes_mouse_capture_active_from_settings_true` — settings.ui.enable_mouse = true ⇒ field is true.
- `test_appstate_initializes_mouse_capture_active_from_settings_false` — settings.ui.enable_mouse = false ⇒ field is false.
- `test_copy_log_entry_message_round_trips` — construct, pattern-match, assert `entry_id` survives (matches the style of existing message-shape tests).
- `test_toggle_mouse_capture_message_is_unit_variant` — trivial constructor test for symmetry with the cluster.
- `test_set_mouse_capture_action_variant_round_trips` — same for `UpdateAction`.

## Acceptance Criteria

- [ ] `cargo build -p fdemon-app` succeeds.
- [ ] Five new unit tests pass.
- [ ] No handler logic touched — only declarations.
- [ ] `mouse_capture_active` initialized in **every** `AppState` construction path.

## Notes for Reviewer

- Variants are deliberately tiny — putting them all in one task avoids `message.rs` write-overlap across wave-1 tasks.
- `MouseCaptureChanged` exists as a separate message (rather than the runner mutating state directly) because side effects must round-trip through the TEA bus to preserve testability — same pattern as existing daemon-event messages.
- The handler-arm wiring in Task 06 is intentionally a separate task so reviewers can verify message *declarations* and *consumers* independently.
