## Task: `RunWizardStep` executor (spawn install / path-config tasks)

**Objective**: Implement the `UpdateAction::RunWizardStep` arm in
`handle_action`: spawn the async task that runs the Flutter install or PATH-config
work in `fdemon_daemon::toolchain`, bridging daemon `InstallEvent`s / outcomes into
the `WizardStep*` messages over the existing `msg_tx` channel.

**Depends on**: 03, 04, 05

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/mod.rs`: add the `RunWizardStep` match arm
  (alongside `RunToolchainPreflight`, `SwitchFlutterVersion`).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs` (`install_flutter`,
  `resolve_install_dir`, `FlutterInstallTarget`, `InstallEvent`, `InstallMethod`).
- `crates/fdemon-daemon/src/toolchain/path_config.rs` (`add_to_path`,
  `PathConfigOutcome`).
- `crates/fdemon-app/src/handler/mod.rs` (`RunWizardStep`, `FlutterStepParams`).
- `crates/fdemon-app/src/message.rs` (`WizardStep*` variants).
- Existing `RunToolchainPreflight` arm as the spawn template.

### Details

Pattern mirrors `RunToolchainPreflight` / `SwitchFlutterVersion`: clone `msg_tx`,
`tokio::spawn`, send a started message, run the daemon work, forward
events/progress, then send a completed or failed message.

```rust
UpdateAction::RunWizardStep { kind, install, path_bin_dir } => {
    let msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        let _ = msg_tx.send(Message::WizardStepStarted { kind }).await;

        match kind {
            WizardStepKind::FlutterSdk => {
                let Some(params) = install else { /* send Failed: missing params */ return; };
                let target = FlutterInstallTarget {
                    method: params.method,
                    channel: params.channel,
                    install_root: resolve_install_dir(params.install_root.as_deref())?, // map err → Failed
                    version_dir_name: params.channel.clone(), // or resolved version
                };
                let tx = msg_tx.clone();
                let result = install_flutter(&target, move |ev| {
                    match ev {
                        InstallEvent::Log(line) =>
                            { let _ = tx.try_send(Message::WizardStepLog { kind, line }); }
                        InstallEvent::Download(p) =>
                            { let _ = tx.try_send(Message::WizardDownloadProgress { kind, received: p.received, total: p.total }); }
                        InstallEvent::Phase(label) =>
                            { let _ = tx.try_send(Message::WizardStepLog { kind, line: format!("[{label}]") }); }
                    }
                }).await;
                match result {
                    Ok(outcome) => { let _ = msg_tx.send(Message::WizardStepCompleted {
                        kind, summary: format!("Installed Flutter {} at {}", outcome.version, outcome.sdk_path.display()),
                        sdk_path: Some(outcome.sdk_path) }).await; }
                    Err(e) => { let _ = msg_tx.send(Message::WizardStepFailed { kind, reason: format!("{e}") }).await; }
                }
            }
            WizardStepKind::PathConfig => {
                let Some(bin) = path_bin_dir else { /* Failed */ return; };
                let shell = HostShell::detect();
                let platform = HostPlatform::detect();
                let result = tokio::task::spawn_blocking(move || add_to_path(shell, platform, &bin)).await;
                // map PathConfigOutcome → WizardStepCompleted (summary mentions rc file + "restart your terminal");
                // map errors → WizardStepFailed
            }
            _ => { let _ = msg_tx.send(Message::WizardStepFailed {
                kind, reason: "This step is not executable in this version".into() }).await; }
        }
    });
}
```

Implementation notes:
- The `on_event` callback runs inside `install_flutter`'s task; use `try_send`
  there (the callback is sync `FnMut`, not async). Use awaited `send` for the
  terminal Started/Completed/Failed messages.
- Run `add_to_path` (sync, does file I/O) under `spawn_blocking`.
- `install_flutter`'s sha256 verify + extraction already use `spawn_blocking`
  internally per task 03 — do not double-wrap.
- Import `WizardStepKind`, `HostShell`, `HostPlatform`, install/path types from
  `fdemon_daemon::toolchain` and `crate::install_wizard`.
- Update the runner's "non-runner variant" warn list (`runner.rs` ~line 487) is
  **not** needed here — `RunWizardStep` is handled by `handle_action` (engine
  side), same as `RunToolchainPreflight`. Confirm it routes through the engine, not
  the runner action queue. (If `handle_action` is the executor for
  `RunToolchainPreflight`, `RunWizardStep` follows the same path.)

### Acceptance Criteria

1. `RunWizardStep { kind: FlutterSdk, .. }` spawns a task that emits
   `WizardStepStarted`, forwards `Log`/`DownloadProgress`, and ends with exactly
   one `WizardStepCompleted` (with `sdk_path: Some(..)`) or `WizardStepFailed`.
2. `RunWizardStep { kind: PathConfig, .. }` runs `add_to_path` off the async
   executor and reports a completion summary naming the rc file, or a failure.
3. Non-executable kinds produce a clear `WizardStepFailed`.
4. Missing required params (no `install` for FlutterSdk, no `path_bin_dir` for
   PathConfig) produce `WizardStepFailed`, never a panic.
5. `cargo check -p fdemon-app` passes; dispatch is unit-tested following the
   existing `PersistSettings`/`RunToolchainPreflight` dispatch-test style.

### Testing

Follow the existing `handle_action` dispatch tests (see the `PersistSettings`
tests in `actions/mod.rs`). Since the spawned task does real I/O, tests should
assert the *dispatch* (that calling `handle_action` with `RunWizardStep` spawns a
task and the channel receives `WizardStepStarted`), plus the param-missing →
`WizardStepFailed` guard paths which need no I/O.

```rust
#[tokio::test]
async fn test_run_wizard_step_pathconfig_missing_bindir_fails() { ... }

#[tokio::test]
async fn test_run_wizard_step_emits_started() { ... }
```

### Notes

- Keep the bridge mechanical; all install logic lives in the daemon (task 03/04).
- `version_dir_name`: using the channel name (`"stable"`) keeps the install at
  `~/fvm/versions/stable`. If task 03 resolves a concrete version cheaply, prefer
  the version string so the Flutter Version panel lists it by version. Coordinate
  with task 03's outcome.

---

## Completion Summary

**Status:** Not Started
</content>
