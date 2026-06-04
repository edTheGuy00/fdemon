## Task: RunWizardStep executor — Android Tools install + PathConfig Android env

**Objective**: Replace the `AndroidTools` "not executable" stub in the
`RunWizardStep` executor with a real async install (`install_android_tools`,
streamed via `InstallEvent`), and extend the `PathConfig` arm to also write
`ANDROID_HOME` via `add_android_env` when an Android SDK root is known.

**Depends on**: 01, 02, 03, 04

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/mod.rs`: the `RunWizardStep` match arm — add the
  `WizardStepKind::AndroidTools` branch; extend the `WizardStepKind::PathConfig`
  branch to write Android env; consume the new `android` / `android_sdk_root`
  action fields.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/android_install.rs`: `install_android_tools`,
  `AndroidInstallTarget`, `AndroidInstallOutcome`.
- `crates/fdemon-daemon/src/toolchain/jdk.rs`: `configure_flutter_jdk_dir`,
  `resolve_jdk_home` (optional post-install jdk-dir wiring).
- `crates/fdemon-daemon/src/toolchain/path_config.rs`: `add_android_env`.
- `crates/fdemon-daemon/src/toolchain/types.rs`: `cmdline_tools_url` /
  `DEFAULT_CMDLINE_TOOLS_BUILD`, `HostPlatform`.
- `crates/fdemon-app/src/handler/mod.rs`: `AndroidStepParams`, new `RunWizardStep`
  fields.
- The existing `FlutterSdk` / `PathConfig` arms as the template (`actions/mod.rs`).

### Details

**AndroidTools arm** — mirror the FlutterSdk arm's streaming bridge:

```rust
WizardStepKind::AndroidTools => {
    let Some(params) = android else {
        // send WizardStepFailed { kind, reason: "Missing Android install parameters" }; return;
    };
    let target = AndroidInstallTarget {
        sdk_root: resolve_android_sdk_root(params.sdk_root),  // default per HostPlatform if None
        api_level: params.api_level,
        cmdline_tools_build: params.cmdline_tools_build
            .unwrap_or_else(|| DEFAULT_CMDLINE_TOOLS_BUILD.to_string()),
        jdk_path: params.jdk_path,
        platform: HostPlatform::detect(),
    };
    let tx = msg_tx.clone();
    let result = install_android_tools(&target, move |ev| match ev {
        InstallEvent::Log(line)   => { let _ = tx.try_send(Message::WizardStepLog { kind, line }); }
        InstallEvent::Download(p) => { let _ = tx.try_send(Message::WizardDownloadProgress { kind, received: p.received, total: p.total }); }
        InstallEvent::Phase(label)=> { let _ = tx.try_send(Message::WizardStepPhase { kind, label: label.to_string() }); }
    }).await;

    match result {
        Ok(outcome) => {
            // Optional: if a JDK path is configured/resolvable and flutter exists,
            // best-effort configure_flutter_jdk_dir (ignore failure, log it).
            let summary = format!(
                "Installed Android tools at {} ({} packages)",
                outcome.sdk_root.display(), outcome.packages_installed.len()
            );
            // WizardStepCompleted carries sdk_path: pass Some(outcome.sdk_root) so the
            // handler (task 07) can persist [toolchain] android_sdk_root.
            let _ = msg_tx.send(Message::WizardStepCompleted { kind, summary, sdk_path: Some(outcome.sdk_root) }).await;
        }
        Err(e) => { let _ = msg_tx.send(Message::WizardStepFailed { kind, reason: format!("{e}") }).await; }
    }
}
```

> **`sdk_path` reuse:** `WizardStepCompleted.sdk_path` is typed `Option<PathBuf>`.
> Phase 2 used it for the Flutter SDK path; here we reuse it to carry the resolved
> Android SDK root so task 07 can persist it. Task 07 disambiguates by `kind`.

**PathConfig arm** — after writing the Flutter PATH (existing `add_to_path`), also
write the Android env when `android_sdk_root` is present:

```rust
WizardStepKind::PathConfig => {
    let shell = HostShell::detect();
    let platform = HostPlatform::detect();
    // 1) existing flutter bin → add_to_path (unchanged), capture rc_file/outcome
    // 2) if let Some(root) = android_sdk_root { add_android_env(shell, platform, &root) }
    // Combine both outcomes into one summary; "restart your terminal" hint.
    // Run both under spawn_blocking (file I/O).
}
```

Build the combined summary so the user sees both the Flutter PATH rc file and the
Android env rc file (or "already present"). Any single failure → `WizardStepFailed`
with a clear message; a partial success (flutter ok, android failed) should fail
loudly rather than report success.

`resolve_android_sdk_root(Option<PathBuf>)`: when `None`, fall back to the
platform default (`$ANDROID_HOME`/`$ANDROID_SDK_ROOT`, else `~/Android/Sdk` /
`~/Library/Android/sdk` / `%LOCALAPPDATA%\Android\Sdk`). Keep this resolution in the
daemon if convenient (export a helper) or compute it here — prefer a daemon helper
to keep the default logic next to `android_sdk_root()` detection.

### Acceptance Criteria

1. `RunWizardStep { kind: AndroidTools, android: Some(..), .. }` spawns a task that
   emits `WizardStepStarted`, forwards `Log`/`Download`/`Phase`, and ends with
   exactly one `WizardStepCompleted` (`sdk_path: Some(sdk_root)`) or
   `WizardStepFailed`.
2. Missing `android` params for AndroidTools → `WizardStepFailed`, never a panic.
3. The `PathConfig` arm writes the Flutter PATH and, when `android_sdk_root` is
   `Some`, also writes `ANDROID_HOME` via `add_android_env`; the summary names both
   rc files and includes the restart hint. A failure in either write → one
   `WizardStepFailed`.
4. All file I/O (`add_to_path`, `add_android_env`) runs under `spawn_blocking`; the
   streaming install callback uses `try_send`, terminal messages use awaited `send`.
5. `cargo check -p fdemon-app`, `clippy`, and the dispatch-level tests pass.

### Testing

Follow the Phase 2 `RunWizardStep` dispatch tests (`actions/mod.rs`):

```rust
#[tokio::test]
async fn test_android_tools_missing_params_fails() { /* android: None → WizardStepFailed */ }

#[tokio::test]
async fn test_android_tools_emits_started() { /* assert WizardStepStarted on the channel */ }

#[tokio::test]
async fn test_pathconfig_without_android_root_still_writes_flutter() { /* android_sdk_root: None */ }
```

Full network install is not unit-tested (mirrors `install_flutter`); assert
dispatch + guard paths only.

### Notes

- Keep the bridge mechanical — all install logic is in the daemon (tasks 02/03).
- The **JDK gate** (refuse to run AndroidTools when JDK is missing) lives in the
  handler that *builds* the action (`handle_run_selected_step`, task 07), not here.
  By the time this executor runs, JDK is assumed present. Still, a runtime
  `sdkmanager` failure due to a missing JDK surfaces as a streamed error +
  `WizardStepFailed` — that's acceptable defense-in-depth.
- `configure_flutter_jdk_dir` is best-effort: only call it if a JDK dir is known and
  `flutter` resolves; never fail the whole step on its error — log via
  `WizardStepLog`.

---

## Completion Summary

**Status:**
**Branch:**

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
