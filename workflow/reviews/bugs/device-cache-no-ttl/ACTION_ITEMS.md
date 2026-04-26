# Action Items: Device Cache No-TTL

**Review Date:** 2026-04-25
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 1 critical, 2 major

---

## Critical Issues (Must Fix)

### 1. Clear `refreshing` flag on background discovery failure

- **Source:** `bug_fix_reviewer`, `architecture_enforcer`, `logic_reasoning_checker` (all flagged independently)
- **File:** `crates/fdemon-app/src/handler/update.rs:405-428`
- **Problem:** `Message::DeviceDiscoveryFailed { is_background: true }` only logs the warning;
  it does not clear `state.new_session_dialog_state.target_selector.refreshing`. Since the new
  `RefreshDevicesAndBootableBackground` action sends background failures through this exact
  path, any transient `flutter devices` failure leaves the `↻` glyph stuck on the Connected
  tab until the dialog is closed and reopened. BUG.md's "Cache Becoming Severely Stale"
  mitigation note (line 191) explicitly — and incorrectly — claims `set_error()` handles
  this case.
- **Required Action:** in the `is_background: true` arm, clear the flag when the dialog is
  visible:
  ```rust
  if is_background {
      tracing::warn!("Background device refresh failed: {}", error);
      if state.ui_mode == UiMode::NewSessionDialog || state.ui_mode == UiMode::Startup {
          state.new_session_dialog_state.target_selector.refreshing = false;
      }
  }
  ```
  Also update BUG.md line 191 to reflect the actual clearing path.
- **Acceptance:** new test in `handler/tests.rs` — open dialog with cached devices, force
  `Message::DeviceDiscoveryFailed { is_background: true }`, assert `refreshing == false`.

---

## Major Issues (Should Fix)

### 2. Cache-miss fallback never triggers bootable discovery

- **Source:** `code_quality_inspector`, `logic_reasoning_checker`
- **File:** `crates/fdemon-app/src/handler/new_session/navigation.rs:258-261`
- **Problem:** When both caches are empty, only `UpdateAction::DiscoverDevices` is dispatched.
  `DiscoverDevices` spawns a connected-device discovery only — bootable discovery is not
  triggered. The Bootable tab will sit at `bootable_loading = true` until the user manually
  switches tabs. This contradicts the milestone deliverable: "Both connected and bootable
  lists are refreshed on every dialog open."
- **Suggested Action:** in the cache-miss branch, after `loading = true`, also dispatch the
  combined background refresh, or extend `DiscoverDevices` to spawn both. Option A:
  ```rust
  // both caches empty: foreground discovery + parallel background bootable
  state.new_session_dialog_state.target_selector.loading = true;
  // (chain DiscoverDevices and a bootable refresh, or replace with a new "discover both" action)
  ```
- **Acceptance:** new test in `navigation.rs` covering "open dialog with no caches → both
  connected and bootable discoveries are kicked off."

### 3. `get_cached_bootable_devices()` clones the cache on every call

- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-app/src/state.rs:1261-1265`
- **Problem:** Returns `Option<(Vec<IosSimulator>, Vec<AndroidAvd>)>`, forcing a clone of both
  Vecs on every dialog open even when devices are unchanged. The connected-device equivalent
  returns a reference and clones at the single call site.
- **Suggested Action:** change return to
  `Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)>`; let the call site in `navigation.rs:228-234`
  clone explicitly when it forwards into `set_bootable_devices`.
- **Acceptance:** the function returns references; existing tests still pass.

---

## Minor Issues (Consider Fixing)

### 4. Add symmetric test for `BootableDevicesDiscovered` clearing `bootable_refreshing`
- **File:** `crates/fdemon-app/src/handler/tests.rs`
- The connected side has `test_devices_discovered_clears_refreshing`; mirror it for the
  bootable side.

### 5. Extract `↻` to a named constant
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`
- Add `const REFRESHING_GLYPH: &str = "↻";` and reference from the render loop. Update test
  assertions in `tab_bar.rs` and `target_selector.rs` to reference the constant.

### 6. Document or fix `set_error()` asymmetric clearing
- **File:** `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:271-276`
- Either add a comment explaining why only `refreshing` is cleared (and not `bootable_refreshing`),
  or clear both for consistency with the symmetric flag design.

### 7. Render `↻` indicator in compact mode
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`
- `render_tabs_compact` doesn't surface the refresh state. Add a small inline indicator or
  document the omission.

### 8. Comment the close+reopen race
- **File:** `crates/fdemon-app/src/handler/new_session/navigation.rs` near `refreshing = true`
- Briefly note that an in-flight discovery from a prior dialog session may clear the new
  flag prematurely if the close+reopen happens fast.

### 9. Resolve or ticket the stale TODO at `target_selector_state.rs:455`
- "deduplicate with device_list::calculate_scroll_offset — move to fdemon-core" — pre-existing,
  but visible in modified-file scope.

### 10. Collapse the dead branch in `handle_close_new_session_dialog`
- **File:** `crates/fdemon-app/src/handler/new_session/navigation.rs:264-279`
- Both arms of the `if has_running_sessions` set `UiMode::Normal`; the misleading comment
  says "stay in startup mode" but doesn't.

### 11. Confirm indicator-on-inactive-tab semantics
- The implementation shows `↻` on every tab whose flag is true, regardless of active. This
  matches BUG.md but worth a quick "yes that's what we want" before considering it final.

---

## Nitpicks

- Multi-line the `RefreshDevicesAndBootableBackground` enum variant with a field-level
  `///` doc to match siblings (`handler/mod.rs`).
- Add an assertion message to `test_tab_bar_renders_bootable_refreshing_indicator`.
- Extract `cached_devices.len()` to a local before `.clone()` in `navigation.rs` for clarity.

---

## Re-review Checklist

After addressing issues:

- [ ] All Critical issues resolved
- [ ] All Major issues resolved or explicitly documented as out-of-scope
- [ ] At least minor items 4, 5, 6 addressed
- [ ] BUG.md updated to reflect the corrected clearing semantics
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace --lib && cargo clippy --workspace --lib -- -D warnings` passes
- [ ] New tests added for: background-failure flag clearing (C1), both-cache-empty bootable discovery (M2)
