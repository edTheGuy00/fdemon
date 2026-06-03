## Task: Startup Auto-Launch Hook, Import Repoint & Widget Cleanup (fdemon-tui)

**Objective**: Close the auto-launch + missing-SDK dead-end by opening the wizard on that path,
repoint the wizard widgets at the new `fdemon-app` re-exports so the crate's direct `fdemon-daemon`
runtime dependency can be dropped, and clean up two widget nitpicks. Addresses review findings
**M2**, **m4 (tui side)**, **n14**, **n15**.

**Depends on**: 04-app-handler-fixes-and-reexports (needs the `fdemon_app::install_wizard`
re-exports before the imports can be repointed and the runtime dep removed)

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/runner.rs` — open the wizard on the auto-launch + no-SDK path (M2).
- `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs` — repoint import (m4).
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — repoint import (m4), remove unused
  param (n14), simplify clamp math (n15).
- `crates/fdemon-tui/Cargo.toml` — move `fdemon-daemon` from a runtime dependency to a dev-dependency
  (m4).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/startup.rs` — `StartupAction` and how `dispatch_startup_action` chooses
  `AutoStart` vs `Ready`.
- `crates/fdemon-app/src/handler/update.rs` — `StartAutoLaunch` no-SDK behavior (context for M2).
- Task 04 re-exports (`fdemon_app::install_wizard::{ComponentCheck, ComponentStatus, DoctorLine, DoctorMarker}`).

### Details

**M2 — open the wizard on auto-launch + missing SDK** (`runner.rs:285-310`,
`dispatch_startup_action`):

Today the wizard-on-missing-SDK hook lives only in the `StartupAction::Ready` branch. The
`AutoStart` branch sends `StartAutoLaunch`, whose handler no-ops (logs a warning) when
`flutter_executable()` is `None` — a silent dead-end for users with `auto_launch` configured.

Fix at the runner chokepoint so both startup paths behave consistently: when there is no resolvable
Flutter executable, open the wizard **regardless** of the auto-start decision. For example, check
`flutter_executable()` before dispatching `AutoStart`:

```rust
fn dispatch_startup_action(engine: &mut Engine, action: startup::StartupAction) {
    // No resolvable SDK: open the diagnostics wizard from either startup path
    // instead of a dead-end (Ready) or a silent no-op (AutoStart).
    if engine.state.flutter_executable().is_none() {
        let _ = engine.msg_sender().try_send(Message::ShowInstallWizard);
        return;
    }
    match action {
        startup::StartupAction::AutoStart { configs } => { /* unchanged */ }
        startup::StartupAction::Ready => {
            // flutter_executable() is Some here; the else-branch wizard fallback
            // is now handled by the early return above.
            if let Some(flutter) = engine.state.flutter_executable() {
                spawn::spawn_device_discovery(engine.msg_sender(), flutter);
            }
        }
    }
}
```

Keep the existing `DeviceDiscoveryFailed` machinery for the present-but-broken / discovery-error
paths. Preserve the documented ordering note (`try_send` for follow-up-message safety).

**m4 (tui side) — repoint imports + drop runtime dep:**

- `doctor_view.rs`: change `use fdemon_daemon::toolchain::{DoctorLine, DoctorMarker};` →
  `use fdemon_app::install_wizard::{DoctorLine, DoctorMarker};`.
- `step_detail.rs`: change `use fdemon_daemon::toolchain::{ComponentCheck, ComponentStatus};` →
  `use fdemon_app::install_wizard::{ComponentCheck, ComponentStatus};`.
- `Cargo.toml`: move `fdemon-daemon` out of `[dependencies]` into `[dev-dependencies]` (it is still
  needed for `#[cfg(test)]` helpers in `target_selector.rs`, `device_groups.rs`,
  `flutter_version_panel/`). Confirm no remaining non-test `use fdemon_daemon::` in the crate after
  the repoint (`grep`).

**n14 — remove unused param** (`step_detail.rs`, `compute_corrected_scroll`):

Drop the `_selected_index: usize` parameter and update both call sites. Re-add only when a future
feature needs it.

**n15 — simplify redundant Doctor-step clamp** (`step_detail.rs`, Doctor branch):

Remove the dead `unwrap_or(1)` (unreachable in the `None` branch) and the redundant
`start.min(lines.len())` (already guaranteed `< len` by the clamp), **or** add a one-line comment
explaining the defensive intent. Do not change the displayed behavior.

### Acceptance Criteria

1. Launching with `auto_launch` configured **and** `flutter_executable() == None` results in
   `UiMode::InstallWizard` (wizard opens, preflight runs) — not a silent Normal screen.
2. Launching with a resolvable SDK is unchanged (auto-launch proceeds; `Ready` discovers devices;
   wizard does not open).
3. `fdemon-tui` has **no** `fdemon-daemon` entry under `[dependencies]` (only `[dev-dependencies]`);
   `grep -rn "use fdemon_daemon" crates/fdemon-tui/src` shows matches only inside `#[cfg(test)]`.
4. The wizard widgets compile against `fdemon_app::install_wizard::*` re-exports.
5. `compute_corrected_scroll` no longer takes an unused parameter; the Doctor-step clamp is
   simplified or commented. Widget render tests pass.
6. Full quality gate green (fmt/check/test/clippy `-D warnings`).

### Testing

- Add/extend a runner or handler test covering the startup-hook branch selection: with
  `flutter_executable() == None`, `dispatch_startup_action` for **both** `AutoStart` and `Ready`
  results in `Message::ShowInstallWizard` (assert via the handler path → `UiMode::InstallWizard` if
  `dispatch_startup_action` is not directly testable, and note the manual check).
- Existing `render/tests.rs` install-wizard render tests must still pass after the import repoint.
- Manual: run `cargo run` with no Flutter on PATH **and** an `auto_launch` config present; confirm
  the wizard opens.

### Notes

- The import repoint depends on task 04's re-export existing — do not start the `Cargo.toml` change
  until that has landed, or the crate will fail to compile.
- Do not reintroduce `DeviceDiscoveryFailed` removal anywhere else; only the startup branch changes.
- If you judge the re-export pattern worth a one-line note in `ARCHITECTURE.md`, flag it in the
  completion summary for a `doc_maintainer` follow-up rather than editing the managed doc.
