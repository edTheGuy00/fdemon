# Task 01 — Clear `refreshing` on Background Discovery Failure

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs`
- `crates/fdemon-app/src/handler/tests.rs`
- `workflow/plans/bugs/device-cache-no-ttl/BUG.md`

---

## Goal

Fix the Critical issue from
[`workflow/reviews/bugs/device-cache-no-ttl/REVIEW.md`](../../../../reviews/bugs/device-cache-no-ttl/REVIEW.md):
when `Message::DeviceDiscoveryFailed { is_background: true }` arrives, clear
`target_selector.refreshing` so the `↻` indicator does not stay stuck on the Connected
tab. Three reviewers (`bug_fix_reviewer`, `architecture_enforcer`,
`logic_reasoning_checker`) flagged this independently.

## Context

The new `RefreshDevicesAndBootableBackground` action sends connected-discovery failures
through `is_background: true` (`crates/fdemon-app/src/spawn.rs:45-68`). The current handler
arm at `update.rs:409-412` only logs:

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

`set_error()` (which clears `refreshing`) is reached only via the foreground branch.
Background failures leave the flag set indefinitely.

The parent plan's mitigation note (`workflow/plans/bugs/device-cache-no-ttl/BUG.md:191`)
incorrectly claims `set_error()` handles this case — that line needs to be corrected.

## Steps

1. Open `crates/fdemon-app/src/handler/update.rs`. Locate the
   `Message::DeviceDiscoveryFailed { error, is_background }` arm (around line 405).

2. In the `is_background` branch, after the existing `tracing::warn!`, clear `refreshing`
   when the new-session dialog is visible:

   ```rust
   if is_background {
       tracing::warn!("Background device refresh failed: {}", error);
       if state.ui_mode == UiMode::NewSessionDialog || state.ui_mode == UiMode::Startup {
           state.new_session_dialog_state.target_selector.refreshing = false;
       }
   } else {
       // ... existing foreground branch unchanged
   }
   ```

   Notes:
   - Mirror the existing `state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog`
     guard used by the foreground branch.
   - Do NOT clear `bootable_refreshing` here. The bootable spawn never sends
     `DeviceDiscoveryFailed`; its errors are swallowed via `unwrap_or_default()` in
     `spawn_bootable_device_discovery`. Touching `bootable_refreshing` here would be
     incorrect — see the parent plan's `BUG.md` for context.
   - Use `tracing::warn!` only (don't elevate to `error!`) — background failures are
     non-fatal; the user still sees the cached list.

3. Add an integration test in `crates/fdemon-app/src/handler/tests.rs`. Place it near the
   existing `test_devices_discovered_clears_refreshing` test (recently added):

   ```rust
   #[test]
   fn test_background_device_discovery_failure_clears_refreshing() {
       let mut state = AppState::new();
       state.show_new_session_dialog(LoadedConfigs::default());
       state.set_device_cache(vec![test_device("dev1", "Device 1")]);
       state.new_session_dialog_state.target_selector.refreshing = true;

       let _ = handler::update(
           &mut state,
           Message::DeviceDiscoveryFailed {
               error: "transient flutter devices error".to_string(),
               is_background: true,
           },
       );

       assert!(
           !state.new_session_dialog_state.target_selector.refreshing,
           "background failure must clear the refreshing flag"
       );
   }
   ```

   Use the `test_device(...)` helper if it exists in scope (mirror the existing
   `test_devices_discovered_clears_refreshing` test for the exact import / helper style).

4. Update the parent plan's BUG.md. Open
   `workflow/plans/bugs/device-cache-no-ttl/BUG.md`, find the "Cache Becoming Severely
   Stale" subsection (around line 187-193), and replace the inaccurate claim about
   `set_error()` clearing `refreshing` with an accurate description. Suggested rewording:

   > **Mitigation:** every dialog open triggers a background refresh, and the discovery
   > failure path (`Message::DeviceDiscoveryFailed`) clears the `refreshing` indicator on
   > both the foreground branch (via `set_error()`) and the background branch (via a
   > direct flag clear). Background failures (`is_background: true`) are logged at warn
   > level only — the user still sees the previous devices, which is the desired UX.

5. Run the verification commands:
   - `cargo fmt --all`
   - `cargo check -p fdemon-app`
   - `cargo test -p fdemon-app --lib`
   - `cargo clippy -p fdemon-app --lib -- -D warnings`

## Acceptance Criteria

- [ ] `update.rs` `is_background: true` arm clears
      `state.new_session_dialog_state.target_selector.refreshing` when the dialog or
      startup is visible.
- [ ] `bootable_refreshing` is **not** modified by this arm (see Notes above).
- [ ] New test `test_background_device_discovery_failure_clears_refreshing` is present
      in `handler/tests.rs` and passes.
- [ ] Parent plan's `BUG.md` "Cache Becoming Severely Stale" mitigation paragraph no
      longer claims `set_error()` is the sole clearing path.
- [ ] `cargo test -p fdemon-app --lib` passes (no regressions).
- [ ] `cargo clippy -p fdemon-app --lib -- -D warnings` clean.

## Out of Scope

- Adding a `BootableDiscoveryFailed` message variant (parent plan's bootable spawn
  swallows errors; surfacing them is a separate UX decision).
- Reworking the `is_background` boolean into a typed enum.
- Changing the `set_error()` implementation in `target_selector_state.rs` (handled in
  task 05's polish bundle).
