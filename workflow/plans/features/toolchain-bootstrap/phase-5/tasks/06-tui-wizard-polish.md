## Task: TUI wizard polish — run-failed badge + cancel hint

**Objective**: Add the display-side affordances for the abort/retry UX: a "run
failed" indicator on the step-list badge after a failed execution, and an "Esc
cancels" hint in the detail pane while a step is running.

**Depends on**: 03-abort-retry-ux-app (the cancel key/state it hints at)

**Estimated Time**: 2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs`: when the active
  execution for a step is `StepExecStatus::Failed`, render a distinct run-failed
  indicator (✗ glyph / red accent) on that step's badge, layered over the preflight
  rollup badge.
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`: while the selected
  step's execution is `Running`, render an "Esc cancels" hint; ensure the retry
  prompt set by task 03 (`status_message`) is shown after a failure.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard` — `StepExecStatus`, the execution state on
  `InstallWizardState` (already exposed; no new fields needed), `WizardStepKind`.

### Details

The step list today renders only the preflight rollup badge
(`StepStatus::Ok/Partial/Missing/Pending`). Execution state already lives on
`InstallWizardState` (the `step_detail` widget reads it via `is_execution_active_for`
at `step_detail.rs:~525`), so this is a render-only change — no new state.

```rust
// step_list.rs — pseudocode
let run_failed = exec.kind == Some(step.kind) && exec.status == StepExecStatus::Failed;
let badge = if run_failed { failed_glyph() /* ✗, red */ } else { rollup_badge(step.status) };
```

- **Badge precedence:** a run-failed indicator should visually override the stale
  preflight badge (which still reads Missing/Partial after a failed run — by design,
  per the audit). Keep it unambiguous that *this run* failed.
- **Cancel hint:** only while `Running`; when idle, restore the existing key hints.
  Keep it consistent with the existing hint styling in `step_detail.rs`.
- Update any render snapshot/assertion tests that pin the old "no badge / later
  phase" output for these states.

### Acceptance Criteria

1. After a failed step execution, the step-list entry for that step shows a
   run-failed indicator distinct from the preflight rollup badge.
2. While a step is `Running`, the detail pane shows an "Esc cancels" hint; the hint
   is absent when idle/succeeded/failed.
3. The retry prompt (`status_message` from task 03) is visible in the detail pane
   after a failure.
4. New render assertions cover the failed-badge and running-hint states; existing
   render tests updated, no regressions.

### Testing

```rust
#[test]
fn step_list_shows_failed_indicator_after_failed_execution() {
    // arrange InstallWizardState with exec.kind=Some(FlutterSdk), status=Failed
    // render step_list to a TestBackend buffer; assert the failed glyph at that row
}
#[test]
fn detail_shows_esc_cancels_hint_while_running() { /* status=Running -> hint present */ }
#[test]
fn detail_hides_cancel_hint_when_idle() { /* no running step -> hint absent */ }
```

### Notes

- Pure rendering; no message/state changes (those are task 03). Keep all logic in the
  widgets.
- Parallel-safe with task 04 (disjoint files: TUI widgets vs app handlers).

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
