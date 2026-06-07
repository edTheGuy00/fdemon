# Bug Fix: Install Wizard re-opens into the new-session flow when the toolchain is already healthy

## Summary

When the toolchain is already complete and the user re-opens the Install Wizard with `I`
from the main logs page (Normal mode) — either immediately after a successful install or on
any later run — the wizard re-runs preflight, sees everything green, and **auto-advances into
the new-session dialog** (device discovery handback). It should instead open as a **read-only
informational view** showing everything that is installed, and never push the user into the
new-session dialog.

The bootstrap flow (fresh machine → preflight → install → hand back to device discovery) must
still work, but only when the wizard was opened *because the toolchain was actually missing*.

## Severity

Medium (UX). No data loss, but it disrupts an active fdemon session: pressing `I` to inspect
the toolchain yanks the user into a new-session dialog over their running session.

## Reproduction

1. Run fdemon on a machine where Flutter is installed and working (start or have a session).
2. From the main logs page (Normal mode), press `I`.
3. **Observed:** wizard flashes its loading/preflight state, then auto-closes and the
   new-session dialog appears.
4. **Expected:** wizard stays open in an informational view with all components shown as Ok;
   pressing `Esc` returns to the logs page (Normal mode). No new-session dialog.

## Root Cause

The handback transition (Phase 5, Task 04) is gated only on *"is Flutter now live?"* and a
one-shot guard — it has **no notion of why the wizard was opened**. Both entry points emit the
same `Message::ShowInstallWizard` and run the identical open path, so there is no way to tell a
**bootstrap** open (toolchain was broken, startup hook opened it) from an **informational**
open (user pressed `I` on a healthy setup).

Key code:

- `crates/fdemon-app/src/handler/install_wizard/actions.rs:61` — auto-close gate:
  ```rust
  if state.install_wizard_state.flutter_now_live() && !state.install_wizard_state.handback_done {
      if let Some(discover) = close_wizard_and_dispatch_discovery(state) { ... }
  }
  ```
  On an informational re-open, `flutter_now_live()` is immediately `true` and `handback_done`
  is `false` (reset by `opening()`), so the wizard auto-closes and dispatches `DiscoverDevices`.

- `crates/fdemon-app/src/handler/install_wizard/actions.rs:88` —
  `close_wizard_and_dispatch_discovery()` unconditionally sets `ui_mode = UiMode::Startup` and
  returns `DiscoverDevices` whenever `flutter_executable()` is `Some`. This is the **single
  source of truth** for handback and is also reached on manual close
  (`navigation.rs:72` → `maybe_dispatch_discovery_on_close`).

- `crates/fdemon-app/src/install_wizard/state.rs:151` — `opening()` carries no origin flag;
  `crates/fdemon-app/src/state.rs:1727` — `show_install_wizard()` takes no arguments.

- Entry points, both indistinguishable:
  - `crates/fdemon-tui/src/runner.rs:297` — startup hook: `if flutter_executable().is_none() { try_send(ShowInstallWizard) }` (bootstrap).
  - `crates/fdemon-app/src/handler/keys.rs:363` — `Char('I') => Message::ShowInstallWizard` (user-invoked).

## Fix Design

Introduce an explicit **`WizardOrigin`** enum, threaded from the entry point through the
`ShowInstallWizard` message, and gate **all** handback on it. Plus an "All set" header hint
when the wizard is opened informationally on a healthy toolchain.

### Discriminator

```rust
/// Why the Install Wizard was opened. Gates the post-install handback to device discovery.
pub enum WizardOrigin {
    /// Auto-opened at startup because the toolchain was missing/broken,
    /// or invoked to fix a broken toolchain. Handback to device discovery is enabled.
    Bootstrap,
    /// User-invoked (`I`) for an informational view of a (typically healthy) toolchain.
    /// No handback — `Esc` returns to the previous mode.
    UserInvoked,
}
```

- The startup hook in `runner.rs` (already only fires when `flutter_executable().is_none()`)
  sends `Bootstrap`.
- The `I` key in `keys.rs` sends `UserInvoked`.
- Invariant kept as a defensive guard: a `UserInvoked` open never hands back regardless of SDK
  state, so the user's "session already running → just show info" case is fully covered.

### Changes

1. **`crates/fdemon-app/src/install_wizard/types.rs`**
   - Define `pub enum WizardOrigin { Bootstrap, UserInvoked }` (derive
     `Debug, Clone, Copy, PartialEq, Eq`; `Default` → `UserInvoked`, the safe/no-handback
     default). Re-export from `install_wizard/mod.rs`.

2. **`crates/fdemon-app/src/install_wizard/state.rs`**
   - Add field `pub origin: WizardOrigin` to `InstallWizardState`.
   - Change `opening()` → `opening(origin: WizardOrigin)` and set the field.
   - Add helpers: `is_bootstrap(&self) -> bool` (`origin == Bootstrap`) and
     `all_components_ok(&self) -> bool` (report present and every `ComponentCheck` is `Ok`) —
     the latter drives the header hint.

3. **`crates/fdemon-app/src/message.rs`**
   - Change `ShowInstallWizard` → `ShowInstallWizard { origin: WizardOrigin }`.

4. **`crates/fdemon-app/src/handler/update.rs`**
   - Pass `origin` through to `install_wizard::handle_show(state, origin)`.

5. **`crates/fdemon-app/src/handler/install_wizard/navigation.rs`**
   - `handle_show(state, origin)` calls `state.show_install_wizard(origin)`.
   - Update `handle_hide`/`handle_escape`/`maybe_dispatch_discovery_on_close` docs to note
     handback only occurs for a `Bootstrap`-origin wizard.

6. **`crates/fdemon-app/src/state.rs`**
   - `show_install_wizard(&mut self, origin: WizardOrigin)` calls
     `InstallWizardState::opening(origin)`.

7. **`crates/fdemon-app/src/handler/install_wizard/actions.rs`**
   - Gate the auto-close handback on origin:
     ```rust
     if state.install_wizard_state.is_bootstrap()
         && state.install_wizard_state.flutter_now_live()
         && !state.install_wizard_state.handback_done
     { ... }
     ```
   - In `close_wizard_and_dispatch_discovery()`, only dispatch discovery / route to
     `UiMode::Startup` when origin is `Bootstrap` *and* no session is running (defensive);
     otherwise always hide to `UiMode::Normal` and return `None`:
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
   - Update doc comments on `handle_preflight_completed` and
     `close_wizard_and_dispatch_discovery` to state the `Bootstrap` precondition.

8. **`crates/fdemon-tui/src/runner.rs`**
   - `dispatch_startup_action`: `try_send(Message::ShowInstallWizard { origin: WizardOrigin::Bootstrap })`.

9. **`crates/fdemon-app/src/handler/keys.rs`**
   - `Char('I') => Message::ShowInstallWizard { origin: WizardOrigin::UserInvoked }`.

10. **`crates/fdemon-tui/src/widgets/install_wizard/` (header hint)**
    - When `origin == UserInvoked` and `all_components_ok()` is true, render a header line such
      as `All set — press Esc to return`. Likely in the panel header in
      `widgets/install_wizard/mod.rs` (or `step_detail.rs` header), reading the already-passed
      `&InstallWizardState`. Falls back to the normal header otherwise (loading / partial /
      bootstrap).

### Why explicit origin over deriving from SDK presence

Threading `WizardOrigin` makes the intent explicit at each call site and enables a future
"force re-setup on a healthy toolchain" command (which a derived-from-SDK-presence flag could
not express). The cost is touching `message.rs` / `update.rs` / `keys.rs` / `runner.rs`, which
is acceptable for the clarity and extensibility.

## Behaviour Matrix (after fix)

| Scenario | Entry point | `origin` | Handback? |
|----------|-------------|----------|-----------|
| Fresh machine, startup hook opens wizard | `runner.rs` | `Bootstrap` | Yes — after install, hand to device discovery |
| Fresh machine, user presses `I`, then installs | `keys.rs` | `UserInvoked` | No (use `Esc` to proceed; or could open as Bootstrap — see Open Question) |
| Healthy toolchain, user presses `I` (no session) | `keys.rs` | `UserInvoked` | No — informational, "All set" hint, `Esc` → Normal |
| Healthy toolchain, session running, user presses `I` | `keys.rs` | `UserInvoked` | No — informational, `Esc` → Normal |
| Bootstrap install completes → session started → user presses `I` again | `keys.rs` | `UserInvoked` | No |

> **Resolved — Option 1 (strict).** A `UserInvoked` open **never** auto-hands-back, regardless
> of toolchain state at open time. If a user presses `I` on a broken toolchain and installs via
> the wizard, they press `Esc` when done (the manual-close path also respects origin, so it
> returns to `UiMode::Normal` — it does **not** push the new-session dialog). Only a
> `Bootstrap`-origin wizard (startup hook, fired when `flutter_executable().is_none()`) performs
> the post-install handback. This keeps the discriminator purely intent-based and predictable.

## Test Plan

Unit tests in `crates/fdemon-app/src/handler/install_wizard/` (co-located with the handlers,
matching the existing test style):

1. **Informational open does not hand back.** Call `handle_show(state, WizardOrigin::UserInvoked)`
   with a live SDK, feed a `ToolchainPreflightCompleted` with an all-Ok report, assert: wizard
   stays `visible`, `ui_mode == InstallWizard`, result contains **no** `DiscoverDevices`, only
   `ScanInstalledSdks`.
2. **Informational close returns to Normal.** From the above state, call `handle_escape`;
   assert `ui_mode == Normal`, wizard hidden, no `DiscoverDevices`.
3. **Bootstrap open still hands back.** Call `handle_show(state, WizardOrigin::Bootstrap)`,
   then feed a `ToolchainPreflightCompleted` whose report shows Flutter Ok (simulating
   post-install) with a live `flutter_executable()`; assert `ui_mode == Startup`,
   `handback_done == true`, result contains `DiscoverDevices`.
4. **Bootstrap manual close hands back once.** `Bootstrap` origin, then `handle_escape` with a
   live `flutter_executable()`; assert single `DiscoverDevices` and the one-shot guard prevents
   a second.
5. **Session-running guard.** `Bootstrap` origin with a running session present and a live SDK:
   assert no handback (defensive path), `ui_mode == Normal`.
6. **Origin plumbing.** `show_install_wizard(origin)` sets `install_wizard_state.origin`;
   `WizardOrigin::default() == UserInvoked`.
7. **Header hint.** With `origin == UserInvoked` and an all-Ok report, a TUI render test
   (TestTerminal) asserts the "All set" header line is present; with `Bootstrap` or a
   partial report it is absent.

Run: `cargo test -p fdemon-app` then `cargo test --workspace`. Confirm no regression in the
existing Phase 5 handback tests (they should be updated to open in bootstrap mode where they
assert handback).

## Edge Cases & Risks

- **Existing handback tests assume handback fires.** Any current test that opens the wizard on
  a *healthy* SDK and asserts `DiscoverDevices` will now (correctly) fail; update those to
  either open with no SDK (bootstrap) or assert the new informational behaviour. List them
  during implementation (`grep` for `DiscoverDevices` / `handback` in the wizard handler tests).
- **`flutter_executable()` semantics at open time.** Confirm it reflects the *currently
  resolved* SDK (post-managed-install `config.toml` write) and not a stale value; the existing
  startup hook already relies on this, so it is consistent.
- **No rendering change required.** The wizard already renders component statuses and the
  embedded `flutter doctor` output, so an all-Ok informational view is already meaningful.
  Optional polish (out of scope): a header hint like "All set — press Esc to return" when
  `!bootstrap`.

## Affected Files

| File | Change |
|------|--------|
| `crates/fdemon-app/src/install_wizard/types.rs` | New `WizardOrigin` enum |
| `crates/fdemon-app/src/install_wizard/mod.rs` | Re-export `WizardOrigin` |
| `crates/fdemon-app/src/install_wizard/state.rs` | Add `origin` field; `opening(origin)`; `is_bootstrap()`, `all_components_ok()` |
| `crates/fdemon-app/src/message.rs` | `ShowInstallWizard { origin }` |
| `crates/fdemon-app/src/handler/update.rs` | Thread `origin` to `handle_show` |
| `crates/fdemon-app/src/state.rs` | `show_install_wizard(origin)` |
| `crates/fdemon-app/src/handler/install_wizard/navigation.rs` | `handle_show(state, origin)`; doc updates |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Gate handback on origin (+ session guard); doc updates |
| `crates/fdemon-tui/src/runner.rs` | Send `Bootstrap` origin from startup hook |
| `crates/fdemon-app/src/handler/keys.rs` | Send `UserInvoked` origin from `I` |
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (or `step_detail.rs`) | "All set" header hint when informational + all-Ok |
| `crates/fdemon-app/src/handler/install_wizard/*tests*` | Update/extend handback tests |

## Out of Scope

- The separate known CRIT from the project memory ("handback drops devices via
  `UiMode::Normal`") and the async abort-handle races are tracked under
  `phase-5-followup/` and are not addressed here, though the session-running guard added above
  reduces the blast radius of the handback path.
- A user-facing "force re-setup" command for a healthy toolchain (would need a `WizardOrigin`
  enum on the message).
