# Task 04 — Wizard phase-label message + stash clearing

**Agent:** implementor
**Status:** Not Started
**Depends On:** -
**Estimated Hours:** 2-3h
**Modules:** `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/actions/mod.rs`,
`crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/handler/update.rs`

## Context

Phase 2 added `StepExecution::phase_label` and a tested `set_step_phase` mutator on
`InstallWizardState`, and the `StepProgress` widget renders that phase label. But the
executor forwards `InstallEvent::Phase(label)` as a **log line**
(`WizardStepLog { line: format!("[{label}]") }`), so `phase_label` is never set and the
widget's phase row always shows the default "Running…" — the phase display is dead code
during real installs. Separately, `installed_sdk_path` is stashed on a successful Flutter
install but never cleared after it is consumed, allowing a stale value to win on a later
PathConfig run.

References: `workflow/reviews/features/toolchain-bootstrap-phase-2/ACTION_ITEMS.md`
(M3, m1) and `REVIEW.md`. `set_step_phase` already exists (Phase 2 task 07) — this task
does **not** modify `install_wizard/state.rs`.

## Findings to Fix

### M3 — Phase label is dead UI (MAJOR) — `actions/mod.rs` ~line 901-907
The `InstallEvent::Phase(label)` arm sends `Message::WizardStepLog` instead of updating
the phase label.

**Fix:**
1. **`message.rs`** — add a new variant:
   `WizardStepPhase { kind: WizardStepKind, label: String }` (place it next to the other
   `WizardStep*` variants; mirror their doc-comment style).
2. **`actions/mod.rs`** — change the `InstallEvent::Phase(label)` arm of the
   `install_flutter` callback to `try_send(Message::WizardStepPhase { kind, label:
   label.to_string() })` instead of formatting a log line. (Keep `try_send` backpressure
   semantics, consistent with `WizardStepLog`/`WizardDownloadProgress`.)
3. **`handler/install_wizard/actions.rs`** — add `handle_step_phase(state, kind, label)`
   that calls `state.install_wizard.set_step_phase(label)` (guard on the kind / running
   step as the sibling handlers do). Mirror `handle_step_log`/`handle_step_progress`.
4. **`handler/update.rs`** — add the `Message::WizardStepPhase { .. }` match arm routing
   to `handle_step_phase`, keeping the `update()` match exhaustive (no catch-all).

### m1 — Stale `installed_sdk_path` not cleared (MINOR) — `handler/install_wizard/actions.rs`
`installed_sdk_path` is set on `WizardStepCompleted(FlutterSdk)` and documented as
"cleared when the wizard is closed," but a PathConfig run prefers it over
`settings.flutter.sdk_path`. If the user changes the setting and re-runs PathConfig
without re-installing, the stale stash wins.

**Fix (choose one, document the choice):**
- Clear `installed_sdk_path` once it has been consumed by a **successful** PathConfig
  completion (`handle_step_completed` for `WizardStepKind::PathConfig`), **or**
- Keep the precedence but add a doc comment stating the session stash intentionally wins
  for the duration of the wizard session.
Prefer clearing-on-consume (less surprising). Add/extend a unit test asserting the stash
is cleared after a successful PathConfig completion.

## Acceptance Criteria

- [ ] `Message::WizardStepPhase { kind, label }` exists and the `update()` match is
      exhaustive (no `_` catch-all hiding it).
- [ ] During an install, phase transitions ("Cloning"/"Downloading"/"Verifying"/
      "Extracting") reach `set_step_phase` and update `StepExecution::phase_label` (no
      longer emitted as `[label]` log lines).
- [ ] `handle_step_phase` is unit-tested (a `WizardStepPhase` updates the phase label;
      ignored when no step is running / kind mismatch, mirroring sibling handlers).
- [ ] `installed_sdk_path` is cleared after a successful PathConfig completion (or the
      session-precedence is explicitly documented) — covered by a unit test.
- [ ] Only the four listed files are modified (NOT `install_wizard/state.rs` — reuse the
      existing `set_step_phase`). `cargo fmt`/`check --workspace --all-targets`/
      `test -p fdemon-app`/`clippy -D warnings` pass.

## Notes

- This task is app-layer only and shares no write files with Task 05 (which owns
  `install_wizard/{types,state}.rs` + `progress.rs`). `set_step_phase` already exists, so
  there is no need to touch `state.rs` here — keeping the two tasks parallel-safe.
- If `StepProgress` needs no change to display the now-populated `phase_label`, leave the
  widget alone (it already reads `phase_label`).
- Deferred (out of scope, see REVIEW.md): m6 "Installed (precache incomplete)" status —
  do not implement here.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a45855be55ecb17b6

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `WizardStepPhase { kind: WizardStepKind, label: String }` variant next to other `WizardStep*` variants |
| `crates/fdemon-app/src/actions/mod.rs` | Changed `InstallEvent::Phase(label)` arm to emit `WizardStepPhase` instead of formatting a `[label]` log line |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Added `handle_step_phase` with kind-match guard; updated `handle_step_completed` to clear `installed_sdk_path` after PathConfig success; added 6 new unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Added `Message::WizardStepPhase { kind, label }` arm routing to `handle_step_phase` |

### Notable Decisions/Tradeoffs

1. **Kind-guard in handle_step_phase**: mirrors `handle_step_log` and `handle_step_progress` — the guard checks `execution.kind == Some(kind)` which covers both "no step running" (execution.kind is None) and "kind mismatch" in a single branch.

2. **Clear-on-consume chosen over documented precedence**: `installed_sdk_path` is cleared when PathConfig completes successfully (not on failure, so retries still work). This is less surprising than session-stash winning silently if the user later changes `sdk_path` and re-runs PathConfig.

3. **Failure does not clear stash**: `handle_step_failed` leaves `installed_sdk_path` untouched so a PathConfig retry can still consume it.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app` - Passed (2722 tests)
- `cargo test --workspace` - Passed (all test suites pass)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **No widget change needed**: `StepProgress` already reads `execution.phase_label`; the widget is live once this task populates the field correctly.
