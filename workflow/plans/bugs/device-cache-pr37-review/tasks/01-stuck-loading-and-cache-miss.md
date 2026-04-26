# Task 01 — Stuck-Loading + Connected Cache-Miss Foreground (F1+F2)

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-app/src/handler/new_session/navigation.rs`

---

## Goal

Fix two Major issues from PR #37's Copilot review, both in
`handle_open_new_session_dialog`:

- **F1:** When `flutter_executable()` returns `None`, the early return leaves
  `loading=true`/`bootable_loading=true` (the defaults from
  `TargetSelectorState::default()`) — the dialog spins forever with no recovery.
- **F2:** When `connected_cached=false` but `bootable_cached=true`, the code
  dispatches `RefreshDevicesAndBootableBackground`. A connected-discovery failure
  on the background path clears only `refreshing` (not `loading`), so the Connected
  tab stays stuck loading.

Both findings are localized to the same function and are tightly coupled (both
touch the post-cache-population branching), so they share a single task.

## Context

Current code at `crates/fdemon-app/src/handler/new_session/navigation.rs:243-274`:

```rust
let Some(flutter) = state.flutter_executable() else {
    tracing::warn!("handle_open_new_session_dialog: no Flutter SDK — skipping device refresh");
    return UpdateResult::none();   // F1: leaves loading=true/bootable_loading=true
};

if connected_cached || bootable_cached {
    // F2: ALWAYS background, even when connected cache is missing
    if connected_cached {
        state.new_session_dialog_state.target_selector.refreshing = true;
    }
    if bootable_cached {
        state.new_session_dialog_state.target_selector.bootable_refreshing = true;
    }
    return UpdateResult::action(UpdateAction::RefreshDevicesAndBootableBackground { flutter });
}

// Both caches empty — foreground combined discovery
tracing::debug!("Device cache miss, triggering combined foreground+bootable discovery");
state.new_session_dialog_state.target_selector.loading = true;
UpdateResult::action(UpdateAction::DiscoverDevicesAndBootable { flutter })
```

Key facts the implementor must know:

- `TargetSelectorState::default()` (in `target_selector_state.rs:76-90`) sets
  `loading: true, bootable_loading: true`. `show_new_session_dialog()` uses this
  default via `NewSessionDialogState::new(configs)`.
- `set_error(msg)` (in `target_selector_state.rs:279-283`) clears `loading` and
  `refreshing` but does **not** clear `bootable_loading` or `bootable_refreshing`
  (bootable discovery is independent — see task 02 for the doc rewrite that makes
  this explicit).
- `UpdateAction::DiscoverDevicesAndBootable` (in `handler/mod.rs:85-95`, wired in
  `actions/mod.rs:92-98`) spawns `spawn_device_discovery` (foreground; failures
  route through `set_error()`) and `spawn_bootable_device_discovery` (background;
  failures swallowed via `unwrap_or_default()`) in parallel.
- `UpdateAction::RefreshDevicesAndBootableBackground` (in `handler/mod.rs:74-83`,
  wired in `actions/mod.rs:85-90`) spawns the *background* connected variant
  (failures only clear `refreshing`) plus the same bootable spawn.
- The pattern `set_error("No Flutter SDK found. Configure sdk_path in
  .fdemon/config.toml or install Flutter.")` is already established at
  `handler/new_session/launch_context.rs:532`.

## Steps

1. **F1 — Surface SDK-missing error and clear bootable in-flight flags.**
   Replace the bare early return with a `set_error()` call plus explicit
   bootable-flag clears. Around line 243:

   ```rust
   let Some(flutter) = state.flutter_executable() else {
       tracing::warn!("handle_open_new_session_dialog: no Flutter SDK — surfacing error to dialog");
       let selector = &mut state.new_session_dialog_state.target_selector;
       // set_error() clears `loading` and `refreshing`; bootable flags must be
       // cleared explicitly because bootable discovery is independent of the
       // Flutter SDK (see task 02 for the doc rewrite explaining this).
       selector.bootable_loading = false;
       selector.bootable_refreshing = false;
       selector.set_error(
           "No Flutter SDK found. Configure sdk_path in .fdemon/config.toml or install Flutter.".to_string(),
       );
       return UpdateResult::none();
   };
   ```

2. **F2 — Branch on `connected_cached` rather than `connected_cached || bootable_cached`.**
   Restructure the post-cache section so the foreground variant is used whenever
   the connected cache is missing:

   ```rust
   if connected_cached {
       // Connected list shown — refresh both in background. Failures on the
       // connected side will only clear `refreshing` (not `loading`), but that's
       // fine because `loading` is already false (set_connected_devices cleared it).
       state.new_session_dialog_state.target_selector.refreshing = true;
       if bootable_cached {
           state.new_session_dialog_state.target_selector.bootable_refreshing = true;
       }
       return UpdateResult::action(UpdateAction::RefreshDevicesAndBootableBackground { flutter });
   }

   // Connected cache missing — foreground discovery so failures route through
   // set_error() and clear `loading`. Bootable spawns in parallel (background).
   if bootable_cached {
       // Bootable already shown; mark its parallel refresh as in-flight.
       state.new_session_dialog_state.target_selector.bootable_refreshing = true;
   }
   // `loading` is already true (default from show_new_session_dialog), but set
   // explicitly for readability and to defend against future refactors.
   state.new_session_dialog_state.target_selector.loading = true;
   tracing::debug!(
       "Device cache miss for connected ({}), triggering foreground combined discovery",
       if bootable_cached { "bootable cached" } else { "neither cached" }
   );
   UpdateResult::action(UpdateAction::DiscoverDevicesAndBootable { flutter })
   ```

   Preserve the existing race-condition comment about close+reopen (currently at
   `navigation.rs:252-256`); move it to the new `if connected_cached { ... }` arm
   since the race scenario is specific to background discovery.

3. **Add inline tests** in the `#[cfg(test)] mod tests` block at the bottom of
   `navigation.rs` (the established pattern from device-cache-followup tasks). All
   tests should follow the existing test style — use `AppState::default()`,
   manipulate caches directly, call `handle_open_new_session_dialog`, assert on
   `target_selector` state and the returned `UpdateResult` action.

   - **`test_open_dialog_no_flutter_sdk_surfaces_error`**: configure state with no
     SDK (i.e. `state.resolved_sdk = None` or whatever makes `flutter_executable()`
     return `None`), call `handle_open_new_session_dialog`, assert:
     - `target_selector.error == Some("No Flutter SDK found. Configure sdk_path in .fdemon/config.toml or install Flutter.".to_string())`
     - `target_selector.loading == false`
     - `target_selector.bootable_loading == false`
     - `target_selector.refreshing == false`
     - `target_selector.bootable_refreshing == false`
     - The returned `UpdateResult` has no action.
   - **`test_open_dialog_bootable_cached_only_uses_foreground`**: populate the
     bootable cache only, leave the connected cache empty, call
     `handle_open_new_session_dialog`, assert:
     - The returned `UpdateResult` carries `UpdateAction::DiscoverDevicesAndBootable { .. }` (not `RefreshDevicesAndBootableBackground`).
     - `target_selector.loading == true`
     - `target_selector.bootable_refreshing == true`
     - `target_selector.refreshing == false` (connected isn't refreshing — it's
       loading from scratch).
   - **`test_open_dialog_both_cached_uses_background`** (sanity test confirming
     existing behaviour is preserved): populate both caches, call the handler,
     assert `RefreshDevicesAndBootableBackground` and both `refreshing` /
     `bootable_refreshing` are true.

   Search the existing test module for similar `test_open_dialog_*` tests and use
   the same setup helpers / assertion style. If the existing tests use a builder
   helper for state setup, reuse it.

4. **Do not modify** `set_error()` itself or `target_selector_state.rs`. Task 02
   handles the doc comment update.

5. Run verification:
   - `cargo fmt --all`
   - `cargo check -p fdemon-app`
   - `cargo test -p fdemon-app --lib`
   - `cargo clippy -p fdemon-app --lib -- -D warnings`

## Acceptance Criteria

- [ ] When `flutter_executable()` returns `None`, the dialog converges to a stable
      error state: `set_error("No Flutter SDK found. ...")` is called, and
      `bootable_loading` + `bootable_refreshing` are explicitly set to `false`.
- [ ] The post-cache branching uses `if connected_cached` (not
      `connected_cached || bootable_cached`) to route between background and
      foreground discovery.
- [ ] When `connected_cached=false` and `bootable_cached=true`, the handler
      dispatches `UpdateAction::DiscoverDevicesAndBootable { flutter }` with
      `bootable_refreshing=true`.
- [ ] When both caches are populated, the handler still dispatches
      `RefreshDevicesAndBootableBackground` with both `refreshing` and
      `bootable_refreshing` set to `true` (no behavior regression).
- [ ] Test `test_open_dialog_no_flutter_sdk_surfaces_error` passes.
- [ ] Test `test_open_dialog_bootable_cached_only_uses_foreground` passes.
- [ ] Test `test_open_dialog_both_cached_uses_background` passes (sanity).
- [ ] `cargo test -p fdemon-app --lib` passes (no regressions in existing tests).
- [ ] `cargo clippy -p fdemon-app --lib -- -D warnings` clean.

## Out of Scope

- Modifying `set_error()` itself (task 02 owns the doc rewrite for that helper).
- Adding new `UpdateAction` variants — both `DiscoverDevicesAndBootable` and
  `RefreshDevicesAndBootableBackground` already exist on the PR branch.
- Adding a real `BootableDiscoveryFailed` message variant — bootable failures
  remain silently swallowed by `unwrap_or_default()` (separate UX decision).
- Restructuring the cache-population blocks earlier in the function (lines
  204-241). They are correct as-is.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ae9aadcc3bbe6bc44

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | F1: replaced bare early return with `set_error()` + explicit bootable flag clears; F2: changed `connected_cached \|\| bootable_cached` to `if connected_cached` routing; added 3 new tests |
| `crates/fdemon-app/src/handler/tests.rs` | Fixed `test_background_discovery_error_is_silent` to inject a fake SDK so it reaches the background path (was relying on the old silent early return) |

### Notable Decisions/Tradeoffs

1. **F1 error message**: Used the exact message from the task spec (`"No Flutter SDK found. Configure sdk_path in .fdemon/config.toml or install Flutter."`) matching the established pattern from `launch_context.rs:532`.

2. **F2 branch restructure**: Changed `if connected_cached || bootable_cached` to `if connected_cached` as specified. The `bootable_cached`-only scenario now falls through to `DiscoverDevicesAndBootable` with `bootable_refreshing=true`, which is correct because the foreground path handles connected-discovery failures via `set_error()` (clearing `loading`).

3. **Existing test fix**: `test_background_discovery_error_is_silent` in `handler/tests.rs` used `AppState::new()` without an SDK. Before this fix, the early return was silent so the dialog opened without error. After the fix, the missing SDK is surfaced as an error, breaking the test. Added `state.resolved_sdk = Some(fake_flutter_sdk())` to match the test's intent (testing background failures, not SDK-missing scenarios).

### Testing Performed

- `cargo fmt --all` - Passed (1 file reformatted)
- `cargo check -p fdemon-app` - Passed (exit code 0)
- `cargo test -p fdemon-app --lib` - Passed (1898 tests, 0 failed)
- `cargo clippy -p fdemon-app --lib -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **SDK-missing error surfacing**: The new behavior surfaces the SDK error immediately on dialog open (not just on launch). This is strictly better UX but is a behavioral change — users with no SDK configured will now see the error in the dialog's Connected tab rather than a perpetual spinner. This aligns with the task's intent.
