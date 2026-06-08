## Task: Wizard step handlers + completion wiring

**Objective**: Wire the wizard step lifecycle into the TEA update loop: handle
`InstallWizardRunSelectedStep` (build + dispatch `RunWizardStep`), ingest
`WizardStepStarted/Log/DownloadProgress/Completed/Failed` into execution state,
and on Flutter-SDK success persist `[flutter] sdk_path`, re-run preflight, and
refresh the FVM-cache version list.

**Depends on**: 05, 06, 07

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`: add the step
  lifecycle handlers (next to `handle_preflight_completed`/`handle_rerun_preflight`).
- `crates/fdemon-app/src/handler/update.rs`: route the new `Message` variants to
  the handlers (replace any task-05 stub arms).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs` (`WizardStep*`, `InstallWizardRunSelectedStep`).
- `crates/fdemon-app/src/install_wizard/state.rs` (execution mutators from task 07).
- `crates/fdemon-app/src/config/types.rs` (`ToolchainSettings`, `install_method()`).
- `crates/fdemon-app/src/handler/mod.rs` (`RunWizardStep`, `FlutterStepParams`,
  `RunToolchainPreflight`, `ScanInstalledSdks`, `PersistSettings`).
- `crates/fdemon-app/src/handler/flutter_version/actions.rs` `handle_switch_completed`
  — template for "re-resolve + re-scan after a successful SDK operation".

### Details

**Trigger handler** — `handle_run_selected_step(state) -> UpdateResult`:
1. Guard: if `state.install_wizard_state.is_step_running()`, return `none()`.
2. Read the selected step's `kind`.
3. For `FlutterSdk`:
   - Build `FlutterStepParams { method: state.settings.toolchain.install_method(),
     channel: state.settings.toolchain.channel.clone(),
     install_root: state.settings.toolchain.flutter_install_dir.clone() }`.
   - Return `UpdateAction::RunWizardStep { kind, install: Some(params), path_bin_dir: None }`.
4. For `PathConfig`:
   - Resolve the Flutter bin dir. Prefer the just-installed `sdk_path`
     (stored after a FlutterSdk completion — see below), else
     `state.settings.flutter.sdk_path.join("bin")` when set, else the resolved SDK
     from the report. If none is known, set a `status_message` ("Install Flutter
     first") and return `none()`.
   - Return `UpdateAction::RunWizardStep { kind, install: None, path_bin_dir: Some(bin) }`.
5. For other kinds: set `status_message` ("Available in a later phase"), `none()`.
6. Call `state.install_wizard_state.begin_step(kind)` before returning the action so
   the UI flips to Running immediately. (Or begin on `WizardStepStarted` — pick one
   and be consistent; beginning on dispatch avoids a one-frame gap.)

**Ingest handlers** (each takes the relevant fields, mutates execution state):
- `WizardStepStarted` → `begin_step(kind)` (idempotent if already begun on dispatch).
- `WizardStepLog { line, .. }` → `push_step_log(line)`.
- `WizardDownloadProgress { received, total, .. }` → `set_step_progress(..)`.
- `WizardStepFailed { reason, .. }` → `finish_step(Failed, reason)`.
- `WizardStepCompleted { kind, summary, sdk_path }` →
  `finish_step(Succeeded, summary)`, then:
  - If `kind == FlutterSdk` and `sdk_path.is_some()`:
    1. `state.settings.flutter.sdk_path = sdk_path.clone();`
    2. Stash `sdk_path` somewhere reachable for the subsequent PathConfig step
       (e.g. `install_wizard_state.installed_sdk_path: Option<PathBuf>` — add this
       field in task 07 if not present, or store on execution result).
    3. **Chain the effects via follow-up messages** (decided approach — do NOT
       introduce a batch action). `UpdateResult` returns at most one `action` +
       one follow-up `message`, so sequence the side effects as a message hop chain:
       - Return `UpdateAction::PersistSettings { settings: state.settings.clone(),
         project_path }` as the action, **and** set the follow-up `message` to
         `Message::InstallWizardRerunPreflight`.
       - `handle_rerun_preflight` (existing) sets `loading = true` and dispatches
         `RunToolchainPreflight`, so preflight re-runs after the persist.
       - For the FVM re-scan, emit `ScanInstalledSdks` on the next hop: have the
         preflight-completed handler (or a dedicated follow-up) trigger
         `ScanInstalledSdks` so the Flutter Version panel refreshes. Reuse the
         `handle_switch_completed` pattern in `flutter_version/actions.rs`.

> Implement this as a deterministic message chain:
> `WizardStepCompleted(FlutterSdk)` → action `PersistSettings` + follow-up
> `InstallWizardRerunPreflight` → `RunToolchainPreflight` → `ToolchainPreflightCompleted`
> (which may additionally trigger `ScanInstalledSdks`). Document the exact hops you
> wire. No new `UpdateAction::Batch` primitive.

**update.rs**: add match arms dispatching each `Message` variant to the handler
above, and `InstallWizardRunSelectedStep` → `handle_run_selected_step`.

### Acceptance Criteria

1. `Enter` on the Flutter SDK step (when idle) dispatches
   `RunWizardStep { kind: FlutterSdk, install: Some(_), .. }` and flips execution to
   Running; a second `Enter` while running is a no-op.
2. `Enter` on the PATH step dispatches `RunWizardStep { kind: PathConfig,
   path_bin_dir: Some(_), .. }` when a Flutter bin dir is known, else sets a
   helpful `status_message` and dispatches nothing.
3. `WizardStepLog`/`WizardDownloadProgress` update execution state; the log tail
   stays bounded (task 07).
4. On `WizardStepCompleted { kind: FlutterSdk, sdk_path: Some(p), .. }`,
   `state.settings.flutter.sdk_path == Some(p)`, settings are persisted, and a
   preflight re-run is triggered (verify via the returned action/message chain).
5. On `WizardStepFailed`, execution status is `Failed` with the reason, and the
   step can be retried with `Enter`.
6. Handlers are unit-tested with synthetic messages; `handler::update` stays
   exhaustive. No clippy warnings.

### Testing

```rust
#[test]
fn test_run_selected_flutter_step_dispatches_install_action() { ... }

#[test]
fn test_run_selected_noop_while_running() { ... }

#[test]
fn test_completed_flutter_persists_sdk_path_and_reruns_preflight() {
    // assert state.settings.flutter.sdk_path set + PersistSettings action + rerun chain
}

#[test]
fn test_pathconfig_without_known_sdk_sets_status_message() { ... }

#[test]
fn test_step_failed_records_reason_and_allows_retry() { ... }
```

### Notes

- Reuse `handle_rerun_preflight` rather than duplicating the preflight dispatch.
- Persisting settings uses the existing `PersistSettings` action + `save_settings`
  — no new config infra (confirmed by research).
- Keep handlers pure: no I/O, no `tokio::spawn`. All effects flow through
  `UpdateAction` / follow-up `Message`.
- If you add an `installed_sdk_path` field, prefer putting it in task 07's state
  (coordinate) so this task only reads/writes it.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a074f4e7ff7292818

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Added `installed_sdk_path: Option<PathBuf>` field to `InstallWizardState`; updated `Debug` impl |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Added 6 new step lifecycle handlers (`handle_run_selected_step`, `handle_step_started`, `handle_step_log`, `handle_step_progress`, `handle_step_completed`, `handle_step_failed`); added `map_install_method` helper; updated `handle_preflight_completed` to fire `ScanInstalledSdks`; added 13 new unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Replaced 5 `TODO(phase2-task-09)` stub arms with real handler calls |

### Notable Decisions/Tradeoffs

1. **`begin_step` on dispatch vs `WizardStepStarted`**: Called `begin_step` on dispatch (in `handle_run_selected_step`) so the UI flips to `Running` immediately without waiting for the executor round-trip. `handle_step_started` remains idempotent and calls `begin_step` again when the executor message arrives — this is safe since `begin_step` resets all fields.

2. **Message chain for FlutterSdk completion**: Used `UpdateResult::message_and_action` with `PersistSettings` as the action and `InstallWizardRerunPreflight` as the follow-up message. The chain is: `WizardStepCompleted(FlutterSdk)` → `PersistSettings` + `InstallWizardRerunPreflight` → `RunToolchainPreflight` → `ToolchainPreflightCompleted` → `ScanInstalledSdks`. No new `UpdateAction::Batch` introduced.

3. **`handle_preflight_completed` now always fires `ScanInstalledSdks`**: This is correct behavior — after any preflight completes (initial open, re-run after install, or manual `r`), the FVM cache should be refreshed. The action is cheap (reads a directory listing).

4. **`installed_sdk_path` stashing**: Added field to `InstallWizardState` so the PathConfig step can find the Flutter `bin/` dir after a FlutterSdk install without requiring the user to re-configure settings manually.

5. **`map_install_method` helper**: Bridges the config-layer `InstallMethod` enum (in `fdemon-app/config/types.rs`) to the daemon-layer equivalent (in `fdemon-daemon/toolchain/types.rs`) so the config layer does not gain a runtime dependency on the daemon.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all crates, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (0 warnings)
- `cargo test -p fdemon-app -- handler::install_wizard::actions` - 18/18 tests pass including all 5 task-specified test functions

### Risks/Limitations

1. **ScanInstalledSdks on every preflight**: The preflight-completed handler now always fires `ScanInstalledSdks`. This is a small change in behavior (previously it was a no-op), but is harmless — the action does a local directory scan and does not make network calls.
</content>
