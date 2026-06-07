# Task 01 — Auto-configure PATH after Flutter & Android installs

**Agent:** implementor
**Severity:** 🟠 MAJOR (primary defect)
**Depends On:** —
**Crate(s):** `fdemon-app`

## Problem

Installing Flutter (and Android tools) via the wizard never writes the SDK `bin`
dir to the shell rc file. `handle_step_completed` for `FlutterSdk`
(`crates/fdemon-app/src/handler/install_wizard/actions.rs:446-464`) chains only
`PersistSettings` + `InstallWizardRerunPreflight`; `AndroidTools`
(`actions.rs:467-487`) only persists `android_sdk_root` + re-runs preflight. The
**PathConfig** step (the only place `add_to_path` / `add_android_env` are called,
via `crates/fdemon-app/src/actions/mod.rs:1144-1175`) must be run **manually**.
Result: after a managed install, `flutter` is not on PATH for new shells, and
`ANDROID_HOME` is not written. See `../BUG.md` for the full root-cause analysis.

## Goal

After a **successful FlutterSdk install** and after a **successful AndroidTools
install**, automatically perform the PathConfig write (reusing the existing
executor) so the rc file is configured with **no manual step**. The write is
idempotent and `apply_fence` (`path_config.rs:453-487`) replaces any stale fdemon
fence block, so a pre-existing `/tmp/.tmp…/bin` entry self-corrects.

## Acceptance Criteria

- [ ] After `WizardStepCompleted { kind: FlutterSdk, sdk_path: Some(..) }`, the
      wizard auto-dispatches the PathConfig run for the just-installed SDK
      (`installed_sdk_path` / `settings.flutter.sdk_path` → `<sdk>/bin`) without the
      user selecting the PathConfig step.
- [ ] After `WizardStepCompleted { kind: AndroidTools, .. }` with a resolved
      Android root, the wizard auto-dispatches the PathConfig run so `ANDROID_HOME`
      + Android `PATH` are written.
- [ ] The existing `PersistSettings` (writes `[flutter] sdk_path` /
      `android_sdk_root`) and the preflight re-check still happen. Preflight must
      run even if the auto PathConfig step fails (e.g. `HostShell::Unknown`) — the
      install itself is still reported successful.
- [ ] No infinite loop: PathConfig completion must not re-trigger FlutterSdk /
      AndroidTools, and the auto-config must not re-fire on its own preflight
      result.
- [ ] The auto-started PathConfig step respects the Phase-7 `run_seq` /
      `install_task` seq-guard — it cannot clobber a live install task and a stale
      completion cannot mis-drive it.
- [ ] A pre-existing stale Flutter fence block is **replaced** (not duplicated)
      after a fresh install (covered by an `apply_fence`-level assertion or an
      integration-style handler test).

## Recommended Approach

Two viable wirings — pick whichever keeps the chain cleanest; document the choice:

1. **One-shot flag on the preflight chain (preferred):** add
   `pending_path_autoconfig: Option<WizardStepKind>` to `InstallWizardState`. Set it
   in `handle_step_completed` for FlutterSdk/AndroidTools (keep the existing
   `PersistSettings` + `InstallWizardRerunPreflight` return). In
   `handle_preflight_completed`, if the flag is set, clear it and dispatch the
   PathConfig run (reuse `handle_run_selected_step`'s `PathConfig` arm). This keeps
   the existing `SdkResolved`/`ScanInstalledSdks` chain intact and runs the PATH
   write after the SDK is resolved.
2. **Dedicated follow-up message:** add
   `Message::InstallWizardAutoConfigurePath { kind }`, emit it from
   `handle_step_completed`, and handle it by producing
   `UpdateAction::RunWizardStep { kind: PathConfig, .. }`. Wire it in
   `handler/update.rs`.

For **scope**: the standard PathConfig executor writes Flutter PATH **and** Android
env when both are resolvable, all idempotent — running the full PathConfig for
either origin is acceptable. Optionally scope the FlutterSdk-origin auto-config to
Flutter-only (`android_sdk_root: None`) so each step's side effects match what it
installed; if you do, that touches `actions/mod.rs` — keep it minimal.

## Files Modified (Write)

- `crates/fdemon-app/src/handler/install_wizard/actions.rs`
- `crates/fdemon-app/src/message.rs`
- `crates/fdemon-app/src/handler/update.rs`
- `crates/fdemon-app/src/install_wizard/state.rs` (if the one-shot flag is added)

## Files Read (Dependencies)

- `crates/fdemon-app/src/actions/mod.rs` (PathConfig executor; only edit if scoping
  `android_sdk_root` by origin)
- `crates/fdemon-daemon/src/toolchain/path_config.rs` (apply_fence semantics — read only)

## Testing

- Handler unit tests: FlutterSdk completion now results in (eventually) a
  `RunWizardStep { kind: PathConfig, path_bin_dir: Some(<sdk>/bin) }` action;
  AndroidTools completion likewise yields the Android env write.
- Test that PathConfig completion does **not** re-trigger an installer step (no
  loop).
- Test the seq-guard: a stale `WizardStepStarted`/completion for the auto step is a
  no-op (build on Phase-7 task-01 patterns).
- Verify `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings` all
  green.

## Notes

- Do **not** perform blocking rc-file I/O on the `update()` path — it must remain in
  the `spawn_blocking` executor (`actions/mod.rs`). This task only changes which
  messages/actions are emitted.
- Headless mode: confirm the auto-config either runs correctly or is intentionally
  inert; do not panic or hang.
