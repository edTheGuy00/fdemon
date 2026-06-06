## Task: Make `WizardStepStarted` seq-aware so a stale cross-kind Started cannot clobber the live install (F-PR53-01)

**Severity:** HIGH (concurrency)

**Objective**: Close the cross-kind race where a cancelled run's delayed
`WizardStepStarted` drives the defensive `begin_step` fallback for the *wrong*
step kind, silently dropping the live run's cancellation token and bumping
`run_seq` — producing a non-cancellable "zombie" install that keeps downloading,
holds the RAII install lock, loses its seq-guard backstop, and desyncs the UI.

**Depends on**: — (chain A start; shares files with task 06)

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs` (add `run_seq` to `WizardStepStarted`)
- `crates/fdemon-app/src/actions/mod.rs` (executor echoes the run_seq it was given)
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (`handle_step_started` ignores stale)
- `crates/fdemon-app/src/install_wizard/state.rs` (only if a helper/accessor is needed)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (`handle_run_selected_step`, `handle_cancel_step`, `handle_install_task_ready`)
- `crates/fdemon-app/src/install_wizard/state.rs` (`begin_step`, `reset_progress_display`, `run_seq`, `install_task`)

### Details

The prior Phase-5 followup (task 02) already hardened the **same-kind** path:
`handle_step_started` takes the safe `reset_progress_display()` branch when the
step is already `Running` for the message's `kind`, preserving the
synchronously-stored `install_task` and `run_seq`. That fix is intact and must
be preserved.

The **cross-kind** leg was missed. In `handler/install_wizard/actions.rs:353-369`:

```rust
pub fn handle_step_started(state: &mut AppState, kind: WizardStepKind) -> UpdateResult {
    let already_running_for_kind = state.install_wizard_state.execution.status
        == StepExecStatus::Running
        && state.install_wizard_state.execution.kind == Some(kind);
    if already_running_for_kind {
        state.install_wizard_state.reset_progress_display();   // safe path
    } else {
        state.install_wizard_state.begin_step(kind);           // DANGEROUS fallback
    }
    UpdateResult::none()
}
```

`WizardStepStarted` carries only `kind` (no `run_seq`) — `message.rs:1770`:
`WizardStepStarted { kind: WizardStepKind }`. The executor emits it
**unconditionally as its first await**, before the cancel token is even bound —
`actions/mod.rs:901-906`:

```rust
msg_tx.send(Message::WizardStepStarted { kind }).await;   // before any cancel check
```

`begin_step` (`state.rs:264-279`) does `self.install_task.take()` **without**
`cancel.cancel()` / `join.abort()`, and bumps `run_seq`.

**Race (reachable via a precise Esc+Enter):**
1. Run A (AndroidTools) begins, `run_seq = 1`.
2. `Esc` → `handle_cancel_step` takes `install_task`, cancels token, resets to Idle, `j.abort()`s Run A's join — but this races Run A's already-queued `WizardStepStarted`.
3. `Enter` → Run B (FlutterSdk) begins via `handle_run_selected_step`: `begin_step` → `run_seq = 2`, `install_task = Some{cancelB}`.
4. Run A's delayed `WizardStepStarted{AndroidTools}` arrives. Current kind is FlutterSdk → `already_running_for_kind == false` → **fallback `begin_step(AndroidTools)`** runs: drops `cancelB` (no cancel/abort) → zombie FlutterSdk install; bumps `run_seq = 3` → Run B's legitimate `WizardInstallTaskReady{run_seq=2}` is now rejected by the seq-guard and its join aborted (backstop lost); `execution.kind = AndroidTools` → UI shows the wrong step Running.

### Proposed Fix

Make `WizardStepStarted` self-validating (mirror `WizardInstallTaskReady`):

1. Add `run_seq: u64` to `Message::WizardStepStarted` in `message.rs`.
2. In `actions/mod.rs`, pass the `run_seq` assigned at dispatch into the executor
   task and have it send `WizardStepStarted { kind, run_seq }`.
3. In `handle_step_started`, **ignore any Started whose `run_seq != state.install_wizard_state.run_seq`** (stale → no-op). Keep the existing same-kind
   `reset_progress_display()` for the current-seq case.
4. Preferred: **drop the `begin_step` fallback entirely** — `handle_run_selected_step`
   always calls `begin_step` before dispatch, so a current-seq Started is by
   definition already `Running` for its kind; a non-matching Started is stale.
   If a defensive `begin_step` is retained, it MUST first
   `cancel.cancel()` + `join.abort()` the existing `install_task` before replacing it.

### Acceptance Criteria

1. A `WizardStepStarted` whose `run_seq` does not equal the current
   `install_wizard_state.run_seq` is a no-op: `install_task`, `run_seq`, and
   `execution.kind/status` are all unchanged.
2. The Esc+Enter cross-kind sequence above leaves Run B's `install_task` (cancelB)
   intact and cancellable, `run_seq` unbumped by the stale Started, and
   `execution.kind == Some(FlutterSdk)`.
3. The same-kind, current-seq path still routes through `reset_progress_display()`
   and preserves `install_task` + `run_seq` (no regression to Phase-5 task 02).
4. No path leaves a running install with `install_task == None`.

### Testing

```rust
// handler/install_wizard/actions.rs test module
// - NEW test_stale_cross_kind_step_started_is_noop:
//     begin_step(FlutterSdk) (seq=N, install_task=Some); feed WizardStepStarted{AndroidTools, run_seq=N-1};
//     assert install_task.is_some(), run_seq==N, execution.kind==Some(FlutterSdk).
// - NEW test_step_started_with_current_seq_same_kind_preserves_task (rename/extend existing):
//     assert reset_progress_display path; install_task & run_seq unchanged.
// - UPDATE existing test_step_started_* to construct WizardStepStarted with a run_seq.
// - Regression: keep test_step_started_preserves_install_task_and_run_seq and
//   test_step_started_is_idempotent_with_begin_step green.
```

### Notes

- This is the load-bearing HIGH fix from the PR #53 review and the natural
  continuation of Phase-5 followup task 02 (same root cause: the async,
  separately-spawned step-lifecycle handoff). Prefer option (4) "drop the
  fallback" for the smallest, least surprising surface.
- Shares `handler/install_wizard/actions.rs` + `install_wizard/state.rs` with
  task 06 — run them serially on the same branch (chain A), not parallel worktrees.
