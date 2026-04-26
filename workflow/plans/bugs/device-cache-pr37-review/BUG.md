# Bugfix Plan: PR #37 Copilot Review Fixes

## TL;DR

Address 5 review findings from Copilot on
[PR #37](https://github.com/edTheGuy00/fdemon/pull/37) ("fix(new-session): drop
device-cache TTL, refresh in background"):

- **F1 (Major):** When `flutter_executable()` returns `None`, the dialog stays stuck on the
  perpetual loading spinner because `loading=true`/`bootable_loading=true` are never
  cleared and no error is surfaced.
- **F2 (Major):** When `connected_cached=false` but `bootable_cached=true`, the code
  dispatches `RefreshDevicesAndBootableBackground`. A background-arm failure clears only
  `refreshing` (not `loading`), so the Connected tab can stay stuck loading on transient
  failure. Foreground discovery is required whenever connected cache is missing.
- **F3 (Minor):** The doc comment on `set_error()` claims it is invoked only from the
  connected-device foreground discovery failure path. In reality it has 13 callers
  spanning boot failures, launch-context errors, and SDK-not-found paths. Doc-only fix.
- **F4 (Minor):** Compact-mode tab labels render the `↻` glyph only when the refreshing
  tab is *active*. Full-mode `TabBar` renders it per-tab regardless of active state. The
  PR description ("Show ↻ on any tab that's refreshing") matches the full-mode behavior;
  compact mode should match.
- **F5 (Major):** `TargetSelector::new()` defaults `icons` to `IconSet::default()` (which
  is Unicode). The two `TargetSelector::new(...)` call sites in
  `widgets/new_session_dialog/mod.rs` (horizontal at line 329, vertical at line 551) do
  not chain `.icons(*self.icons)`, so Nerd Fonts users see the Unicode `↻` glyph despite
  having the Nerd Fonts `IconSet` configured at the dialog level.

PR branch: `fix/remove-cache-device-ttl`. All fixes will be added as commits to this
branch so the PR can land.

Source review:
[PR #37 Copilot review (id PRR_kwDOQ0IA5s744OBm)](https://github.com/edTheGuy00/fdemon/pull/37#pullrequestreview-4175487078)

Parent plans:
- [`workflow/plans/bugs/device-cache-no-ttl/BUG.md`](../device-cache-no-ttl/BUG.md)
- [`workflow/plans/bugs/device-cache-followup/BUG.md`](../device-cache-followup/BUG.md)

---

## Bug Report

### Symptom

After PR #37 was opened, Copilot's automated review flagged five concrete issues. The
two Major behavioral issues both produce stuck-loading states under specific (but
realistic) conditions; the third Major issue is a cross-mode glyph regression for
Nerd Fonts users; the two Minor issues are doc accuracy and visual consistency.

1. **Open new-session dialog with no Flutter SDK configured.** Spinner spins forever
   on both tabs; user has no recovery path inside the dialog. They must close and
   reconfigure their `.fdemon/config.toml` blind.
2. **Open new-session dialog with empty connected cache and populated bootable cache.**
   `flutter devices` fails transiently. Connected tab stays stuck on the loading
   spinner. (After follow-up task 01's fix, the `↻` indicator clears, but `loading`
   remains true.)
3. **`set_error()` doc misleads future maintainers.** Comment says it's called only
   from `Message::DeviceDiscoveryFailed { is_background: false }`. Grep shows 13
   call sites including boot failures, validation errors, launch-context errors. Risk:
   future maintainer assumes it's safe to extend `set_error()` semantics for that single
   path and breaks the others.
4. **Compact mode hides the refresh indicator on inactive refreshing tabs.** Resize the
   terminal so the dialog enters compact mode, then trigger a background bootable
   refresh while on the Connected tab. The `↻` indicator that the full mode would show
   on the inactive Bootable tab does not appear.
5. **Nerd Fonts users see the wrong refresh glyph.** Configure `icon_mode = "nerd-fonts"`
   in `.fdemon/config.toml`. Open the new-session dialog. The `↻` Unicode glyph appears
   in the Connected/Bootable tab labels even though the user expects the Nerd Fonts
   refresh glyph (which `TabBar` correctly resolves from `IconSet`, but only because
   `target_selector.rs` defaults to `IconSet::default()` when no `.icons()` is chained).

### Expected

- Missing Flutter SDK surfaces an actionable error in the dialog and clears all
  in-flight indicators.
- Connected cache miss always uses foreground discovery so failures clear `loading`.
- `set_error()` doc accurately reflects all callers and clearing semantics.
- Compact mode and full mode show the `↻` glyph on the same tabs under the same
  conditions.
- Nerd Fonts users see the configured Nerd Fonts refresh glyph in tab labels.

### Root Causes

#### F1 — Stuck loading on missing Flutter SDK

`crates/fdemon-app/src/handler/new_session/navigation.rs:243-246`:

```rust
let Some(flutter) = state.flutter_executable() else {
    tracing::warn!("handle_open_new_session_dialog: no Flutter SDK — skipping device refresh");
    return UpdateResult::none();
};
```

`show_new_session_dialog()` (`state.rs:1117-1120`) reconstructs `NewSessionDialogState`
from scratch via `NewSessionDialogState::new(configs)`, which uses
`TargetSelectorState::default()` — and that default sets `loading: true,
bootable_loading: true` (see `target_selector_state.rs:84-85`). When connected cache
populates, `set_connected_devices()` clears `loading`. When bootable cache populates,
`set_bootable_devices()` clears `bootable_loading`. But when neither cache populates
*and* `flutter_executable()` returns `None`, neither setter is called, so both flags
remain `true`. The dialog renders the loading spinner perpetually with no in-flight
discovery.

The other `set_error()` call site in `launch_context.rs:532` already establishes the
pattern of surfacing this error message: `"No Flutter SDK found. Configure sdk_path
in .fdemon/config.toml or install Flutter."`.

#### F2 — Cache-miss connected-only routes failures to background arm

`crates/fdemon-app/src/handler/new_session/navigation.rs:248-267`:

```rust
if connected_cached || bootable_cached {
    // ...
    return UpdateResult::action(UpdateAction::RefreshDevicesAndBootableBackground { flutter });
}
```

The condition `connected_cached || bootable_cached` fires the background variant
whenever *either* cache is hit. When `connected_cached=false` but `bootable_cached=true`:

- `set_bootable_devices()` was called → `bootable_loading=false`.
- `set_connected_devices()` was *not* called → `loading=true` (default).
- `RefreshDevicesAndBootableBackground` is dispatched.
- Connected discovery uses `spawn_device_discovery_background` which routes failures
  through `Message::DeviceDiscoveryFailed { is_background: true }`. The post-task-01
  handler arm clears `refreshing` but not `loading`.
- Connected tab stays stuck on the loading spinner.

Fix: when `connected_cached=false`, dispatch the foreground variant
`DiscoverDevicesAndBootable` (already exists from device-cache-followup task 03). It
uses `spawn_device_discovery` (foreground) which routes failures through `set_error()`,
clearing `loading`. Bootable still spawns in parallel and updates the (already-shown)
bootable list in the background.

#### F3 — `set_error()` doc inaccuracy

`crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:271-278`:

The doc comment claims `set_error()` is invoked only from
`Message::DeviceDiscoveryFailed { is_background: false }`. Grep confirms 13 call sites:

- `handler/update.rs:427` (the one the comment describes)
- `handler/update.rs:982, 997, 1272` (session creation, boot failures)
- `handler/new_session/target_selector.rs:170, 202` (selection / boot failures)
- `handler/new_session/launch_context.rs:416, 430, 532, 550, 584, 608` (config /
  launch / SDK-not-found errors)

The comment must be updated to reflect that `set_error()` is a general "new-session
error" helper, not a connected-discovery-failure-specific helper, and that
`bootable_loading`/`bootable_refreshing` are intentionally not touched (callers that
need to clear bootable indicators must do so themselves).

#### F4 — Compact-mode glyph hidden on inactive refreshing tabs

`crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs:233-252`:

```rust
let connected_label = if connected_active {
    if self.state.refreshing {
        format!("[1 Connected {}]", self.icons.refresh())
    } else {
        "[1 Connected]".to_string()
    }
} else {
    "1 Connected".to_string()  // <-- no glyph if inactive, even if refreshing
};
```

The nesting puts the `refreshing` check *inside* the `connected_active` branch, so an
inactive tab's refreshing flag is ignored. Full-mode `TabBar` (the source of the
PR-stated behavior) renders the glyph per-tab regardless of active state.

Fix: invert the nesting — compute the base label (with/without brackets) first based on
active state, then conditionally append the glyph based on the refreshing flag.

#### F5 — `IconSet` not threaded from `NewSessionDialog` to `TargetSelector`

`crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs:35-47`:

```rust
pub fn new(
    state: &'a TargetSelectorState,
    tool_availability: &'a ToolAvailability,
    is_focused: bool,
) -> Self {
    Self {
        state,
        tool_availability,
        icons: IconSet::default(),  // <-- always Unicode unless .icons() chained
        is_focused,
        compact: false,
    }
}
```

Both `TargetSelector::new()` call sites in `widgets/new_session_dialog/mod.rs` (line 329
horizontal layout, line 551 vertical layout) do not chain `.icons(*self.icons)`, so the
`NewSessionDialog`'s configured `IconSet` (which the user has set to Nerd Fonts via
`config.toml`) is dropped on the floor. Result: `TabBar.icons.refresh()` returns the
Unicode glyph instead of the Nerd Fonts glyph.

Fix: chain `.icons(*self.icons)` at both call sites. (`IconSet` is `Copy` per
`theme/icons.rs`, so `*self.icons` is a cheap copy.) Add a lock-in test in
`new_session_dialog/mod.rs` (or `target_selector.rs`) that constructs the dialog with a
non-default `IconSet` and asserts the rendered output contains the configured glyph.

---

## Affected Modules

| Module | Change |
|---|---|
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | F1: surface SDK error + clear all 4 flags on early return; F2: branch on `connected_cached` for foreground vs background routing |
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | F3: rewrite `set_error()` doc to reflect actual usage and clearing semantics |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | F4: refactor compact-mode label construction so the glyph appears regardless of active state |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | F5: chain `.icons(*self.icons)` at both `TargetSelector::new()` call sites; add a lock-in test |

---

## Phases

### Phase 1 — Review fixes (single wave, all parallel)

**Goal:** every Copilot review finding is addressed and verified.

**Steps (per task):**

1. **F1 + F2 — Stuck-loading + connected cache-miss foreground.** Replace the bare
   `flutter_executable()` early return with a `set_error("No Flutter SDK found. ...")`
   call plus explicit `bootable_loading = false; bootable_refreshing = false;` (since
   `set_error()` only clears connected-side flags). Restructure the post-cache branch:
   `if connected_cached → background (RefreshDevicesAndBootableBackground); else →
   foreground (DiscoverDevicesAndBootable) with bootable_refreshing=true if
   bootable_cached`. Add inline tests for both new branches.

2. **F3 — `set_error()` doc accuracy.** Rewrite the doc comment using the reviewer's
   suggested wording (or near-verbatim). Pure doc edit; no behavior change.

3. **F4 — Compact-mode glyph for inactive refreshing tabs.** Refactor
   `render_tabs_compact` so the base label (active brackets / inactive bare) is
   computed first, then the refresh glyph is appended whenever the corresponding
   refreshing flag is set. Mirror the `TabBar` per-tab semantics. Add a render test
   for the inactive-tab-refreshing case.

4. **F5 — Thread `IconSet` from `NewSessionDialog` to `TargetSelector`.** Add
   `.icons(*self.icons)` chain to both `TargetSelector::new()` call sites in
   `widgets/new_session_dialog/mod.rs` (lines 329, 551). Add a lock-in render test
   in `mod.rs` (or extend an existing one in `target_selector.rs`) that constructs
   the widget with a non-default `IconSet` and asserts the rendered output contains
   the configured refresh glyph.

**Measurable Outcomes:**

- Opening the dialog with no Flutter SDK shows an error message ("No Flutter SDK
  found. ...") with no spinner.
- Opening the dialog with `connected_cached=false`, `bootable_cached=true` and a
  failing `flutter devices` command shows a surfaced error on the Connected tab
  (not a stuck spinner).
- `set_error()` doc accurately describes the helper's general role and
  bootable-flag-non-clearing semantics.
- Compact mode shows the `↻` glyph on the inactive refreshing tab(s) just like full
  mode does.
- Nerd Fonts users see the Nerd Font refresh glyph in compact and full mode tabs
  when a refresh is in flight.
- `cargo fmt --all && cargo check --workspace && cargo test --workspace --lib && cargo clippy --workspace --lib -- -D warnings` clean.

---

## Edge Cases & Risks

### Foreground discovery dispatched while bootable is also being discovered
- **Scenario:** Connected cache empty, bootable cache populated. F2 fix dispatches
  `DiscoverDevicesAndBootable` (foreground connected, background bootable).
- **Behavior:** `bootable_refreshing=true` is set so the user sees the indicator on
  the (already-populated) Bootable tab. Connected goes through `set_error()` on
  failure or `set_connected_devices()` on success, both of which clear `loading`.
  The bootable spawn updates the bootable cache in parallel.

### `set_error()` called from a non-discovery path (e.g. boot failure)
- **Risk:** F3's doc rewrite must not imply that `set_error()` clears bootable
  indicators. Bootable discovery is independent (xcrun/emulator tools), and its
  flags should only be cleared by their own success/failure handlers.
- **Mitigation:** The new doc explicitly states `bootable_loading` and
  `bootable_refreshing` are not cleared, and callers must clear them if needed.

### Nerd Fonts test depends on `IconSet::default()` producing the expected glyph
- **Risk:** F5's lock-in test must use an `IconSet` constructor that produces a
  *different* refresh glyph than `IconSet::default()`. Otherwise the test cannot
  distinguish "icons were threaded through" from "icons were defaulted to the same
  thing."
- **Mitigation:** The implementor must consult `theme/icons.rs` to confirm there's
  a Nerd Fonts variant constructor (or build one inline via struct literal) and use
  a glyph that differs from the Unicode `↻`.

---

## Out of Scope

- Making `IconSet` a required constructor argument on `TargetSelector::new()`. The
  reviewer offered both options (required arg or chain at call sites); we chose the
  smaller-blast-radius option to minimize test churn in `target_selector.rs` (which
  has ~6 test call sites that would all need updating).
- Adding a real `BootableDiscoveryFailed` message variant. The current
  `unwrap_or_default()` pattern in `spawn_bootable_device_discovery` silently hides
  failures; surfacing them is a separate UX decision parked from the parent plan.
- Restructuring `set_error()` to clear bootable flags too. Bootable discovery is
  independent and its flags should be managed by their own paths.
- Restructuring the cache-miss branching beyond what F2 requires. The new shape
  (`if connected_cached { bg } else { fg }`) is the minimal change.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `flutter_executable() == None` early return calls `set_error(...)` and clears
      `bootable_loading`/`bootable_refreshing`.
- [ ] Test `test_open_dialog_no_flutter_sdk_surfaces_error` passes (asserts error is
      set and all 4 flags are cleared).
- [ ] `connected_cached=false` always dispatches `DiscoverDevicesAndBootable`
      regardless of bootable cache state.
- [ ] Test `test_open_dialog_bootable_cached_only_uses_foreground` passes (asserts
      `DiscoverDevicesAndBootable` action and `bootable_refreshing=true`).
- [ ] `set_error()` doc accurately describes general usage and bootable-flag
      non-clearing.
- [ ] `render_tabs_compact` shows the refresh glyph on inactive refreshing tabs.
- [ ] Test `test_target_selector_compact_renders_refreshing_glyph_on_inactive_tab`
      passes.
- [ ] Both `TargetSelector::new()` call sites in `widgets/new_session_dialog/mod.rs`
      chain `.icons(*self.icons)`.
- [ ] Lock-in test asserts a non-default `IconSet` flows through to the rendered
      output.
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace --lib && cargo clippy --workspace --lib -- -D warnings` clean.

---

## Task Dependency Graph

```
Phase 1 (single wave, all parallel worktrees)
├── 01-stuck-loading-and-cache-miss        (navigation.rs)
├── 02-set-error-doc-accuracy              (target_selector_state.rs)
├── 03-compact-mode-glyph-inactive-tab     (target_selector.rs)
└── 04-thread-iconset-to-target-selector   (mod.rs)
```

No write-file overlap → all four can run in parallel worktrees.

---

## Milestone Deliverable

When Phase 1 is complete:

- All 5 Copilot review findings on PR #37 are resolved.
- The PR is ready to land (no open review threads from Copilot's automated review).
- Manual reviewers can verify the fixes against the original review comments at
  https://github.com/edTheGuy00/fdemon/pull/37.
