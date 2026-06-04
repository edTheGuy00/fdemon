## Task: Wizard handlers — Android dispatch + JDK gate + completion + copy command

**Objective**: Wire the Android Tools step into the wizard handlers: build
`AndroidStepParams` from settings, **gate** dispatch on a present JDK 17 (surface the
guided command instead when missing), persist the discovered SDK root and re-run
preflight on completion, and handle the `InstallWizardCopyCommand` (`c`) message by
copying the selected step's guided command to the clipboard.

**Depends on**: 04, 05

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`: extend
  `handle_run_selected_step` with the `AndroidTools` arm + JDK gate + PathConfig
  android-root wiring; extend `handle_step_completed` with the `AndroidTools` arm
  (persist `android_sdk_root` → `PersistSettings` → re-run preflight); add
  `handle_copy_command`.
- `crates/fdemon-app/src/handler/update.rs`: route
  `Message::InstallWizardCopyCommand` → `handle_copy_command`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs`: `InstallWizardCopyCommand`, `WizardStep*`.
- `crates/fdemon-app/src/handler/mod.rs`: `AndroidStepParams`, `RunWizardStep`
  fields, `UpdateAction::WriteClipboard`, `PersistSettings`.
- `crates/fdemon-app/src/install_wizard/state.rs`: `selected_guided_command()`,
  `WizardStep`, `WizardStepKind`, report access for JDK status.
- `crates/fdemon-app/src/config/types.rs` + `config/settings.rs`:
  `ToolchainSettings` fields, settings-persistence pattern.
- The Phase 2 `handle_run_selected_step` (FlutterSdk/PathConfig arms) and
  `handle_step_completed(FlutterSdk)` completion chain as the template.

### Details

**`handle_run_selected_step` — AndroidTools arm with JDK gate:**

```rust
WizardStepKind::AndroidTools => {
    // JDK gate: sdkmanager requires a JDK. If the preflight JDK component is not Ok,
    // do NOT auto-run a privileged install — point the user at the guided command.
    if jdk_status(state) != ComponentStatus::Ok {
        state.install_wizard.status_message = Some(
            "Install JDK 17 first (see the command below), then press 'r' to re-check.".into()
        );
        // Optionally finish a Failed step so the detail pane reflects the block,
        // or simply set the status message + return none(). Pick one and be consistent.
        return UpdateResult::none();
    }
    let ts = &state.settings.toolchain;
    let params = AndroidStepParams {
        sdk_root: ts.android_sdk_root.clone(),
        api_level: ts.android_api_level,
        cmdline_tools_build: ts.cmdline_tools_build.clone(),
        jdk_path: ts.jdk_path.clone(),
    };
    begin_step(state, kind);
    UpdateResult::action(UpdateAction::RunWizardStep {
        kind, install: None, path_bin_dir: None, android_sdk_root: None,
        android: Some(params),
    })
}
```

**PathConfig arm** — also pass the Android SDK root so the executor writes
`ANDROID_HOME`. Source it from `settings.toolchain.android_sdk_root` (or the
preflight-discovered root); pass `None` when no Android SDK is present:

```rust
WizardStepKind::PathConfig => {
    // existing: compute flutter bin dir → path_bin_dir
    let android_sdk_root = state.settings.toolchain.android_sdk_root.clone();
    UpdateResult::action(UpdateAction::RunWizardStep {
        kind, install: None, path_bin_dir: Some(bin), android_sdk_root, android: None,
    })
}
```

**`handle_step_completed` — AndroidTools arm** (mirror the FlutterSdk chain):

```rust
WizardStepKind::AndroidTools => {
    finish_step(state, StepExecStatus::Succeeded, summary);
    if let Some(root) = sdk_path {              // reused field carries the Android SDK root
        state.settings.toolchain.android_sdk_root = Some(root);
        // persist + re-run preflight so the Android checks flip to Ok
        return UpdateResult::message_and_action(
            Message::InstallWizardRerunPreflight,
            UpdateAction::PersistSettings { settings: Box::new(state.settings.clone()), project_path: state.project_path.clone() },
        );
    }
    UpdateResult::message(Message::InstallWizardRerunPreflight) // still re-check even if root unknown
}
```

**`handle_copy_command`** — `c` key:

```rust
fn handle_copy_command(state: &mut AppState) -> UpdateResult {
    match state.install_wizard.selected_guided_command() {
        Some(cmd) => {
            // WriteClipboard is a runner-side side effect applied via pending_runner_actions
            state.pending_runner_actions.push(UpdateAction::WriteClipboard { text: cmd.command.clone() });
            state.install_wizard.status_message = Some(format!("Copied: {}", cmd.command));
            UpdateResult::none()
        }
        None => {
            state.install_wizard.status_message = Some("No command to copy for this step.".into());
            UpdateResult::none()
        }
    }
}
```

`jdk_status(state)` reads the `ComponentKind::Jdk` entry from
`state.install_wizard.report`; treat a missing report as not-Ok (don't dispatch).

### Acceptance Criteria

1. Pressing `Enter` on `AndroidTools` when JDK is `Ok` returns
   `RunWizardStep { kind: AndroidTools, android: Some(..), .. }` with params sourced
   from `settings.toolchain`.
2. Pressing `Enter` on `AndroidTools` when JDK is **not** `Ok` does **not** return a
   `RunWizardStep` action; it sets a status message directing the user to the guided
   command and `r` to re-check.
3. `handle_step_completed(AndroidTools, .., sdk_path: Some(root))` persists
   `[toolchain] android_sdk_root`, returns `PersistSettings`, and triggers a
   preflight re-run (which flips the Android checks to `Ok`).
4. `InstallWizardCopyCommand` pushes `UpdateAction::WriteClipboard` with the selected
   step's guided command, or sets a "no command" status when there is none.
5. PathConfig dispatch now includes `android_sdk_root` when present.
6. `update.rs` routes `InstallWizardCopyCommand`. `cargo check`/`test -p fdemon-app`
   pass; handlers stay pure (no I/O).

### Testing

```rust
#[test]
fn test_android_step_gated_when_jdk_missing() {
    let mut state = wizard_state_with_jdk(ComponentStatus::Missing);
    select_step(&mut state, WizardStepKind::AndroidTools);
    let r = handle_run_selected_step(&mut state);
    assert!(r.action.is_none());
    assert!(state.install_wizard.status_message.unwrap().contains("JDK 17"));
}

#[test]
fn test_android_step_dispatches_when_jdk_ok() {
    let mut state = wizard_state_with_jdk(ComponentStatus::Ok);
    select_step(&mut state, WizardStepKind::AndroidTools);
    let r = handle_run_selected_step(&mut state);
    assert!(matches!(r.action, Some(UpdateAction::RunWizardStep { kind: WizardStepKind::AndroidTools, android: Some(_), .. })));
}

#[test]
fn test_completed_android_persists_sdk_root_and_reruns() { /* assert PersistSettings + InstallWizardRerunPreflight */ }

#[test]
fn test_copy_command_pushes_write_clipboard() { /* guided command present → WriteClipboard pushed */ }

#[test]
fn test_copy_command_no_command_sets_status() { /* no guided command → status message */ }
```

### Notes

- **JDK gate is the linchpin of the "guided, never auto-run" decision** — keep it
  in the handler so no privileged install is ever dispatched. The executor (task 06)
  has a defense-in-depth failure path but the primary gate is here.
- Reuse the FlutterSdk completion chain verbatim where possible; the only difference
  is the settings field written (`android_sdk_root` vs `flutter.sdk_path`) and that
  no `ScanInstalledSdks` is needed for Android.
- `WriteClipboard` already exists and is applied by the runner via
  `pending_runner_actions` (same as log-copy) — do not invent a new mechanism.
- Decide once whether the JDK-gate path leaves the step `Pending` (status message
  only) or marks it `Failed`; the task-08 detail rendering should match that choice.
  Recommended: status message + leave step status as-is (don't mark Failed for a
  precondition), so the guided command stays visible without a red error.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-afec499e5b7af5010

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Added `AndroidTools` arm to `handle_run_selected_step` (JDK gate + param sourcing); extended `PathConfig` arm to pass `android_sdk_root`; added `AndroidTools` arm to `handle_step_completed` (persist + preflight re-run); added `handle_copy_command`; added `jdk_status` helper; added 12 new unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Replaced the no-op stub for `InstallWizardCopyCommand` with a call to `install_wizard::handle_copy_command(state)` |

### Notable Decisions/Tradeoffs

1. **`WriteClipboard` via `UpdateResult::action` not `pending_runner_actions`**: The task description suggested pushing directly to `pending_runner_actions`, but the established pattern in the codebase (e.g. `CopyLogEntryToClipboard`) returns `UpdateResult::action(UpdateAction::WriteClipboard { .. })`. The `process.rs` interceptor already moves `WriteClipboard` actions into `pending_runner_actions` automatically — using the same pattern avoids a special case and keeps the handler pure.

2. **JDK gate leaves step status unchanged (not `Failed`)**: Per the task's recommendation, when JDK is not Ok the step stays in its current status with only a `status_message` set. This keeps the guided command visible in the detail pane (a `Failed` status would visually suggest something went wrong with the install itself, not just a precondition).

3. **`android_sdk_root` passed via reused `sdk_path` slot in `handle_step_completed`**: The `WizardStepCompleted` message's `sdk_path: Option<PathBuf>` field is reused to carry the Android SDK root back from the executor. The field name matches its semantic use for `FlutterSdk`, and the Android use is documented in the handler comment.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (2749 fdemon-app tests; all workspace tests pass)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- 12 new unit tests added covering: JDK-gate blocking (Missing, Partial, no entry), JDK-gate pass-through (Ok), params sourced from settings, completed AndroidTools persists sdk_root + reruns preflight, completed without sdk_root still reruns preflight, PathConfig includes android_sdk_root, copy command pushes WriteClipboard, copy command with no command sets status, copy command text matches guided command

### Risks/Limitations

1. **Executor (task 06) not yet merged**: The `RunWizardStep { android: Some(..) }` action will reach `handle_action` which must have an `AndroidTools` arm. Until task 06 is wired, pressing Enter on AndroidTools (with JDK Ok) will produce an unhandled action in `actions/mod.rs`. This is a known deferred dependency — the gate is correct and will work end-to-end once task 06 lands.
