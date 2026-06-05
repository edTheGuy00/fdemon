## Task: Fix auto-close handback so discovered devices reach the new-session dialog (F1, F10)

**Severity:** CRITICAL (F1) + MEDIUM test gap (F10)

**Objective**: Make the *primary* handback path actually work — after a managed
Flutter install succeeds and the wizard auto-closes, device discovery must populate
the new-session dialog (not silently drop the devices). Today the auto-close path
leaves `UiMode::Normal`, where the `DevicesDiscovered` handler discards results.

**Depends on**: — (first in chain A; touches files later chain-A tasks also edit)

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` (`hide_install_wizard`, `UiMode`, `show_new_session_dialog`)
- `crates/fdemon-app/src/handler/update.rs` (`DevicesDiscovered` handler, the `ui_mode` guard)

### Details

The auto-close branch of `handle_preflight_completed`
(`handler/install_wizard/actions.rs:52-60`) does:

```rust
state.install_wizard_state.handback_done = true;
state.hide_install_wizard();      // <-- sets ui_mode = UiMode::Normal (state.rs:1733-1736)
// returns UpdateResult with UpdateAction::DiscoverDevices
```

But the `DevicesDiscovered` handler only loads devices into the dialog when the mode
is `Startup`/`NewSessionDialog`:

```rust
// handler/update.rs:444
if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
    state.new_session_dialog_state.target_selector.set_connected_devices(devices);
}
```

With `ui_mode == Normal` the branch is skipped, `set_connected_devices` is never
called, the discovered devices are dropped, and `Normal` renders no dialog. This
directly fails the Phase 5 success criterion *"the new-session dialog is populated
without restarting fdemon."* The **manual-close** path
(`navigation.rs:60-72`, `maybe_dispatch_discovery_on_close`) is correct — it
explicitly sets `state.ui_mode = UiMode::Startup` (line 67) before dispatching
`DiscoverDevices`. The auto-close path simply omits that transition.

> Note: `show_new_session_dialog(configs)` and config loading are already handled by
> startup (`startup_flutter` → `load_all_configs`), so **do not** add config loading
> here — the only missing piece is the `Startup` mode transition. (A reviewer's
> "configs never load" claim was adversarially rejected for exactly this reason.)

**Fix:**
1. In the auto-close branch of `handle_preflight_completed`, after
   `state.hide_install_wizard();`, set `state.ui_mode = crate::state::UiMode::Startup;`
   (mirroring `navigation.rs:67`) so the subsequent `DevicesDiscovered` populates the
   selector and the dialog is shown.
2. **De-duplicate**: factor the shared post-close handback logic
   (`handback_done = true` → set `ui_mode = Startup` → return `DiscoverDevices`, with
   the existing double-discovery guard) into a single helper used by **both** the
   auto-close path and `maybe_dispatch_discovery_on_close`, so the two paths cannot
   drift again. Keep the existing `handback_done` / discovery-in-flight guard intact.

### Acceptance Criteria

1. After `handle_preflight_completed`'s auto-close path runs with a live Flutter SDK,
   `state.ui_mode == UiMode::Startup` (not merely `!= InstallWizard`).
2. Driving the resulting `Message::DevicesDiscovered` through `handler::update`
   leaves `state.new_session_dialog_state.target_selector` populated with the
   discovered devices (non-empty).
3. The manual-close path (`maybe_dispatch_discovery_on_close`) still sets `Startup`
   and still dispatches `DiscoverDevices` — behaviour unchanged.
4. The double-discovery guard still prevents a second `DiscoverDevices` when discovery
   is already in flight (auto-close + a racing manual `Esc`).

### Testing

```rust
// handler/install_wizard/actions.rs + navigation.rs test modules
// - UPDATE preflight_completed_with_live_flutter_autocloses_and_discovers:
//     assert state.ui_mode == UiMode::Startup (was: assert_ne!(.., InstallWizard))
// - NEW: after the auto-close, feed Message::DevicesDiscovered(vec![dev]) through
//     handler::update and assert target_selector.connected_devices() is non-empty.
// - ADD to the manual-close test (navigation.rs): assert ui_mode == Startup.
// - KEEP the double-discovery guard test green.
```

### Notes

- F10 is folded in: the existing tests passed *because* they only asserted
  `ui_mode != InstallWizard`, which `Normal` trivially satisfies — that weak
  assertion is what let F1 ship. AC#1/#2 above replace it with the meaningful checks.
- This task shares `handler/install_wizard/actions.rs` + `navigation.rs` with tasks
  02 and 03; run them serially on the same branch (chain A), not in parallel worktrees.
- Do **not** add a launch.toml/launch.json assertion — launch configs are not part of
  `target_selector` and the handback code does not touch them.
