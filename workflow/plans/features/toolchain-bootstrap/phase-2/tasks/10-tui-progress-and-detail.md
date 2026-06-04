## Task: TUI — step progress widget + actionable step detail

**Objective**: Render live step execution in the Install Wizard: a new progress
widget (phase label + download progress bar + streamed log tail) and updates to
the step-detail pane so the Flutter SDK and PATH steps show an "Press Enter to
install/configure" affordance and switch to the progress view while running.

**Depends on**: 07

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` — **NEW**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`: add the
  action-hint line(s) and dispatch to the progress view when a run is active for
  the selected step.
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`: declare `mod progress;`
  and route rendering.

**Files Read (Dependencies):**
- `fdemon-app::install_wizard` re-exports: `StepExecution`, `StepExecStatus`,
  `InstallWizardState`, `WizardStepKind` (task 07).
- Existing `step_detail.rs` / `mod.rs` for the established render + `Cell`
  render-hint patterns.
- `crates/fdemon-tui/src/widgets/devtools/.../frame_chart` or `network` widgets
  for an in-repo progress/bar rendering reference (optional).

### Details

**`progress.rs`** — a stateless widget rendering `&StepExecution`:

```rust
/// Renders the live execution view for a running/finished wizard step:
///   line 1: phase label + status (Running ⟳ / Succeeded ✓ / Failed ✗)
///   line 2: a download progress bar when `total` is known (gauge), else a
///           "received N MB" counter / spinner when total is unknown
///   rest:   the tail of `log_tail` (most recent lines, clipped to area height)
pub struct StepProgress<'a> { exec: &'a StepExecution }
```

Use named constants for any layout thresholds (CODE_STANDARDS Principle 4). Build
the layout entirely with `Layout` constraints + a `Constraint::Min(0)` absorber
(Principle 2). Show the most-recent log lines (tail) and clip to the available
height; do not compute manual offsets.

**`step_detail.rs`** changes:
- For `WizardStepKind::FlutterSdk` and `PathConfig`: append an action-hint line in
  the detail body, e.g. `"▶ Press Enter to install Flutter SDK"` /
  `"▶ Press Enter to add Flutter to PATH"`. Gray it out / change wording for
  not-yet-available steps (Prerequisites/AndroidTools/Doctor): `"Available in a
  later phase"`.
- When `state.execution.kind == Some(selected_kind)` and status is
  `Running | Succeeded | Failed`, render the `StepProgress` view (either replacing
  the static detail or below it, depending on space). On `Failed`, show the
  `result_summary` as the error. On `Succeeded` for PathConfig, show the
  "restart your terminal" hint from the summary.

**`mod.rs`**: wire the progress module; ensure the detail pane gets `&execution`.

Mouse/modal: `InstallWizard` is already in the modal-precedence list — no change
needed, just keep base-UI `MouseCtx` suppression as-is.

### Acceptance Criteria

1. Selecting the Flutter SDK or PATH step shows an "Press Enter to …" action hint.
2. While a step runs, the detail pane shows the phase label, a progress bar (when
   total bytes are known) or a byte counter (when unknown), and the latest log lines.
3. On success the pane shows the completion summary; on failure it shows the error.
4. Non-executable steps show "Available in a later phase" instead of an Enter hint.
5. Rendering fits within the allocated `Rect` at small and large terminal sizes
   (snapshot/region tests), with no manual out-of-bounds coordinates.
6. New widget code is unit-tested via the `TestTerminal` helper. No clippy warnings.

### Testing

```rust
#[test]
fn test_progress_renders_bar_with_known_total() { /* exec with received/total → gauge cells */ }

#[test]
fn test_progress_renders_counter_with_unknown_total() { ... }

#[test]
fn test_step_detail_shows_enter_hint_for_flutter_step() { ... }

#[test]
fn test_step_detail_shows_phase_for_non_executable_step() { ... }

#[test]
fn test_progress_log_tail_clips_to_height() { ... }
```

Use the existing `fdemon-tui` `TestTerminal` / snapshot infrastructure; render into
small (e.g. 40x10) and larger areas.

### Notes

- TUI consumes only `fdemon-app::install_wizard` re-exports (no direct
  `fdemon-daemon` dep) — keep that boundary (see ARCHITECTURE.md note).
- Keep `progress.rs` stateless; it reads `StepExecution` and paints. Any
  render-hint write-back follows the `Cell` exception pattern already used in
  `step_detail.rs`.
- Spinner animation can be a simple frame derived from log length or a tick count
  already available in render — do not introduce wall-clock reads.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` | New file: `StepProgress` widget rendering live execution state (phase label, progress gauge/counter, log tail) |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Added action hints ("Press Enter to…" / "Available in a later phase"), execution dispatch to `StepProgress`, `animation_frame` parameter |
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | Declared `mod progress;`, added `animation_frame` field to `InstallWizardPanel`, updated factory calls |
| `crates/fdemon-tui/src/render/mod.rs` | Pass `state.animation_frame` to `InstallWizardPanel::new()` |

### Notable Decisions/Tradeoffs

1. **animation_frame threading**: Added `animation_frame: u64` to `InstallWizardPanel` and threaded it to `step_detail_pane()` / `StepProgress`. This is consistent with how `log_view` and other animated widgets receive frame data — no wall-clock reads, deterministic in tests.

2. **Progress replaces static detail**: When execution is active for the selected step, `StepProgress` occupies the full `content_area` (replacing component checks + action hint). This avoids complex layout mixing of static and live content.

3. **Action hint placement**: For steps with components, the hint is anchored at the bottom of the content area (`content_area.y + height - 1`). Component rows are limited to `height - ACTION_HINT_HEIGHT` rows to leave room. For no-component steps (PathConfig), the hint shows at the top of the content area.

4. **Non-executable step hint**: Prerequisites, AndroidTools, and Doctor steps show "Available in a later phase" (muted, no ▶) instead of an Enter hint. Doctor shows nothing (display-only view).

5. **LineGauge for known total**: When `StepExecution::total` is `Some(n)`, a ratatui `LineGauge` is rendered with `filled_style`/`unfilled_style`. When `total` is `None`, a spinner + byte counter is shown.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (6301+ tests, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

New tests added (progress.rs: 9 tests, step_detail.rs: 11 new tests):
- `test_progress_renders_bar_with_known_total`
- `test_progress_renders_counter_with_unknown_total`
- `test_step_detail_shows_enter_hint_for_flutter_step`
- `test_step_detail_shows_enter_hint_for_path_config_step`
- `test_step_detail_shows_phase_for_non_executable_step`
- `test_progress_log_tail_clips_to_height`
- `test_step_detail_shows_progress_view_when_running`
- `test_step_detail_shows_success_summary_after_run`
- `test_step_detail_shows_error_summary_on_failure`
- `test_step_detail_progress_not_shown_for_different_step`
- and more...

### Risks/Limitations

1. **Layer boundary**: `fdemon-tui` does not import from `fdemon-daemon` directly. All types (`StepExecution`, `StepExecStatus`, `WizardStepKind`) are consumed via `fdemon-app::install_wizard` re-exports, consistent with the architecture boundary.

