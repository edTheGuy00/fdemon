## Task: Reuse preflight-resolved SDK to harden the handback (Finding 1)

**Objective**: Make `run_preflight` return the `FlutterSdk` it already resolves while building
the report, and have the `RunToolchainPreflight` executor populate `AppState::resolved_sdk` from
that single result — eliminating the second `find_flutter_sdk` call, its failure hole, and the
TOCTOU window. After this, `report FlutterSdk Ok ⟺ resolved_sdk Some` holds by construction, so a
successful **Bootstrap** install can never silently close to `UiMode::Normal` without dispatching
device discovery.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3–5 hours

> **Atomicity warning.** Changing the `run_preflight` return type is a signature change that
> ripples to every caller (`actions/mod.rs`, `src/doctor.rs`, daemon tests). The workspace will
> not compile until all callers are updated together. Do not split.

### Scope

**Files Modified (Write):**

- `crates/fdemon-daemon/src/toolchain/mod.rs` — change `run_preflight` (def at ~line 89) to
  return the resolved `FlutterSdk` alongside the report. Prefer a named return type for clarity,
  e.g. `pub struct PreflightOutcome { pub report: ToolchainReport, pub flutter_sdk: Option<FlutterSdk> }`,
  or a `(ToolchainReport, Option<FlutterSdk>)` tuple if that matches local convention. Capture the
  `FlutterSdk` from the `find_flutter_sdk` call that already runs to determine the `FlutterSdk`
  component status (today that struct is discarded). `run_preflight` must still **never** return
  `Err` — `flutter_sdk` is simply `None` when no live SDK was found. Update the module/`run_preflight`
  doc comments.
- `crates/fdemon-daemon/src/lib.rs` — re-export the new `PreflightOutcome` type if one is added.
- `crates/fdemon-app/src/actions/mod.rs` — in the `RunToolchainPreflight` executor (~line 800):
  consume the returned `flutter_sdk`; when `Some`, send `Message::SdkResolved { sdk }` from it and
  **remove** the now-redundant second `find_flutter_sdk` / `spawn_blocking` block (~lines 827–856).
  Preserve the existing ordering: `SdkResolved` is still sent **before**
  `ToolchainPreflightCompleted`. Update the comment that documents the discard/second-call pattern.
- `src/doctor.rs` — update the `run_preflight(...)` call site to the new return type (it only needs
  the `report`).
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — add a regression test (see Testing).

**Files Read (Dependencies):**

- `crates/fdemon-app/src/install_wizard/state.rs` — `flutter_now_live()` (report-based predicate).
- `crates/fdemon-app/src/state.rs` — `flutter_executable()` (~line 1643; reads `resolved_sdk`).
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — `handle_preflight_completed`
  (~line 49) and `close_wizard_and_dispatch_discovery` (~line 97); no logic change required there,
  but understand the two gates.

### Details

The handback gates stay as-is (`is_bootstrap() && flutter_now_live() && !handback_done`, then
`is_bootstrap() && !has_running_sessions()` + `flutter_executable().is_some()`). The fix is purely
upstream: by resolving the SDK **once** in `run_preflight` and threading it to the executor, the
`flutter_executable()` gate can no longer be `None` when `flutter_now_live()` is `true`.

- Keep `run_preflight` infallible.
- Do **not** change `flutter_now_live()` or `close_wizard_and_dispatch_discovery` signatures.
- Watch for other `find_flutter_sdk` uses in the executor that should remain (e.g. the
  version-switch path is unrelated — do not touch it).

### Acceptance Criteria

1. `run_preflight` returns both the report and the `Option<FlutterSdk>` it resolved; still never
   returns `Err`.
2. The `RunToolchainPreflight` executor no longer calls `find_flutter_sdk` a second time; it emits
   `SdkResolved` from the value returned by `run_preflight`, before `ToolchainPreflightCompleted`.
3. All `run_preflight` callers (`actions/mod.rs`, `src/doctor.rs`, daemon tests) compile and pass.
4. A regression test proves a Bootstrap wizard hands back (DiscoverDevices + `UiMode::Startup`)
   when the post-install report shows `FlutterSdk: Ok`.
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`,
   `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Use the existing scaffolding in `crates/fdemon-app/src/handler/install_wizard/actions.rs` tests
(`inject_live_sdk`, `make_live_flutter_report`). Add/confirm a handler-level regression test
mirroring the existing handback test (~`actions.rs:2690`):

```rust
#[test]
fn bootstrap_handback_fires_when_report_live_and_sdk_resolved() {
    let mut state = AppState::new();
    state.show_install_wizard(WizardOrigin::Bootstrap);
    inject_live_sdk(&mut state); // resolved_sdk = Some(...)
    let result = handle_preflight_completed(&mut state, make_live_flutter_report());
    assert_eq!(state.ui_mode, UiMode::Startup);
    assert!(state.install_wizard_state.handback_done);
    assert!(result.actions().iter().any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })));
}
```

If `run_preflight`'s resolved-SDK behaviour is unit-testable in the daemon crate (e.g. with a
`tempdir` fake SDK on PATH like the existing `toolchain/mod.rs` tests), add a daemon-level test
asserting that when the `FlutterSdk` component is `Ok`, the returned `flutter_sdk` is `Some`.

### Notes

- This removes a real (if narrow) silent-degradation path **and** the TOCTOU window between the two
  former `find_flutter_sdk` calls. See BUG.md "Finding 1" for the full root-cause trace.
- `run_preflight` returning a struct also makes the "report says Ok but SDK unresolved" state
  unrepresentable for the live case, which is the cleanest possible fix.

---

## Completion Summary

**Status:** Not Started
**Branch:** <fill in>

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
