## Task: App executor — manifest-fetch body + `version_tag` into the install target

**Objective**: Replace the two stubs in `crates/fdemon-app/src/actions/mod.rs`: implement the
`FetchFlutterReleaseManifest` executor arm (spawn → `fetch_release_manifest` →
`FlutterManifestFetched`/`…FetchFailed`), and thread `params.version_tag` into the
`FlutterInstallTarget` (`version_tag` field + `version_dir_name`).

**Depends on**: Task 01 (daemon field + stub line), Task 03 (action/message variants + stub arm).

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/mod.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction::FetchFlutterReleaseManifest`,
  `FlutterStepParams.version_tag`
- `fdemon_daemon::toolchain` — `fetch_release_manifest`, `HostPlatform`, `FlutterInstallTarget`
- In-file precedent: the `RunToolchainPreflight` arm (~`actions/mod.rs:800-834`) — spawn +
  `msg_tx.send` shape, error-string formatting

### Details

> Locate by symbol; line numbers drift.

#### 1. `FetchFlutterReleaseManifest` arm (replace Task 03's no-op stub)

```rust
UpdateAction::FetchFlutterReleaseManifest => {
    let msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        let platform = fdemon_daemon::toolchain::HostPlatform::detect();
        match fdemon_daemon::toolchain::fetch_release_manifest(platform).await {
            Ok(manifest) => {
                let _ = msg_tx.send(Message::FlutterManifestFetched { manifest }).await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::FlutterManifestFetchFailed { error: format!("{e}") })
                    .await;
            }
        }
    });
}
```

Match the surrounding arms' exact idioms (clone names, `let _ =`, tracing). No cancellation token:
the fetch is short, read-only, and idempotent; a stale result landing after picker close is cached
harmlessly (Task 03 §3 contract).

#### 2. FlutterSdk install arm — thread the tag (~`actions/mod.rs:924-932`)

Replace Task 01's `version_tag: None,` stub:

```rust
let target = FlutterInstallTarget {
    method: params.method,
    channel: params.channel.clone(),
    install_root,
    // Pinned installs land at `~/fvm/versions/<version>`; channel installs keep
    // landing at `~/fvm/versions/<channel>` (legacy behaviour when no pick was made).
    version_dir_name: params
        .version_tag
        .clone()
        .unwrap_or_else(|| params.channel.clone()),
    version_tag: params.version_tag.clone(),
};
```

Everything else in the arm (event callback, `WizardStepCompleted { sdk_path }`, failure paths,
`WizardInstallTaskReady`) is unchanged — `sdk_path` already comes back from the daemon computed off
`version_dir_name`, so the completion chain (settings write, PATH auto-config, rescan) needs nothing.

### Acceptance Criteria

1. Executing `FetchFlutterReleaseManifest` sends `FlutterManifestFetched` on success and
   `FlutterManifestFetchFailed { error }` on failure (exercise with the executor test seam used by
   neighbouring arms, or factor the spawn body into a testable `async fn` like the file's precedent
   if one exists — otherwise cover via a wiremock-backed test only if the file already has that
   harness; do not invent new test infrastructure for this arm).
2. `RunWizardStep` with `version_tag: Some("3.24.0")` builds a target with
   `version_dir_name == "3.24.0"` and `version_tag == Some("3.24.0")`; with `None` both fall back to
   the channel (assert via a unit test on the target-construction logic — extract a small pure
   helper `fn flutter_install_target(params, install_root) -> FlutterInstallTarget` if the literal
   is not directly testable).
3. No `version_tag: None` stub or no-op stub arm remains in the file.
4. `cargo test -p fdemon-app --lib` green; `cargo test --workspace --lib` green; fmt + clippy clean.

### Testing

```bash
cargo test -p fdemon-app --lib actions
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

### Notes

- Keep the helper-extraction footprint minimal — this file is the executor; logic belongs upstream.
- `HostPlatform::detect()` at fetch time (not captured at action-creation) matches how the install
  arm already detects the platform.
