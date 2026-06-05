## Task: Make the install abort handle race-free — store the token synchronously (F3, F4, F7, F8, F19, F9)

**Severity:** HIGH (F3, F4) + MEDIUM (F7, F8, F9) + LOW (F19)

**Objective**: Guarantee that `Esc` while a step is running cancels the **actually
running** install (and releases the install lock), at every point in a step's
lifecycle. Eliminate the async-handoff races that currently let `Esc` cancel a
finished/wrong task while the real download keeps running in the background.

**Depends on**: 01 (same files: `handler/install_wizard/actions.rs`)

**Estimated Time**: 5–7 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` (`begin_step`, `InstallTaskHandle`, run-seq)
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (`handle_run_selected_step`, `handle_install_task_ready`, `handle_cancel_step`)
- `crates/fdemon-app/src/message.rs` (`WizardInstallTaskReady` variant)
- `crates/fdemon-app/src/actions/mod.rs` (`RunWizardStep` handler — token creation/handoff)
- `crates/fdemon-app/src/state.rs` (`hide_install_wizard`)
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` (read: cancel/close paths)

### Root cause

The cancel handle (`install_task: Option<InstallTaskHandle { cancel, join }>`) is
delivered to state **asynchronously** via `Message::WizardInstallTaskReady`, which is
sent from a **separate** `tokio::spawn` (`actions/mod.rs:1209-1216`) than the install
work task (`actions/mod.rs:898`) that emits `WizardStepStarted/Completed/Failed`.
Meanwhile `begin_step` flips `status = Running` **synchronously**
(`state.rs:232-242`), and `is_step_running()` keys only on `status == Running`
(`state.rs:200-202`). This creates four defects:

- **F3 (HIGH):** Window where `is_step_running()==true` but `install_task==None`.
  `Esc` → `handle_cancel_step` finds `take()==None`, skips `cancel()`/`abort()`, but
  *unconditionally* calls `reset_running_step_to_idle()` + shows "Cancelled". The
  orphaned task keeps downloading and holds the RAII `LockGuard`
  (`.fdemon-install.lock`, `flutter_install.rs:124-133`); the next retry fails with
  "another install is in progress".
- **F4 (HIGH):** `handle_install_task_ready` (`actions.rs:419-446`) stores the handle
  **unconditionally** (no kind/seq check). Cancel run A → retry run B → a late
  ready-A (its sender is never aborted by `handle_cancel_step`) clobbers B's handle
  with A's dead token → a later `Esc` cancels dead A while live B is uncancellable.
- **F7/F8 (MEDIUM):** For a fast-finishing step the terminal message can be processed
  first (clearing `install_task`), then a late ready re-installs a stale handle;
  `begin_step` does **not** clear `install_task` (`state.rs:232-242`), so the stale
  handle survives into the next step — `Esc` then cancels the wrong (finished) task
  while the new step's real download runs.
- **F19 (LOW):** `hide_install_wizard` (`state.rs:1733-1736`) never clears/cancels
  `install_task`.

### Fix

Redesign so the cancellation token is owned by state **the instant the step starts**,
and demote `WizardInstallTaskReady` to a *handle upgrade* that is validated:

1. **Create + store the token synchronously.** In `handle_run_selected_step`
   (`actions.rs`), before/at `begin_step(kind)`, mint the `CancellationToken`, store
   `install_task = Some(InstallTaskHandle { cancel: token.clone(), join: None })`
   (make `join` an `Option<JoinHandle<()>>`), and pass the **same** token into
   `UpdateAction::RunWizardStep` so `handle_action` (`actions/mod.rs:~888`) reuses it
   instead of minting a fresh `CancellationToken::new()`. Because every install loop
   already polls the token and maps cancellation to a terminal `WizardStepFailed`
   (which drops the `LockGuard`), `cancel.cancel()` alone now stops the install and
   releases the lock — closing F3 entirely.
2. **Add a run sequence id.** Add a monotonically increasing `run_seq: u64` to
   `InstallWizardState`, bumped in `begin_step` each time a run starts. Add
   `kind: WizardStepKind` and `run_seq: u64` to `Message::WizardInstallTaskReady`
   (`message.rs`) and to the ready-sender (`actions/mod.rs:1209`).
3. **Validate the ready message.** In `handle_install_task_ready`, store the `join`
   handle **only** when `is_step_running()` **and** `execution.kind == Some(kind)`
   **and** `run_seq == current run_seq` — i.e. only *upgrade* the already-stored
   handle's `join` field. Otherwise `join.abort()` the just-delivered handle and drop
   it without touching `install_task`. (Kind alone is insufficient — a cancel→retry
   of the *same* kind needs the seq to distinguish A from B.)
4. **Clear on step start.** In `begin_step` add `let _ = self.install_task.take();`
   so a new run can never inherit a previous step's handle (F8).
5. **Clear on close.** In `hide_install_wizard`, drain any handle first:
   `if let Some(t) = self.install_wizard_state.install_task.take() { t.cancel.cancel(); if let Some(j) = t.join { j.abort(); } }` (F19) — idempotent/harmless when None.
6. **Defensive backstop.** In `handle_cancel_step`, only run
   `reset_running_step_to_idle()` + the "Cancelled" message when a token was actually
   fired, so a future regression can't silently flip to Idle without cancelling.

### Acceptance Criteria

1. Pressing `Esc` immediately after a step transitions to `Running` (before the ready
   message would arrive) cancels the real install and releases the install lock — a
   subsequent retry into the same `install_root` does not fail with "another install
   is in progress" (F3).
2. A `WizardInstallTaskReady` whose `kind`/`run_seq` does not match the current run is
   discarded (its `join` aborted), never overwriting `install_task` (F4/F7).
3. `begin_step` clears any prior `install_task`; `hide_install_wizard` cancels+clears
   any handle (F8/F19).
4. After a cancel→retry of the same `WizardStepKind`, the live (retried) install is
   cancellable and the dead run's late ready is ignored (F4).
5. `cargo clippy --workspace --all-targets -- -D warnings` clean (watch for unused
   `join: None` warnings / `Option` handling).

### Testing

```rust
// install_wizard/state.rs + handler/install_wizard/actions.rs test modules
// - NEW: begin_step clears a pre-set install_task (assert None after begin_step).
// - NEW: handle_install_task_ready with a non-matching kind/run_seq is a no-op
//        (install_task unchanged / stays the live one); matching kind+seq upgrades join.
// - NEW: late ready AFTER a terminal WizardStepFailed/Completed does NOT re-install a
//        handle (install_task stays None).
// - NEW: cancel during the "running but no handle yet" window fires the synchronously
//        stored token (assert token.is_cancelled()) and resets to idle.
// - NEW: cancel run A (kind K) -> begin_step(K) again (run B) -> late ready for A is
//        discarded; install_task is B's; cancel fires B's token.
// - KEEP cancel_step_clears_handle_and_resets_status / *_idempotent_when_no_task green.
```

### Notes

- `WizardInstallTaskReady` must remain the carrier for the `JoinHandle` only (the
  handle genuinely cannot exist until `tokio::spawn` returns inside `handle_action`).
  The **token** is what closes the races, and it is now available synchronously.
- This is the load-bearing concurrency task; tasks 03 builds on its `install_task`
  semantics. Serialise on the same branch (chain A).
- F9 (the missing-test finding) is satisfied by the Testing block above.
