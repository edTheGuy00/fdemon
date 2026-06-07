## Task: Reset per-run `execution` on `apply_report` so a re-check shows the refreshed component list (F-PR53-12)

**Severity:** MEDIUM (correctness / UX)

**Objective**: Stop the wizard detail pane from showing a stale
Failed/Cancelled/Succeeded `StepProgress` view over a freshly re-checked,
now-passing component list. After a re-check (`apply_report`), the per-run
`execution` display state must be cleared.

**Depends on**: 01 (shares `install_wizard/state.rs` + `handler/install_wizard/actions.rs`)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (`is_execution_active_for` at 605-616, render gate at 654-665)

### Details

`apply_report` (`install_wizard/state.rs:164-172`) rebuilds `steps`, clears
`loading`, clamps `selected_index`, and resets `selected_command_index`, but
**never touches `self.execution`**. `execution` is reset only by `begin_step`
(state.rs:270, new run) and `reset_running_step_to_idle` (cancel).

The detail pane gate `is_execution_active_for` (step_detail.rs:605-616) renders
the `StepProgress`/result view (replacing the static component list) whenever
`execution.kind == Some(selected_step.kind)` and status is
Running/Succeeded/Failed/Cancelled. So after `handle_preflight_completed` →
`apply_report`:

- **Failure + `r` re-check**: `handle_step_failed` leaves
  `execution = {Failed, ...}`; the user fixes the cause and re-checks, but the pane
  keeps showing the stale "Failed" view over the now-Ok component list.
- **Auto re-check after success**: `handle_step_completed` auto-fires
  `InstallWizardRerunPreflight` on successful AndroidTools/PathConfig steps
  (actions.rs:456-491), leaving `execution = {Succeeded, ...}` while `apply_report`
  rebuilds the now-Ok steps — the stale success view hides the refreshed list.

(The common FlutterSdk-success path is masked because it auto-closes via handback;
this affects the non-handback steps.)

### Proposed Fix

In `apply_report`, after rebuilding steps, reset the per-run execution display:

```rust
self.execution = StepExecution::default();
```

This is safe: the handback predicate `flutter_now_live()` (state.rs:241) reads
`report.components`, not `execution`, so clearing execution does not affect
auto-close. (Confirm whether `installed_sdk_path` should also be cleared on a
fresh report; leave it unless it visibly leaks across re-checks.) Alternatively
reset in `handle_preflight_completed` after `apply_report` — prefer doing it in
`apply_report` so every report-application path is covered.

### Acceptance Criteria

1. After `begin_step(kind); finish_step(Failed/Cancelled, ...); apply_report(report_now_ok)`,
   `execution.status == Idle` (default) and `execution.kind == None`, so the static
   component list renders.
2. The same holds after a successful AndroidTools/PathConfig step triggers an auto
   re-check.
3. Auto-close handback still works (a live-Flutter report still closes the wizard) —
   no regression, since handback reads `report.components`.

### Testing

```rust
// install_wizard/state.rs test module
// - test_apply_report_resets_execution: begin_step + finish_step(Failed);
//     apply_report(report); assert execution == StepExecution::default().
// handler/install_wizard/actions.rs test module
// - drive handle_step_failed then handle_preflight_completed(report_ok); assert the
//   detail pane would render the component list (execution inactive for the step).
// - regression: handback auto-close test still passes.
```

### Notes

- Shares both files with task 01 — run serially on the same branch (chain A:
  01 → 06), not parallel worktrees.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Added `self.execution = StepExecution::default();` in `apply_report`; added 3 new unit tests covering Failed/Cancelled/Succeeded reset |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Added 3 new tests: `test_preflight_completed_after_failed_step_clears_execution`, `test_preflight_completed_after_cancelled_step_clears_execution`, and `test_preflight_completed_handback_still_fires_after_execution_reset` |

### Notable Decisions/Tradeoffs

1. **Fix location in `apply_report` not `handle_preflight_completed`**: The task explicitly preferred fixing it in `apply_report` so every report-application path is covered, not just the `handle_preflight_completed` call site. This is the correct choice since `apply_report` is also called directly (e.g. in tests and via `handle_rerun_preflight` → `handle_preflight_completed`).
2. **`installed_sdk_path` not cleared**: The task said to leave it unless it visibly leaks across re-checks. Since `installed_sdk_path` is used by the PathConfig step to resolve the Flutter bin dir, clearing it on a re-check would break the PathConfig step after a FlutterSdk success. Left untouched.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all results ok, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

New tests added:
- `state::tests::test_apply_report_resets_execution` - verifies Failed execution cleared
- `state::tests::test_apply_report_resets_cancelled_execution` - verifies Cancelled execution cleared
- `state::tests::test_apply_report_resets_succeeded_execution` - verifies Succeeded execution cleared
- `actions::tests::test_preflight_completed_after_failed_step_clears_execution` - integration path: Failed → apply_report → Idle
- `actions::tests::test_preflight_completed_after_cancelled_step_clears_execution` - integration path: Cancelled → apply_report → Idle
- `actions::tests::test_preflight_completed_handback_still_fires_after_execution_reset` - regression: handback auto-close still fires after execution reset

### Risks/Limitations

1. **None identified**: The change is minimal (one line in `apply_report`) and the handback predicate `flutter_now_live()` reads `report.components`, not `execution`, confirming no regression risk to the auto-close path.
