# Bugfix Plan: Device Cache No-TTL Review Follow-up

## TL;DR

Address blocking findings from
[`workflow/reviews/bugs/device-cache-no-ttl/REVIEW.md`](../../../reviews/bugs/device-cache-no-ttl/REVIEW.md):

- **C1 (Critical):** background discovery failures leave the `↻` indicator stuck on the
  Connected tab.
- **M1 (Major):** when both caches are empty, only the connected discovery is kicked off;
  the bootable list silently sits at its default `loading` state until the user manually
  switches tabs — contradicting the milestone deliverable of the parent plan.
- **M2 (Major):** `get_cached_bootable_devices()` clones the entire bootable cache on
  every call, asymmetric with the connected accessor and hidden in a code path advertised
  as "instant."

Plus selected minor cleanups: route the `↻` glyph through the existing `IconSet` (which
already has Nerd Fonts and Unicode variants but is bypassed by an inline literal), surface
the indicator in compact mode, and bundle small polish items.

Parent plan: [`workflow/plans/bugs/device-cache-no-ttl/BUG.md`](../device-cache-no-ttl/BUG.md)

---

## Bug Report

### Symptom

After the device-cache-no-ttl change merged:

1. **Stuck `↻` indicator on background failure.** Open the new-session dialog with a cached
   device list. If the background `flutter devices` refresh fails (transient subprocess
   error, missing executable, etc.), the `↻` glyph on the Connected tab remains visible
   indefinitely — until the user closes and reopens the dialog. The data is fine (cached
   list still shown); the indicator is misleading.
2. **Bootable tab frozen on first dialog open with empty caches.** On a clean startup
   where neither cache has populated yet, opening the dialog dispatches only the connected
   discovery. The Bootable tab sits showing its default loading state with no
   discovery actually in flight, until the user manually switches to it (which fires
   `handle_switch_tab` and triggers discovery on demand).
3. **Hidden allocation in the bootable cache hit path.** Every dialog open with a populated
   bootable cache clones two `Vec`s inside `get_cached_bootable_devices()`. The connected
   accessor returns a borrow.

### Expected

- The `↻` indicator clears on every discovery completion path, including background
  failure.
- Opening the new-session dialog refreshes both lists, regardless of cache state — matching
  the parent plan's stated milestone.
- The bootable cache accessor returns references; the single caller clones explicitly.

### Root Causes

#### C1 — Background-failure clearing path

`crates/fdemon-app/src/handler/update.rs:405-428`. The `is_background: true` arm of
`Message::DeviceDiscoveryFailed` only logs a warning:

```rust
if is_background {
    tracing::warn!("Background device refresh failed: {}", error);
} else {
    if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
        state.new_session_dialog_state.target_selector.set_error(error.clone());
    }
    tracing::error!("Device discovery failed: {}", error);
}
```

`set_error()` (the only place `refreshing` is cleared on a *failure*) is reached only
through the foreground branch. Background failures fall through to `UpdateResult::none()`
without touching `refreshing`. The new `RefreshDevicesAndBootableBackground` action sends
its connected-discovery failures through `is_background: true`
(`crates/fdemon-app/src/spawn.rs:45-68`).

The parent plan's "Cache Becoming Severely Stale" mitigation
(`workflow/plans/bugs/device-cache-no-ttl/BUG.md:191`) explicitly — and incorrectly —
claims `set_error()` handles this case.

**Note:** The bootable side is not affected by this exact bug. `spawn_bootable_device_discovery`
(`spawn.rs:570-591`) swallows errors via `unwrap_or_default()` and always sends a successful
`BootableDevicesDiscovered` message, which calls `set_bootable_devices()` and clears
`bootable_refreshing`. Bootable failures are silently hidden, but the indicator does clear.

#### M1 — Cache-miss bootable discovery

`crates/fdemon-app/src/handler/new_session/navigation.rs:258-261`. When both caches are
empty, the cache-miss fallback dispatches `UpdateAction::DiscoverDevices { flutter }`,
which (in `actions/mod.rs:75-77`) calls only `spawn::spawn_device_discovery`. There is no
sibling spawn for bootable discovery.

The existing `RefreshDevicesAndBootableBackground` is a background variant only — it sends
`is_background: true` failures and doesn't set `loading`. We need the same parallel-spawn
shape but with the foreground connected discovery so the user gets a proper loading
indicator and surfaced errors.

#### M2 — Owned-return bootable accessor

`crates/fdemon-app/src/state.rs:1261-1266`:

```rust
pub fn get_cached_bootable_devices(&self) -> Option<(Vec<IosSimulator>, Vec<AndroidAvd>)> {
    match (&self.ios_simulators_cache, &self.android_avds_cache) {
        (Some(sims), Some(avds)) => Some((sims.clone(), avds.clone())),
        _ => None,
    }
}
```

The owned return forces clones inside the accessor. The single caller
(`navigation.rs:221-234`) takes ownership via `set_bootable_devices(simulators, avds)` —
the ownership requirement is real, but the clone should live at the call site to mirror
the connected-device pattern (`get_cached_devices` returns `Option<&Vec<Device>>`).

#### m2 — `↻` glyph bypasses `IconSet`

`crates/fdemon-tui/src/theme/icons.rs:96` already exposes `IconSet::refresh()` returning
`"\u{21bb}"` (= `↻`) for `IconMode::Unicode` and the Nerd Font equivalent for
`IconMode::NerdFonts`. `tab_bar.rs:71` ignores this and hardcodes the literal `"↻"`:

```rust
let label = format!("{} ↻", tab.label());
```

Result: Nerd Fonts users see the wrong glyph. The fix is to thread `&IconSet` into
`TabBar::new()` (mirroring the `&'a IconSet` pattern already in
`new_session_dialog/mod.rs:162-175`) and call `icons.refresh()`.

---

## Affected Modules

| Module | Change |
|---|---|
| `crates/fdemon-app/src/handler/update.rs` | C1: clear `refreshing` in `is_background: true` arm when dialog is visible |
| `crates/fdemon-app/src/handler/tests.rs` | C1: integration test for background-failure clearing; m1: symmetric `BootableDevicesDiscovered` test |
| `crates/fdemon-app/src/handler/mod.rs` | M1: add `UpdateAction::DiscoverDevicesAndBootable { flutter }`; n1: multi-line `RefreshDevicesAndBootableBackground` doc |
| `crates/fdemon-app/src/actions/mod.rs` | M1: wire new action — foreground connected + background bootable in parallel |
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | M1: cache-miss branch dispatches new action; M2: clone at call site; m5: race comment; m7: collapse dead branch; n3: extract `len()` |
| `crates/fdemon-app/src/state.rs` | M2: `get_cached_bootable_devices()` returns refs |
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | m3: comment on `set_error` asymmetric clearing |
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | m2: thread `&IconSet`, call `icons.refresh()`; n2: assertion message |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | m2: pass `&IconSet` into `TabBar::new()`; m4: render `↻` in compact mode |
| `workflow/plans/bugs/device-cache-no-ttl/BUG.md` | Correct line 191 mitigation note |

---

## Phases

### Phase 1 — Blocking fixes

**Goal:** every reviewer-flagged blocking finding is addressed and verified.

**Steps (per task):**

1. **C1 — Clear `refreshing` on background failure.** Edit the `is_background: true`
   arm in `update.rs` to clear the flag when the dialog is visible. Add an integration
   test that opens the dialog with a cached list, fires
   `Message::DeviceDiscoveryFailed { is_background: true }`, and asserts
   `refreshing == false`. Update the parent plan's BUG.md line 191.

2. **M2 — Reference-returning accessor.** Change `get_cached_bootable_devices()` to
   return `Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)>`. The single caller in
   `navigation.rs` clones explicitly when forwarding to `set_bootable_devices`. Pure
   refactor; existing tests should pass unchanged.

3. **M1 — `DiscoverDevicesAndBootable` action.** Add the new variant to `UpdateAction`,
   wire its match arm in `actions/mod.rs` to spawn both `spawn_device_discovery`
   (foreground) and `spawn_bootable_device_discovery` (background) in parallel. Replace
   the `DiscoverDevices` dispatch in the cache-miss fallback with the new variant. Add
   a unit test.

**Measurable Outcomes:**

- After a transient `flutter devices` failure during a background refresh, the `↻`
  glyph clears within one frame.
- Opening the dialog with both caches empty kicks off both discoveries (foreground
  connected with loading spinner; background bootable populates the Bootable tab on
  arrival).
- `cargo test -p fdemon-app --lib` passes.

### Phase 2 — Polish

**Goal:** route the refresh glyph through the existing `IconSet`, surface the indicator
in compact mode, and apply minor cleanups.

**Steps (per task):**

4. **m2 + m4 + n2 — Icon routing + compact mode + assertion message.** Replace the
   inline `"↻"` literal in `tab_bar.rs:71` with `icons.refresh()`. Thread `&IconSet`
   through `TabBar::new()` (mirroring `new_session_dialog/mod.rs:162-175`). Update the
   `target_selector.rs` call site. Surface the same glyph in `render_tabs_compact` when
   the active tab's flag is set. Update test assertions in both files to use
   `IconSet::default().refresh()` instead of the literal `"↻"`. Add a render test for
   compact-mode glyph visibility. Add the diagnostic message to
   `test_tab_bar_renders_bootable_refreshing_indicator`.

5. **m1 + m3 + m5 + m7 + n1 + n3 — Polish bundle.** Add the symmetric
   `BootableDevicesDiscovered` clearing test (m1). Add a comment in
   `target_selector_state.rs` explaining `set_error()`'s asymmetric clearing (m3). Add
   a comment near the `refreshing = true` write in `navigation.rs` explaining the
   close+reopen race (m5). Collapse the dead `if/else` in
   `handle_close_new_session_dialog` to a single `state.ui_mode = UiMode::Normal`,
   removing the misleading "stay in startup mode" comment (m7). Multi-line the
   `RefreshDevicesAndBootableBackground` enum variant with a `///` doc on `flutter` (n1).
   Capture `cached_devices.len()` to a local before `.clone()` for clarity (n3).

**Measurable Outcomes:**

- Nerd Fonts users see the Nerd Font refresh glyph; Unicode users see `↻`.
- Compact mode shows a refresh indicator when a tab's flag is set.
- All cleanup items addressed without changing behaviour.
- `cargo test --workspace --lib && cargo clippy --workspace --lib -- -D warnings` clean.

---

## Edge Cases & Risks

### Background failure during cache-miss foreground discovery
- **Scenario:** `DiscoverDevicesAndBootable` runs; the connected discovery fails (foreground)
  while the bootable discovery completes (background).
- **Behavior:** the foreground failure goes through the `is_background: false` arm,
  hits `set_error()`, clears `loading` and `refreshing`. The bootable discovery completes
  normally. Both behaviours are pre-existing and correct.

### Compact mode glyph crowding
- **Risk:** appending ` ↻` to a compact-mode tab label may push the line over its row width
  on very narrow terminals.
- **Mitigation:** the compact rendering is centered; minor truncation by ratatui is
  acceptable. Indicator is a low-priority cue. If needed, the implementation can use a
  single-character glyph without the leading space.

### Test reliance on the literal `"↻"`
- **Risk:** existing test assertions match `"↻"`. After routing through `IconSet`, the
  exact resolved glyph depends on `IconMode`. Tests must use `IconSet::default()` (which
  is `Unicode` per `icons.rs:79`) so the literal `"↻"` still appears in the rendered
  output during tests.
- **Mitigation:** Task 04 explicitly uses `IconSet::default()` in its test setup. No
  test breakage expected.

---

## Out of Scope

- Generic multi-action `UpdateResult` redesign. The `DiscoverDevicesAndBootable` variant
  is a narrow workaround mirroring `RefreshDevicesAndBootableBackground`; the broader
  design question is parked.
- Moving `calculate_scroll_offset` to `fdemon-core` (m9, pre-existing TODO at
  `target_selector_state.rs:455`).
- Indicator-on-inactive-tab semantic change (m11). The implementation matches the parent
  plan's Phase 2 §1 ("Show the indicator on both tabs simultaneously when both are
  refreshing") and the user has confirmed keeping that behaviour.
- Adding a real `BootableDiscoveryFailed` message variant to surface bootable errors. The
  current `unwrap_or_default()` in `spawn_bootable_device_discovery` silently hides
  failures — that's a separate UX decision and is not blocking.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `Message::DeviceDiscoveryFailed { is_background: true }` clears
      `target_selector.refreshing` when the dialog is visible.
- [ ] Test `test_background_device_discovery_failure_clears_refreshing` passes.
- [ ] `get_cached_bootable_devices()` returns
      `Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)>`.
- [ ] All existing bootable cache tests pass.
- [ ] `UpdateAction::DiscoverDevicesAndBootable { flutter }` exists; cache-miss fallback
      dispatches it.
- [ ] Test `test_open_dialog_no_caches_dispatches_combined_discovery` passes.
- [ ] BUG.md line 191 in the parent plan is corrected.

### Phase 2 Complete When:
- [ ] `tab_bar.rs` calls `icons.refresh()` (no inline `↻` literal).
- [ ] `TabBar::new()` accepts `&IconSet`.
- [ ] Compact mode renders the refresh glyph when an active tab's flag is set.
- [ ] Test assertions in `tab_bar.rs` and `target_selector.rs` reference
      `IconSet::default().refresh()` rather than `"↻"`.
- [ ] All polish items applied (m1, m3, m5, m7, n1-n3).
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace --lib && cargo clippy --workspace --lib -- -D warnings` clean.

---

## Task Dependency Graph

```
Phase 1
├── 01-clear-refreshing-on-bg-failure   (handler/update.rs + tests + BUG.md doc fix)
├── 02-bootable-accessor-refs           (state.rs + navigation.rs single-call refactor)
└── 03-discover-and-bootable-action     (handler/mod.rs + actions/mod.rs + navigation.rs cache-miss)
        depends on: 02 (file overlap on navigation.rs)

Phase 2
├── 04-icon-routing-and-compact         (tab_bar.rs + target_selector.rs)
└── 05-polish-bundle                    (handler/tests.rs + handler/mod.rs + navigation.rs + target_selector_state.rs)
```

---

## Milestone Deliverable

When both phases are complete:

- The `↻` indicator clears on every discovery outcome — success, foreground failure,
  and **background failure**.
- Both connected and bootable lists are refreshed on every dialog open, regardless of
  cache state — matching the parent plan's stated milestone.
- The bootable cache accessor returns references with no hidden allocations.
- The refresh glyph respects the user's `IconMode` setting (Unicode vs Nerd Fonts).
- The compact-mode tab bar surfaces the indicator alongside the full-mode tab bar.
- The parent plan's `BUG.md` mitigation note is factually correct.
