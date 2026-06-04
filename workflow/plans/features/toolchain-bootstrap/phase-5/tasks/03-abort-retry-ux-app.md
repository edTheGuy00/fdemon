## Task: Abort wiring + retry-failure UX (app layer)

**Objective**: Wire the daemon cancellation API into the wizard — store the install
task handle, add an `InstallWizardCancelStep` message, make `Esc` cancel a running
step (and still close the wizard when nothing is running) — and add the small
retry-failure affordance (a "press Enter to retry / r to re-check" status prompt).

**Depends on**: 02-abortable-downloads-daemon (needs the `CancellationToken` API)

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/mod.rs`: in the `RunWizardStep` spawn (~`:838`),
  create a `CancellationToken`, pass it to the installer, and **store** the
  `JoinHandle` + token instead of dropping them.
- `crates/fdemon-app/src/install_wizard/state.rs`: add `install_task: Option<...>`
  (handle + `CancellationToken`) to `InstallWizardState`; a `cancel_running_step()`
  helper; set the retry-failure `status_message` in/near `finish_step(Failed, _)`.
- `crates/fdemon-app/src/message.rs`: add `Message::InstallWizardCancelStep`.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`: `handle_cancel_step`
  (call `token.cancel()`, reset exec status to idle, set a "cancelled" status_message);
  in `handle_step_failed`, set `status_message` = "Failed — press Enter to retry or
  r to re-check".
- `crates/fdemon-app/src/handler/keys.rs`: in `handle_key_install_wizard`, branch
  `Esc`: if `is_step_running()` → `InstallWizardCancelStep`, else existing
  `InstallWizardEscape` (close).
- `crates/fdemon-app/src/handler/update.rs`: route `InstallWizardCancelStep` →
  `handle_cancel_step`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain` cancel API (task 02).
- `crates/fdemon-app/src/install_wizard/types.rs`: `StepExecStatus`, `WizardPane`.

### Details

**Handle storage.** Today `actions/mod.rs:~838` does `tokio::spawn(async move {…})`
and drops the `JoinHandle`. Store both the handle and the token so the UI can cancel:

```rust
// install_wizard/state.rs
pub struct InstallTaskHandle {
    pub join: tokio::task::JoinHandle<()>,
    pub cancel: tokio_util::sync::CancellationToken,
}
// field on InstallWizardState:
pub install_task: Option<InstallTaskHandle>,
```

- On `WizardStepCompleted` / `WizardStepFailed` / cancel, clear `install_task`
  (`take()`), so a stale handle never lingers.

**Esc overload (important).** `Esc` currently always closes the wizard. It must now:

```rust
KeyCode::Esc => {
    if state.install_wizard_state.is_step_running() {
        Some(Message::InstallWizardCancelStep)   // cancel takes precedence
    } else {
        Some(Message::InstallWizardEscape)       // existing close behavior
    }
}
```

**Cancel handler.**

```rust
fn handle_cancel_step(state: &mut AppState) -> UpdateResult {
    if let Some(task) = state.install_wizard_state.install_task.take() {
        task.cancel.cancel();             // signal the streaming loop
        // optionally task.join.abort() as a backstop
    }
    state.install_wizard_state.reset_running_step_to_idle();
    state.install_wizard_state.status_message = Some("Cancelled. Press Enter to retry.".into());
    UpdateResult::none()
}
```

- When the daemon task observes the token and returns `Cancelled`, it still emits a
  terminal `WizardStepFailed`/completion message; `handle_step_failed` must treat the
  `Cancelled` error specially (no red "install failed" framing — a neutral
  "cancelled" message). Distinguish via the error variant from task 02.

**Retry-failure prompt (folded from audit).** `handle_step_failed` already keeps the
log and enables `Enter`-retry; it just lacks a visible affordance. Set
`status_message` to guide the user. One line + a test. (The step-list run-failed
**badge** and the "Esc cancels" **hint** are the TUI side — task 06.)

### Acceptance Criteria

1. The `RunWizardStep` spawn stores a `CancellationToken` + `JoinHandle` on
   `InstallWizardState`; the handle is cleared on completion/failure/cancel.
2. `Esc` while a step is `Running` dispatches `InstallWizardCancelStep` and does
   **not** close the wizard; `Esc` when idle still closes the wizard.
3. `handle_cancel_step` cancels the token, resets the step to idle, and shows a
   neutral "cancelled" message; a subsequent `Enter` retries the step.
4. A `Cancelled` terminal message is rendered as cancelled, not as a failure.
5. After a genuine failure, `status_message` reads "Failed — press Enter to retry or
   r to re-check".

### Testing

```rust
#[test]
fn esc_while_running_cancels_not_closes() {
    // arrange a running step; assert key Esc -> Message::InstallWizardCancelStep
    // and that ui_mode is still InstallWizard.
}
#[test]
fn esc_while_idle_closes_wizard() { /* -> InstallWizardEscape, UiMode::Normal */ }
#[test]
fn cancel_step_clears_handle_and_resets_status() { /* install_task is None, status set */ }
#[test]
fn step_failed_sets_retry_prompt() { /* status_message contains "press Enter to retry" */ }
```

### Notes

- Keep the cancel path **idempotent** — a second `Esc`/cancel with no running task is
  a no-op.
- Guard the spawn so a new `RunWizardStep` while one is in flight is rejected (the
  existing `is_step_running()` guard in `handle_run_selected_step` already covers
  `Enter`; ensure cancel doesn't race it).
- This task shares `install_wizard/state.rs` + `handler/install_wizard/actions.rs`
  with task 04 — the 03→04 edge keeps them sequential.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
