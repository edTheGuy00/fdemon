## Task: Render cancellation as cancelled (not failed) + distinct run-failed badge (F6, F12, F11, F17, F18)

**Severity:** MEDIUM (F6, F12, F11) + LOW (F17, F18)

**Objective**: A user-initiated cancellation must never look like an install failure
(no red "Failed" summary, no red run-failed badge, no scary retry-as-failure framing),
and the genuine run-failed badge must be visually distinct from a plain `Missing`
badge. Eliminate the brittle `starts_with("Cancelled:")` string convention.

**Depends on**: 02 (same files: `handler/install_wizard/actions.rs`, `state.rs`)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/types.rs` (`StepExecStatus`)
- `crates/fdemon-app/src/install_wizard/state.rs` (`finish_step` / status transitions)
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (`handle_step_failed` cancel branch)
- `crates/fdemon-app/src/actions/mod.rs` (cancel reason formatting — F17)
- `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` (result-summary + glyph colour)
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` (run-failed glyph distinctness — F11)
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (terminal-state match)
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (`failed_execution_kind`)

### Details

**F6/F12 — cancel rendered as Failed.** There are two divergent cancel end-states.
The clean `Esc` path (`handle_cancel_step`) calls `reset_running_step_to_idle()` →
neutral. But the daemon can win the race: `download_to_file`'s `tokio::select!`
returns `Error::Cancelled` and the executor enqueues
`WizardStepFailed { reason: "Cancelled: …" }` *before* `join.abort()` lands (abort
can't recall a sent message). `handle_step_failed` takes the `Cancelled:` branch but
still calls `finish_step(StepExecStatus::Failed, reason)` (`actions.rs:396-398`). The
progress widget colours a `Failed` `result_summary` `STATUS_RED`
(`progress.rs:259-264`), and `failed_execution_kind()` (`mod.rs:194-198`) returns
`Some(kind)` whenever `status == Failed` → red `✗` run-failed badge
(`step_list.rs:157`) + retry-as-failure framing — exactly what Task 03 AC#4 forbids.
There is **no** `Cancelled` variant in `StepExecStatus` (`types.rs:65` has only
Idle/Running/Succeeded/Failed), so the TUI cannot tell cancel from failure.

**F11 — run-failed badge not distinct.** `GLYPH_RUN_FAILED` (`step_list.rs:50`) is the
**same** codepoint `✗` as `GLYPH_MISSING` (`step_list.rs:41`), and both render
`STATUS_RED`. After a failed install the step's preflight status is almost always
`Missing`, so the run-failed override is byte-for-byte identical to the plain Missing
badge — Task 06 AC#1 ("distinct from the preflight rollup badge") is unmet for the
dominant case.

**F17 — double prefix.** `Error::Cancelled` Display is `#[error("Cancelled: {message}")]`
(`error.rs:106`), so `format!("Cancelled: {e}")` (`actions/mod.rs:1002,1099`) yields
`"Cancelled: Cancelled: …"`, stored verbatim in `result_summary` and shown in the
progress widget.

### Fix

1. **Add `StepExecStatus::Cancelled`** (`types.rs:65`). Route the `Cancelled:` branch
   of `handle_step_failed` through `finish_step(StepExecStatus::Cancelled, reason)`
   instead of `::Failed` (or converge it onto the neutral `reset_running_step_to_idle`
   path + a "Cancelled. Press Enter to retry." `status_message`).
2. **Neutral rendering for `Cancelled`:** `failed_execution_kind()` already returns
   `None` for non-`Failed`, so the red badge is suppressed automatically. Add a neutral
   render arm for `Cancelled` in `progress.rs` (muted glyph / `TEXT_SECONDARY`, not
   `STATUS_RED`) for both the status glyph and `render_result_summary`. Include
   `Cancelled` alongside `Failed` in `step_detail.rs`'s terminal-state match so the
   retry hint still shows, but without failure styling.
3. **F17:** in `actions/mod.rs:1002` and `:1099`, forward the error directly —
   `reason: format!("{e}")` — since Display already carries the `"Cancelled: "` prefix.
   The `reason.starts_with("Cancelled:")` check (if still used) keeps working; once the
   `Cancelled` variant lands you can drop the string check entirely in favour of the
   variant. Prefer eventually replacing the magic-string convention with the
   variant-driven branch.
4. **F11:** give the run-failed badge a distinguishing attribute a plain `Missing`
   badge never has — e.g. `Modifier::BOLD` on the glyph style in `render_step_row`
   when `run_failed` (apply in the selected+focused branch too), or a distinct
   codepoint. Keep red as the colour.

### Acceptance Criteria

1. A cancellation that arrives as a terminal `WizardStepFailed("Cancelled:…")` leaves
   the step in `Cancelled` (not `Failed`): no `STATUS_RED` result summary, no red
   run-failed badge, neutral glyph; the step is still retriable via `Enter` (F6/F12).
2. The cancel reason shown to the user contains exactly one `Cancelled:` prefix (F17).
3. The run-failed badge carries a distinguishing visual attribute (e.g. `BOLD`) that a
   plain `Missing` badge does not — a test asserts the run-failed glyph cell has the
   attribute and the plain `Missing` cell does not (F11).
4. The actions-layer `is_cancelled()` → reason mapping is unit-tested (F18).

### Testing

```rust
// app: install_wizard/state.rs + handler/install_wizard/actions.rs + actions/mod.rs
// - NEW (F18): construct fdemon_core::Error::cancelled("download cancelled");
//     assert format!("{e}") starts with exactly one "Cancelled:" (no doubling);
//     feed a WizardStepFailed{reason} through handle_step_failed and assert the step
//     status is Cancelled (not Failed) with a neutral status_message.
// tui: widgets/install_wizard/{step_list,progress,mod}.rs
// - NEW (F11): run-failed Missing step → glyph cell has Modifier::BOLD; plain Missing
//     step → glyph cell does NOT have it (assert distinctness, not just STATUS_RED).
// - NEW (F12): a step in StepExecStatus::Cancelled renders no red run-failed badge and
//     a neutral result summary.
// - KEEP existing failed-rendering tests green (genuine Failed still renders red).
```

### Notes

- This depends on Task 02's `install_task`/cancel semantics and shares
  `handler/install_wizard/actions.rs`, `actions/mod.rs`, `state.rs` with it — serialise
  on the same branch (chain A).
- Prefer the `StepExecStatus::Cancelled` variant over a TUI-layer
  `result_summary.starts_with("Cancelled:")` gate — the latter keeps the fragile
  string coupling and forces touching every Failed-styled widget.
- Optionally clear `execution.kind` in `reset_running_step_to_idle` for defensiveness;
  the `Cancelled` variant already makes any stale-kind render benign.
