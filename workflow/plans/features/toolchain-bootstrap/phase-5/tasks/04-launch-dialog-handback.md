## Task: Launch-dialog handback — re-trigger device discovery once Flutter is live

**Objective**: After a managed Flutter install succeeds and `flutter_executable()`
resolves, automatically close the wizard and re-trigger device discovery so the
new-session dialog is populated without restarting fdemon — both on auto-completion
and on manual close.

**Depends on**: 03-abort-retry-ux-app (shares `install_wizard/state.rs` and
`handler/install_wizard/actions.rs`)

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`: add a `flutter_now_live` /
  handback predicate helper; add an exhaustive component-kind routing test (folded
  from audit).
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`: in
  `handle_preflight_completed`, after `apply_report`, if Flutter is now live and the
  wizard was opened for a missing SDK → close the wizard + dispatch device discovery.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs`: in `handle_hide` /
  `handle_escape`, if `flutter_executable()` is `Some` after closing, spawn device
  discovery and transition toward the new-session/startup flow rather than bare
  `UiMode::Normal`.
- `crates/fdemon-app/src/state.rs`: confirm/trigger `resolved_sdk` re-resolution so
  `flutter_executable()` is live after the managed install.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/runner.rs`: the startup hook (`dispatch_startup_action`,
  `:296-298`) that opens the wizard on a missing SDK — mirror its discovery spawn.
- `crates/fdemon-app/src/handler/mod.rs`: the `UpdateAction` used to discover
  devices (reuse the existing discovery action, do not invent a new one).

### Details

**Handback trigger (resolved scope): "Flutter SDK live."** Hand back as soon as
`flutter_executable()` returns `Some` — even if Android tools / prerequisites remain
missing. This matches the PLAN goal "fdemon can launch sessions." Do **not** gate on
all-5-steps-Ok.

**SDK re-resolution is the critical precondition.** `WizardStepCompleted` writes
`settings.flutter.sdk_path` (`actions.rs:~307`), but `flutter_executable()` reads
`resolved_sdk` on `AppState`, which is populated earlier and not refreshed. **Audit
this first.** If `apply_report` + the `sdk_path` write does not refresh
`resolved_sdk`, add an explicit re-resolution (reuse the locator /
`Engine`-resolution path) before evaluating the handback predicate. Without this the
device list will be empty despite a successful install.

```rust
// handle_preflight_completed, after apply_report(...)
if state.flutter_executable().is_some() && !state.install_wizard_state.handback_done {
    state.hide_install_wizard();
    state.install_wizard_state.handback_done = true;     // prevent re-fire
    // transition toward the launch flow + discover devices
    return UpdateResult::action(UpdateAction::DiscoverDevices { flutter });
}
```

**Manual close.** `handle_hide` / `handle_escape` currently return
`UpdateResult::none()`. When a live SDK exists at close time, return a discovery
action and route to the new-session/startup mode so a user who Escs after a
successful install still lands in a populated launch dialog.

**Double-discovery guard.** Auto-close (in `handle_preflight_completed`) and a manual
`Esc` could both spawn discovery. Guard with a one-shot flag (`handback_done`) and/or
by checking that discovery is not already in flight (e.g. `target_selector.loading`).

**Folded test gap (from audit):** add one exhaustive test asserting all nine
`ComponentKind` variants (`Prerequisites, Git, AndroidCmdlineTools,
AndroidPlatformTools, AndroidPlatform, AndroidBuildTools, AndroidLicenses, Jdk,
FlutterSdk`) route to the correct `WizardStep` bucket in `build_steps`.

### Acceptance Criteria

1. When a preflight re-run after a successful Flutter install shows Flutter live, the
   wizard auto-closes and a device-discovery action is dispatched exactly once.
2. `resolved_sdk` is confirmed/refreshed so `flutter_executable()` is `Some`
   immediately after the managed install (no fdemon restart needed) — verified by an
   audit note in the task summary and a test.
3. Manual close (`Esc`/`HideInstallWizard`) with a live SDK also spawns discovery and
   routes to the new-session/startup flow, not bare `UiMode::Normal`.
4. Discovery is never spawned twice for one install (guard verified).
5. Partial toolchains (Flutter live, Android missing) still hand back — handback is
   gated on Flutter only.
6. Exhaustive 9-`ComponentKind` routing test passes.

### Testing

```rust
#[test]
fn preflight_completed_with_live_flutter_autocloses_and_discovers() {
    // arrange: wizard visible, report flips Flutter to Ok, flutter_executable() Some
    // assert: ui_mode left InstallWizard, DiscoverDevices action returned, handback_done set
}
#[test]
fn manual_close_with_live_sdk_spawns_discovery() { /* handle_escape -> DiscoverDevices */ }
#[test]
fn handback_does_not_fire_twice() { /* second preflight completion -> no second discovery */ }
#[test]
fn all_nine_component_kinds_route_to_correct_step() { /* build_steps routing exhaustive */ }
```

### Notes

- Reuse the **existing** device-discovery `UpdateAction` and the startup hook's spawn
  shape — do not add a parallel discovery path.
- If the SDK re-resolution turns out to be non-trivial, surface it: the handback is
  worthless if `flutter_executable()` is still `None` at the trigger point.
- `DeviceDiscoveryFailed` is unrelated here (it only drives the target-selector error
  text) — do not couple to it.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
