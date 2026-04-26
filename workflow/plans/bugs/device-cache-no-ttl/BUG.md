# Bugfix Plan: Device Cache Drops After 30s (Issue #33 follow-up)

## TL;DR

The new-session dialog sometimes shows the cached device list instantly and sometimes
forces the user to wait for `flutter devices` again. Root cause: `get_cached_devices()`
and `get_cached_bootable_devices()` enforce a hardcoded **30-second TTL**, after which
they return `None` even though the cache data is still in memory. Once the TTL elapses,
opening the dialog falls into the cache-miss branch and shows a loading screen.

The previous plan (`workflow/plans/bugs/new-session-dialog-fixes/BUG.md`, Phase 1 / Bug 1)
introduced the cache + background-refresh wiring but kept the TTL, so the bug recurs in
production whenever the user opens the dialog more than 30 seconds after the last
discovery.

**Fix:** remove the TTL entirely (cache lives until replaced), trigger a background
refresh on every dialog open (connected **and** bootable), and surface a small
"refreshing" indicator on the active target-selector tab so the user knows the list is
being updated in place.

GitHub issue: https://github.com/edTheGuy00/fdemon/issues/33

---

## Bug Report

### Symptom

1. Start `fdemon`, wait for the device list to populate.
2. Launch a session — the new session dialog closes.
3. After working for a while (≥30 s), open the new session dialog again to start a
   second session on a different device.
4. **Sometimes** the cached device list is shown instantly with a brief background
   refresh (works as designed). **Sometimes** the dialog shows a loading state and the
   user has to wait for `flutter devices` to return.

The behaviour is time-correlated: opening the dialog soon after the previous discovery
works; waiting longer drops the cached list.

### Expected

The cached device list should always appear instantly when the dialog opens. A small
indicator should make it obvious that the list is being refreshed in the background.
Discovery results, when they arrive, replace the list in place without flashing a
loading screen.

### Root Cause

`crates/fdemon-app/src/state.rs:1239-1250` — `get_cached_devices()`:

```rust
pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
    const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);
    if let (Some(ref devices), Some(updated)) = (&self.device_cache, self.devices_last_updated) {
        if updated.elapsed() < CACHE_TTL {
            return Some(devices);
        }
    }
    None
}
```

Same TTL gating exists in `get_cached_bootable_devices()` at `state.rs:1270-1284`.

`crates/fdemon-app/src/handler/new_session/navigation.rs:202-250` —
`handle_open_new_session_dialog()` calls `get_cached_devices()`. When the TTL is
exceeded it returns `None`, and the handler falls into the cache-miss branch:

```rust
state.new_session_dialog_state.target_selector.loading = true;
UpdateResult::action(UpdateAction::DiscoverDevices { flutter })
```

— which is the foreground discovery path (loading screen, no cached list shown).

The `device_cache` field itself is **not** dropped from memory; only the accessor's
TTL gate makes it appear so. Removing the TTL gate makes the cache survive for the
full lifetime of the `AppState`, which is the intended behaviour.

### Secondary Observation: Bootable Cache Never Refreshes on Dialog Open

`navigation.rs:219-241` only triggers a background refresh for **connected** devices on
cache hit. Bootable devices are populated from cache but no
`UpdateAction::RefreshBootableDevicesBackground` is dispatched, so once bootable
discovery has run at startup, the bootable list is frozen for the rest of the session.
This is a latent bug surfaced by the same plan and is fixed here for parity.

---

## Affected Modules

| Module | Change |
|---|---|
| `crates/fdemon-app/src/state.rs` | Remove TTL constants and `elapsed() < CACHE_TTL` checks in `get_cached_devices()` and `get_cached_bootable_devices()`. Update / replace expiration tests. |
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Add `refreshing: bool` and `bootable_refreshing: bool` fields. Clear them in `set_connected_devices()` / `set_bootable_devices()` / `set_error()`. |
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | On dialog open, set the appropriate refreshing flag(s) and dispatch the new combined background-refresh action. |
| `crates/fdemon-app/src/handler/mod.rs` | Add `UpdateAction::RefreshDevicesAndBootableBackground { flutter }` variant. (`UpdateResult` carries a single action, so we combine both discoveries in one variant rather than introducing batching.) |
| `crates/fdemon-app/src/actions/mod.rs` | Wire `RefreshDevicesAndBootableBackground` to call both `spawn::spawn_device_discovery_background` and `spawn::spawn_bootable_device_discovery`. |
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | Accept two `refreshing` flags; append a `↻` glyph (dim style) to the active tab label when its flag is set. |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Pass the two refreshing flags into `TabBar::new()`. |

---

## Phases

### Phase 1: Remove TTL and add refreshing-state plumbing (Critical)

**Goal:** the cache survives indefinitely; per-tab `refreshing` flags exist on the
target-selector state and are correctly toggled by the dialog open / discovery flow.

**Steps:**

1. **Remove TTL** in `state.rs` (`get_cached_devices`, `get_cached_bootable_devices`).
   - Keep `devices_last_updated` / `bootable_last_updated` — still used by tracing
     debug logs in `navigation.rs`.
   - Replace `test_device_cache_expires` with `test_device_cache_does_not_expire`
     (verify cache returns Some even after a long simulated elapsed duration).

2. **Add refreshing flags** to `TargetSelectorState`:
   - `pub refreshing: bool` (default `false`)
   - `pub bootable_refreshing: bool` (default `false`)
   - Clear `refreshing` in `set_connected_devices()` and `set_error()`.
   - Clear `bootable_refreshing` in `set_bootable_devices()`.

3. **Set flags in `handle_open_new_session_dialog`** (`navigation.rs`):
   - When **either** a connected or bootable cache is present: set the corresponding
     `refreshing` / `bootable_refreshing` flag, then dispatch the new combined
     `UpdateAction::RefreshDevicesAndBootableBackground { flutter }`. This single
     action triggers both background discoveries.
   - When only one of the two caches is present, still dispatch the combined action —
     the missing-cache side will populate as if it were a fresh discovery (its tab's
     `loading` flag may already be set from default state, in which case
     `set_…_devices()` will clear it on completion).
   - On cache miss for **both** (no connected cache and no bootable cache): keep
     existing behaviour (`loading = true` + foreground `DiscoverDevices`). Do **not**
     set `refreshing` here — `loading` covers this case.

4. **Add `UpdateAction::RefreshDevicesAndBootableBackground { flutter }`** in
   `handler/mod.rs`. Wire it in `actions/mod.rs` to call both
   `spawn::spawn_device_discovery_background(msg_tx.clone(), flutter)` and
   `spawn::spawn_bootable_device_discovery(msg_tx, tool_availability)`. Errors are
   logged only on both sides (UI already shows cached data).

**Measurable Outcomes:**
- After waiting >30s, opening the dialog still shows the cached device list instantly.
- `target_selector.refreshing` becomes `true` on dialog open and `false` once
  `DevicesDiscovered` arrives.
- Same behaviour for `bootable_refreshing` and `BootableDevicesDiscovered`.

---

### Phase 2: Render the refreshing indicator on the tab bar

**Goal:** the user sees a clear visual cue that the device list is being refreshed,
without losing the cached content.

**Steps:**

1. **Update `TabBar`** (`tab_bar.rs`):
   - Add `connected_refreshing: bool` and `bootable_refreshing: bool` fields.
   - Update `TabBar::new()` signature to accept them.
   - In the render loop, when the tab being rendered has its refreshing flag set,
     append ` ↻` to the label (or use a styled `Span` with `palette::TEXT_SECONDARY`
     and `Modifier::DIM` so it's visible but unobtrusive).
   - Show the indicator on **both** tabs simultaneously when both are refreshing.

2. **Wire flags through** (`target_selector.rs`):
   - Pass `self.state.refreshing` and `self.state.bootable_refreshing` into
     `TabBar::new()`.

3. **Tests:**
   - Snapshot/buffer test confirming the `↻` glyph appears in the active tab label
     when its flag is set, and disappears once cleared.
   - No regression in existing tab bar render tests (`test_tab_bar_renders`,
     `test_tab_bar_renders_with_bootable_active`, `test_tab_bar_unfocused`).

**Measurable Outcomes:**
- Opening the dialog with cached devices shows the list immediately and the active tab
  label shows ` ↻` until the background refresh completes.
- Indicator disappears within ~1 frame of `DevicesDiscovered` / `BootableDevicesDiscovered`.

---

## Edge Cases & Risks

### Cache Becoming Severely Stale
- **Risk:** with no TTL, if `flutter devices` keeps failing, the cache could show
  outdated devices indefinitely.
- **Mitigation:** every dialog open triggers a background refresh, and the discovery
  failure path (`Message::DeviceDiscoveryFailed`) clears the `refreshing` indicator on
  both the foreground branch (via `set_error()`) and the background branch (via a
  direct flag clear). Background failures (`is_background: true`) are logged at warn
  level only — the user still sees the previous devices, which is the desired UX.

### Concurrent Discovery
- **Risk:** triggering a refresh while a previous discovery is still in flight could
  produce out-of-order `DevicesDiscovered` messages.
- **Mitigation:** `set_device_cache` is idempotent — last write wins. The
  `refreshing` flag is cleared by whichever discovery returns last; transient flicker
  is acceptable.

### Bootable Refresh on Every Dialog Open
- **Risk:** added cost of running `xcrun simctl list` / `emulator -list-avds` on every
  dialog open.
- **Mitigation:** these commands return in a few hundred ms; user has explicitly
  opened the dialog so the cost is justified. Errors are background-logged and don't
  disturb the cached list.

### Indicator Glyph Compatibility
- **Risk:** `↻` may not render on every terminal.
- **Mitigation:** the glyph is already used elsewhere or has a safe fallback; if it
  causes problems, swap for `*` or `…`. Treat as a follow-up if reported.

---

## Out of Scope

- Reworking the tracing-debug log lines that print cache age (they continue to work
  via the still-present `devices_last_updated` field).
- Adding cache invalidation on session-lifecycle events (start/stop) — the user
  confirmed dialog-open refresh is sufficient.
- Making the cache TTL configurable — the decision is "no TTL", not "configurable
  TTL".

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `get_cached_devices()` returns `Some(...)` whenever `device_cache.is_some()`,
      regardless of elapsed time.
- [ ] `get_cached_bootable_devices()` does the same.
- [ ] Opening the dialog after >30 s of inactivity shows the cached list instantly.
- [ ] Opening the dialog dispatches both a connected-device background refresh and a
      bootable-device background refresh when those caches are populated.
- [ ] All existing device-cache tests pass (after expiration test is replaced).

### Phase 2 Complete When:
- [ ] `TabBar` accepts `connected_refreshing` and `bootable_refreshing` flags.
- [ ] Active tab label shows ` ↻` while its refresh is in flight, disappears on
      completion.
- [ ] Existing tab bar render tests still pass.
- [ ] New unit test: indicator visible while refreshing, hidden after
      `set_connected_devices` / `set_bootable_devices`.

---

## Task Dependency Graph

```
Phase 1
├── 01-remove-ttl              (state.rs: cache accessors + tests)
├── 02-refreshing-state-flags  (target_selector_state.rs: new fields + setters)
├── 03-combined-bg-action      (handler/mod.rs + actions/mod.rs: RefreshDevicesAndBootableBackground)
└── 04-dialog-open-wiring      (navigation.rs: set flags, dispatch combined action)
        depends on: 02, 03

Phase 2
├── 05-tab-bar-indicator       (tab_bar.rs: new fields + glyph rendering)
└── 06-target-selector-wiring  (target_selector.rs: pass flags into TabBar)
        depends on: 05, 02
```

---

## Milestone Deliverable

When both phases are complete:
- The new session dialog **always** shows the last-known device list instantly when
  opened, regardless of how much time has passed since the previous discovery.
- A subtle `↻` indicator on the active tab makes the in-flight background refresh
  visible without obscuring the cached content.
- Both connected and bootable lists are refreshed on every dialog open, eliminating
  the latent bug where bootable devices were frozen after startup.
