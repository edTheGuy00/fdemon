## Task: InstallWizard UiMode, Messages, Handlers & Async Wiring (fdemon-app)

**Objective**: Wire the read-only wizard into the TEA loop: add `UiMode::InstallWizard`, the
`install_wizard_state` field + `show/hide` helpers, the wizard `Message` variants, the
`UpdateAction::RunToolchainPreflight` action, the `handler/install_wizard/` handlers, key + mouse
routing, and the async preflight task spawn. These changes are introduced **together** so every
exhaustive `match` stays complete and the crate compiles green.

**Depends on**: 02-install-wizard-state-types

**Agent:** implementor

**Estimated Time**: 6-8 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs` — `UiMode::InstallWizard`; `install_wizard_state: InstallWizardState`
  field; `show_install_wizard()` / `hide_install_wizard()` helpers.
- `crates/fdemon-app/src/message.rs` — wizard message variants.
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction::RunToolchainPreflight { .. }`; declare
  `mod install_wizard;` (handler submodule).
- `crates/fdemon-app/src/handler/install_wizard/mod.rs` (NEW) — `mod navigation; mod actions; pub use ...`.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` (NEW) — open/close/escape, pane
  switch, step nav + detail scroll.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (NEW) — preflight-completed ingest,
  re-run.
- `crates/fdemon-app/src/handler/update.rs` — route the new `Message` variants.
- `crates/fdemon-app/src/handler/keys.rs` — `'I'` in Normal mode; `handle_key_install_wizard`.
- `crates/fdemon-app/src/handler/mouse/mod.rs` — dispatch arm for `UiMode::InstallWizard`.
- `crates/fdemon-app/src/handler/mouse/install_wizard.rs` (NEW) — scroll → Up/Down.
- `crates/fdemon-app/src/actions/mod.rs` — handle `RunToolchainPreflight` (tokio spawn).
- `docs/KEYBINDINGS.md` — document the Phase 1 wizard keys.

**Files Read (Dependencies):**
- Task 02 types (`InstallWizardState`, `WizardPane`, `WizardStep`).
- `crates/fdemon-app/src/handler/flutter_version/{navigation.rs,actions.rs}` — handler template.
- `crates/fdemon-app/src/handler/mouse/flutter_version.rs` — mouse scroll template.
- `crates/fdemon-app/src/actions/mod.rs` Flutter-Version block (≈ lines 799–922) — spawn template.
- `crates/fdemon-app/src/handler/keys.rs` `handle_key_flutter_version` (≈ lines 376–395) — key template.

### Details

**`state.rs`** (mirror `show_flutter_version`/`hide_flutter_version` at ≈1698):

```rust
// UiMode enum — add variant
InstallWizard,

// AppState — add field
pub install_wizard_state: InstallWizardState,

pub fn show_install_wizard(&mut self) {
    self.install_wizard_state = InstallWizardState::opening();
    self.ui_mode = UiMode::InstallWizard;
}
pub fn hide_install_wizard(&mut self) {
    self.install_wizard_state.visible = false;
    self.ui_mode = UiMode::Normal;
}
```

**`message.rs`** — add variants (place in a `// ── Install Wizard ──` section):

```rust
ShowInstallWizard,
HideInstallWizard,
InstallWizardEscape,
InstallWizardSwitchPane,
InstallWizardUp,
InstallWizardDown,
InstallWizardRerunPreflight,
ToolchainPreflightCompleted { report: fdemon_daemon::toolchain::ToolchainReport },
```

**`handler/mod.rs`** — `UpdateAction`:

```rust
RunToolchainPreflight {
    project_path: std::path::PathBuf,
    explicit_sdk_path: Option<std::path::PathBuf>,
},
```

**`handler/install_wizard/navigation.rs`:**
- `handle_show(state)` → `state.show_install_wizard()`, returns `UpdateAction::RunToolchainPreflight`
  (read `project_path` and `settings.flutter.sdk_path` off `state`). Mirrors
  `flutter_version::handle_show` returning `ScanInstalledSdks`.
- `handle_hide` / `handle_escape` → `state.hide_install_wizard()`.
- `handle_switch_pane` → toggle `focused_pane`.
- `handle_up` / `handle_down` → when pane is `StepList`, move `selected_index` (clamp to
  `steps.len()`), reset `detail_scroll`; when pane is `Detail`, scroll `detail_scroll` using the
  `last_known_visible_height` render-hint with a fallback (mirror flutter-version `adjust_scroll`).

**`handler/install_wizard/actions.rs`:**
- `handle_preflight_completed(state, report)` → `state.install_wizard_state.apply_report(report)`;
  clear any status message.
- `handle_rerun_preflight(state)` → set `loading = true`, return `RunToolchainPreflight`.

**`handler/update.rs`** — add match arms routing each `Message::InstallWizard*` /
`ShowInstallWizard` / `HideInstallWizard` / `ToolchainPreflightCompleted` to the handlers above
(mirror the flutter-version block at ≈3181–3229).

**`handler/keys.rs`:**
- In `handle_key_normal`, add `InputKey::Char('I') => Some(Message::ShowInstallWizard),` (verified
  free — `'V'` is FlutterVersion, `'I'` capital is unused in Normal mode).
- Add top-level dispatch `UiMode::InstallWizard => handle_key_install_wizard(key, state),`.
- `handle_key_install_wizard`:

| Key | Message |
|-----|---------|
| `Ctrl+C` | `Quit` |
| `Esc` | `InstallWizardEscape` |
| `Tab` | `InstallWizardSwitchPane` |
| `k` / `Up` | `InstallWizardUp` |
| `j` / `Down` | `InstallWizardDown` |
| `r` | `InstallWizardRerunPreflight` |

> `Enter` is intentionally unbound in Phase 1 (step execution is Phase 2). Do not add `c`
> (copy command) yet.

**`handler/mouse/mod.rs` + `mouse/install_wizard.rs`** — mirror `mouse/flutter_version.rs`: scroll
up/down → `InstallWizardUp`/`InstallWizardDown`. No click hit-testing required in Phase 1.

**`actions/mod.rs`** — handle the action (mirror `ScanInstalledSdks` spawn at ≈800):

```rust
UpdateAction::RunToolchainPreflight { project_path, explicit_sdk_path } => {
    let msg_tx = /* engine msg sender clone */;
    tokio::spawn(async move {
        let report = fdemon_daemon::toolchain::run_preflight(
            &project_path, explicit_sdk_path.as_deref(),
        ).await;
        let _ = msg_tx.send(Message::ToolchainPreflightCompleted { report }).await;
    });
}
```

`run_preflight` never returns `Err`, so there is no failure message variant.

**`docs/KEYBINDINGS.md`** — add an "Install Wizard" row group documenting `I`, `Esc`, `Tab`,
`j/k`, `r`, `Ctrl+C`.

### Acceptance Criteria

1. Pressing `I` in Normal mode opens `UiMode::InstallWizard` and triggers a preflight task; the
   state shows `loading = true` until `ToolchainPreflightCompleted` arrives.
2. `ToolchainPreflightCompleted` populates `steps` and clears `loading`.
3. `Esc` closes the wizard back to `UiMode::Normal`.
4. `Tab` toggles the focused pane; `j/k` navigate steps (StepList) or scroll detail (Detail);
   `r` re-runs preflight.
5. All exhaustive matches (`update.rs`, `keys.rs`, `actions/mod.rs`, `mouse/mod.rs`) remain complete;
   the workspace compiles and clippy is clean.

### Testing

```rust
#[cfg(test)]
mod tests {
    // handler unit tests — pure (State, Message) -> State transitions
    #[test] fn test_show_install_wizard_sets_mode_and_loading() { /* show -> visible+loading */ }
    #[test] fn test_escape_returns_to_normal() { /* */ }
    #[test] fn test_preflight_completed_populates_steps_clears_loading() { /* */ }
    #[test] fn test_switch_pane_toggles_focus() { /* */ }
    #[test] fn test_step_nav_clamps_selected_index() { /* */ }
}
```

- Follow the flutter-version handler test style (construct `AppState`, drive `handle_*`, assert
  state). Use a small `ToolchainReport` fixture for the completed-preflight test.

### Notes

- Read `project_path` / explicit `sdk_path` off `AppState` exactly as `Engine::new` /
  `find_flutter_sdk` do (`settings.flutter.sdk_path`).
- The `Cell<usize>` render-hint is only **read** here (handler) and **written** in the TUI task —
  the write site carries the `// EXCEPTION:` annotation, not this file.
- Do not add `[toolchain]` config keys or `RunWizardStep` — Phase 2+.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-aa1791162300811c6

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `UiMode::InstallWizard` variant; `install_wizard_state: InstallWizardState` field; `show_install_wizard()` / `hide_install_wizard()` helpers; `use crate::install_wizard::InstallWizardState` import |
| `crates/fdemon-app/src/message.rs` | Added 8 wizard message variants: `ShowInstallWizard`, `HideInstallWizard`, `InstallWizardEscape`, `InstallWizardSwitchPane`, `InstallWizardUp`, `InstallWizardDown`, `InstallWizardRerunPreflight`, `ToolchainPreflightCompleted` |
| `crates/fdemon-app/src/handler/mod.rs` | Added `pub(crate) mod install_wizard;`; `UpdateAction::RunToolchainPreflight { project_path, explicit_sdk_path }` variant |
| `crates/fdemon-app/src/handler/install_wizard/mod.rs` | NEW — re-exports from `actions` and `navigation` submodules |
| `crates/fdemon-app/src/handler/install_wizard/navigation.rs` | NEW — `handle_show`, `handle_hide`, `handle_escape`, `handle_switch_pane`, `handle_up`, `handle_down` with 14 unit tests |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | NEW — `handle_preflight_completed`, `handle_rerun_preflight` with 5 unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Added `install_wizard` to imports; 8 new match arms routing wizard messages to handlers |
| `crates/fdemon-app/src/handler/keys.rs` | Added `UiMode::InstallWizard => handle_key_install_wizard(key, state)` dispatch arm; `InputKey::Char('I') => ShowInstallWizard` in normal mode; new `handle_key_install_wizard` function |
| `crates/fdemon-app/src/handler/mouse/install_wizard.rs` | NEW — scroll routing: Up→`InstallWizardUp`, Down→`InstallWizardDown`, with 4 unit tests |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Added `mod install_wizard;`; `UiMode::InstallWizard` press arm (no-op); `UiMode::InstallWizard` scroll arm routed to `install_wizard::handle_scroll` |
| `crates/fdemon-app/src/actions/mod.rs` | Added `UpdateAction::RunToolchainPreflight` handler that spawns `fdemon_daemon::toolchain::run_preflight` and sends `ToolchainPreflightCompleted` |
| `crates/fdemon-tui/src/render/mod.rs` | Added `UiMode::InstallWizard` stub arm (empty, task 04 fills in widget rendering) |
| `crates/fdemon-tui/src/runner.rs` | Added `UpdateAction::RunToolchainPreflight { .. }` to non-runner variants list |
| `docs/KEYBINDINGS.md` | Added TOC entries for Install Wizard; `I` key in Normal Mode Flutter SDK section; full "Install Wizard Mode" section |

### Notable Decisions/Tradeoffs

1. **Minimal fdemon-tui stubs**: The task said not to touch `fdemon-tui`, but the workspace cannot compile without exhaustive match arms. Added the minimal stubs (`UiMode::InstallWizard => {}` in render and `RunToolchainPreflight` in runner's non-runner list) required to make the workspace compile. Task 04 will replace the render stub with the actual widget.

2. **Detail pane scroll design**: When `WizardPane::Detail` is focused, `handle_up`/`handle_down` scroll `detail_scroll` directly. The render-hint `last_known_visible_height` is read but only used conceptually — the actual content-length clamping is delegated to the TUI renderer (task 04), consistent with the `Cell<usize>` pattern.

3. **Flaky pre-existing test**: `toolchain::checks::tests::test_android_sdk_root_from_env_android_home` fails when tests run in parallel (env var contamination from other tests). Passes when run with `--test-threads=1` or in isolation. This is a pre-existing issue not introduced by this task.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace -- --test-threads=1` - Passed (all tests pass)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Task 04 dependency**: The `UiMode::InstallWizard` stub in `render/mod.rs` renders nothing. Users opening the wizard (pressing `I`) will see a blank screen until task 04 ships the widget. The state transitions (loading, preflight completion, navigation) all work correctly.
2. **Pre-existing flaky test**: The `test_android_sdk_root_from_env_android_home` test fails under parallel execution — pre-existing issue, not introduced by this task.
