# Task 01: Add `handle_set_mode` handler and wire the message route

**Files:** `crates/fdemon-app/src/handler/new_session/launch_context.rs`, `crates/fdemon-app/src/handler/update.rs`
**Depends on:** None
**Wave:** 1 (Worktree A, sequential — blocks Task 02)

## Background

`Message::NewSessionDialogSetMode { mode: FlutterMode }` is defined in
`crates/fdemon-app/src/message.rs:548` but is currently a no-op stub in the
catch-all arm at `crates/fdemon-app/src/handler/update.rs:1180-1185`. The
existing `handle_mode_next` / `handle_mode_prev` (lines 12-110 of the launch
context handler) cycle the mode but cannot set a specific mode by value.

## What to do

1. In `crates/fdemon-app/src/handler/new_session/launch_context.rs`, add a new
   `pub fn handle_set_mode(state: &mut AppState, mode: FlutterMode) -> UpdateResult`
   modeled on `handle_mode_next` (lines 12-62):

   - Early-return `UpdateResult::none()` if `state.new_session_dialog_state.launch_context.is_mode_editable()` returns `false`.
   - Set `state.new_session_dialog_state.launch_context.focused_pane = DialogPane::LaunchContext` and `.focused_field = LaunchContextField::Mode` so a click also focuses the row (matching the row-level `FocusField` region's effect).
   - Set `state.new_session_dialog_state.launch_context.mode = mode`.
   - If the selected config is `ConfigSource::FDemon`, return `UpdateResult::action(UpdateAction::AutoSaveConfig { configs: ... })` (use the same construction as `handle_mode_next`). Otherwise return `UpdateResult::none()`.
   - Do **not** check whether `mode` equals the current `state.mode` — clicking the already-selected button is still allowed to focus the field. (Cheap; idempotent.)

2. Import `crate::config::FlutterMode` at the top of the handler file if not
   already imported.

3. In `crates/fdemon-app/src/handler/update.rs`, remove
   `Message::NewSessionDialogSetMode { .. }` from the catch-all stub arm at
   line 1180. Add a new arm immediately after `NewSessionDialogModePrev` at
   line 1198:

   ```rust
   Message::NewSessionDialogSetMode { mode } => new_session::handle_set_mode(state, mode),
   ```

## Verification

- `cargo check -p fdemon-app` compiles.
- `cargo test -p fdemon-app -- handler::new_session::launch_context` passes.
- Add unit tests in the same file:
  - `test_handle_set_mode_sets_mode_when_editable`
  - `test_handle_set_mode_is_noop_when_not_editable`
  - `test_handle_set_mode_returns_auto_save_for_fdemon_config`
  - `test_handle_set_mode_returns_none_for_vscode_config`
  - `test_handle_set_mode_focuses_mode_field`

  Mirror the test scaffolding used by the existing `handle_mode_next` tests if
  they exist; otherwise build the `AppState` minimally via existing test
  helpers.

- `cargo clippy -p fdemon-app -- -D warnings` passes.
