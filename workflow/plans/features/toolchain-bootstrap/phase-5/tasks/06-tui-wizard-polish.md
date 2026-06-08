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

**Status:** Done
**Branch:** worktree-agent-adac4a9c02a702851

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` | Added `GLYPH_RUN_FAILED` constant, `failed_step_kind: Option<WizardStepKind>` field on `StepListPane`, run-failed badge override logic in `render_step_row`, updated `step_list_pane()` constructor signature, updated existing tests to 4-arg constructor, added 3 new run-failed badge tests |
| `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` | Added `CANCEL_HINT_HEIGHT` constant, `show_cancel_hint: bool` field on `StepProgress`, `render_cancel_hint()` method, updated `Widget::render` to include a cancel-hint layout branch for Running state, updated all existing tests to 3-arg constructor, added 4 new cancel-hint tests |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Updated `StepProgress::new` call to pass `show_cancel_hint = execution.status == Running`, added 4 new detail-pane cancel-hint/status tests |
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | Added `failed_execution_kind()` helper method, updated both `step_list_pane()` call sites to pass the computed `failed_step_kind` |

### Notable Decisions/Tradeoffs

1. **`show_cancel_hint` on `StepProgress` vs overlay in `step_detail.rs`**: Added the field to `StepProgress` so the hint is part of the progress widget's layout system, avoiding manual coordinate arithmetic for a reserved hint row. This keeps all layout math in `StepProgress::render`.
2. **Terminal states don't show cancel hint**: Even if `show_cancel_hint=true` is passed (which `step_detail.rs` never does for terminal states), the terminal branch in `StepProgress::render` is taken first, so the cancel hint never appears for Succeeded/Failed. Belt-and-suspenders.
3. **Run-failed badge colour when selected+focused**: When the failed step is the currently selected+focused row, the glyph uses `CONTRAST_FG` (black on accent bg) rather than `STATUS_RED`. This is consistent with the existing badge behaviour for all states — the accent row overrides individual glyph colours for readability. Tests reflect this by selecting a different row as current when checking the red colour.
4. **`status_message` (retry prompt)**: Already rendered in the wizard footer by `InstallWizardPanel::render_footer` (line 239 in `mod.rs`). No new rendering needed — acceptance criterion 3 is met by the existing footer.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all crates: 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- New tests added: 3 step_list run-failed badge tests, 4 progress cancel-hint tests, 4 step_detail cancel-hint/idle tests

### Risks/Limitations

1. **Cancel hint layout with very short area**: When `area.height < 5` (PHASE+PROGRESS+SEP+HINT = 4 rows minimum + 0 for log tail), the `Layout` system gracefully clips the hint row to 0 height. The `render_cancel_hint` guard `if area.height < 1 { return; }` provides an additional safety net.
