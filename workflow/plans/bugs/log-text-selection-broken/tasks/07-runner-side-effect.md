# Task 07 — Runner glue: observe `UpdateAction::SetMouseCapture` / `WriteClipboard`

**Agent:** implementor
**Wave:** 3
**Depends on:** Task 01 (`set_mouse_capture`), Task 03 (action variants), Task 06 (action emission + `WriteClipboard` variant)
**Files written:** `crates/fdemon-tui/src/runner.rs`

---

## Goal

In the runner's `UpdateAction` consumer loop, add two arms:

1. `UpdateAction::SetMouseCapture(target)` → call `terminal::set_mouse_capture(target)`; on success, enqueue `Message::MouseCaptureChanged { active: target }`; on failure, push a warning toast (via a new `Message::ShowToast`-equivalent path or a direct `state.push_toast` if the runner already does that) and do not enqueue a state change.

2. `UpdateAction::WriteClipboard { text }` → call `clipboard.write_text(&text)` on the runner-owned `Box<dyn Clipboard>`; on failure, push a warning toast `"Clipboard write failed: <error>"`.

Also: instantiate the `SystemClipboard` once at runner startup, immediately after the engine is constructed. Hold it as a `Box<dyn Clipboard>` so tests / demo mode can swap in `MemoryClipboard`.

## Background

The runner already pulls `UpdateAction`s from a channel and performs side effects (process spawning, file watching). Mouse-capture toggling and clipboard writes follow the same pattern. See the existing `UpdateAction` arms in `runner.rs` for the local convention on error reporting and follow-up message enqueuing.

## Implementation

1. After engine construction, build the clipboard:

   ```text
   let mut clipboard: Box<dyn Clipboard> = match SystemClipboard::new() {
       Ok(cb) => Box::new(cb),
       Err(e) => {
           warn!("system clipboard unavailable: {e}");
           // Fall back to MemoryClipboard so right-click toast still works
           // (the toast says "Copied: ...", the underlying write is silent).
           Box::new(MemoryClipboard::default())
       }
   };
   ```

   Decision rationale: if the system clipboard is unavailable (rare — typically Linux without an X/Wayland session), we choose a silent fallback over a hard failure. The right-click toast remains accurate from the user's perspective ("Copied: X") even though the system clipboard does not actually contain `X`. **Open verification: confirm with the user whether they want a hard error toast in this case instead.** If yes, swap `MemoryClipboard` for a `NullClipboard` impl that returns an error from `write_text`, and let the existing failure-toast path fire.

2. In the action-dispatch loop, add:

   ```text
   UpdateAction::SetMouseCapture(target) => {
       match terminal::set_mouse_capture(target) {
           Ok(()) => {
               // Round-trip the state change via the bus.
               event_tx.send(Event::Message(Message::MouseCaptureChanged { active: target }))?;
           }
           Err(e) => {
               warn!("set_mouse_capture({target}) failed: {e}");
               // Toast via the runner-owned state handle.
               state.push_toast(ToastLevel::Warning, format!("Mouse capture toggle failed: {e}"));
           }
       }
   }
   UpdateAction::WriteClipboard { text } => {
       if let Err(e) = clipboard.write_text(&text) {
           warn!("clipboard write failed: {e}");
           state.push_toast(ToastLevel::Warning, format!("Clipboard write failed: {e}"));
       }
   }
   ```

   The exact event-tx / state-access shape depends on the runner's local conventions — verify by reading neighboring arms. Use the same conventions; do not invent a new channel pattern.

3. The startup `terminal::enable_mouse_capture()` call site (currently in `runner.rs:38` and similar) **does not change** in this task. It continues to be the boot-time enable; the runtime toggle is a separate concern.

## Tests

Runner tests are harder than handler tests because the runner owns I/O. Two test approaches:

- **Unit tests on a runner helper** — if the new action-dispatch is factored into a `handle_update_action(action, ctx)` helper that takes a mockable context, unit-test that helper.
- **Integration tests** — if the runner is monolithic, add an integration test under `tests/` that runs the runner with a `MemoryClipboard` and asserts a `WriteClipboard` action results in the right `writes` entry. Verify `tests/` directory exists and the existing pattern by looking at a sibling integration test.

Minimum bar:
- `test_set_mouse_capture_action_enqueues_followup_message` — mock terminal call, assert `Message::MouseCaptureChanged` flows back through the bus.
- `test_write_clipboard_action_writes_to_clipboard` — use `MemoryClipboard`, dispatch action, assert one entry in `writes`.
- `test_write_clipboard_failure_pushes_warning_toast` — use a clipboard impl that errors, assert toast.

## Acceptance Criteria

- [ ] Three new tests pass (location flexible — runner unit-test if reasonable, integration test otherwise).
- [ ] The startup `enable_mouse_capture()` call site is unchanged (verified by `git diff`).
- [ ] Boot-time clipboard instantiation logs at `warn` and falls back gracefully — does not panic the process.
- [ ] No new dependencies introduced in this task (clipboard came in via Task 02).

## Notes for Reviewer

- The runner is the only place in the codebase that owns both the terminal handle and the clipboard handle, which is why both side-effect arms live here.
- If the user (or a reviewer) prefers a hard-error toast for clipboard unavailable rather than silent `MemoryClipboard`, switch the fallback in step 1 of Implementation. Recommend leaving it as-written; the warn log gives operators a breadcrumb.

---

## Completion Summary

**Status:** Done
**Branch:** plan/log-text-selection-fix

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `pending_runner_actions: Vec<UpdateAction>` field to `AppState`; initialized in `with_settings` |
| `crates/fdemon-app/src/process.rs` | Intercept `SetMouseCapture` and `WriteClipboard` actions before `handle_action`; push to `state.pending_runner_actions` instead |
| `crates/fdemon-app/src/engine.rs` | Added `drain_runner_actions()` method that returns and clears the pending runner actions queue |
| `crates/fdemon-app/src/lib.rs` | Exported `ToastLevel` from the public API |
| `crates/fdemon-tui/src/terminal.rs` | Removed `#[allow(dead_code)]` from `set_mouse_capture` (now has a caller) |
| `crates/fdemon-tui/src/runner.rs` | Added clipboard instantiation (SystemClipboard with MemoryClipboard fallback) in all 3 entry points; updated `run_loop` signature to accept `&mut dyn Clipboard`; added `handle_runner_actions` side-effect dispatcher; added 3 unit tests |

### Notable Decisions/Tradeoffs

1. **Pending queue on AppState**: Rather than a new channel or Engine-level field (which would require passing more through `process.rs`), `pending_runner_actions` lives on `AppState`. `process.rs` has `&mut AppState` and can push to it; the runner drains via `engine.drain_runner_actions()`. This is consistent with the existing `pending_watcher_errors` pattern.

2. **`handle_runner_actions` called twice per loop iteration**: Once after `drain_pending_messages()` (processes engine-channel messages that may produce runner actions) and once after `event::poll()` (processes the user-input message). This ensures no runner actions are left pending before render.

3. **`set_mouse_capture(false)` idempotency used for testing**: In non-TTY test environments, calling `set_mouse_capture(false)` when `MOUSE_CAPTURE_ON = false` returns `Ok(())` via the idempotency guard — no stdout write occurs. This lets the test confirm the success path (Info toast, not Warn toast) without requiring a real terminal.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (all 3 new tests + full workspace suite)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **`SetMouseCapture(true)` in tests**: The enable path (`set_mouse_capture(true)`) writes to stdout and fails in non-TTY CI environments, so the test covers the disable direction only. The failure-toast path for enable would require mocking `MOUSE_CAPTURE_ON` (private static), which was avoided to keep tests simple. Both directions share the same `match` arm in `handle_runner_actions`, so the logic is covered.
