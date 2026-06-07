## Task: Core origin-gated handback fix (`WizardOrigin`)

**Objective**: Introduce an explicit `WizardOrigin` enum, thread it from both wizard entry points
through `Message::ShowInstallWizard`, and gate the post-install handback so only a `Bootstrap`
origin auto-advances to device discovery. A `UserInvoked` open (the `I` key) becomes a read-only
informational view that returns to `UiMode::Normal` on close.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3–5 hours

> **Atomicity warning.** This is a single compile unit. The message-variant and signature changes
> below ripple across many files and ~25 in-crate test call sites; the workspace will not build
> until all are updated together. Do not split.

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/install_wizard/types.rs` — add `WizardOrigin` enum.
- `crates/fdemon-app/src/install_wizard/mod.rs` — re-export `WizardOrigin` (extend the
  `pub use types::{...}` list at line ~15).
- `crates/fdemon-app/src/install_wizard/state.rs` — add `origin` field to `InstallWizardState`;
  change `opening()` → `opening(origin: WizardOrigin)` (line ~151); add `is_bootstrap()` and
  `all_components_ok()` helpers.
- `crates/fdemon-app/src/state.rs` — `show_install_wizard()` → `show_install_wizard(origin: WizardOrigin)`
  (line ~1727); pass through to `InstallWizardState::opening(origin)`.
- `crates/fdemon-app/src/message.rs` — `ShowInstallWizard` → `ShowInstallWizard { origin: WizardOrigin }`
  (line ~1711). Import `WizardOrigin`.
- `crates/fdemon-app/src/handler/update.rs` — dispatch at line ~3234:
  `Message::ShowInstallWizard { origin } => install_wizard::handle_show(state, origin)`.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — `handle_show(state, origin)`
  (line ~15); update `handle_hide`/`handle_escape`/`maybe_dispatch_discovery_on_close` doc
  comments to note handback is `Bootstrap`-only. Update in-file tests (lines ~188, 397, 491+).
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — gate handback on `is_bootstrap()`
  (+ session guard) in `handle_preflight_completed` (line ~61) and
  `close_wizard_and_dispatch_discovery` (line ~88); update docs; update/extend in-file tests
  (many `show_install_wizard()` call sites + the Phase-5 handback tests).
- `crates/fdemon-app/src/handler/keys.rs` — line 363:
  `InputKey::Char('I') => Some(Message::ShowInstallWizard { origin: WizardOrigin::UserInvoked })`.
- `crates/fdemon-tui/src/runner.rs` — line 297:
  `try_send(Message::ShowInstallWizard { origin: WizardOrigin::Bootstrap })`; fix the in-file
  startup tests at lines ~766–797 that enqueue `ShowInstallWizard`.
- `docs/KEYBINDINGS.md` — note `I` opens an informational view when the toolchain is healthy.

**Files Read (Dependencies):**

- `crates/fdemon-app/src/session_manager.rs` — `has_running_sessions()` for the defensive guard.
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentStatus`/`ComponentCheck` for
  `all_components_ok()`.

### Details

**1. `WizardOrigin` enum (`install_wizard/types.rs`):**

```rust
/// Why the Install Wizard was opened. Gates the post-install handback to device discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardOrigin {
    /// Auto-opened at startup because the toolchain was missing/broken. After the toolchain
    /// becomes healthy, the wizard hands back to device discovery (new-session dialog).
    Bootstrap,
    /// User-invoked (`I`) informational view of a (typically healthy) toolchain.
    /// Never hands back; `Esc` returns to the previous mode.
    #[default]
    UserInvoked,
}
```

**2. State (`install_wizard/state.rs`):**

```rust
pub struct InstallWizardState {
    // ... existing fields ...
    /// Why the wizard was opened. Gates the handback (see `close_wizard_and_dispatch_discovery`).
    pub origin: WizardOrigin,
}

impl InstallWizardState {
    pub fn opening(origin: WizardOrigin) -> Self {
        Self { visible: true, loading: true, origin, ..Self::default() }
    }

    /// `true` when the wizard was opened to bootstrap a missing/broken toolchain.
    pub fn is_bootstrap(&self) -> bool {
        self.origin == WizardOrigin::Bootstrap
    }

    /// `true` when a report is present and every component is `Ok` (drives the "All set" hint).
    pub fn all_components_ok(&self) -> bool {
        self.report.as_ref().is_some_and(|r| {
            !r.components.is_empty()
                && r.components.iter().all(|c| c.status == ComponentStatus::Ok)
        })
    }
}
```

> Confirm the exact field name for the per-component status on `ComponentCheck` (`status`) and
> the `ComponentStatus::Ok` variant by reading `toolchain/types.rs`; `flutter_now_live()` already
> uses `c.status == ComponentStatus::Ok`, so mirror that.

**3. `show_install_wizard` (`state.rs`):**

```rust
pub fn show_install_wizard(&mut self, origin: WizardOrigin) {
    self.install_wizard_state = InstallWizardState::opening(origin);
    self.ui_mode = UiMode::InstallWizard;
}
```

**4. Handback gating (`handler/install_wizard/actions.rs`):**

`handle_preflight_completed` auto-close gate (line ~61):

```rust
if state.install_wizard_state.is_bootstrap()
    && state.install_wizard_state.flutter_now_live()
    && !state.install_wizard_state.handback_done
{
    if let Some(discover) = close_wizard_and_dispatch_discovery(state) {
        return UpdateResult::actions_vec(vec![scan_action, discover]);
    }
}
```

`close_wizard_and_dispatch_discovery` (line ~88) — single source of truth, also reached on
manual close:

```rust
pub(super) fn close_wizard_and_dispatch_discovery(state: &mut AppState) -> Option<UpdateAction> {
    let should_handback = state.install_wizard_state.is_bootstrap()
        && !state.session_manager.has_running_sessions();
    if should_handback {
        if let Some(flutter) = state.flutter_executable() {
            state.install_wizard_state.handback_done = true;
            state.hide_install_wizard();
            state.ui_mode = crate::state::UiMode::Startup;
            return Some(UpdateAction::DiscoverDevices { flutter });
        }
    }
    state.hide_install_wizard(); // → UiMode::Normal
    None
}
```

**5. Entry points:** `keys.rs` → `UserInvoked`; `runner.rs` → `Bootstrap`; `update.rs` threads
`origin` into `handle_show`.

**6. Test call-site sweep:** every `state.show_install_wizard()` in `actions.rs` (~17 sites) and
`navigation.rs` (~5 sites) needs an origin argument. For tests that assert a handback occurs
(e.g. `actions.rs` `test_preflight_completed_handback_still_fires_after_execution_reset` ~1044,
the Phase-5 block ~2682/2722/3083, and `navigation.rs` ~491/587), pass
`WizardOrigin::Bootstrap`. For tests that merely open the wizard and assert
loading/navigation/no-handback, `WizardOrigin::UserInvoked` (or `default()`) is fine — but verify
each: `handback_does_not_fire_twice` (~2767) and `preflight_completed_without_live_flutter_does_not_handback`
(~2787) must keep their existing semantics.

### Acceptance Criteria

1. `WizardOrigin { Bootstrap, UserInvoked }` exists, `Default == UserInvoked`, re-exported from
   `install_wizard::mod`.
2. `Message::ShowInstallWizard { origin }`; `keys.rs` emits `UserInvoked`, `runner.rs` emits
   `Bootstrap`.
3. `show_install_wizard(origin)` and `InstallWizardState::opening(origin)` set
   `install_wizard_state.origin`.
4. Auto-close handback fires **only** when `is_bootstrap() && flutter_now_live() && !handback_done`.
5. `close_wizard_and_dispatch_discovery` dispatches `DiscoverDevices` + `UiMode::Startup` only for
   `Bootstrap` with no running session; otherwise hides to `UiMode::Normal` and returns `None`.
6. `cargo build --workspace` and `cargo test -p fdemon-app` pass.

### Testing

New/updated unit tests in `actions.rs` and `navigation.rs` (co-located, matching existing style):

```rust
#[test]
fn user_invoked_open_does_not_handback_on_healthy_toolchain() {
    let mut state = /* AppState with live SDK */;
    state.show_install_wizard(WizardOrigin::UserInvoked);
    let result = handle_preflight_completed(&mut state, all_ok_report());
    assert_eq!(state.ui_mode, UiMode::InstallWizard);
    assert!(state.install_wizard_state.visible);
    assert!(!result.actions().iter().any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })));
}

#[test]
fn user_invoked_escape_returns_to_normal() {
    let mut state = /* AppState with live SDK, wizard UserInvoked + report applied */;
    let result = handle_escape(&mut state);
    assert_eq!(state.ui_mode, UiMode::Normal);
    assert!(!state.install_wizard_state.visible);
    assert!(!result.actions().iter().any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })));
}

#[test]
fn bootstrap_open_still_handbacks() {
    let mut state = /* AppState, no SDK initially → live after report */;
    state.show_install_wizard(WizardOrigin::Bootstrap);
    let result = handle_preflight_completed(&mut state, flutter_ok_report());
    assert_eq!(state.ui_mode, UiMode::Startup);
    assert!(state.install_wizard_state.handback_done);
    assert!(result.actions().iter().any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })));
}

#[test]
fn bootstrap_handback_skipped_when_session_running() {
    let mut state = /* AppState with live SDK + a running session */;
    state.show_install_wizard(WizardOrigin::Bootstrap);
    let result = handle_preflight_completed(&mut state, all_ok_report());
    assert_ne!(state.ui_mode, UiMode::Startup);
    assert!(!result.actions().iter().any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })));
}

#[test]
fn origin_default_is_user_invoked() {
    assert_eq!(WizardOrigin::default(), WizardOrigin::UserInvoked);
}
```

Run: `cargo test -p fdemon-app`, then `cargo clippy --workspace` and `cargo fmt --all`.

### Notes

- **Option 1 (strict):** `UserInvoked` never auto-hands-back and never dispatches discovery on
  close — even if the user happened to install Flutter during that session. They press `Esc` →
  `UiMode::Normal`. This is intentional and simpler than auto-upgrading origin.
- The session guard (`has_running_sessions()`) is defensive: a `Bootstrap` origin already implies
  no running session, but the explicit check documents intent and protects future callers.
- Do **not** change the TUI header here — that is task 02 (it reads `origin`/`all_components_ok`).
- Watch for other `show_install_wizard()` / `Message::ShowInstallWizard` references surfacing at
  compile time (run `rg "ShowInstallWizard|show_install_wizard"` after editing) — fix every site.

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
