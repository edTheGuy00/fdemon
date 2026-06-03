## Task: Render Dispatch, Modal Gating & Startup Hook (fdemon-tui)

**Objective**: Connect the wizard to the renderer and the startup flow: add the
`UiMode::InstallWizard` render branch, register the mode in the modal-precedence (mouse
suppression) list, and change the startup hook so that when no Flutter SDK resolves, fdemon opens
the wizard (running preflight) instead of only emitting `DeviceDiscoveryFailed`.

**Depends on**: 03-install-wizard-app-wiring (UiMode, messages), 04-install-wizard-tui-widget (widget)

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs` — `UiMode::InstallWizard` render arm; add `InstallWizard`
  to `is_modal_ui_mode`.
- `crates/fdemon-tui/src/runner.rs` — startup hook: open the wizard when `flutter_executable()`
  is `None`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` *(optional)* — add a
  "Press `I` to set up Flutter" hint near the SDK-not-found error.

**Files Read (Dependencies):**
- Task 03 (`Message::ShowInstallWizard`, `UiMode::InstallWizard`).
- Task 04 (`widgets::InstallWizardPanel`).
- `crates/fdemon-tui/src/render/mod.rs` — `view()` dispatch + `is_modal_ui_mode` (≈115–151, 399).
- `crates/fdemon-tui/src/runner.rs` — `dispatch_startup_action` (≈285–311).

### Details

**`render/mod.rs` — render arm** (mirror the `FlutterVersion` arm at ≈399):

```rust
UiMode::InstallWizard => {
    let panel = widgets::InstallWizardPanel::new(&state.install_wizard_state);
    frame.render_widget(panel, area);
}
```

**`render/mod.rs` — modal gating** (≈115): add the variant to `is_modal_ui_mode`:

```rust
matches!(
    mode,
    UiMode::Startup | UiMode::NewSessionDialog | UiMode::ConfirmDialog
        | UiMode::Settings | UiMode::FlutterVersion | UiMode::EmulatorSelector
        | UiMode::InstallWizard,   // <-- added
)
```

This ensures base-UI click regions are suppressed (`None` passed as `MouseCtx`) while the wizard is
open, consistent with the other full-screen modals.

**`runner.rs` — startup hook** (≈299–308, the `StartupAction::Ready` branch):

```rust
startup::StartupAction::Ready => {
    if let Some(flutter) = engine.state.flutter_executable() {
        spawn::spawn_device_discovery(engine.msg_sender(), flutter);
    } else {
        // Phase 1: open the diagnostics wizard instead of a dead-end error.
        let _ = engine.msg_sender().try_send(Message::ShowInstallWizard);
    }
}
```

- `Message::ShowInstallWizard` flows through the handler (task 03), which sets
  `UiMode::InstallWizard` and emits `RunToolchainPreflight`.
- Phase 1 scope: only the `flutter_executable().is_none()` case opens the wizard. (Opening on a
  *present-but-broken* toolchain is a later refinement.)

**`target_selector.rs` (optional hint):** near `render_error` (≈339), when the error is the
SDK-not-found message, append a dim line "Press `I` to set up Flutter". Skip if it complicates the
layout — it is a nice-to-have, not a Phase 1 requirement.

### Acceptance Criteria

1. With `UiMode::InstallWizard` active, `view()` renders `InstallWizardPanel` over the full area.
2. `is_modal_ui_mode(&UiMode::InstallWizard)` returns `true`; base-UI widgets receive `None` as
   `MouseCtx` while the wizard is open (no click fall-through).
3. Launching fdemon with no resolvable Flutter SDK opens the wizard at startup (mode becomes
   `InstallWizard`, preflight runs), rather than only showing the red "Flutter SDK not found" text.
4. Launching with a resolvable SDK is unchanged (device discovery proceeds; wizard does not open).
5. Workspace compiles; existing `render/tests.rs` snapshot/transition tests pass or are updated.

### Testing

- Add/adjust a render test in `crates/fdemon-tui/src/render/tests.rs` asserting that
  `UiMode::InstallWizard` renders the panel (snapshot or smoke render without panic).
- Add a unit test (or reuse an existing runner-level harness if present) covering the startup-hook
  branch selection for `Some`/`None` `flutter_executable()`. If `dispatch_startup_action` is not
  directly testable, assert the behavior via the handler path (`ShowInstallWizard` →
  `UiMode::InstallWizard`) and note the manual verification.
- Manual check: run `cargo run` in an environment with no Flutter on PATH and confirm the wizard
  opens with a populated diagnostics screen.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/render/mod.rs` | Replaced stub `UiMode::InstallWizard => {}` with real render arm constructing `InstallWizardPanel::new(&state.install_wizard_state)`; added `UiMode::InstallWizard` to `is_modal_ui_mode()` |
| `crates/fdemon-tui/src/runner.rs` | In `dispatch_startup_action` `StartupAction::Ready` branch, replaced `DeviceDiscoveryFailed` error send with `Message::ShowInstallWizard` when `flutter_executable()` is `None` |
| `crates/fdemon-tui/src/render/tests.rs` | Added two new tests: `install_wizard_mode_renders_panel_without_panic` (smoke render + title check) and `install_wizard_mode_suppresses_base_ui_header_regions` (modal gate invariant) |

### Notable Decisions/Tradeoffs

1. **DeviceDiscoveryFailed removed from SDK-not-found path**: Per task spec, replaced with `ShowInstallWizard`. The `DeviceDiscoveryFailed` machinery remains intact for present-but-broken / discovery-error paths; only the `flutter_executable().is_none()` branch changed.
2. **Tests reuse `count_hot_reload_regions` helper**: This helper already existed in the test file for Phase 5.5 invariant tests — the new modal-gate test follows the same pattern without duplicating helper code.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all existing tests pass; 2 new tests added)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Manual verification**: The startup hook change (SDK-not-found → wizard) cannot be exercised in a unit test without a real PATH manipulation. Behavior is verified via the handler path (`ShowInstallWizard` → `UiMode::InstallWizard`) which is covered by existing task-03 handler tests and the new render tests here.

### Notes

- Keep the startup change minimal — a single `try_send(Message::ShowInstallWizard)`. Do not remove
  the `DeviceDiscoveryFailed` machinery; it remains for the present-but-broken / discovery-error
  paths.
- If a render snapshot test exercises every `UiMode`, ensure a fixture `install_wizard_state` is
  provided so the new arm is covered.
